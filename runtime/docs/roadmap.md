# Roadmap

## Layer model

The anti-detect surface has three layers. Each layer is harder and stronger
than the last; the project climbs them in order.

### Layer 1 — JS init scripts (DONE, v0.1)

`Page.addScriptToEvaluateOnNewDocument` patches the JS surface per-page:

- navigator: platform, cores, deviceMemory, languages, webdriver
- screen: width/height/avail/dpr
- plugins: real Chrome plugin-array shape
- WebGL: UNMASKED_VENDOR / UNMASKED_RENDERER
- canvas: per-identity deterministic noise (getImageData)
- timezone: resolvedOptions + getTimezoneOffset
- geolocation + permissions.query silencing

**Limitations (known, accepted):**
- CDP's `Runtime.enable` and the console API are visible to advanced
  detectors. Mitigation on the roadmap: `--enable-automation` removal is done;
  CDP-level evasions (reading `Runtime.enable` leaks) are partial.
- Font presence probing is content-based (text measurement) — cannot be
  beaten from JS. We do NOT fake it; faking creates a worse signal than truth.
- `Notification.permission` still reads the real value in some flows.

### Layer 2 — engine patches (NEXT)

Two candidate paths:

- **(a) Firefox fork (Camoufox-style):** spoof at C++ level — canvas,
  WebGL, fonts, media devices all report through the engine, no JS
  overrides visible. Strongest; maintained as a patch series against
  Firefox releases. Large ongoing cost (rebase every 6 weeks).
- **(b) Chromium fork:** patch `--headless=new` tells (HeadlessChrome UA
  is handled; deeper: `navigator.webdriver` at Blink level, font metrics
  in Skia). Chromium's release cadence is brutal (4 weeks).

Decision: start (a) after the eval harness proves it can measure the
difference. A patch series nobody can score is a patch series nobody can trust.

### Layer 3 — network fingerprint (LATER)

- TLS/JA3: stock Chromium/Firefox network stacks are *fine* — a custom
  stack is the red flag. Do not touch.
- HTTP/2 frame order, header order: comes free with a real engine.
- What we CAN do: proxy consistency (WebRTC IP must match proxy exit),
  timezone vs. IP geolocation coherence — already enforced by the auditor.

## Eval harness tiers

| tier | what | how |
|---|---|---|
| T0 | identity coherence | offline audit, pure Rust, CI gate — **500/500 PASS** |
| T1 | JS surface | headless launch + evaluate battery, JSONL — **PASS** (UA/platform/locale/tz/cores match identity, webdriver=false) |
| T2 | detector pages | see baseline below |
| T3 | challenge pages | Turnstile / Akamai / PerimeterX — manual for now, don't automate abuse |

### T2 baseline (2026-09-01)

| site | result |
|---|---|
| example.com | full pass: navigate, JS surface, snapshot, clean shutdown |
| EFF CoverYourTracks | works; snapshot captured post-redirect; JS surface needs settle-wait on redirect chains |
| CreepJS | **crashes the browser itself** (NS_ERROR_FAILURE in removeProgressListener → BrowserHandler cleanup) — upstream Camoufox/Firefox bug, not a driver bug. Revisit after a Camoufox update. |


T3 has an ethics boundary: we score our own browser against these pages at
low rate for research; we do not build or ship automated bypass.

## MCP surface

v0.1: session_create, page_open, page_snapshot, page_click, page_type,
identity_generate, identity_audit.

Next:
- resource-style page registry (list pages per session)
- screenshot tool
- human-handoff tool (pause session, flag for a human)
- a11y-tree snapshot (replaces HTML-to-text string munging)
