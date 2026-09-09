# Changelog

All notable changes to ghostcloak will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-09-03

First public release. Rust-native agent browser runtime with a patched-Firefox engine.

### Added

- **ghostcloak-core** — engine-agnostic runtime: `Engine`/`PageHandle` traits, session vault, engine registry.
- **ghostcloak-fingerprint** — declarative identities as TOML: coherent device-preset generator (platform/screen/GPU/fonts that actually ship together), auditor that rejects contradictory signals, content-hash fingerprinting per identity.
- **ghostcloak-camoufox** — patched-Firefox engine adapter speaking the Juggler wire protocol natively from Rust (fd 3/4 pipes, `\0`-framed JSON, per-target session routing, live execution-context tracking). Zero Python/Node at runtime.
  - Identity injection via `CAMOU_CONFIG` env — spoofing happens inside the engine at the C++ level.
  - Per-character key-event typing (Enter/Tab/Space with correct key codes), named-key `press_key`, mouse-event clicks with JS-click fallback for form controls.
  - Self-healing snapshot (context-loss recovery via reload), process-group teardown, ephemeral profile lifecycle.
- **ghostcloak-mcp** — MCP server (stdio) exposing 9 narrow, typed tools: `session_create`, `page_open`, `page_snapshot`, `page_click`, `page_type`, `page_fill`, `page_press`, `identity_generate`, `identity_audit`.
- **ghostcloak-eval** — the stealth referee: offline identity-coherence audits (T0), JS-surface probes against live pages (T1/T2).

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
