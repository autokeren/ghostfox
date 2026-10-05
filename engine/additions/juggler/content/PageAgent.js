/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

const Ci = Components.interfaces;
const Cr = Components.results;
const Cu = Components.utils;

const {Helper} = ChromeUtils.importESModule('chrome://juggler/content/Helper.js');
const {NetUtil} = ChromeUtils.importESModule('resource://gre/modules/NetUtil.sys.mjs');
const {setTimeout} = ChromeUtils.importESModule('resource://gre/modules/Timer.sys.mjs');

const dragService = Cc["@mozilla.org/widget/dragservice;1"].getService(
  Ci.nsIDragService
);
const obs = Cc["@mozilla.org/observer-service;1"].getService(
  Ci.nsIObserverService
);

const helper = new Helper();

class WorkerData {
  constructor(pageAgent, browserChannel, worker) {
    this._workerRuntime = worker.channel().connect('runtime');
    this._browserWorker = browserChannel.connect(worker.id());
    this._worker = worker;
    const emit = name => {
      return (...args) => this._browserWorker.emit(name, ...args);
    };
    this._eventListeners = [
      worker.channel().register('runtime', {
        runtimeConsole: emit('runtimeConsole'),
        runtimeExecutionContextCreated: emit('runtimeExecutionContextCreated'),
        runtimeExecutionContextDestroyed: emit('runtimeExecutionContextDestroyed'),
      }),
      browserChannel.register(worker.id(), {
        evaluate: (options) => this._workerRuntime.send('evaluate', options),
        callFunction: (options) => this._workerRuntime.send('callFunction', options),
        getObjectProperties: (options) => this._workerRuntime.send('getObjectProperties', options),
        disposeObject: (options) => this._workerRuntime.send('disposeObject', options),
      }),
    ];
  }

  dispose() {
    this._workerRuntime.dispose();
    this._browserWorker.dispose();
    helper.removeListeners(this._eventListeners);
  }
}

export class PageAgent {
  constructor(browserChannel, frameTree) {
    this._browserChannel = browserChannel;
    this._browserPage = browserChannel.connect('page');
    this._frameTree = frameTree;
    this._runtime = frameTree.runtime();

    this._workerData = new Map();

    const docShell = frameTree.mainFrame().docShell();
    this._docShell = docShell;

    // Dispatch frameAttached events for all initial frames
    for (const frame of this._frameTree.frames()) {
      this._onFrameAttached(frame);
      if (frame.url())
        this._onNavigationCommitted(frame);
      if (frame.pendingNavigationId())
        this._onNavigationStarted(frame);
    }

    // Report created workers.
    for (const worker of this._frameTree.workers())
      this._onWorkerCreated(worker);

    // Report execution contexts.
    this._browserPage.emit('runtimeExecutionContextsCleared', {});
    for (const context of this._runtime.executionContexts())
      this._onExecutionContextCreated(context);

    if (this._frameTree.isPageReady()) {
      this._browserPage.emit('pageReady', {});
      const mainFrame = this._frameTree.mainFrame();
      const domWindow = mainFrame.domWindow();
      const document = domWindow ? domWindow.document : null;
      const readyState = document ? document.readyState : null;
      // Sometimes we initialize later than the first about:blank page is opened.
      // In this case, the page might've been loaded already, and we need to issue
      // the `DOMContentLoaded` and `load` events.
      if (mainFrame.url() === 'about:blank' && readyState === 'complete')
        this._emitAllEvents(this._frameTree.mainFrame());
    }

    this._eventListeners = [
      helper.addObserver(this._linkClicked.bind(this, false), 'juggler-link-click'),
      helper.addObserver(this._linkClicked.bind(this, true), 'juggler-link-click-sync'),
      helper.addObserver(this._onWindowOpenInNewContext.bind(this), 'juggler-window-open-in-new-context'),
      helper.addObserver(this._filePickerShown.bind(this), 'juggler-file-picker-shown'),
      helper.addObserver(this._onDocumentOpenLoad.bind(this), 'juggler-document-open-loaded'),
      helper.on(this._frameTree, 'frameattached', this._onFrameAttached.bind(this)),
      helper.on(this._frameTree, 'framedetached', this._onFrameDetached.bind(this)),
      helper.on(this._frameTree, 'navigationstarted', this._onNavigationStarted.bind(this)),
      helper.on(this._frameTree, 'navigationcommitted', this._onNavigationCommitted.bind(this)),
      helper.on(this._frameTree, 'navigationaborted', this._onNavigationAborted.bind(this)),
      helper.on(this._frameTree, 'samedocumentnavigation', this._onSameDocumentNavigation.bind(this)),
      helper.on(this._frameTree, 'pageready', () => this._browserPage.emit('pageReady', {})),
      helper.on(this._frameTree, 'workercreated', this._onWorkerCreated.bind(this)),
      helper.on(this._frameTree, 'workerdestroyed', this._onWorkerDestroyed.bind(this)),
      helper.on(this._frameTree, 'websocketcreated', event => this._browserPage.emit('webSocketCreated', event)),
      helper.on(this._frameTree, 'websocketopened', event => this._browserPage.emit('webSocketOpened', event)),
      helper.on(this._frameTree, 'websocketframesent', event => this._browserPage.emit('webSocketFrameSent', event)),
      helper.on(this._frameTree, 'websocketframereceived', event => this._browserPage.emit('webSocketFrameReceived', event)),
      helper.on(this._frameTree, 'websocketclosed', event => this._browserPage.emit('webSocketClosed', event)),
      helper.on(this._frameTree, 'inputevent', inputEvent => {
        this._browserPage.emit('pageInputEvent', inputEvent);
        if (inputEvent.type === 'dragstart') {
          // After the dragStart event is dispatched and handled by Web,
          // it might or might not create a new drag session, depending on its preventing default.
          setTimeout(() => {
            const session = this._getCurrentDragSession();
            this._browserPage.emit('pageInputEvent', { type: 'juggler-drag-finalized', dragSessionStarted: !!session });
          }, 0);
        }
      }),
      helper.addObserver(this._onWindowOpen.bind(this), 'webNavigation-createdNavigationTarget-from-js'),
      this._runtime.events.onErrorFromWorker((domWindow, message, stack, location) => {
        const frame = this._frameTree.frameForDocShell(domWindow.docShell);
        if (!frame)
          return;
        this._browserPage.emit('pageUncaughtError', {
          frameId: frame.id(),
          message,
          stack,
          location,
        });
      }),
      this._runtime.events.onConsoleMessage(msg => this._browserPage.emit('runtimeConsole', msg)),
      this._runtime.events.onRuntimeError(this._onRuntimeError.bind(this)),
      this._runtime.events.onExecutionContextCreated(this._onExecutionContextCreated.bind(this)),
      this._runtime.events.onExecutionContextDestroyed(this._onExecutionContextDestroyed.bind(this)),
      this._runtime.events.onBindingCalled(this._onBindingCalled.bind(this)),
      browserChannel.register('page', {
        adoptNode: this._adoptNode.bind(this),
        crash: this._crash.bind(this),
        describeNode: this._describeNode.bind(this),
        dispatchKeyEvent: this._dispatchKeyEvent.bind(this),
        dispatchDragEvent: this._dispatchDragEvent.bind(this),
        dispatchTouchEvent: this._dispatchTouchEvent.bind(this),
        dispatchTapEvent: this._dispatchTapEvent.bind(this),
        getContentQuads: this._getContentQuads.bind(this),
        getFullAXTree: this._getFullAXTree.bind(this),
        scrollAccessibleIntoView: this._scrollAccessibleIntoView.bind(this),
        captureCanvasBuffer: this._captureCanvasBuffer.bind(this),
        startMutationWhispers: this._startMutationWhispers.bind(this),
        collectAllRects: this._collectAllRects.bind(this),
        a11ySetText: this._a11ySetText.bind(this),
        getProprioState: this._getProprioState.bind(this),
        readCookieEvents: this._readCookieEvents.bind(this),
        readMutationWhispers: this._readMutationWhispers.bind(this),
        readAccEvents: this._readAccEvents.bind(this),
        readFrameStats: this._readFrameStats.bind(this),
        sendWebSocketMessage: this._sendWebSocketMessage.bind(this),
        readTimingReport: this._readTimingReport.bind(this),
        waitVisualStable: this._waitVisualStable.bind(this),
        insertText: this._insertText.bind(this),
        scrollIntoViewIfNeeded: this._scrollIntoViewIfNeeded.bind(this),
        setFileInputFiles: this._setFileInputFiles.bind(this),
        evaluate: this._runtime.evaluate.bind(this._runtime),
        callFunction: this._runtime.callFunction.bind(this._runtime),
        getObjectProperties: this._runtime.getObjectProperties.bind(this._runtime),
        disposeObject: this._runtime.disposeObject.bind(this._runtime),
      }),
    ];
  }

