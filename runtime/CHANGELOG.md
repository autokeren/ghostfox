# Changelog

All notable changes to ghostfox will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.1] — 2026-10-04

### TLS fingerprint capability (dormant, E2E-verified)
- Engine patch `tls-v1-spoofing.patch`: per-session ClientHello reorder
  via MaskConfig (`tls:groups` / `tls:cipherSuites`) using NSS
  SSL_NamedGroupConfig + SSL_CipherSuiteOrderSet in nsSSLIOLayer.
- Identity carries a `TlsProfile` (Stock / Chrome141). The camoufox
  engine presents a FIREFOX UA, so the default is Stock — Firefox UA +
  Firefox JA3 stays coherent. Chrome141 reorders cipher suites and
  groups to Chrome's exact order (verified against a local JA3 capture
  server: 1301,1302,1303,c02b... / x25519,p256,p384) for a future
  chrome-mode surface. Extension reordering (JA4) = v1.5.


MCP Registry listing published: io.github.autokeren/ghostfox v0.9.1
(mcp-publisher GitHub device auth; npm `mcpName` + PyPI `mcp-name`
ownership verification tokens confirmed live). All five distribution
channels are current: PyPI, npm, GHCR, GitHub Releases, MCP Registry.

**The hearing + smell release**: M3 AccEvent stream and M3.5 (jank
detector, timing sense, visual stability) complete the senses. 67 tools.

### Added
- **WebGPU v1 spoofing**: `GHOSTFOX_WEBGPU=1` re-enables
  `navigator.gpu` for personas whose platform ships WebGPU (the
  Win/Mac-ARM leak — a spoofed persona without WebGPU was a red flag).
  The engine reports a hardware adapter even on GPU-less hosts
  (`isFallbackAdapter` masked false when the persona mask is active;
  vendor/description empty per stock Firefox, subgroups 4/128, limits
  at Firefox defaults). E2E-verified on a GPU-less host.
- `page_ws_frames` (M3 hearing — the WebSocket stream): every socket
  (url, opened/closed, error) + frames in BOTH directions (opcode,
  data, ts; binary base64, data capped 2000 chars/frame). The engine's
  FrameTree already observed nsIWebSocketEventService; the runtime now
  buffers Page.webSocket* protocol events per page. **M3 hearing is
  complete — all five senses are live.**
- `page_a11y_events` (M3 hearing — the AccEvent stream): compacted
  {type, role, name, + detail} records from the engine's own
  "accessible-event" topic — focus moves, text inserted/removed
  (with modifiedText + offset), name/value/state changes, caret
  moves, live-region announcements. The a11y tree as a STREAM: the
  incremental diff, no more full-tree re-walks after every action.
- `page_frame_stats` (M3.5 smell — the jank detector): rAF frame-
  cadence stats from the privileged frame script (avg/p50/p95/p99/
  max frame delta, jankyFrames, throttledFrames). Clean page ≈16.7ms;
  a busy main thread shows as a delta spike — read it before clicking.
- `page_timing_report` (M3.5 smell — timing sense): Navigation Timing
  (dnsMs/tlsMs/connectMs/ttfbMs/domInteractive/loadEvent) + paint
  entries (first-paint, first-contentful-paint) for the current
  document.
- `page_wait_visual` (M3.5 smell — visual stability): block until the
  refresh driver has delivered frames at normal cadence for quiet_ms
  consecutive — "wait until VISUALLY ready", the true successor to
  arbitrary sleep(3).
- netcap entries now carry per-request `timing` {dnsMs, tlsMs,
  connectMs, ttfbMs} (+ rawTiming) straight from nsITimedChannel —
  DNS/TLS/TTFB for every request in `page_network_read`.
- `page_a11y_set_text` (M5/Flutter): the AT-native text input route —
  the accessibly protocol action that syncs Flutter web semantics-mode
  text fields (focus + value + trusted input event). The DOM routes
  write a mirror nobody reads; this is the real one.
- `page_proprio` (M4 proprioception): the honest body state — load
  state, a11y focus, native selection/caret (the typing receipt),
  scrollers, viewport. Frame-script reads; pages cannot fake it.
