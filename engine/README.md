# Ghostfox

**A stealth browser engine for AI agents. Self-built fork of [Camoufox](https://github.com/daijro/camoufox) (Firefox / MPL-2.0), driven by the Rust runtime in ../runtime/.**

Ghostfox produces its own hardened, fingerprint-spoofing browser instead of
depending on upstream binary releases: it fetches the Firefox source, applies
the Camoufox anti-detect patch stack (spoofing at the **C++ level** — navigator,
screen, fonts, WebGL, WebRTC, timezone, audio), adds Ghostfox branding, and
packages the engine.

The Rust side — sessions, coherent identities, MCP server — lives in `../runtime/` (this monorepo).
separate **ghostcloak** repository:

```
Firefox (MPL-2.0)
  └─ Camoufox (anti-detect patches, by daijro)
       └─ Ghostfox   ← this repo: our own builds + branding
            └─ driven by ghostcloak (Rust MCP runtime)
```

## Why a fork

- **Independence** — no dependency on upstream release schedules for the
  engine powering agent workflows.
- **Own branding** — `ghostfox` / `ghostfox-bin`, Ghostfox identity in-app.
- **Upstream tracking** — `upstream` remote follows daijro/camoufox; rebase +
  rebuild whenever we want their latest patches.

Internal protocol names are intentionally **kept** for compatibility:
`camoufox.cfg` (autoconfig), the `CAMOU_CONFIG` env var, `camoucfg.jvv`, and
the Juggler wire protocol.

## Build

Designed for Linux hosts (Windows/macOS cross-compiled):

```bash
bash scripts/install-deps.sh   # host deps (Python ≥3.11, Rust, aria2, p7zip, go, ...)
make dir                       # fetch Firefox source, apply patches + additions
make bootstrap                 # one-time: system deps + mach bootstrap
make build                     # ./mach build
make run                       # launch the built browser
python3 multibuild.py --target linux --arch x86_64   # build + package
```

Docker: `docker build -t ghostfox-builder .` then
`docker run -v "$(pwd)/dist:/app/dist" ghostfox-builder --target <os> --arch <arch>`.

## Credits & upstream dependencies

This project is a fork and would not exist without:

- **[Camoufox](https://github.com/daijro/camoufox)** by daijro — the entire
  anti-detect patch stack, the patched Juggler, the build system. 🙏
- **[Mozilla Firefox](https://www.mozilla.org/firefox/)** — the browser engine
  underneath everything.
- **[LibreWolf](https://librewolf.net/)** — the patch tooling lineage.
- **[Playwright](https://github.com/microsoft/playwright)** — the Juggler
  protocol documentation.

## License

**MPL-2.0** (inherited from Firefox/Camoufox). See `LICENSE`.