  _emitAllEvents(frame) {
    this._browserPage.emit('pageEventFired', {
      frameId: frame.id(),
      name: 'DOMContentLoaded',
    });
    this._browserPage.emit('pageEventFired', {
      frameId: frame.id(),
      name: 'load',
    });
  }

  _onExecutionContextCreated(executionContext) {
    this._browserPage.emit('runtimeExecutionContextCreated', {
      executionContextId: executionContext.id(),
      auxData: executionContext.auxData(),
    });
  }

  _onExecutionContextDestroyed(executionContext) {
    this._browserPage.emit('runtimeExecutionContextDestroyed', {
      executionContextId: executionContext.id(),
    });
  }

  _onWorkerCreated(worker) {
    const workerData = new WorkerData(this, this._browserChannel, worker);
    this._workerData.set(worker.id(), workerData);
    this._browserPage.emit('pageWorkerCreated', {
      workerId: worker.id(),
      frameId: worker.frame().id(),
      url: worker.url(),
    });
  }

  _onWorkerDestroyed(worker) {
    const workerData = this._workerData.get(worker.id());
    if (!workerData)
      return;
    this._workerData.delete(worker.id());
    workerData.dispose();
    this._browserPage.emit('pageWorkerDestroyed', {
      workerId: worker.id(),
    });
  }

  _onWindowOpen(subject) {
    if (!(subject instanceof Ci.nsIPropertyBag2))
      return;
    const props = subject.QueryInterface(Ci.nsIPropertyBag2);
    const hasUrl = props.hasKey('url');
    const createdDocShell = props.getPropertyAsInterface('createdTabDocShell', Ci.nsIDocShell);
    if (!hasUrl && createdDocShell === this._docShell && this._frameTree.forcePageReady())
      this._emitAllEvents(this._frameTree.mainFrame());
  }

  _linkClicked(sync, anchorElement) {
    // Firefox 152 renamed `ownerGlobal` to `documentGlobal` on nodes.
    if ((anchorElement.documentGlobal || anchorElement.ownerGlobal).docShell !== this._docShell)
      return;
    this._browserPage.emit('pageLinkClicked', { phase: sync ? 'after' : 'before' });
  }

  _onWindowOpenInNewContext(docShell) {
    // TODO: unify this with _onWindowOpen if possible.
    const frame = this._frameTree.frameForDocShell(docShell);
    if (!frame)
      return;
    this._browserPage.emit('pageWillOpenNewWindowAsynchronously');
  }

  _filePickerShown(inputElement) {
    const frame = this._findFrameForNode(inputElement);
    if (!frame)
      return;
    this._browserPage.emit('pageFileChooserOpened', {
      executionContextId: frame.mainExecutionContext().id(),
      element: frame.mainExecutionContext().rawValueToRemoteObject(inputElement)
    });
  }

  _findFrameForNode(node) {
    return this._frameTree.frames().find(frame => {
      const doc = frame.domWindow().document;
      return node === doc || node.ownerDocument === doc;
    });
  }

  onWindowEvent(event) {
    if (event.type !== 'DOMContentLoaded' && event.type !== 'load')
      return;
    // Firefox 152: `ownerGlobal` may be null here; fall back to `defaultView`.
    const win = event.target.ownerGlobal || event.target.defaultView;
    if (!win)
      return;
    const docShell = win.docShell;
    const frame = this._frameTree.frameForDocShell(docShell);
    if (!frame)
      return;
    this._browserPage.emit('pageEventFired', {
      frameId: frame.id(),
      name: event.type,
    });
  }

  _onRuntimeError({ executionContext, message, stack, location }) {
    this._browserPage.emit('pageUncaughtError', {
      frameId: executionContext.auxData().frameId,
      message: message.toString(),
      stack: stack.toString(),
      location,
    });
  }

  _onDocumentOpenLoad(document) {
    // Firefox 152: `ownerGlobal` may be null; fall back to `defaultView`.
    const win = document.ownerGlobal || document.defaultView;
    if (!win)
      return;
    const docShell = win.docShell;
    const frame = this._frameTree.frameForDocShell(docShell);
    if (!frame)
      return;
    this._browserPage.emit('pageEventFired', {
      frameId: frame.id(),
      name: 'load'
    });
  }

  _onNavigationStarted(frame) {
    this._browserPage.emit('pageNavigationStarted', {
      frameId: frame.id(),
      navigationId: frame.pendingNavigationId(),
    });
  }

  _onNavigationAborted(frame, navigationId, errorText) {
    this._browserPage.emit('pageNavigationAborted', {
      frameId: frame.id(),
      navigationId,
      errorText,
    });
    if (!frame._initialNavigationDone && frame !== this._frameTree.mainFrame())
      this._emitAllEvents(frame);
    frame._initialNavigationDone = true;
  }

  _onSameDocumentNavigation(frame) {
    this._browserPage.emit('pageSameDocumentNavigation', {
      frameId: frame.id(),
      url: frame.url(),
    });
  }