- `page_cookie_events` (M4 proprioception): the cookie/session
  heartbeat — every cookie change as {kind, host, name, path, flags},
  NEVER the value. An auth cookie deletion = the earliest
  session-death signal.
- `session_vitals` (M4.5 interoception): the engine process-group
  health — per-process CPU ticks + memory (Linux /proc).
- **Network interception** (the spike became the build): `page_network_intercept`
  (hold every request), `page_network_resume` (modify url/method/headers/
  postData), `page_network_abort` (block), `page_network_fulfill` (mock the
  response). The netcap entries now carry `intercepted` so the agent sees
  which requests are held.
- `page_captcha_vision` (tier-5 vision): send any captcha element
  (or the viewport) to the host vision model (Workers AI GLM-5.3-flash)
  and get what it reads — the OCR tier the local CRNN cannot reach
  (heavily warped text captchas). Same CLOUDFLARE_API_KEY /
  CLOUDFLARE_ACCOUNT_ID env as page_hcaptcha.
- **MCP Streamable HTTP transport**: set `GHOSTFOX_HTTP_PORT` (e.g.
  "5000") and the server serves the MCP Streamable HTTP protocol on
  127.0.0.1 (stateful sessions, SSE) instead of stdio — the mode
  remote agents and web MCP hosts connect to. E2E: initialize →
  Mcp-Session-Id → session_create → page_open, all over HTTP.
- `GHOSTFOX_WEBGL=1` launch env: re-enables WebGL for Flutter/
  CanvasKit app E2E (the hardening prefs block it and the CPU-only
  fallback crashes some screens).

### Fixed
- netcap: Juggler Network events are FLAT (url/status at params top
  level), not CDP-shaped — the URLs were captured empty. Both
  listeners corrected (the first-insert race masked the first fix).
- a11ySetText: the schema rejects unknown return properties — the
  tool results stay minimal now.
- **`t.Object` protocol schema did not exist** — the Juggler protocol
  validator threw `ILLDEFINED SCHEME` for every tool returning free-
  form objects (`page_proprio`, `page_cookie_events`, and the new
  stream tools). `t.Object` is now defined in PrimitiveTypes.js.
- `Page.getProprioState` returned the bare state object instead of
  `{state: ...}` the protocol schema declares — surfaced once the
  schema fix above made the validator actually run.
- netcap timing: nsITimedChannel phases are epoch MICROSECONDS — the
  derived ms values were 1000x too large (and 0-phase fallbacks
  polluted connect/tls math). Now converted to real ms with
  unavailable phases nulled.
- netcap timing enrichment ran only in the secondary listener (the
  net_capture_start one); the primary page-open listener now also
  records timing per response.

## [0.9.0] — 2026-09-30

**The eyes release**: M2 → M2.5 → M2.9 → M3 → M3.5 → M5 in one train, plus the one-name rename. 52 tools.

### Added
- `page_wait_stable` (M3.5 smell): wait until the render truly settles
  (whispers + rect hash quiet) — the successor to arbitrary sleep().
- `page_ui_audit` (M5 the Critic): visual QA for AI-generated UIs —
  clipped captions, missing padding, viewport overflow, overlaps and
  crowding detected from the engine's native a11y geometry.
- `page_mutations` (M3 hearing): privileged DOM-change whispers — every
  childList/attributes/characterData mutation as pullable records,
  observed from the unhookable frame-script realm.
- **Layer X-Ray**: `page_pixels include_hidden=true` — hidden
  (visibility:hidden/collapsed) content paints in the privileged
  snapshot via a new RenderDocumentFlags::ForceVisibleContent.
- **One name**: ghostcloak* renamed to ghostfox* everywhere — crates,
  `ghostfox-mcp` binary, RUST_LOG targets, CI assets.
- **Page.captureCanvasBuffer**: canvas DRAWING BUFFER reads from the
  privileged frame script (native toDataURL via Xray — page hooks
  cannot poison or observe). For canvases the compositor never renders
  (CSS 0x0, display:none — the GeeTest fullbg case).
