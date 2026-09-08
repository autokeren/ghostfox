# ghostcloak

**A stealth browser runtime for AI agents. Rust-native, MCP-first, fingerprint-coherent.**

```
     ┌─ AI agent ─────────────────────────────┐
     │  Claude / GPT / any MCP client          │
     └──────────────┬──────────────────────────┘
                    │ MCP (stdio / streamable HTTP)
     ┌──────────────▼──────────────────────────┐
     │  ghostcloak-mcp                          │
     │  narrow, typed tools                    │
     ├──────────────────────────────────────────┤
     │  ghostcloak-core                         │
     │  sessions · identity vault · engine API  │
     ├──────────┬───────────────┬───────────────┤
     │ chromium │ firefox(fork) │ servo (exp.)  │
     └──────────┴───────────────┴───────────────┘
```

## Credits & upstream dependencies

ghostcloak stands on the shoulders of giants — please know them:

- **[Camoufox](https://github.com/daijro/camoufox)** (MPL-2.0, by daijro) — the
  patched-Firefox engine doing C++-level fingerprint spoofing (navigator, screen,
  WebGL, fonts, WebRTC, audio, timezone). ghostcloak's default engine builds on
  top of Camoufox's patches. Without this project, ghostcloak would be an order
  of magnitude smaller. 🙏
- **[Mozilla Firefox](https://www.mozilla.org/firefox/)** (MPL-2.0) — the
  browser underneath everything.
- **[Playwright](https://github.com/microsoft/playwright)** — the Juggler wire
  protocol we speak natively from Rust was documented by their Firefox
  implementation.
- **[LibreWolf](https://librewolf.net/)** — the build system lineage Camoufox
  forked from.

The Rust code in this repository (runtime, MCP server, identity engine, eval
harness) is original work, dual-licensed MIT OR Apache-2.0. The engine is
licensed MPL-2.0 via its upstream.

## Why

| tool | hosted | MCP-native | anti-detect | Rust |
|---|---|---|---|---|
| Browserbase / Steel | yes (paid) | ~ | ~ | no |
| playwright-mcp | no | yes | none | no (node) |
| Camoufox | no | no | strong (C++ patches) | no (python) |
| **ghostcloak** | **no** | **yes** | **layered, eval-scored** | **yes** |

## Quick start

```sh
cargo build --release

# The stealth referee: generate + audit 50 identities offline
cargo run -p ghostcloak-eval -- identity --count 50

# End-to-end probe: real page, real identity, real snapshot
cargo run -p ghostcloak-eval --bin web_probe -- https://example.com

# MCP server (stdio) — wire into any MCP client
cargo run -p ghostcloak-mcp
```

Wire into Claude Code (`~/.claude.json` or project `.mcp.json`):

```json
{
  "mcpServers": {
    "ghostcloak": {
      "command": "/home/ubuntu/ghostcloak/target/release/ghostcloak-mcp"
    }
  }
}
```

Then the agent can: `session_create` → `page_open` → `page_snapshot` →
`page_click` / `page_type`, plus `identity_generate` / `identity_audit`.

## Design rules

1. **Identities are data.** One TOML file = one persona. Generated from
   coherent device presets, audited before use, hashable for diffs.
2. **No contradictions, ever.** The generator draws every field from one
   DevicePreset; the auditor re-checks. A spoofed browser's worst enemy is
   itself saying "4 cores on a MacBook".
3. **The eval harness is the referee.** Every stealth change must pass
   `ghostcloak-eval` before merge. Scores are JSONL, diffable between commits.
4. **JS-layer spoofing is the floor, not the ceiling.** The init scripts get
   you past naive detectors. Engine patches (the Firefox fork) are the real
   ceiling — see `docs/roadmap.md`.
5. **Tools are narrow.** One MCP tool = one core operation. No god-tools
   a hostile page could steer.

## Crates

| crate | role |
|---|---|
| `ghostcloak-core` | engine trait, sessions, registry — no browser code |
| `ghostcloak-fingerprint` | identity schema, coherent generator, auditor |
| `ghostcloak-chromium` | CDP adapter + stealth init scripts |
| `ghostcloak-camoufox` | **primary engine**: patched Firefox (C++-level spoofing) driven over the Juggler pipe, fd 3/4, `\0`-framed JSON — Rust-native, zero Python/Node at runtime |
| `ghostcloak-mcp` | MCP server: the agent-facing surface |
| `ghostcloak-eval` | the stealth referee (identity + web modes) |

## The Camoufox engine

`ghostcloak-camoufox` speaks the Juggler protocol directly (the one
Playwright uses with Firefox):

- launch `camoufox-bin` with `--juggler-pipe`; the channel is **fd 3**
  (commands we→browser) and **fd 4** (responses browser→we)
- messages are `\0`-terminated JSON (Playwright PipeTransport framing)
- the fingerprint is injected via `CAMOU_CONFIG_1` env + `FONTCONFIG_PATH`,
  translated from our TOML identity — spoofing happens **inside the engine
  (C++ level)**, so nothing JS-observable leaks
- pages are driven through per-target sessions (`Browser.attachedToTarget`
  → sessionId) with a live event pump tracking the newest execution context

Verified: `navigator.hardwareConcurrency`, screen size, timezone and
locale all match the generated identity; `navigator.webdriver` is false.

**Full round-trip verified over MCP** (2026-09-01): `session_create` →
`page_open` (https://example.com) → `page_snapshot` returning the page's
extracted text, driven by plain JSON-RPC over stdio.

## Status

Pre-alpha. Layer 1 (JS init scripts) works; layers 2–3 (engine patches,
network fingerprint) are on the roadmap. **Do not use against targets you
don't have permission to test.** This is a testing / research tool.

## License

Rust code and documentation: **MIT OR Apache-2.0** (dual-licensed, see
`LICENSE-MIT` / `LICENSE-APACHE`).
The browser engine is **MPL-2.0** via Firefox/Camoufox upstream.