  _onNavigationCommitted(frame) {
    this._browserPage.emit('pageNavigationCommitted', {
      frameId: frame.id(),
      navigationId: frame.lastCommittedNavigationId() || undefined,
      url: frame.url(),
      name: frame.name(),
    });
    frame._initialNavigationDone = true;
  }

  _onFrameAttached(frame) {
    this._browserPage.emit('pageFrameAttached', {
      frameId: frame.id(),
      parentFrameId: frame.parentFrame() ? frame.parentFrame().id() : undefined,
    });
  }

  _onFrameDetached(frame) {
    this._browserPage.emit('pageFrameDetached', {
      frameId: frame.id(),
    });
  }

  _onBindingCalled({executionContextId, name, payload}) {
    this._browserPage.emit('pageBindingCalled', {
      executionContextId,
      name,
      payload
    });
  }

  dispose() {
    for (const workerData of this._workerData.values())
      workerData.dispose();
    this._workerData.clear();
    helper.removeListeners(this._eventListeners);
  }

  async _adoptNode({frameId, objectId, executionContextId}) {
    const frame = this._frameTree.frame(frameId);
    if (!frame)
      throw new Error('Failed to find frame with id = ' + frameId);
    let unsafeObject;
    if (!objectId) {
      unsafeObject = frame.domWindow().frameElement;
    } else {
      unsafeObject = frame.unsafeObject(objectId);
    }
    const context = this._runtime.findExecutionContext(executionContextId);
    const fromPrincipal = unsafeObject.nodePrincipal;
    const toFrame = this._frameTree.frame(context.auxData().frameId);
    const toPrincipal = toFrame.domWindow().document.nodePrincipal;
    if (!toPrincipal.subsumes(fromPrincipal))
      return { remoteObject: null };
    return { remoteObject: context.rawValueToRemoteObject(unsafeObject) };
  }

  async _setFileInputFiles({objectId, frameId, files}) {
    const frame = this._frameTree.frame(frameId);
    if (!frame)
      throw new Error('Failed to find frame with id = ' + frameId);
    const unsafeObject = frame.unsafeObject(objectId);
    if (!unsafeObject)
      throw new Error('Object is not input!');
    let nsFiles;
    if (unsafeObject.webkitdirectory) {
      nsFiles = await new Directory(files[0]).getFiles(true);
    } else {
      nsFiles = await Promise.all(files.map(filePath => File.createFromFileName(filePath)));
    }
    unsafeObject.mozSetFileArray(nsFiles);
    // The file picker's own shape (DispatchEvents(), HTMLInputElement.cpp).
    // Upstream marks both cancelable and composed, which reads as synthetic.
    const events = [
      new (frame.domWindow().Event)('input', { bubbles: true, composed: true }),
      new (frame.domWindow().Event)('change', { bubbles: true }),
    ];
    for (const event of events)
      unsafeObject.dispatchEvent(event);
  }

  _getContentQuads({objectId, frameId}) {
    const frame = this._frameTree.frame(frameId);
    if (!frame)
      throw new Error('Failed to find frame with id = ' + frameId);
    const unsafeObject = frame.unsafeObject(objectId);
    if (!unsafeObject.getBoxQuads)
      throw new Error('RemoteObject is not a node');
    const quads = unsafeObject.getBoxQuads({relativeTo: this._frameTree.mainFrame().domWindow().document, recurseWhenNoFrame: true}).map(quad => {
      return {
        p1: {x: quad.p1.x, y: quad.p1.y},
        p2: {x: quad.p2.x, y: quad.p2.y},
        p3: {x: quad.p3.x, y: quad.p3.y},
        p4: {x: quad.p4.x, y: quad.p4.y},
      };
    });
    return {quads};
  }

  _describeNode({objectId, frameId}) {
    const frame = this._frameTree.frame(frameId);
    if (!frame)
      throw new Error('Failed to find frame with id = ' + frameId);
    const unsafeObject = frame.unsafeObject(objectId);
    const browsingContextGroup = frame.docShell().browsingContext.group;
    const frames = this._frameTree.allFramesInBrowsingContextGroup(browsingContextGroup);
    let contentFrame;
    let ownerFrame;
    for (const frame of frames) {
      if (unsafeObject.contentWindow && frame.docShell() === unsafeObject.contentWindow.docShell)
        contentFrame = frame;
      const document = frame.domWindow().document;
      if (unsafeObject === document || unsafeObject.ownerDocument === document)
        ownerFrame = frame;
    }
    return {
      contentFrameId: contentFrame ? contentFrame.id() : undefined,
      ownerFrameId: ownerFrame ? ownerFrame.id() : undefined,
    };
  }

  async _scrollIntoViewIfNeeded({objectId, frameId, rect}) {
    const frame = this._frameTree.frame(frameId);
    if (!frame)
      throw new Error('Failed to find frame with id = ' + frameId);
    const unsafeObject = frame.unsafeObject(objectId);
    if (!unsafeObject.isConnected)
      throw new Error('Node is detached from document');
    if (!rect)
      rect = { x: -1, y: -1, width: -1, height: -1};
    if (unsafeObject.scrollRectIntoViewIfNeeded)
      unsafeObject.scrollRectIntoViewIfNeeded(rect.x, rect.y, rect.width, rect.height);
    else
      throw new Error('Node does not have a layout object');
  }

  _getNodeBoundingBox(unsafeObject) {
    if (!unsafeObject.getBoxQuads)
      throw new Error('RemoteObject is not a node');
    const quads = unsafeObject.getBoxQuads({relativeTo: this._frameTree.mainFrame().domWindow().document});
    if (!quads.length)
      return;
    let x1 = Infinity;
    let y1 = Infinity;
    let x2 = -Infinity;
    let y2 = -Infinity;
    for (const quad of quads) {
      const boundingBox = quad.getBounds();
      x1 = Math.min(boundingBox.x, x1);
      y1 = Math.min(boundingBox.y, y1);
      x2 = Math.max(boundingBox.x + boundingBox.width, x2);
      y2 = Math.max(boundingBox.y + boundingBox.height, y2);
    }
    return {x: x1, y: y1, width: x2 - x1, height: y2 - y1};
  }

  async _dispatchKeyEvent({type, keyCode, code, key, repeat, location, text}) {
    const frame = this._frameTree.mainFrame();
    const tip = frame.textInputProcessor();
    let keyEvent = new (frame.domWindow().KeyboardEvent)("", {
      key,
      code,
      location,
      repeat,
      keyCode
    });
    if (type === 'keydown') {
      if (text && text !== key) {
        tip.commitCompositionWith(text, keyEvent);
      } else {
        const flags = 0;
        tip.keydown(keyEvent, flags);
      }
    } else if (type === 'keyup') {
      if (text)
        throw new Error(`keyup does not support text option`);
      const flags = 0;
      tip.keyup(keyEvent, flags);
    } else {
      throw new Error(`Unknown type ${type}`);
    }
  }