- Rotate solver: feedback reader now covers `[role=alert]`/`[role=status]`
  and the human-replay re-registration matches English button labels
  (the 2captcha demo re-rendered as an English mock).
- **M2.9 (the sixth sense)** scoped in docs/JOURNEY.md: Layer X-Ray,
  native framebuffer reads (worker OffscreenCanvas), unhookable mutation
  whispers, profiler nervous system, truth-vs-lie detector, ghost frame.
- **GfxXray**: native, JS-free canvas buffer reads (2D/WebGL/offscreen)
  from a new XPCOM service — hidden canvases read exactly like visible
  ones; page hooks cannot poison or observe the read.
- **M2.5 compositor pixel capture**: `Page.captureSurface` — raw RGBA of
  any content-viewport rect straight from the compositor (`drawSnapshot`
  onto a CHROME-realm canvas). No page-realm canvas, no toDataURL, no
  getImageData: pages cannot hook, poison or even observe the read. The
  hole every wrapper-based competitor has in their captcha pipelines.
  Handles the Firefox quirk where `drawSnapshot` returns black for
  non-zero-origin rects (full-viewport snap + canvas crop).
- `page_pixels` v2: pixels from the compositor in three modes —
  `{x,y,width,height}` raw region, `{role,name}` semantic element
  (native a11y bounds, scroll-first), `{ref}` walk-ref. Luminance grids
  computed in Rust; no pixel math left in page JS.
- GeeTest slide solver: the last page-realm toDataURL read in the
  captcha suite migrated to captureSurface (canvas rects from JS, pixels
  from the compositor) — CORS taint is now impossible by construction.
