# Show HN — launch post

**Title (under 80 chars, no fluff):**

> Show HN: Ghostfox – self-hosted stealth browser for AI agents (Rust + Firefox fork)

**Text:**

Hi HN, we built Ghostfox because of a pattern we kept hitting: AI agents get
blocked (headless Chrome trips Cloudflare on ~20% of the web), and the
"stealth browser" products that solve it are either closed-source or hosted —
meaning your agent's cookies, identities and sessions flow through someone
else's cloud.

Ghostfox is the self-hosted alternative. One repo, two halves, both yours:

- **The engine** — our own fork of Camoufox (which itself patches Firefox at
  the C++ level: navigator, screen, fonts, WebGL, WebRTC, timezone, audio).
  We build and ship it for Linux/Windows/macOS, x86_64 and arm64.
- **The runtime** — a Rust MCP server that speaks Firefox's Juggler protocol
  natively (no Node, no Python at runtime). Agents get narrow, typed tools:
  session_create, page_open/snapshot/click/type/screenshot, identity_generate.

Three things we care about beyond "don't get detected":

1. **Identity coherence.** Personas are generated from coherent device
   presets and audited before use — the #1 giveaway of a spoofed browser is
   contradiction ("4 cores on a MacBook"). 500/500 generated identities pass
   the auditor. There's also an Android mode where touch points, pointer
   media queries and the viewport are patched to match a real phone — that
   required an engine patch that's dead even upstream.

2. **Evidence.** Every session writes an append-only event log, page
   snapshots, screenshots and the identity used. You can replay what your
   agent actually did — nobody in the stealth space ships this, and it's the
   part enterprises keep asking for.

3. **Owning the stack.** Upstream Camoufox has signaled some future patches
   may go closed-source. Wrappers that depend on their binary releases
   inherit that risk; a fork that controls its build doesn't. MPL-2.0 for the
   engine, MIT/Apache-2.0 for the runtime.

Honest caveats: this is pre-alpha, the benchmark numbers on the README were
run from a datacenter IP (biased against us), CreepJS-class fingerprint
testers can crash the engine (upstream bug we track), and a Firefox fork
means monthly rebases — the maintenance burden is real and we'd rather say
so up front.

Quickstart: `curl -fsSL https://raw.githubusercontent.com/autokeren/ghostfox/main/install.sh | bash`
or `docker run ghcr.io/autokeren/ghostfox` or `pip install ghostfox`.

Docs: https://autokeren.github.io/ghostfox/
Code: https://github.com/autokeren/ghostfox

Built on the shoulders of Camoufox (daijro), Firefox, LibreWolf's patch
tooling and Playwright's protocol docs — GPL/MPL gratitude included.

Happy to answer anything about the fingerprint internals, the Juggler
implementation in Rust, or why we chose Firefox over Chromium when everyone
else patches Chromium.