  async _dispatchTouchEvent({type, touchPoints, modifiers}) {
    // Firefox 152+: windowUtils.sendTouchEvent (parallel-array API) was removed.
    // Synthetic touch now goes through Window.synthesizeTouchEvent, which takes a
    // sequence of SynthesizeTouchEventData objects. Mirrors upstream Playwright.
    const frame = this._frameTree.mainFrame();
    const defaultPrevented = frame.domWindow().synthesizeTouchEvent(
      type.toLowerCase(),
      touchPoints.map((point, id) => ({
        identifier: id,
        offsetX: point.x,
        offsetY: point.y,
        radiiX: point.radiusX ?? 1.0,
        radiiY: point.radiusY ?? 1.0,
        rotationAngle: point.rotationAngle ?? 0.0,
        pressure: point.force ?? 1.0,
        tiltX: 0,
        tiltY: 0,
        twist: 0,
      })),
      modifiers
    );
    return {defaultPrevented};
  }

  async _dispatchTapEvent({x, y, modifiers}) {
    // Force a layout at the point in question, because touch events
    // do not seem to trigger one like mouse events.
    this._frameTree.mainFrame().domWindow().windowUtils.elementFromPoint(
      x,
      y,
      false /* aIgnoreRootScrollFrame */,
      true /* aFlushLayout */);

    await this._dispatchTouchEvent({
      type: 'touchstart',
      modifiers,
      touchPoints: [{x, y}]
    });
    await this._dispatchTouchEvent({
      type: 'touchend',
      modifiers,
      touchPoints: [{x, y}]
    });
  }

  _getCurrentDragSession() {
    const frame = this._frameTree.mainFrame();
    const domWindow = frame?.domWindow();
    return domWindow ? dragService.getCurrentSession(domWindow) : undefined;
  }

  async _dispatchDragEvent({type, x, y, modifiers}) {
    const session = this._getCurrentDragSession();
    const dropEffect = session.dataTransfer.dropEffect;

    if ((type === 'drop' && dropEffect !== 'none') || type ===  'dragover') {
      const win = this._frameTree.mainFrame().domWindow();
      win.windowUtils.jugglerSendMouseEvent(
        type,
        x,
        y,
        0, /*button*/
        0, /*clickCount*/
        modifiers,
        false /*aIgnoreRootScrollFrame*/,
        0.0 /*pressure*/,
        0 /*inputSource*/,
        true /*isDOMEventSynthesized*/,
        false /*isWidgetEventSynthesized*/,
        0 /*buttons*/,
        win.windowUtils.DEFAULT_MOUSE_POINTER_ID /* pointerIdentifier */,
        false /*disablePointerEvent*/,
      );
      return;
    }
    if (type === 'dragend') {
      const session = this._getCurrentDragSession();
      session?.endDragSession(true);
      return;
    }
  }

  async _insertText({text}) {
    const frame = this._frameTree.mainFrame();
    const win = frame.domWindow();
    const doc = win.document;
    const active = doc.activeElement;
    // Fast path: if focus is on an editable input/textarea, set the value
    // directly and fire a single trusted-shape input event. This avoids the
    // double `input` event we get from nsITextInputProcessor on Firefox 146
    // (one for compositionupdate, one after compositionend), and matches the
    // upstream test expectation of exactly one `input` event.
    const isEditableField = active && (
      (active.tagName === 'INPUT' && /^(text|search|url|tel|email|password|number|)$/i.test(active.type || '')) ||
      active.tagName === 'TEXTAREA'
    );
    if (isEditableField) {
      const start = active.selectionStart ?? active.value.length;
      const end = active.selectionEnd ?? active.value.length;
      const before = active.value.slice(0, start);
      const after = active.value.slice(end);
      active.value = before + text + after;
      const caret = (before + text).length;
      try { active.setSelectionRange(caret, caret); } catch (e) {}
      const InputEvent = win.InputEvent;
      active.dispatchEvent(new InputEvent('input', {
        bubbles: true,
        cancelable: false,
        composed: true,
        inputType: 'insertText',
        data: text,
      }));
      return;
    }
    // Fallback: contenteditable / other editing hosts use the TIP path.
    frame.textInputProcessor().commitCompositionWith(text);
  }

  async _crash() {
    dump(`Crashing intentionally\n`);
    // This is to intentionally crash the frame.
    // We crash by using js-ctypes and dereferencing
    // a bad pointer. The crash should happen immediately
    // upon loading this frame script.
    const { ctypes } = ChromeUtils.importESModule('resource://gre/modules/ctypes.sys.mjs');
    ChromeUtils.privateNoteIntentionalCrash();
    const zero = new ctypes.intptr_t(8);
    const badptr = ctypes.cast(zero, ctypes.PointerType(ctypes.int32_t));
    badptr.contents;
  }

  // M4 Proprioception: the honest body state — load state, a11y focus,
  // native selection/caret, scrollers, viewport. The privileged frame
  // script reads the real Selection and the a11y focus tree; pages
  // cannot fake either from content JS.
  async _getProprioState() {
    const frame = this._frameTree.mainFrame();
    const win = frame.domWindow();
    const doc = win.document;
    const out = {
      readyState: doc.readyState,
      url: String(win.location.href).slice(0, 500),
      title: doc.title,
      viewport: { w: win.innerWidth, h: win.innerHeight, dpr: win.devicePixelRatio },
      scroll: { x: win.scrollX, y: win.scrollY },
      scrollers: [],
      focus: null,
      selection: null,
    };
    const service = Cc["@mozilla.org/accessibilityService;1"]
      .getService(Ci.nsIAccessibilityService);
    try {
      const docAcc = service.getAccessibleFor(doc);
      if (docAcc) {
        const f = service.getFocusedChild(docAcc);
        if (f) {
          out.focus = {
            role: service.getStringRole(f.role),
            name: (f.name || '').slice(0, 80),
          };
        }
      }
    } catch (e) { /* a11y may be off */ }
    if (!out.focus) {
      const el = doc.activeElement;
      if (el && el !== doc.body && el !== doc.documentElement) {
        out.focus = {
          tag: el.tagName,
          id: el.id || null,
          name: (el.getAttribute && (el.getAttribute('aria-label') || el.getAttribute('name') || el.getAttribute('placeholder'))) || null,
          type: el.type || null,
        };
      }
    }
    const sel = win.getSelection();
    if (sel) {
      const txt = sel.toString();
      const an = sel.anchorNode;
      out.selection = {
        text: txt ? txt.slice(0, 120) : null,
        anchorOffset: sel.anchorOffset,
        focusOffset: sel.focusOffset,
        collapsed: sel.isCollapsed,
        anchorTag: an ? (an.nodeType === 1 ? an.tagName : '#text') : null,
        anchorText: an && an.nodeType === 3 ? an.data.slice(Math.max(0, sel.anchorOffset - 30), sel.anchorOffset + 30) : null,
      };
    }
    const els = doc.querySelectorAll('*');
    const scrollers = [];
    for (const el of els) {
      if (el.scrollHeight > el.clientHeight + 2 || el.scrollWidth > el.clientWidth + 2) {
        if (scrollers.length < 16) {
          const cs = win.getComputedStyle(el);
          scrollers.push({
            tag: el.tagName,
            id: el.id || null,
            cls: (typeof el.className === 'string' ? el.className : '').slice(0, 40),
            st: el.scrollTop || 0,
            sl: el.scrollLeft || 0,
            sh: el.scrollHeight,
            ch: el.clientHeight,
            overflowY: cs.overflowY,
          });
        }
      }
    }
    out.scrollers = scrollers;
    return { state: out };
  }

