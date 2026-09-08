# Reddit — r/webscraping

**Title:**

> I open-sourced my self-hosted anti-detect browser for AI agents — Rust runtime + own Firefox fork (500/500 coherent identities, session replay built in)

**Text:**

Long-time scraper, first time posting my own tool.

Every stealth option I had was either a paid anti-detect suite (Multilogin,
GoLogin — closed, per-seat pricing, aimed at affiliate multi-accounting) or a
Python wrapper around someone else's patched browser. And the hosted agent
platforms (Browserbase etc) see all your cookies/identities by design.

So over the past weeks I built Ghostfox and just shipped v0.2.0:

- Own fork of the Camoufox Firefox patches (C++-level spoofing: navigator,
  fonts, WebGL, WebRTC, audio, timezone) — I control the build, so upstream
  going partially closed-source doesn't strand me. Builds for Linux, Windows,
  macOS (x86_64 + arm64) straight from CI.
- Rust MCP runtime speaking the Juggler protocol natively — single binary,
  no Node/Python runtime deps. Works with Claude Code, Cursor, anything MCP.
- Identities are generated from coherent device presets and audited: 500/500
  pass with 0 contradictions (the classic spoof-tell is "Windows UA + Mac
  GPU + laptop fonts").
- Every session records evidence: append-only events.jsonl, page snapshots,
  screenshots, the identity used. `session_evidence` returns everything —
  being able to show *what the agent actually did* has been more useful to me
  than any stealth score.
- Android personas: touch points patched at the C++ level (a config key
  that's dead in upstream Camoufox — I had to patch Navigator myself),
  coarse-pointer media queries, portrait viewports from the identity.
- Live-target probe from my datacenter IP: 4/4 targets OK, 0 gated
  (sannysoft panel: webdriver clean, WebGL=Mozilla). Numbers + methodology
  in the repo — run it yourself, don't take my word.

Honest downsides: pre-alpha, CreepJS-class testers can crash the engine
(upstream bug, tracked), and forking Firefox means monthly rebases. It's a
testing/research tool — don't point it at things you don't have permission
to test.

Repo: https://github.com/autokeren/ghostfox
Docs: https://autokeren.github.io/ghostfox/
Install: `curl -fsSL https://raw.githubusercontent.com/autokeren/ghostfox/main/install.sh | bash`

What would make you switch from your current setup? Especially curious
what's missing for your workflows — the roadmap has captcha-solver hooks,
team profile sharing and a credentials vault queued.

---

# Reddit — r/selfhosted

**Title:**

> Self-hosted "stealth browser" for AI agents — own Firefox fork + Rust MCP runtime, session replay included

**Text:**

The hosted agent-browser platforms all have the same shape: your agent's
cookies and identities pass through their cloud. I wanted the opposite, so I
built and open-sourced one that runs entirely on your box.

Ghostfox = a fork of the Camoufox anti-detect Firefox patches (MPL-2.0) + a
Rust MCP server runtime (MIT/Apache). Docker image on ghcr.io, one-command
installer, prebuilt engines for Linux/Windows/macOS x86_64/arm64.

The part I didn't expect to care about: evidence. Every session writes an
append-only event log, snapshots and the exact identity used, and a live
view lets you watch the agent work. When an agent does something weird at
3am, replaying its session beats guessing from logs.

The anti-detect side: C++-level fingerprint spoofing (navigator, fonts,
WebGL, WebRTC, timezone, audio), audited coherent identities (500/500),
Android phone personas with touch/pointer/viewport coherence (that one
needed an engine patch even upstream doesn't have).

Caveats: pre-alpha, needs Linux to build the engine yourself (prebuilts for
everything), monthly Firefox rebases are the real maintenance cost, and
it's a research/testing tool — use it on things you're allowed to test.

Repo: https://github.com/autokeren/ghostfox ·
Docs: https://autokeren.github.io/ghostfox/ ·
Docker: `docker run ghcr.io/autokeren/ghostfox`
