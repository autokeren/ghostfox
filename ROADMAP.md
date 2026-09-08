# Roadmap

Features the competitive landscape has that we don't yet — tracked here so
"we want everything" has an order. Roughly in impact-per-effort order.

## Now (done 2026-09-08)

- [x] Evidence recording: `events.jsonl` + snapshot files per session,
      `session_evidence` MCP tool (parity with hosted platforms' recordings)
- [x] Android personas: portrait screens, Adreno/Mali GPUs, Android font
      stacks, Firefox-on-Android UAs, `session_create {"platform":"android"}`
- [x] One-command installer (`install.sh`), prebuilt engine + runtime on
      GitHub Releases
- [x] Published benchmark run (see `runtime/docs/benchmark-2026-09-08.md`)

## Next

- [ ] **Touch/mobile emulation** — the engine has no touch-event patch yet;
      Android personas currently spoof UA/screen/fonts/GPU but not
      `maxTouchPoints` or pointer media queries. Needs an engine patch
      (`engine/patches/`, `make edits` workflow).
- [ ] **Screenshots + live view** — juggler screenshot command wired to a
      `page_screenshot` MCP tool; live view as a tiny HTTP server streaming
      snapshots for debugging agents.
- [ ] **CAPTCHA solve hook** — optional config for a solver API, surfaced as
      a `captcha_solve` tool (only when gated; stealth-first philosophy).
- [ ] **macOS / Windows / ARM engine builds** — `engine/multibuild.py`
      already cross-compiles; needs CI runners + release uploads.
- [ ] **Docker image** — `docker pull ghcr.io/autokeren/ghostfox` with the
      MCP server as entrypoint.

## Later

- [ ] **Live-target benchmark automation** — run the Patterson-style 31-target
      suite on every release, publish the scorecard (credibility engine).
- [ ] **Team features** — profile sharing/seats needs a server; decide whether
      self-hosted Durable Objects or plain files + git is the Ghostfox way.
- [ ] **Credential vault** — keep passwords out of agent context (Steel-style
      Credentials API), self-hosted.
- [ ] **Python package** — `pip install ghostfox` wrapping the MCP runtime for
      the Scrapling/browser-use audience.