  // M4 Proprioception: the cookie/session heartbeat. Changes to cookies
  // are recorded as {kind, host, name, flags} — NEVER the value. An
  // auth cookie being deleted/cleared = the earliest possible signal
  // that a session died (the LinkedIn silent-revocation lesson).
  _readCookieEvents({ clear } = {}) {
    if (!this._cookieObserver) {
      this._cookieEvents = [];
      this._cookieObserver = {
        observe: (subject, topic, data) => {
          try {
            const kind = data || topic;
            let host = '', name = '', path = '', httpOnly = false, secure = false, expiry = 0;
            try {
              if (subject) {
                host = subject.host || '';
                name = subject.name || '';
                path = subject.path || '';
                httpOnly = !!subject.isHttpOnly;
                secure = !!subject.isSecure;
                expiry = subject.expiry || 0;
              }
            } catch (e) { /* subject may lack cookie fields */ }
            this._cookieEvents.push({ kind, host, name, path, httpOnly, secure, expiry });
            if (this._cookieEvents.length > 200) this._cookieEvents.shift();
          } catch (e) { /* never let the observer throw */ }
        },
      };
      Services.obs.addObserver(this._cookieObserver, 'cookie-changed');
      Services.obs.addObserver(this._cookieObserver, 'private-cookie-changed');
      Services.obs.addObserver(this._cookieObserver, 'cookie-batch-deleted');
    }
    const out = this._cookieEvents || [];
    if (clear) this._cookieEvents = [];
    return { events: out };
  }

  // M3 Hearing: the accessibility EVENT STREAM — the incremental diff.
  // One persistent observer on the engine's own "accessible-event" topic
  // (the a11y tree as a stream, not full snapshots). Every event is
  // compacted to {type, role, name, + detail} — read it instead of
  // re-walking the full tree after every action.
  _readAccEvents({ clear } = {}) {
    if (!this._accEventObserver) {
      this._accEvents = [];
      const service = Cc["@mozilla.org/accessibilityService;1"]
        .getService(Ci.nsIAccessibilityService);
      this._accEventObserver = {
        observe: (subject, topic) => {
          try {
            if (topic !== "accessible-event")
              return;
            const event = subject.QueryInterface(Ci.nsIAccessibleEvent);
            const acc = event.accessible;
            const rec = {
              type: service.getStringEventType(event.eventType),
              role: acc ? service.getStringRole(acc.role) : null,
              name: acc ? (acc.name || '').slice(0, 80) : null,
              ts: Date.now(),
            };
            if (event.eventType === Ci.nsIAccessibleEvent.EVENT_TEXT_INSERTED ||
                event.eventType === Ci.nsIAccessibleEvent.EVENT_TEXT_REMOVED) {
              try {
                const te = subject.QueryInterface(Ci.nsIAccessibleTextChangeEvent);
                rec.text = (te.modifiedText || '').slice(0, 120);
                rec.start = te.start;
              } catch (e) { /* not a text change */ }
            }
            if (event.eventType === Ci.nsIAccessibleEvent.EVENT_STATE_CHANGE) {
              try {
                const se = subject.QueryInterface(Ci.nsIAccessibleStateChangeEvent);
                const names = service.getStringStates(se.state, 0);
                const arr = [];
                if (names) {
                  for (const nm of names)
                    arr.push(String(nm));
                }
                rec.state = arr.join(',');
                rec.isEnabled = se.isEnabled;
              } catch (e) { /* not a state change */ }
            }
            if (event.eventType === Ci.nsIAccessibleEvent.EVENT_VALUE_CHANGE && acc) {
              rec.value = (acc.value || '').slice(0, 120);
            }
            if (event.eventType === Ci.nsIAccessibleEvent.EVENT_TEXT_CARET_MOVED) {
              try {
                const ce = subject.QueryInterface(Ci.nsIAccessibleCaretMoveEvent);
                rec.caretOffset = ce.caretOffset;
              } catch (e) { /* not a caret move */ }
            }
            if (event.eventType === Ci.nsIAccessibleEvent.EVENT_ANNOUNCEMENT) {
              try {
                const an = subject.QueryInterface(Ci.nsIAccessibleAnnouncementEvent);
                rec.announcement = (an.announcement || '').slice(0, 160);
              } catch (e) { /* not an announcement */ }
            }
            this._accEvents.push(rec);
            if (this._accEvents.length > 300)
              this._accEvents.shift();
          } catch (e) { /* never let the observer throw */ }
        },
      };
      Services.obs.addObserver(this._accEventObserver, "accessible-event");
    }
    const out = this._accEvents || [];
    if (clear) this._accEvents = [];
    return { events: out };
  }

  // M3.5 Smell — the frame sampler: rAF cadence measured from the
  // privileged frame script (the content main thread the page JS runs
  // on). Frame deltas ARE the jank signal: a busy main thread shows as
  // a delta spike, a clean page sits at ~16.7ms. Re-armed per document
  // so navigations start a fresh sampler.
  _ensureFrameSampler() {
    const win = this._frameTree.mainFrame().domWindow();
    if (this._frameSampler && this._frameSampler.doc === win.document)
      return;
    this._frameSampler = {
      doc: win.document,
      startedAt: win.performance.now(),
      frames: 0,
      deltas: [],
      lastTs: null,
    };
    const s = this._frameSampler;
    const step = (t) => {
      if (s.lastTs !== null) {
        const d = t - s.lastTs;
        if (d > 0) {
          s.deltas.push(d);
          if (s.deltas.length > 600)
            s.deltas.shift();
        }
      }
      s.lastTs = t;
      s.frames++;
      win.requestAnimationFrame(step);
    };
    win.requestAnimationFrame(step);
  }

  _sendWebSocketMessage({ wsid, message }) {
    // WS injection: the service instance in THIS process holds the
    // serialID -> impl map for page-created sockets (DevTools inspector
    // route). wsid is the serialID as a string (see _readAccEvents/
    // FrameTree wsid).
    const serialID = parseInt(wsid, 10);
    if (!Number.isFinite(serialID)) {
      return { ok: false, error: 'bad wsid' };
    }
    const service = this._frameTree._webSocketEventService;
    if (!service) {
      return { ok: false, error: 'no websocket event service' };
    }
    try {
      service.sendMessage(serialID, message);
      return { ok: true };
    } catch (e) {
      return { ok: false, error: String(e) };
    }
  }

