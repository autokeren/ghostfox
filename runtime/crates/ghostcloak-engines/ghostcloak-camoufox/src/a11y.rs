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

      // v0.5: Hidden content detection — check for invisible text planted
      // in the accessible name that might contain injection attempts
      const rawName = el.innerText || el.value || '';
      const hiddenPatterns = [
        /\bignore (all )?(previous|prior) (instructions?|prompts?)/i,
        /\bdisregard (your|all|any) (previous|prior)/i,
        /\bforget (your|all) (training|instructions)/i,
        /\byou are now (a|an|the)/i,
        /\bsystem prompt\b/i,
        /\bapi key\b.*here/i,
        /\bpassword\b.*here/i,
        /<\|im_start\|>/i,
      ];
      let suspicious = false;
      for (const pat of hiddenPatterns) {
        if (pat.test(rawName) || pat.test(name)) {
          suspicious = true;
          break;
        }
      }
      // Check computed styles for hidden text injection
      const st = getComputedStyle(el);
      if (st.fontSize === '0px' || (st.opacity !== '' && parseFloat(st.opacity) < 0.01 && el.innerText && el.innerText.length > 20)) {
        suspicious = true;
      }
    
      let ref = el.__gfxRef;
      if (!ref) {
        n += 1;
        ref = 'e' + n;
        el.__gfxRef = ref;
        M.set(ref, el);
      }
      const entry = { ref: ref, role: role, name: name };
      if (suspicious) entry.suspicious = true;
      if (role === 'textbox' || el.tagName === 'SELECT') {
        entry.value = String(el.value != null ? el.value : (el.innerText || '')).slice(0, 200);
      }
      if (el.checked !== undefined && el.type !== 'text') entry.checked = !!el.checked;
      if (el.disabled) entry.disabled = true;
      // v0.5.3: richer element context — tag, expanded, required, description.
      entry.tag = el.tagName.toLowerCase();
      var exp = el.getAttribute('aria-expanded');
      if (exp !== null) entry.expanded = exp === 'true';
      else if (el.hasAttribute('open')) entry.expanded = true;
      if (el.required === true || el.getAttribute('aria-required') === 'true' ||
          el.hasAttribute('required')) entry.required = true;
      var desc = el.getAttribute('aria-description') || el.getAttribute('description');
      if (desc) entry.description = desc.trim().slice(0, 120);
      // v0.5.3: viewport position — "visible" | "below" (with scroll_pages) | "hidden".
      var rect = el.getBoundingClientRect();
      var vh = window.innerHeight || document.documentElement.clientHeight;
      if (rect.bottom < 0) entry.visibility = 'hidden';
      else if (rect.top >= vh) {
        entry.visibility = 'below';
        entry.scroll_pages = Math.max(1, Math.round((rect.top - vh) / vh) + 1);
      } else entry.visibility = 'visible';
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

    // v0.5: Danger zone detection — flag sensitive page categories
    var pageText = document.body.innerText.toLowerCase();
    var dangerZones = {
      'financial': /bank|payment|credit card|loan|mortgage|invest|trading|crypto|wallet|paypal|stripe/i,
      'medical': /health|medical|hospital|pharmacy|prescription|diagnosis/i,
      'legal': /legal|lawyer|attorney|court|contract|nda|lawsuit/i,
      'authentication': /password|login|sign in|2fa|otp|verify your identity/i,
    };
    var danger = null;
    for (var zone in dangerZones) {
      if (dangerZones[zone].test(pageText) || dangerZones[zone].test(document.title)) {
        danger = zone;
        break;
      }
    }

    // v0.5: Count suspicious elements
    var suspiciousCount = out.filter(function(e) { return e.suspicious; }).length;

    // v0.5.3: Page state — archived / read-only detection.
    var archived = /this (post|thread|topic) (has been )?archived/i.test(pageText) ||
                   /archived post\.? (new comments|cannot)/i.test(pageText) ||
                   /new comments (cannot|can't|may not) be posted/i.test(pageText) ||
                   /comments (are|is) closed/i.test(pageText) ||
                   document.querySelector('[data-archived="true"]') !== null;

    // v0.5.3: Username detection — WHO is logged in?
    // Strategy: profile links in the header/nav area are OURS.
    // Reddit: a[href*="/user/NAME"], X: a[href^="/@handle"], HN: logout link.
    var uname = null;
    try {
      var profLinks = document.querySelectorAll('a[href*="/user/"], a[href^="/user/"]');
      for (var i = 0; i < profLinks.length && !uname; i++) {
        var pl = profLinks[i];
        var m1 = (pl.getAttribute('href') || '').match(/\/user\/([A-Za-z0-9_-]{2,25})/);
        if (!m1) continue;
        // Header/nav profile link = ours (top-of-page chrome).
        if (pl.closest('header, nav, [role="banner"]')) { uname = m1[1]; break; }
      }
      // Fallback: first /user/ link near the top of the page (y < 300px).
      if (!uname) {
        for (var j = 0; j < profLinks.length && !uname; j++) {
          var pl2 = profLinks[j];
          var r2 = pl2.getBoundingClientRect();
          if (r2.top < 300 && r2.top > 0) {
            var m2 = (pl2.getAttribute('href') || '').match(/\/user\/([A-Za-z0-9_-]{2,25})/);
            if (m2) uname = m2[1];
          }
        }
      }
      // X/Twitter: profile link in the side nav.
      if (!uname) {
        var xlinks = document.querySelectorAll('a[href^="/@"]');
        for (var k = 0; k < xlinks.length && !uname; k++) {
          if (xlinks[k].closest('nav, header, [data-testid="AppTabBar_Profile_Link"]')) {
            var m3 = (xlinks[k].getAttribute('href') || '').match(/\/@([A-Za-z0-9_]{2,20})/);
            if (m3) uname = m3[1];
          }
        }
      }
      // HN: the "logout" link encodes the user in its href.
      if (!uname) {
        var lg = document.querySelector('a[href*="logout"]');
        if (lg) {
          var m4 = (lg.getAttribute('href') || '').match(/(?:whodoneit|user)=?([A-Za-z0-9_-]{2,20})/);
          if (m4) uname = m4[1];
        }
      }
    } catch (e) {}

    // v0.5.3: Mark OUR OWN content — any element whose accessible name
    // contains the logged-in username (e.g. "Comment from Healthy_Gas_683").
    var ownCount = 0;
    if (uname) {
      var lowerU = String(uname).toLowerCase();
      out.forEach(function(e) {
        if (e.name && e.name.toLowerCase().indexOf(lowerU) >= 0) {
          e.own = true;
          ownCount++;
        }
      });
    }

    // v0.5.3: Scroll context — how much of the interactive inventory
    // lives below the fold, and how far down.
    var belowCount = 0, maxPages = 0;
    out.forEach(function(e) {
      if (e.visibility === 'below') {
        belowCount++;
        if (e.scroll_pages > maxPages) maxPages = e.scroll_pages;
      }
    });

    // v0.5.3: NOTIFICATIONS — toasts, alerts, rate limits (self health).
    // The agent MUST see errors/warnings after every action, or it acts
    // blind (e.g. clicking submit while rate-limited, retrying into a wall).
    var notifications = [];
    var rateLimit = null;
    function collectNotifs(root) {
      if (!root || !root.querySelectorAll) return;
      try {
        root.querySelectorAll('faceplate-alert, faceplate-toast, shreddit-async-error, [role="alert"], [role="status"], [class*="toast" i], [class*="banner" i], [class*="error" i], [class*="warning" i], [class*="notice" i]').forEach(function(el) {
          var r = el.getBoundingClientRect();
          if (r.width < 2 || r.height < 2) return; // skip invisible
          var t = (el.innerText || el.getAttribute('aria-label') || '').trim();
          if (!t || t.length < 4 || t.length > 300) return;
          if (notifications.indexOf(t) === -1) notifications.push(t.slice(0, 250));
        });
        root.querySelectorAll('*').forEach(function(el) {
          if (el.shadowRoot) collectNotifs(el.shadowRoot);
        });
      } catch (e) {}
    }
    collectNotifs(document);
    // Parse rate limit signals ("try again in X seconds" — Reddit, HN, etc).
    var notifText = notifications.join(' | ');
    var mRate = notifText.match(/try again in (\d+)\s*(seconds?|minutes?)/i) ||
                notifText.match(/(?:wait|wait for)\s+(\d+)\s*(seconds?|minutes?)/i) ||
                notifText.match(/you(?:'re| are) doing that too much[^]{0,80}?(\d+)\s*(seconds?|minutes?)/i);
    if (mRate) {
      rateLimit = parseInt(mRate[1], 10);
      if (mRate[2] && /^min/i.test(mRate[2])) rateLimit *= 60;
    }

    return JSON.stringify({
      elements: out.slice(0, 400),
      login_state: loginState,
      page_url: location.href,
      page_title: document.title,
      danger_zone: danger,
      suspicious_elements: suspiciousCount,
      page_archived: archived,
      own_elements: ownCount,
      username: uname,
      below_viewport: belowCount,
      max_scroll_pages: maxPages,
      notifications: notifications.slice(0, 10),
      rate_limit_seconds: rateLimit,
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

/// Humanized typing: inserts text with randomized inter-character timing.
/// Simulates keystroke dynamics — fast for common chars, slow for punctuation,
/// pauses at spaces and newlines. Fire-and-verify pattern.
#[allow(dead_code)] // v0.5 feature — pending integration into type_ref
pub(crate) fn type_ref_human_js(r: &str, text: &str) -> String {
    format!(
        r#"(function() {{
  var el = (window.__gfxRefs || new Map()).get({r});
  if (!el || !el.isConnected) return 'STALE-REF';
  var text = {text};
  el.scrollIntoView({{block: 'center'}});
  el.focus();

  // For plain inputs: set value with realistic event timing
  if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {{
    el.value = '';
    var i = 0;
    var base = 45 + Math.random() * 65; // 45-110ms per char base
    function typeNext() {{
      if (i >= text.length) {{
        el.dispatchEvent(new Event('input', {{bubbles: true}}));
        el.dispatchEvent(new Event('change', {{bubbles: true}}));
        return;
      }}
      var ch = text[i];
      el.value += ch;
      // Keystroke dynamics:
      var delay = base;
      if (ch === ' ') delay += 30 + Math.random() * 50;         // pause at spaces
      if (ch === '\n') delay += 120 + Math.random() * 200;     // longer pause at newlines
      if (/[.!?]/.test(ch)) delay += 80 + Math.random() * 150;  // pause at sentence end
      if (/[,;:]/.test(ch)) delay += 40 + Math.random() * 80;   // pause at commas
      if (Math.random() < 0.05) delay += 150 + Math.random() * 300; // random "thinking" pause
      el.dispatchEvent(new Event('input', {{bubbles: true}}));
      i++;
      setTimeout(typeNext, delay);
    }}
    typeNext();
    return 'FIRED';
  }}

  // For contenteditable: paste then let async editors settle
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
  return 'NOT-EDITABLE';
}})()"#,
        r = serde_json::to_string(r).unwrap_or_default(),
        text = serde_json::to_string(text).unwrap_or_default()
    )
}

/// Detect prompt injection patterns and hidden content in a11y output.
#[allow(dead_code)] // v0.5 feature — pending integration into page_a11y
pub(crate) const INJECTION_PATTERNS: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "disregard your instructions",
    "forget your training",
    "you are now a",
    "act as if",
    "pretend you are",
    "system prompt",
    "### instruction",
    "<|im_start|>",
    "download from this link",
    "enter your password",
    "api key here",
    "secret key",
    "click here to download",
    "install this extension",
];
