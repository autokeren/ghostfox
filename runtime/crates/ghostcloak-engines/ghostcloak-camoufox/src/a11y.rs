//! The a11y walker: page-side JS that gives agents EYES.
//!
//! Walks the DOM INCLUDING shadow roots (the #1 blind spot of selector-based
//! automation — modern UIs like Reddit's shreddit-* components hide their
//! fields there), computes a semantic role + accessible name for every
//! visible interactive element, reads live input values, and tags each
//! element with a stable ref ("e12") the runtime can act on via
//! click_ref/type_ref.

/// One snapshot pass. Assigns refs to elements (idempotent: elements keep
/// their ref across snapshots), returns the visible interactive inventory.
pub(crate) const WALK_JS: &str = r#"(
  function () {
    const M = (window.__gfxRefs = window.__gfxRefs || new Map());
    let n = M.size;
    const out = [];
    const INTERACTIVE_SELECTOR = [
      'a[href]', 'button', 'input', 'select', 'textarea', 'summary',
      '[role]', '[contenteditable="true"]', '[onclick]', '[tabindex]',
    ].join(',');

    function nameOf(el) {
      const labelled = el.getAttribute('aria-labelledby');
      if (labelled && document.getElementById(labelled)) {
        return document.getElementById(labelled).textContent.trim().slice(0, 80);
      }
      return (
        el.getAttribute('aria-label') ||
        (el.labels && el.labels[0] ? el.labels[0].textContent.trim() : '') ||
        el.getAttribute('placeholder') ||
        el.getAttribute('title') ||
        (el.innerText || el.value || '').trim().replace(/\s+/g, ' ').slice(0, 80)
      );
    }

    function roleOf(el) {
      const aria = el.getAttribute('role');
      if (aria) return aria;
      const t = el.tagName;
      if (t === 'A') return 'link';
      if (t === 'BUTTON' || t === 'SUMMARY') return 'button';
      if (t === 'INPUT') {
        const ty = (el.getAttribute('type') || 'text').toLowerCase();
        if (ty === 'checkbox') return 'checkbox';
        if (ty === 'radio') return 'radio';
        if (ty === 'submit' || ty === 'button') return 'button';
        return 'textbox';
      }
      if (t === 'TEXTAREA') return 'textbox';
      if (t === 'SELECT') return 'combobox';
      if (el.isContentEditable) return 'textbox';
      if (/^H[1-6]$/.test(t)) return 'heading';
      if (t === 'IMG') return 'img';
      return '';
    }

    function visible(el) {
      const r = el.getBoundingClientRect();
      if (r.width < 3 || r.height < 3) return false;
      const st = getComputedStyle(el);
      return st.visibility !== 'hidden' && st.display !== 'none';
    }

    function process(el) {
      if (el.closest('[aria-hidden="true"]')) return;
      if (!visible(el)) return;
      const role = roleOf(el);
      if (!role) return;
      const name = nameOf(el);
      if (!name && (role === 'button' || role === 'link')) return;
      let ref = el.__gfxRef;
      if (!ref) {
        n += 1;
        ref = 'e' + n;
        el.__gfxRef = ref;
        M.set(ref, el);
      }
      const entry = { ref: ref, role: role, name: name };
      if (role === 'textbox' || el.tagName === 'SELECT') {
        entry.value = String(el.value != null ? el.value : (el.innerText || '')).slice(0, 200);
      }
      if (el.checked !== undefined && el.type !== 'text') entry.checked = !!el.checked;
      if (el.disabled) entry.disabled = true;
      out.push(entry);
    }

    function walk(root) {
      let nodes;
      try { nodes = root.querySelectorAll(INTERACTIVE_SELECTOR); } catch (e) { return; }
      nodes.forEach(function (el) {
        if (el.__gfxSeen) { process(el); return; }
        el.__gfxSeen = true;
        process(el);
      });
      // Pierce shadow roots.
      root.querySelectorAll('*').forEach(function (el) {
        if (el.shadowRoot) walk(el.shadowRoot);
      });
      // Pierce same-origin iframes (cross-origin is blocked by browser security — by design).
      try {
        root.querySelectorAll('iframe').forEach(function (f) {
          if (f.contentDocument) walk(f.contentDocument);
        });
      } catch (e) {}
    }

    walk(document);

    // Session health: detect login/logout signals for the agent.
    var txt = document.body.innerText.toLowerCase();
    // Session health: use STRONG signals only.
    // "Expand user menu" is WEAK — exists even when logged out on most sites.
    var all = out.map(function (e) { return e.name.toLowerCase(); });
    // STRONG logged-in: these only appear with an authenticated session
    var strongIn = all.some(function (n) {
      return n.indexOf('open inbox') >= 0 || n.indexOf('open chat') >= 0 ||
             n.indexOf('log out') >= 0 || n.indexOf('logout') >= 0 ||
             n.indexOf('karma') >= 0 || n.indexOf('my profile') >= 0;
    });
    // STRONG logged-out: explicit login/sign-up CTAs as primary actions
    var strongOut = all.some(function (n) {
      return n === 'log in' || n === 'login' || n === 'sign up' || n === 'sign in' ||
             n === 'log in / sign up' || n === 'sign up or log in';
    });
    var loginState = strongIn ? 'logged-in' :
                     strongOut ? 'logged-out' : 'unknown';

    return JSON.stringify({
      elements: out.slice(0, 400),
      login_state: loginState,
      page_url: location.href,
      page_title: document.title,
    });
  }
)()"#;

