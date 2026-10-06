/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// Services is available as a global in XPCOM component context

// Load SimpleChannel in browser-process global.
Services.scriptloader.loadSubScript('chrome://juggler/content/SimpleChannel.js');
const {Dispatcher} = ChromeUtils.importESModule("chrome://juggler/content/protocol/Dispatcher.js");
const {BrowserHandler} = ChromeUtils.importESModule("chrome://juggler/content/protocol/BrowserHandler.js");
const {NetworkObserver} = ChromeUtils.importESModule("chrome://juggler/content/NetworkObserver.js");
const {TargetRegistry} = ChromeUtils.importESModule("chrome://juggler/content/TargetRegistry.js");
const {Helper} = ChromeUtils.importESModule('chrome://juggler/content/Helper.js');
const {ActorManagerParent} = ChromeUtils.importESModule('resource://gre/modules/ActorManagerParent.sys.mjs');
const helper = new Helper();

const Cc = Components.classes;
const Ci = Components.interfaces;

// Register JSWindowActors that will be instantiated for each frame.
ActorManagerParent.addJSWindowActors({
  JugglerFrame: {
    parent: {
      esModuleURI: 'chrome://juggler/content/JugglerFrameParent.sys.mjs',
    },
    child: {
      esModuleURI: 'chrome://juggler/content/JugglerFrameChild.sys.mjs',
      events: {
        // Normally, we instantiate an actor when a new window is created.
        DOMWindowCreated: {},
        // However, for same-origin iframes, the navigation from about:blank
        // to the URL will share the same window, so we need to also create
        // an actor for a new document via DOMDocElementInserted.
        DOMDocElementInserted: {},
        // Also, listening to DOMContentLoaded.
        DOMContentLoaded: {},
        DOMWillOpenModalDialog: {},
        DOMModalDialogClosed: {},
      },
    },
    allFrames: true,
  },
});

let browserStartupFinishedCallback;
let browserStartupFinishedPromise = new Promise(x => browserStartupFinishedCallback = x);

export class Juggler {
  get classDescription() { return "Sample command-line handler"; }
  get classID() { return Components.ID('{f7a74a33-e2ab-422d-b022-4fb213dd2639}'); }
  get contractID() { return "@mozilla.org/remote/juggler;1" }
  get QueryInterface() {
    return ChromeUtils.generateQI([ Ci.nsICommandLineHandler, Ci.nsIObserver ]);
  }
  get helpInfo() {
    return "  --juggler            Enable Juggler automation\n";
  }

  handle(cmdLine) {
    // flag has to be consumed in nsICommandLineHandler:handle
    // to avoid issues on macos. See Marionette.jsm::handle() for more details.
    // TODO: remove after Bug 1724251 is fixed.
    cmdLine.handleFlag("juggler-pipe", false);
  }

  // This flow is taken from Remote agent and Marionette.
  // See https://github.com/mozilla-firefox/firefox/blob/35e22180b0b61413dd8eccf6c00b1c6fac073eee/remote/components/RemoteAgent.sys.mjs#L417
  async observe(subject, topic) {
    switch (topic) {
      case "profile-after-change":
        Services.obs.addObserver(this, "command-line-startup");
        Services.obs.addObserver(this, "browser-idle-startup-tasks-finished");
        // Ghostfox Android: GeckoView has no command line and no pipe —
        // serve the juggler protocol over a loopback TCP socket instead.
        // The runtime reaches it through `adb forward tcp:<port> tcp:<port>`.
        if (Services.appinfo.OS === "Android") {
          this._androidPort = Services.prefs.getIntPref("ghostfox.juggler.port", 9222);
          Services.tm.dispatchToMainThread(() => this._androidInit());
        }
        break;
      case "command-line-startup":
        Services.obs.removeObserver(this, topic);
        const cmdLine = subject;
        const jugglerPipeFlag = cmdLine.handleFlag('juggler-pipe', false);
        if (!jugglerPipeFlag)
          return;

        this._silent = cmdLine.findFlag('silent', false) >= 0;
        if (this._silent) {
          Services.startup.enterLastWindowClosingSurvivalArea();
          browserStartupFinishedCallback();
        }
        Services.obs.addObserver(this, "final-ui-startup");
        break;
      case "browser-idle-startup-tasks-finished":
        browserStartupFinishedCallback();
        break;
      // Used to wait until the initial application window has been opened.
      case "final-ui-startup":
        Services.obs.removeObserver(this, topic);
        this._startJuggler(false /* useTcp */);
        break;
    }
  }

