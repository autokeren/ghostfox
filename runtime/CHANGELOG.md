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