/// Resolve a ref to an action. `action` is one of "click" | "focus".
#[allow(dead_code)]
pub(crate) fn resolve_js(r: &str) -> String {
    format!(
        r#"(function() {{
  var M = window.__gfxRefs;
  var el = M && M.get({r});
  if (!el) return 'STALE-REF';
  if (!el.isConnected) return 'STALE-REF';
  return 'OK';
}})()"#,
        r = serde_json::to_string(r).unwrap_or_default()
    )
}

/// Click the element a ref points at.
pub(crate) fn click_ref_js(r: &str) -> String {
    format!(
        r#"(function() {{
  var el = (window.__gfxRefs || new Map()).get({r});
  if (!el || !el.isConnected) return 'STALE-REF';
  el.scrollIntoView({{block: 'center'}});
  el.click();
  return 'CLICKED';
}})()"#,
        r = serde_json::to_string(r).unwrap_or_default()
    )
}

/// Fire text into the element a ref points at (no verification — async
/// editors like Lexical process input on later ticks; verify with
/// read_ref_js after a pause).
pub(crate) fn type_ref_action_js(r: &str, text: &str) -> String {
    format!(
        r#"(function() {{
  var el = (window.__gfxRefs || new Map()).get({r});
  if (!el || !el.isConnected) return 'STALE-REF';
  var text = {text};
  el.scrollIntoView({{block: 'center'}});
  if (el.isContentEditable) {{
    el.focus();
    var sel = window.getSelection();
    var range = document.createRange();
    range.selectNodeContents(el);
    sel.removeAllRanges();
    sel.addRange(range);
    try {{
      var dt = new DataTransfer();
      dt.setData('text/plain', text);
      el.dispatchEvent(new ClipboardEvent('paste', {{clipboardData: dt, bubbles: true, cancelable: true}}));
    }} catch (e) {{}}
    if (el.textContent.length < text.length * 0.9) {{
      document.execCommand('insertText', false, text);
    }}
    return 'FIRED';
  }}
  el.focus();
  el.value = text;
  el.dispatchEvent(new Event('input', {{bubbles: true}}));
  el.dispatchEvent(new Event('change', {{bubbles: true}}));
  return 'FIRED';
}})()"#,
        r = serde_json::to_string(r).unwrap_or_default(),
        text = serde_json::to_string(text).unwrap_or_default()
    )
}

/// Read back the current length of the element a ref points at
/// (async-editor friendly: called after a pause).
pub(crate) fn read_ref_js(r: &str) -> String {
    format!(
        r#"(function() {{
  var el = (window.__gfxRefs || new Map()).get({r});
  if (!el || !el.isConnected) return 'STALE-REF';
  if (el.isContentEditable) return 'LEN:' + el.textContent.length;
  return 'LEN:' + String(el.value != null ? el.value.length : 0);
}})()"#,
        r = serde_json::to_string(r).unwrap_or_default()
    )
}

/// Read the FULL value of the element a ref points at (no truncation —
/// use this when the a11y snapshot's 200-char preview isn't enough).
pub(crate) fn read_ref_full_js(r: &str) -> String {
    format!(
        r#"(function() {{
  var el = (window.__gfxRefs || new Map()).get({r});
  if (!el || !el.isConnected) return 'STALE-REF';
  var v = el.value != null ? String(el.value) : (el.innerText || '');
  return v;
}})()"#,
        r = serde_json::to_string(r).unwrap_or_default()
    )
}

/// Wait until a CSS selector becomes visible (or timeout).
pub(crate) fn wait_for_js(selector: &str) -> String {
    format!(
        r#"(function() {{
  var el = document.querySelector({sel});
  if (!el) return 'NOT-FOUND';
  var r = el.getBoundingClientRect();
  var st = getComputedStyle(el);
  if (r.width > 0 && r.height > 0 && st.visibility !== 'hidden') return 'VISIBLE';
  return 'NOT-VISIBLE';
}})()"#,
        sel = serde_json::to_string(selector).unwrap_or_default()
    )
}