  // Ghostfox Android: no pipe — serve the juggler protocol over a loopback
  // TCP socket with the SAME \0 framing as Playwright's PipeTransport.
  _androidInit() {
    if (this._androidStarted)
      return;
    this._androidStarted = true;

    dump(`Juggler: android init, port ${this._androidPort}\n`);
    let connection;
    try {
      connection = this._startJuggler(true /* useTcp */);
    } catch (e) {
      dump(`Juggler: _startJuggler failed: ${e}\n`);
      // The LISTENER comes up regardless — the port must exist so the
      // tunnel has something to connect to, and we can debug from afar.
    }
    const self = this;
    let server;
    try {
      server = Cc["@mozilla.org/network/server-socket;1"].createInstance(Ci.nsIServerSocket);
      server.init(this._androidPort, true /* loopbackOnly */, -1);
    } catch (e) {
      dump(`Juggler: server init failed: ${e}\n`);
      return;
    }
    dump(`Juggler: Android TCP listener on 127.0.0.1:${this._androidPort}\n`);
    server.asyncListen({
      onSocketAccepted(sock, transport) {
        if (!connection) {
          // juggler never started — nothing to serve; log and drop.
          dump(`Juggler: accept but juggler not started\n`);
          try {
            sock.close();
          } catch (e) {}
          return;
        }
        self._androidTransport = transport;
        connection.setSocket(transport);
        const inStream = transport.openInputStream(0, 0, 0);
        const scriptable = Cc["@mozilla.org/scriptableinputstream;1"].createInstance(Ci.nsIScriptableInputStream);
        scriptable.init(inStream);
        let buf = "";
        const pump = {
          QueryInterface: ChromeUtils.generateQI([Ci.nsIInputStreamCallback]),
          onInputStreamReady(stream) {
            try {
              let available;
              while ((available = stream.available()) > 0) {
                buf += scriptable.readBytes(available);
              }
              let idx;
              while ((idx = buf.indexOf("\0")) !== -1) {
                const message = buf.slice(0, idx);
                buf = buf.slice(idx + 1);
                if (message)
                  connection.receiveMessage(message);
              }
              stream.asyncWait(this, 0, 0, Services.tm.currentThread);
            } catch (e) {
              dump(`Juggler TCP read error: ${e}\n`);
            }
          },
        };
        try {
          inStream.asyncWait(pump, 0, 0, Services.tm.currentThread);
        } catch (e) {
          dump(`Juggler TCP asyncWait failed: ${e}\n`);
        }
      },
      onStopListening() {},
    });
  }

  // Common juggler startup: target registry, network observer, dispatcher,
  // and the transport — pipe on desktop, loopback TCP on Android. Returns
  // the connection object so the TCP listener can hand over its socket.
  _startJuggler(useTcp) {
        // Pre-initialize the accessibility service at startup. Lazy init
        // triggered from a synchronous pipe handler deadlocks: the handler
        // blocks the main thread while a11y init needs the main-thread
        // event loop, and the ATK bridge (when enabled) blocks on the
        // desktop AT-SPI socket. With NO_AT_BRIDGE=1 set by the runtime
        // (engine patch atk-bridge-env.patch honors it), this init builds
        // the internal tree only — Page.getFullAXTree then just walks it.
        try {
          Cc['@mozilla.org/accessibilityService;1'].getService(Ci.nsIAccessibilityService);
        } catch (e) {
          dump(`Juggler: a11y pre-init failed: ${e}\n`);
        }

        const targetRegistry = new TargetRegistry();
        new NetworkObserver(targetRegistry);

        const loadStyleSheet = () => {
          if (Cc["@mozilla.org/gfx/info;1"].getService(Ci.nsIGfxInfo).isHeadless) {
            const styleSheetService = Cc["@mozilla.org/content/style-sheet-service;1"].getService(Components.interfaces.nsIStyleSheetService);
            const ioService = Cc["@mozilla.org/network/io-service;1"].getService(Components.interfaces.nsIIOService);
            const uri = ioService.newURI('chrome://juggler/content/content/hidden-scrollbars.css', null, null);
            styleSheetService.loadAndRegisterSheet(uri, styleSheetService.AGENT_SHEET);
          }
        };

        // Force create hidden window here, otherwise its creation later closes the web socket!
        // Since https://phabricator.services.mozilla.com/D219834, hiddenDOMWindow is only available on MacOS.
        if (Services.appShell.hasHiddenWindow) {
          Services.appShell.hiddenDOMWindow;
        }

        let pipeStopped = false;
        let browserHandler;
        let androidSocket = null;
        // The desktop pipe component may not exist on Android builds —
        // only resolve it when the pipe transport is actually used.
        const pipe = useTcp ? null
                            : Cc['@mozilla.org/juggler/remotedebuggingpipe;1'].getService(Ci.nsIRemoteDebuggingPipe);
        const connection = {
          QueryInterface: ChromeUtils.generateQI([Ci.nsIRemoteDebuggingPipeClient]),
          setSocket(transport) {
            androidSocket = transport;
          },
          receiveMessage(message) {
            if (this.onmessage)
              this.onmessage({ data: message });
          },
          disconnected() {
            if (browserHandler)
              browserHandler['Browser.close']();
          },
          send(message) {
            if (pipeStopped) {
              // We are missing the response to Browser.close,
              // but everything works fine. Once we actually need it,
              // we have to stop the pipe after the response is sent.
              return;
            }
            if (androidSocket) {
              const out = androidSocket.openOutputStream(0, 0, 0);
              const framed = message + "\0";
              out.write(framed, framed.length);
              out.flush();
              out.close();
              return;
            }
            pipe.sendMessage(message);
          },
        };
        if (!useTcp)
          pipe.init(connection);
        const dispatcher = new Dispatcher(connection);
        browserHandler = new BrowserHandler(dispatcher.rootSession(), dispatcher, targetRegistry, browserStartupFinishedPromise, () => {
          if (this._silent)
            Services.startup.exitLastWindowClosingSurvivalArea();
          connection.onclose();
          if (!useTcp) {
            pipe.stop();
            pipeStopped = true;
          }
        });
        dispatcher.rootSession().setHandler(browserHandler);
        loadStyleSheet();
        if (useTcp)
          dump(`\nJuggler listening on TCP\n`);
        else
          dump(`\nJuggler listening to the pipe\n`);
        return connection;
  }

}

const jugglerInstance = new Juggler();

// This is used by the XPCOM codepath which expects a constructor
export var JugglerFactory = function() {
  return jugglerInstance;
};