- `page_click_native` (tool #49): the M2 touch primitive — semantic
  click whose coordinates come from the ENGINE's a11y tree, not page JS.
  Page scripts can hook `getBoundingClientRect` to poison coordinates;
  the native tree can't be tampered. Scroll-first: deterministic instant
  `scrollTo` from a11y geometry brings below-fold targets into the
  viewport (a11y bounds are window-relative; the fixed chrome offset is
  `mozInnerScreenY - screenY` — no scroll compensation, no staleness).
  Receipts carry the trusted bounds + click point.

## [0.8.2] — 2026-09-29

The agent-reliability release: observation, determinism, evidence.

### Added
- `page_extract` (tool #43): typed, token-efficient a11y filtering —
  agents ask for what they need ({key: {role, name, text, tag, first}})
  instead of paying for full-snapshot dumps.
- Recipes (#44-47): `recipe_record` / `recipe_save` / `recipe_list` /
  `recipe_replay` — record a successful flow, replay it deterministically
  without an LLM in the loop. Ref-based steps capture SEMANTIC anchors
  (role + accessible name) so replays survive DOM churn; strict mode
  escalates with {failed_at, anchor, hint} for the LLM to take over.
- Universal action receipts: `page_click_ref` / `page_type_ref` return
  Act→Observe→Compare evidence — cached-before snapshot diffed against a
  fresh walk (500ms settle), plus target/url/url_changed/login_state.
  Graceful degradation when no cached before exists.
- `page_diff` (tool #48): the standalone observeDiff primitive — what
  changed since the last snapshot, capped at 40 entries.
- `page_a11y(native=true)`: observation from the engine's OWN
  accessibility tree via the existing Juggler `Page.getFullAXTree`
  (the ariaSnapshot plumbing). Trusted source — page scripts cannot
  tamper; shadow DOM, iframes and ARIA semantics handled by Gecko
  itself. Richer states (focused/required/checked/expanded/disabled/
  level/tag). Native refs (n1..n) are observation handles; acting goes
  through role+name anchors or the walk source. Phase 1 of the
  Gecko-native agent runtime.

### Changed
- Juggler bootstrap: the first `Browser.enable` on a fresh connection
  now waits up to 120s (was a fixed 20s) — engine boots legitimately
  take tens of seconds on loaded hosts; the old cap reported every slow
  boot as dead.
- Headless engine spawns strip `DISPLAY`/`WAYLAND_DISPLAY` so they
  never couple to a live desktop session. `LD_PRELOAD` is deliberately
  NOT stripped (required libs route through it on some hosts).

## [0.8.1] — 2026-09-28

Typing fixed everywhere.

### Fixed
- `page_type` now synthesizes Playwright-canonical key events. The old
  build fabricated DOM `code` values ("Key0", "Key.") that Firefox's
  TextInputProcessor reads as non-printable — digits and punctuation
  were silently eaten in Lexical-based editors (X, Facebook, Notion).
  Every printable ASCII char now carries its real US-layout code/keyCode
  pair (91-entry table ported 1:1 from Playwright's usKeyboardLayout),
  and non-layout chars (em-dash, CJK, emoji) use Juggler `Page.insertText`.
  Verified: 123/123 chars locally, 223/223 in X's Lexical composer with
  the Post button armed.

## [0.8.0] — 2026-09-27

Linux arm64 support end-to-end.

### Added
- `ghostfox-mcp-arm64` release asset — the runtime builds natively on
  `ubuntu-24.04-arm` runners (onnxruntime prebuilts link cleanly on aarch64).
- Arch-aware installers: `install.sh`, Python `install_engine()` /
  `install_runtime()` and the npm installer all pick `lin.arm64` +
  `ghostfox-mcp-arm64` on Linux aarch64 (engine zips already shipped
  since v0.7.0; the runtime was the missing half).
- `runtime-build.yml` now runs an x86_64 + arm64 matrix on tag pushes.

### Fixed
- `install.sh` runtime download could match the `-arm64` asset on x86_64
  (substring grep); now exact-matches the asset for the host arch.

## [0.1.0] — 2026-09-03

First public release. Rust-native agent browser runtime with a patched-Firefox engine.

### Added

- **ghostfox-core** — engine-agnostic runtime: `Engine`/`PageHandle` traits, session vault, engine registry.
- **ghostfox-fingerprint** — declarative identities as TOML: coherent device-preset generator (platform/screen/GPU/fonts that actually ship together), auditor that rejects contradictory signals, content-hash fingerprinting per identity.
- **ghostfox-camoufox** — patched-Firefox engine adapter speaking the Juggler wire protocol natively from Rust (fd 3/4 pipes, `\0`-framed JSON, per-target session routing, live execution-context tracking). Zero Python/Node at runtime.
  - Identity injection via `CAMOU_CONFIG` env — spoofing happens inside the engine at the C++ level.
  - Per-character key-event typing (Enter/Tab/Space with correct key codes), named-key `press_key`, mouse-event clicks with JS-click fallback for form controls.
  - Self-healing snapshot (context-loss recovery via reload), process-group teardown, ephemeral profile lifecycle.
- **ghostfox-mcp** — MCP server (stdio) exposing 9 narrow, typed tools: `session_create`, `page_open`, `page_snapshot`, `page_click`, `page_type`, `page_fill`, `page_press`, `identity_generate`, `identity_audit`.
- **ghostfox-eval** — the stealth referee: offline identity-coherence audits (T0), JS-surface probes against live pages (T1/T2).

### Verified

- Identity coherence: 500/500 generated identities pass the auditor.
- Full E2E over MCP stdio (8/8): create session → open page → fill form → click submit → value present in the server's POST echo.
- Anti-detect JS surface matches the generated identity (UA/platform/locale/timezone/cores), `navigator.webdriver` false.
- Multi-page sessions keep independent live execution contexts (per-page event routing).

### Known issues

- Heavy fingerprint-test sites (CreepJS) crash the engine's content channel — upstream engine bug, tracked.
- Google may serve `/sorry/` challenge pages on datacenter IPs following a homepage→search pattern; pass a `proxy` to `session_create` or navigate directly to search URLs.

## [0.2.0] — 2026-09-08

### Added

- **Evidence recording** — every session writes an append-only `events.jsonl`
  plus full page snapshots and `identity.toml` under
  `~/.ghostfox/recordings/<session>/`; new `session_evidence` MCP tool
  returns the log, files and persona (evidence primitive for run audit and
  replay).
- **Android personas** — new `Platform::Android`: portrait screens with
  dpr ≥ 2, Adreno/Mali GPU strings, Android font stacks, Firefox-on-Android
  UAs (Android version tracked from the identity), `navigator.platform`
  `Linux aarch64`, auditor rules for portrait/dpr/GPU coherence.
  `session_create {"platform":"android"}`.
- Identity benchmark published: 500/500 coherent (Windows 135 · Android 161 ·
  MacOS 103 · Linux 101), see `docs/benchmark-2026-09-08.md`.

## [0.3.0] — 2026-09-08 (same day, second wave)

### Added

- **Touch-coherent Android personas** — `navigator.maxTouchPoints` spoofed at
  the C++ level (engine patch `navigator-touch-spoofing.patch`), Juggler touch
  override at launch (coarse pointer + touch events), viewport follows the
  identity screen class. Verified end-to-end: 5 touch points, pointer:coarse,
  no hover, portrait viewport, Linux aarch64 platform.
- **page_screenshot MCP tool** — Juggler `Page.screenshot` (viewport or full
  page), PNG evidence under `~/.ghostfox/recordings/<session>/screenshots/`.
- **Live view (opt-in)** — `GHOSTFOX_LIVE_VIEW_PORT` serves per-page latest
  PNGs + index, with a 5s auto-capture ticker.
- **captcha_solve MCP tool** — 2captcha hook (Turnstile + image), env-config
  (GHOSTFOX_CAPTCHA_PROVIDER/GHOSTFOX_CAPTCHA_KEY), attempts recorded in
  evidence without key material.
- **eval `targets` mode** — live-target probe (sannysoft detector panel +
  areyouheadless referee + sanity pages) with JSONL scorecard; first run:
  4/4 ok, 0 gated from a datacenter IP.
- **Python package** — `pip install ghostfox` (installer + sync MCP client).

### Fixed

- **iframe context hijack (major)** — the execution-context pump tracked the
  newest context, so pages with iframes (most real sites, bot.sannysoft.com's
  srcdoc test frames) made snapshots/clicks/typing evaluate inside an iframe.
  The pump now tracks the page's main frame only (via auxData.frameId).
- Android viewport now portrait-sized from the identity (was hardcoded
  1280x800).
- maxTouchPoints config check ordered before the RDM-pane branch (a juggler
  viewport request sets inRDMPane, which masked the config).

## [0.3.0] — 2026-09-09

### Added

- **`page_a11y` — eyes for agents.** Semantic snapshot of the page: every
  visible interactive element with a stable ref, role, accessible name and
  CURRENT value. Walks shadow DOM (the #1 blind spot of selector-based
  automation — modern web-component UIs like Reddit's shreddit-* hide
  fields there). Values are read live, so form state is always visible.
- **`page_click_ref` / `page_type_ref`** — act by ref, no selectors. Clicks
  scroll into view first; typing handles plain inputs AND rich editors
  (Lexical/Draft/ProseMirror) via synthetic-paste + insertText with a
  fire-then-verify receipt (async editors settle before the check).
- Proven the same day it was born: posted to Reddit end-to-end (3 tool
  calls) on a composer that had defeated selector automation for hours.

## [0.3.1] — 2026-09-09 (gap-closing wave)

### Added

- **`page_read_ref`** — full-value read by ref (no 200-char truncation);
  use when the a11y snapshot's preview isn't enough
- **`page_wait_for`** — poll until a CSS selector becomes visible (with
  timeout); replaces agent-side `sleep 8` guessing
- **`page_upload_file`** — upload a local file to `input[type=file]` via
  synthetic DataTransfer (bypasses the native file picker)
- **a11y walker now pierces same-origin iframes** in addition to shadow
  roots (cross-origin is blocked by browser security — by design)

## [0.4.0] — 2026-09-09

### Added

- **`page_a11y` now reports `login_state`** — "logged-in" | "logged-out" |
  "unknown" — detected from login buttons vs user-menu signals, so agents
  check session health before acting instead of discovering a dead session
  the hard way.
- **`page_a11y` returns page URL + title** alongside elements.