  _readFrameStats({ reset } = {}) {
    this._ensureFrameSampler();
    const s = this._frameSampler;
    const deltas = s.deltas.slice();
    const now = this._frameTree.mainFrame().domWindow().performance.now();
    if (reset) {
      s.deltas = [];
      s.startedAt = now;
      s.frames = 0;
      s.lastTs = null;
    }
    const sorted = deltas.slice().sort((a, b) => a - b);
    const pct = p => sorted.length ?
      sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))] : 0;
    const r1 = x => Math.round(x * 10) / 10;
    return {
      frames: s.frames,
      elapsedMs: Math.round(now - s.startedAt),
      avgMs: deltas.length ? r1(deltas.reduce((a, b) => a + b, 0) / deltas.length) : 0,
      p50Ms: r1(pct(0.50)),
      p95Ms: r1(pct(0.95)),
      p99Ms: r1(pct(0.99)),
      maxMs: r1(pct(1.0)),
      jankyFrames: deltas.filter(d => d > 50).length,
      throttledFrames: deltas.filter(d => d > 250).length,
    };
  }

  // M3.5 Smell — visual stability: wait until the refresh driver has been
  // delivering frames at normal cadence (<=32ms) for quietMs consecutively.
  // The true successor to arbitrary sleep(3): "wait until VISUALLY ready".
  async _waitVisualStable({ quietMs = 250, timeoutMs = 5000 } = {}) {
    this._ensureFrameSampler();
    const win = this._frameTree.mainFrame().domWindow();
    const started = win.performance.now();
    let quietStart = null;
    while (win.performance.now() - started < timeoutMs) {
      await new Promise(x => win.requestAnimationFrame(x));
      const s = this._frameSampler;
      const last = s.deltas.length ? s.deltas[s.deltas.length - 1] : undefined;
      if (last !== undefined && last <= 32) {
        if (quietStart === null)
          quietStart = win.performance.now();
        if (win.performance.now() - quietStart >= quietMs)
          return { stable: true, ms: Math.round(win.performance.now() - started) };
      } else {
        quietStart = null;
      }
    }
    return { stable: false, ms: Math.round(win.performance.now() - started) };
  }

  // M3.5 Smell — timing sense: the modern Navigation Timing + Paint
  // entries for the current document. DNS/TLS/connect/TTFB + first-paint
  // & first-contentful-paint, from the window's own performance buffer.
  _readTimingReport() {
    const win = this._frameTree.mainFrame().domWindow();
    const perf = win.performance;
    const nav = {};
    const paint = {};
    try {
      const nt = perf.getEntriesByType('navigation')[0];
      if (nt) {
        nav.dnsMs = Math.round(nt.domainLookupEnd - nt.domainLookupStart);
        nav.tlsMs = Math.round(nt.connectEnd - nt.secureConnectionStart);
        nav.connectMs = Math.round(nt.connectEnd - nt.connectStart);
        nav.ttfbMs = Math.round(nt.responseStart - nt.requestStart);
        nav.domInteractiveMs = Math.round(nt.domInteractive);
        nav.domContentLoadedMs = Math.round(nt.domContentLoadedEventEnd);
        nav.loadEventMs = Math.round(nt.loadEventEnd);
        nav.transferSize = nt.transferSize;
        nav.redirectCount = nt.redirectCount;
      }
    } catch (e) { /* timing may be unavailable */ }
    try {
      for (const p of perf.getEntriesByType('paint'))
        paint[p.name] = Math.round(p.startTime);
    } catch (e) { /* paint entries may be unavailable */ }
    return { navigation: nav, paint };
  }

  // M5/Flutter: set a text field's content through the ACCESSIBILITY
  // protocol (nsIAccessibleEditableText.setTextContents) — the AT-native
  // route. Flutter web in semantics mode edits via the a11y layer, not
  // DOM input events, so the AT action is the only route that syncs the
  // Dart controllers.
  async _a11ySetText({role, name, text}) {
    const service = Cc["@mozilla.org/accessibilityService;1"]
      .getService(Ci.nsIAccessibilityService);
    const win = this._frameTree.mainFrame().domWindow();
    const docAcc = service.getAccessibleFor(win.document);
    if (!docAcc) return { error: 'no document accessible' };
    // The tree builds lazily — wait for the a11y update to complete
    // before walking (same loop as _getFullAXTree).
    let waits = 0;
    while (docAcc.document.isUpdatePendingForJugglerAccessibility && waits++ < 50) {
      await new Promise(x => win.requestAnimationFrame(x));
    }
    const find = (acc) => {
      const r = service.getStringRole(acc.role);
      if ((role === '' || r === role) && acc.name && acc.name.includes(name))
        return acc;
      for (let child = acc.firstChild; child; child = child.nextSibling) {
        const hit = find(child);
        if (hit) return hit;
      }
      return null;
    };
    const target = find(docAcc);
    if (!target) {
      const seen = [];
      const dump = (acc) => {
        if (seen.length < 20) {
          seen.push(service.getStringRole(acc.role) + ':' + (acc.name || '').slice(0, 24));
        }
        for (let child = acc.firstChild; child; child = child.nextSibling) dump(child);
      };
      dump(docAcc);
      return { error: `no accessible for role=${role} name=${name} — tree: ${seen.join(' | ')}` };
    }
    // The Flutter web semantics text field consumes edits through its
    // OWN DOM element: the strategy activates on focus and the input
    // handler reads the element's .value (EditingState.fromDomElement).
    // setTextContents writes the a11y mirror but nobody reads it, so
    // the winning route = focus + set value + trusted input event.
    target.takeFocus();
    for (let i = 0; i < 2; i++)
      await new Promise(x => win.requestAnimationFrame(x));
    let node = null;
    try {
      node = target.DOMNode;
    } catch (e) {
      node = null;
    }
    if (node) {
      try {
        node.value = String(text);
        try {
          node.setSelectionRange(String(text).length, String(text).length);
        } catch (e) {
          // Xray may refuse the selection set on some inputs — the
          // value itself is what the framework's input handler reads.
        }
        const ev = new node.ownerGlobal.InputEvent('input', {
          inputType: 'insertText',
          data: String(text),
          bubbles: true,
          composed: true,
        });
        node.dispatchEvent(ev);
        return { ok: true };
      } catch (e) {
        // fall through to the editable-text route
      }
    }
    let editable = null;
    try {
      editable = target.QueryInterface(Ci.nsIAccessibleEditableText);
    } catch (e) {
      return { error: 'accessible is not editable text' };
    }
    editable.setTextContents(String(text));
    return { ok: true };
  }

  // M5 the Critic: every element's honest layout rect from the frame
  // script (Xray getBoundingClientRect — page hooks cannot reach this
  // call). The a11y tree prunes non-accessible elements; the full DOM
  // walk sees empty divs too (the overlap detector needs them).
  _collectAllRects() {
    const doc = this._frameTree.mainFrame().domWindow().document;
    const out = [];
    const els = doc.querySelectorAll('*');
    for (const el of els) {
      const r = el.getBoundingClientRect();
      if (r.width < 2 || r.height < 2) continue;
      out.push({
        tag: el.tagName,
        x: Math.round(r.x),
        y: Math.round(r.y),
        w: Math.round(r.width),
        h: Math.round(r.height),
      });
      if (out.length >= 1200) break;
    }
    return { rects: out.map((r) => JSON.stringify(r)) };
  }

  // M3 Hearing: unhookable DOM-change whispers. The observer lives in
  // the PRIVILEGED frame-script realm — the page cannot hook or poison
  // the observer itself; it observes the REAL DOM mutations natively.
  _startMutationWhispers() {
    const win = this._frameTree.mainFrame().domWindow();
    if (!this._whisperObserver) {
      this._whispers = [];
      const push = (m) => {
        this._whispers.push(m);
        if (this._whispers.length > 300) this._whispers.shift();
      };
      this._whisperCount = 0;
      this._whisperObserver = new win.MutationObserver((records) => {
        this._whisperCount += records.length;
        try {
          for (const rec of records) {
            const t = rec.target;
            const tag = t && t.nodeType === 1 ? t.tagName : (t && t.nodeType === 3 ? '#text' : '#?');
            let text = '';
            if (rec.type === 'characterData') {
              text = (rec.target.data || '').slice(0, 60);
            } else if (rec.type === 'childList') {
              const added = [];
              for (const n of rec.addedNodes) {
                if (n.nodeType === 1) added.push(n.tagName);
                else if (n.nodeType === 3 && (n.data || '').trim()) added.push('text:' + n.data.trim().slice(0, 40));
              }
              text = added.slice(0, 4).join(',');
            }
            push({
              type: rec.type,
              tag,
              attr: rec.type === 'attributes' ? rec.attributeName : null,
              text,
              added: rec.addedNodes.length,
              removed: rec.removedNodes.length,
            });
          }
        } catch (e) {
          push({ type: 'observer-error', text: String(e && e.message || e).slice(0, 120) });
        }
      });
      this._whisperObserver.observe(win.document, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
      });
    }
    return { ok: true };
  }

  _readMutationWhispers({clear}) {
    const out = this._whispers || [];
    if (clear) this._whispers = [];
    return { whispers: out.map((w) => JSON.stringify(w)) };
  }

  async _captureCanvasBuffer({selector, ref}) {
    // Read a canvas's DRAWING BUFFER from the privileged frame script:
    // the Xray wrapper calls the NATIVE APIs, so page-level hooks on
    // HTMLCanvasElement.prototype cannot poison or observe the read.
    // Needed for hidden canvases (CSS 0x0) that the compositor never
    // renders — the GeeTest fullbg case.
    const win = this._frameTree.mainFrame().domWindow();
    const doc = win.document;
    let el = null;
    if (selector) {
      el = doc.querySelector(selector);
    } else {
      // Page-realm Map: waive the Xray to read the page's own registry.
      try {
        const map = Cu.waiveXrays(win).__gfxRefs;
        el = map ? map.get(ref) : null;
      } catch (e) {
        return { error: 'ref resolve failed: ' + e.message };
      }
    }
    if (!el || el.tagName !== 'CANVAS')
      return { error: `canvas not found: ${selector || ref} (el=${el ? el.tagName : 'null'}, map=${(Cu.waiveXrays(win).__gfxRefs) ? 'ada' : 'kosong'})` };
    // transferControlToOffscreen case: the placeholder element reports
    // 0x0 while a WORKER owns the real buffer. The GfxXray service
    // reads the worker-side frame through the main-thread display
    // helper (mutex-guarded, below the JS layer).
    // GfxXray native read first — covers 2D, WebGL AND worker-transferred
    // OffscreenCanvas with zero JS in the pixel path. Returns '' when
    // the canvas has no frame; fall through to the JS paths otherwise.
    try {
      const xray = Cc["@mozilla.org/juggler/gfx-xray;1"].getService(Ci.nsIGfxXray);
      const packed = xray.canvasBuffer(el);
      if (packed) {
        const sep = packed.indexOf(':');
        const dims = packed.slice(0, sep).split('x');
        return { data: packed.slice(sep + 1), width: parseInt(dims[0], 10), height: parseInt(dims[1], 10), raw: true };
      }
    } catch (e) {
      // no GfxXray (old engine) — fall through to the JS paths
    }
    // WebGL path first: toDataURL returns BLANK on WebGL canvases with
    // preserveDrawingBuffer:false (the classic captcha trap) — read the
    // drawing buffer straight from the GL context instead.
    let gl = null;
    try {
      gl = el.getContext('webgl2') || el.getContext('webgl');
    } catch (e) {
      gl = null;
    }
    if (gl) {
      try {
        const w = gl.drawingBufferWidth, h = gl.drawingBufferHeight;
        const buf = new Uint8Array(w * h * 4);
        gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, buf);
        let binary = '';
        for (let i = 0; i < buf.length; i += 8192)
          binary += String.fromCharCode.apply(null, buf.subarray(i, Math.min(i + 8192, buf.length)));
        return { data: btoa(binary), width: w, height: h, raw: true };
      } catch (e) {
        return { error: 'webgl buffer read failed: ' + e.message };
      }
    }
    try {
      return { data: el.toDataURL(), width: el.width, height: el.height };
    } catch (e) {
      return { error: 'buffer read failed: ' + e.message };
    }
  }

  async _scrollAccessibleIntoView({role, name}) {
    const service = Cc["@mozilla.org/accessibilityService;1"]
      .getService(Ci.nsIAccessibilityService);
    const document = this._frameTree.mainFrame().domWindow().document;
    const docAcc = service.getAccessibleFor(document);
    if (!docAcc) return null;
    const find = (acc) => {
      const r = service.getStringRole(acc.role);
      if ((role === '' || r === role) && acc.name && acc.name.includes(name))
        return acc;
      for (let child = acc.firstChild; child; child = child.nextSibling) {
        const hit = find(child);
        if (hit) return hit;
      }
      return null;
    };
    const target = find(docAcc);
    if (!target) return null;
    const win = this._frameTree.mainFrame().domWindow();
    const cssScale = win.devicePixelRatio || 1;
    // Content-area origin inside the browser window — window-relative
    // via the doc accessible's origin (immune to the window's screen
    // position; scroll compensated). Fresh after the a11y settle.
    let chromeX = 0, chromeY = 0;
    {
      let ox = {}, oy = {}, ow = {}, oh = {};
      docAcc.getBoundsInCSSPixels(ox, oy, ow, oh);
      chromeX = ox.value / cssScale + win.scrollX;
      chromeY = oy.value / cssScale + win.scrollY;
    }
    const readBounds = () => {
      let bx = {}, by = {}, bw = {}, bh = {};
      target.getBoundsInCSSPixels(bx, by, bw, bh);
      return {
        x: bx.value / cssScale - chromeX,
        y: by.value / cssScale - chromeY,
        width: bw.value / cssScale,
        height: bh.value / cssScale,
      };
    };
    let b = readBounds();
    const vpW = win.innerWidth;
    const vpH = win.innerHeight;
    // Deterministic instant scroll (scrollTo has no animation): bring
    // the target into the content viewport with a small margin.
    const MARGIN = 24;
    let dy = 0, dx = 0;
    if (b.y < 0) dy = b.y - MARGIN;
    else if (b.y + b.height > vpH) dy = b.y + b.height - vpH + MARGIN;
    if (b.x < 0) dx = b.x - MARGIN;
    else if (b.x + b.width > vpW) dx = b.x + b.width - vpW + MARGIN;
    if (dy !== 0 || dx !== 0) {
      win.scrollTo(win.scrollX + dx, win.scrollY + dy);
      await new Promise(resolve => win.setTimeout(resolve, 120));
    }
    b = readBounds();
    return { bounds: b };
  }

  async _getFullAXTree({objectId}) {
    let unsafeObject = null;
    if (objectId) {
      unsafeObject = this._frameTree.mainFrame().unsafeObject(objectId);
      if (!unsafeObject)
        throw new Error(`No object found for id "${objectId}"`);
    }

    const service = Cc["@mozilla.org/accessibilityService;1"]
      .getService(Ci.nsIAccessibilityService);
    // DPR for normalizing a11y bounds into the page's CSS-pixel space
    // (closure for buildNode, which has no `this`).
    const cssScale = this._frameTree.mainFrame().domWindow().devicePixelRatio || 1;
    const document = this._frameTree.mainFrame().domWindow().document;
    const docAcc = service.getAccessibleFor(document);

    // a11y bounds are window-relative; page CSS is content-viewport-
    // relative. The DOCUMENT accessible's own origin is the anchor —
    // window-relative, immune to the window's screen position (the
    // mozInnerScreenX-screenX trick breaks when a WM places the window
    // off-origin, e.g. x=294 on a remote desktop). The doc origin moves
    // with scroll, so the fixed chrome = docOrigin + scroll.
    const cwin = this._frameTree.mainFrame().domWindow();
    let docOriginX = 0, docOriginY = 0;
    {
      let ox = {}, oy = {}, ow = {}, oh = {};
      docAcc.getBoundsInCSSPixels(ox, oy, ow, oh);
      docOriginX = ox.value / cssScale + cwin.scrollX;
      docOriginY = oy.value / cssScale + cwin.scrollY;
    }

    while (docAcc.document.isUpdatePendingForJugglerAccessibility)
      await new Promise(x => this._frameTree.mainFrame().domWindow().requestAnimationFrame(x));

    async function waitForQuiet() {
      let state = {};
      docAcc.getState(state, {});
      if ((state.value & Ci.nsIAccessibleStates.STATE_BUSY) == 0)
        return;
      let resolve, reject;
      const promise = new Promise((x, y) => {resolve = x, reject = y});
      let eventObserver = {
        observe(subject, topic) {
          if (topic !== "accessible-event") {
            return;
          }

          // If event type does not match expected type, skip the event.
          let event = subject.QueryInterface(Ci.nsIAccessibleEvent);
          if (event.eventType !== Ci.nsIAccessibleEvent.EVENT_STATE_CHANGE) {
            return;
          }

          // If event's accessible does not match expected accessible,
          // skip the event.
          if (event.accessible !== docAcc) {
            return;
          }

          Services.obs.removeObserver(this, "accessible-event");
          resolve();
        },
      };
      Services.obs.addObserver(eventObserver, "accessible-event");
      return promise;
    }
    function buildNode(accElement) {
      let a = {}, b = {};
      accElement.getState(a, b);
      const tree = {
        role: service.getStringRole(accElement.role),
        name: accElement.name || '',
      };
      if (unsafeObject && unsafeObject === accElement.DOMNode)
        tree.foundObject = true;
      for (const userStringProperty of [
        'value',
        'description'
      ]) {
        tree[userStringProperty] = accElement[userStringProperty] || undefined;
      }

      const states = {};
      for (const name of service.getStringStates(a.value, b.value))
        states[name] = true;
      for (const name of ['selected',
        'focused',
        'pressed',
        'focusable',
        'required',
        'invalid',
        'modal',
        'editable',
        'busy',
        'checked',
        'multiselectable']) {
        if (states[name])
          tree[name] = true;
      }

      if (states['multi line'])
        tree['multiline'] = true;
      if (states['editable'] && states['readonly'])
        tree['readonly'] = true;
      if (states['checked'])
        tree['checked'] = true;
      if (states['mixed'])
        tree['checked'] = 'mixed';
      if (states['expanded'])
        tree['expanded'] = true;
      else if (states['collapsed'])
        tree['expanded'] = false;
      if (!states['enabled'])
        tree['disabled'] = true;

      const attributes = {};
      if (accElement.attributes) {
        for (const { key, value } of accElement.attributes.enumerate()) {
          attributes[key] = value;
        }
      }
      for (const numericalProperty of ['level']) {
        if (numericalProperty in attributes)
          tree[numericalProperty] = parseFloat(attributes[numericalProperty]);
      }
      for (const stringProperty of ['tag', 'roledescription', 'valuetext', 'orientation', 'autocomplete', 'keyshortcuts', 'haspopup']) {
        if (stringProperty in attributes)
          tree[stringProperty] = attributes[stringProperty];
      }

      // Native layout bounds — the trusted geometry for M2 semantic
      // clicks. JS can lie about getBoundingClientRect; this comes from
      // the accessibility tree itself. NOTE: getBoundsInCSSPixels returns
      // values in the SPOOFED-DPR space (Camoufox scales the layout to
      // match the identity's devicePixelRatio) — divide by the page's
      // dpr so the numbers land in the same CSS-pixel space the spoofed
      // page scripts see (and our mouse dispatch expects).
      {
        let bx = {}, by = {}, bw = {}, bh = {};
        accElement.getBoundsInCSSPixels(bx, by, bw, bh);
        if (bw.value > 0 && bh.value > 0) {
          tree.bounds = {
            x: bx.value / cssScale - docOriginX,
            y: by.value / cssScale - docOriginY,
            width: bw.value / cssScale, height: bh.value / cssScale,
          };
        }
      }
      const children = [];

      for (let child = accElement.firstChild; child; child = child.nextSibling) {
        children.push(buildNode(child));
      }
      if (children.length)
        tree.children = children;
      return tree;
    }
    await waitForQuiet();
    return {
      tree: buildNode(docAcc)
    };
  }
}

