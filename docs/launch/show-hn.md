# Show HN: Ghostfox — self-hosted stealth browser for AI agents

## Title

Show HN: Ghostfox — self-hosted stealth browser for AI agents (Firefox fork + Rust MCP)

## Body

Hi HN, I built Ghostfox because I kept running into two problems when giving AI agents a browser:

1. They're blind on the web — they can't see shadow DOM, they can't tell if a session is logged in, and CSS selectors break constantly.
2. The stealth tools that help agents survive are either closed-source, hosted, or both — meaning your agent's cookies and identities flow through someone else's cloud.

Ghostfox is the opposite of that: a complete, self-hosted browser stack for agents.

The engine is a fork of the Camoufox Firefox patches (MPL-2.0) — fingerprint spoofing happens at the C++ level (navigator, screen, fonts, WebGL, WebRTC, timezone, audio), not via injected JS that detectors can read.

The runtime is Rust (MIT/Apache-2.0), speaks Firefox's Juggler protocol natively, and exposes 19 MCP tools:

- `session_create` — launches a fresh, coherent device persona
- `page_a11y` — semantic snapshot with stable refs, roles, names, and live values; pierces shadow DOM and same-origin iframes
- `page_click_ref`, `page_type_ref`, `page_fill`, `page_press` — act by ref instead of guessing selectors
- `page_wait_for` — poll until visible, no more `sleep(8)`
- `page_read_ref` — full untruncated values from editors and inputs
- `page_upload_file` — synthetic DataTransfer upload, no native file picker
- `identity_generate`, `identity_audit` — generate and check coherent identities
- `session_evidence` — append-only event log, snapshots, screenshots, and the identity used
- `captcha_solve` — optional hook for solver services

Agents also get `login_state` in every `page_a11y` snapshot, so they can check if a session is still alive before acting, not after failing.

Identity coherence is the part I care most about. A spoofed browser's worst enemy is contradiction — a "MacBook UA" with Windows fonts and a Linux GPU. Ghostfox generates from coherent device presets (platform, screen, GPU, fonts, timezone, hardware) and audits before launch: 500/500 generated identities pass with no contradictions.

There's also an Android persona mode where touch points, pointer media queries, and viewport are patched at the engine level — not just a Chrome DevTools emulation.

Every session writes evidence under `~/.ghostfox/recordings/`: an append-only `events.jsonl`, page snapshots, screenshots, and the exact identity used. You can replay what your agent actually did. That's been more useful to me than any stealth score.

Prebuilt engines ship for Linux, Windows, and macOS (x86_64 + arm64). There's a one-command Linux installer, Docker image, npm package, PyPI package, and a Rust MCP binary.

Honest caveats:

- Pre-alpha. Bugs exist.
- CreepJS-class fingerprint testers can crash the engine (upstream bug, tracked).
- Benchmark numbers were run from a datacenter IP, which is biased against the engine.
- Forking Firefox means monthly rebases — the maintenance burden is real.
- This is a testing/research tool. Don't point it at anything you don't have permission to test.

Repo: https://github.com/autokeren/ghostfox
Docs: https://autokeren.github.io/ghostfox/
Install (Linux x86_64): `curl -fsSL https://raw.githubusercontent.com/autokeren/ghostfox/main/install.sh | bash`
Docker: `docker run ghcr.io/autokeren/ghostfox`

Built on the shoulders of Camoufox (daijro), Firefox, LibreWolf patch tooling, and Playwright's Juggler protocol docs. Huge gratitude to all of them.

I'd love feedback on three things:

1. Would `page_a11y` + `page_click_ref` replace your current selector-based automation?
2. Does the evidence log matter for your use case, or is it nice-to-have?
3. What's the missing piece before you'd actually use this in production?
