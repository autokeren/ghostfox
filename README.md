<div align="center">

<img src="engine/additions/browser/branding/ghostfox/logo.png" width="180" alt="Ghostfox" />

# Ghostfox

**The agent-native stealth browser you can own.**

Self-hosted · Open source · MCP-first · Engine-level anti-detect

[![License](https://img.shields.io/badge/engine-MPL--2.0-orange)](engine/LICENSE)
[![License](https://img.shields.io/badge/runtime-MIT%2FApache--2.0-blue)](runtime/LICENSE-MIT)
[![Engine](https://img.shields.io/badge/engine-Firefox%20152-red)](engine/README.md)

</div>

---

AI agents get blocked. Headless Chrome triggers Cloudflare 403s on ~20% of the
web, and hosted "stealth browsers" route your agent's cookies, identities and
sessions through someone else's cloud.

Ghostfox is the alternative: a **complete browser stack you run yourself** —
a fingerprint-coherent stealth engine plus a Rust MCP runtime, in one repo.

```
Firefox (MPL-2.0)
  └─ Camoufox (anti-detect patches, by daijro)
       └─ Ghostfox engine          engine/   — spoofing at the C++ level
            └─ Ghostfox runtime     runtime/  — Rust: sessions, identities, MCP
```

| | Ghostfox | Hosted stealth (Browserbase etc.) | playwright-mcp | Anti-detect suites (Multilogin etc.) |
|---|---|---|---|---|
| Self-hosted | **✓** | ✗ | ✓ | partially |
| Open source | **✓** | ✗ | ✓ | ✗ |
| MCP-native | **✓** | ✓ | ✓ | ✗ |
| Engine-level anti-detect | **✓ (C++/Firefox)** | vendor partnerships | ✗ | ✓ (closed) |
| Coherent identities + auditor | **✓** | ✗ | ✗ | partial |
| Runtime language | **Rust** | — | Node | — |

**One identity, no contradictions.** Identities are generated from coherent
device presets (platform, screen, GPU, fonts that actually ship together),
injected at the engine level, and audited before use — a spoofed browser's
worst enemy is itself saying "4 cores on a MacBook".

## Quickstart

> Requires: Rust toolchain, Linux x86_64. Prebuilt engine binaries: see
> [Releases](../../releases).

```bash
# 1) Get the engine (prebuilt) and unpack it somewhere, e.g. /opt
unzip ghostfox-<ver>-lin.x86_64.zip -d /opt/ghostfox

# 2) Build the runtime
git clone https://github.com/autokeren/ghostfox.git
cd ghostfox/runtime
cargo build --release

# 3) Wire it into any MCP client (Claude Code, Cursor, ...)
```

```json
{
  "mcpServers": {
    "ghostcloak": {
      "command": "/path/to/ghostfox/runtime/target/release/ghostcloak-mcp",
      "env": { "GHOSTFOX_HOME": "/opt/ghostfox" }
    }
  }
}
```

Then the agent can: `session_create` → `page_open` → `page_snapshot` →
`page_click` / `page_type` / `page_fill` / `page_press`, plus
`identity_generate` / `identity_audit`.

From source end-to-end (build the engine yourself):
see [engine/README.md](engine/README.md) — `make dir && make build`.

## Repository layout

```
runtime/   Rust: ghostcloak-{core,fingerprint,mcp,eval}     (MIT OR Apache-2.0)
engine/    Browser fork: patches, branding, build system    (MPL-2.0)
```

Two directories, two licenses, one product. The runtime speaks
[Juggler](https://github.com/microsoft/playwright) natively — no Node, no
Python at runtime.

## Why own the engine?

- **Anti-detect that survives inspection.** Spoofing happens inside the
  engine (navigator, screen, WebGL, fonts, WebRTC, timezone, audio) — not in
  injected JS that detectors can read.
- **No cloud dependency.** Your agent's identities and cookies never touch a
  third-party host.
- **Upstream insurance.** `engine/` tracks [daijro/camoufox](https://github.com/daijro/camoufox)
  as `upstream`; Ghostfox applies its own branding and can rebase whenever it
  wants — including if upstream patches go closed-source.

## Status

Pre-alpha. Verified: identity coherence (500/500), full MCP round-trip
E2E (create → open → fill → submit), multi-page sessions. Known limits are
tracked in the changelogs under `runtime/` and `engine/`.

**Do not use against targets you don't have permission to test.** This is a
testing / research tool.

## Credits

Ghostfox stands on the shoulders of giants —
[Camoufox](https://github.com/daijro/camoufox) (daijro) for the anti-detect
patch stack, [Mozilla Firefox](https://www.mozilla.org/firefox/) for the
engine, [LibreWolf](https://librewolf.net/) for the patch tooling lineage, and
[Playwright](https://github.com/microsoft/playwright) for the Juggler protocol.

## License

- `engine/` — **MPL-2.0** (inherited from Firefox / Camoufox). See [engine/LICENSE](engine/LICENSE).
- `runtime/` — **MIT OR Apache-2.0**. See [runtime/LICENSE-MIT](runtime/LICENSE-MIT).
