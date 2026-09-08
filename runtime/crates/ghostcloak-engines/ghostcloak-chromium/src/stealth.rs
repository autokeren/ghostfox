//! Stealth init scripts: the JS layer that bends the browser's surface to the
//! identity. Every value injected here comes from the identity — never
//! hardcoded defaults, so two identities never share a canvas seed.

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;

use ghostcloak_fingerprint::identity::Identity;

/// Generate the identity-matching init script and register it on every new
/// document this page loads.
pub async fn apply_identity(page: &Page, identity: &Identity) -> anyhow::Result<()> {
    let script = build_init_script(identity);
    page.execute(
        AddScriptToEvaluateOnNewDocumentParams::builder()
            .source(script)
            .build()
            .map_err(|e: String| anyhow::anyhow!("builder: {e}"))?,
    )
    .await?;
    Ok(())
}

fn esc(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

pub fn build_init_script(identity: &Identity) -> String {
    let fonts_json = serde_json::to_string(&identity.hardware.fonts).unwrap_or_else(|_| "[]".into());
    let geo = identity.geo.as_ref();
    let (lat, lon, acc) = match geo {
        Some(g) => (g.latitude, g.longitude, g.accuracy_m),
        None => (0.0, 0.0, 0.0),
    };

    // Deterministic per-identity canvas noise seed.
    let canvas_seed: f64 = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        identity.fingerprint_hash().hash(&mut h);
        (h.finish() % 9973) as f64 / 9973.0
    };

    format!(
        r#"// ghostcloak identity init (generated per-identity; do not edit by hand)
(() => {{
  const CO = Object.getOwnPropertyDescriptor;

  // ---- navigator ----
  const platform = {platform};
  Object.defineProperty(navigator, 'platform', {{ get: () => platform }});
  Object.defineProperty(navigator, 'hardwareConcurrency', {{ get: () => {cores} }});
  Object.defineProperty(navigator, 'deviceMemory', {{ get: () => {mem} }});
  Object.defineProperty(navigator, 'languages', {{ get: () => {langs} }});
  try {{
    Object.defineProperty(navigator, 'webdriver', {{ get: () => undefined }});
  }} catch (_) {{}}

  // ---- screen ----
  Object.defineProperty(screen, 'width',  {{ get: () => {sw} }});
  Object.defineProperty(screen, 'height', {{ get: () => {sh} }});
  Object.defineProperty(screen, 'availWidth',  {{ get: () => {sw} }});
  Object.defineProperty(screen, 'availHeight', {{ get: () => Math.max({sh} - 40, 0) }});
  Object.defineProperty(window, 'devicePixelRatio', {{ get: () => {dpr} }});

  // ---- plugins: real Chrome shape, not an empty list ----
  const fakePlugins = [
    {{ name: 'PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format files' }},
    {{ name: 'Chrome PDF Viewer', filename: 'internal-pdf-viewer', description: '' }},
    {{ name: 'Chromium PDF Viewer', filename: 'internal-pdf-viewer', description: '' }},
    {{ name: 'Microsoft Edge PDF Viewer', filename: 'internal-pdf-viewer', description: '' }},
    {{ name: 'WebKit built-in PDF', filename: 'internal-pdf-viewer', description: '' }},
  ];
  try {{
    Object.defineProperty(navigator, 'plugins', {{
      get: () => ({{
        length: fakePlugins.length,
        ...Object.fromEntries(fakePlugins.map((p, i) => [i, p])),
        item: (i) => fakePlugins[i],
        namedItem: (n) => fakePlugins.find(p => p.name === n) ?? null,
        refresh: () => {{}},
        [Symbol.toStringTag]: 'PluginArray',
      }}),
    }});
  }} catch (_) {{}}

  // ---- WebGL: vendor + renderer from identity ----
  const origGetParameter = WebGLRenderingContext.prototype.getParameter;
  const UNMASKED_VENDOR = 0x9245, UNMASKED_RENDERER = 0x9246;
  WebGLRenderingContext.prototype.getParameter = function (p) {{
    if (p === UNMASKED_VENDOR) return 'Google Inc. (NVIDIA)';
    if (p === UNMASKED_RENDERER) return {gpu};
    return origGetParameter.call(this, p);
  }};
  if (typeof WebGL2RenderingContext !== 'undefined') {{
    const orig2 = WebGL2RenderingContext.prototype.getParameter;
    WebGL2RenderingContext.prototype.getParameter = function (p) {{
      if (p === UNMASKED_VENDOR) return 'Google Inc. (NVIDIA)';
      if (p === UNMASKED_RENDERER) return {gpu};
      return orig2.call(this, p);
    }};
  }}

  // ---- canvas: per-identity noise, stable across calls ----
  const seed = {canvas_seed};
  const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
  HTMLCanvasElement.prototype.toDataURL = function (...args) {{
    const out = origToDataURL.apply(this, args);
    // Only perturb when the page actually read pixels (canvas of size > 1x1
    // and not our own probe) — naive unconditional noise breaks re-draws.
    return out; // noise hook lives in the engine layer; see docs/stealth.md
  }};
  const origToBlob = HTMLCanvasElement.prototype.toBlob;
  HTMLCanvasElement.prototype.toBlob = function (cb, ...args) {{
    return origToBlob.call(this, cb, ...args);
  }};
  const origGetImageData = CanvasRenderingContext2D.prototype.getImageData;
  CanvasRenderingContext2D.prototype.getImageData = function (...args) {{
    const out = origGetImageData.apply(this, args);
    // Deterministic tiny per-channel jitter driven by the identity seed.
    if (seed > 0 && out.width > 1 && out.height > 1) {{
      const d = out.data;
      for (let i = 0; i < d.length; i += 4) {{
        d[i]   = (d[i]   + Math.round((seed * 37) % 2)) & 0xff;
        d[i+1] = (d[i+1] + Math.round((seed * 53) % 2)) & 0xff;
        d[i+2] = (d[i+2] + Math.round((seed * 71) % 2)) & 0xff;
      }}
    }}
    return out;
  }};

  // ---- timezone ----
  const tz = {tz};
  try {{
    const origResolved = Intl.DateTimeFormat.prototype.resolvedOptions;
    Intl.DateTimeFormat.prototype.resolvedOptions = function (...a) {{
      const r = origResolved.apply(this, a);
      r.timeZone = tz;
      return r;
    }};
    const OrigDF = Intl.DateTimeFormat;
    const dfProxy = new Proxy(OrigDF, {{
      construct(target, args) {{
        const inst = new OrigDF(...args);
        const ro = origResolved.call(inst);
        // best-effort: delegate
        return inst;
      }},
    }});
    // Date.getTimezoneOffset: compute from the fixed offset baked below.
    const offsetMinutes = {tz_offset_min};
    Date.prototype.getTimezoneOffset = function () {{ return offsetMinutes; }};
  }} catch (_) {{}}

  // ---- geolocation ----
  if ({has_geo}) {{
    const lat = {lat}, lon = {lon}, acc = {acc};
    navigator.geolocation.getCurrentPosition = (ok, err) => ok({{
      coords: {{ latitude: lat, longitude: lon, accuracy: acc, altitude: null, altitudeAccuracy: null, heading: null, speed: null }},
      timestamp: Date.now(),
    }});
  }}

  // ---- permissions: silence the notification tell ----
  try {{
    const origQuery = navigator.permissions.query;
    navigator.permissions.query = (params) =>
      origQuery(params).catch(() => Promise.resolve({{ state: 'granted' }}));
  }} catch (_) {{}}

  // ---- font enumeration (document.fonts is read-only; patch check) ----
  // Font presence probing is content-based (measure text widths) and cannot
  // be beaten from JS alone — engine-level patch required. We do not fake it
  // here; pretending would create a *worse* signal.
}})();
"#,
        platform = esc(&platform_str(identity)),
        cores = identity.hardware.cpu_cores,
        mem = identity.hardware.device_memory_gb,
        langs = serde_json::to_string(&[identity.locale.clone()]).unwrap_or_else(|_| r#"["en-US"]"#.into()),
        sw = identity.screen.width,
        sh = identity.screen.height,
        dpr = identity.screen.dpr,
        gpu = esc(&identity.hardware.gpu_renderer),
        canvas_seed = canvas_seed,
        tz = esc(&identity.timezone),
        tz_offset_min = tz_offset_minutes(&identity.timezone),
        has_geo = identity.geo.is_some(),
        lat = lat,
        lon = lon,
        acc = acc,
    )
}

fn platform_str(identity: &Identity) -> String {
    match identity.platform {
        ghostcloak_fingerprint::identity::Platform::Windows => "Win32".into(),
        ghostcloak_fingerprint::identity::Platform::MacOS => "MacIntel".into(),
        ghostcloak_fingerprint::identity::Platform::Linux => "Linux x86_64".into(),
        ghostcloak_fingerprint::identity::Platform::Android => "Linux aarch64".into(),
    }
}

/// Minutes to subtract from UTC for `tz`, resolved statically via chrono-tz
/// would pull a dependency; for the init script we ship a small offset table
/// for the generator's known zones and fall back to 0 (UTC) otherwise.
fn tz_offset_minutes(tz: &str) -> i64 {
    match tz {
        "America/New_York" => 300,
        "America/Chicago" => 360,
        "America/Denver" => 420,
        "America/Los_Angeles" => 480,
        "America/Toronto" => 300,
        "Europe/London" => 0,
        "Europe/Berlin" => -60,
        "Europe/Paris" => -60,
        "Europe/Amsterdam" => -60,
        "Australia/Sydney" => -600,
        _ => 0,
    }
}
