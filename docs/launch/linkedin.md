# LinkedIn Post — Ghostfox Launch

## Option A (Story-driven, recommended):

---

I spent weeks building a browser. Not for humans — for AI agents. 🦊

Here's the problem: when you give an AI agent a browser, it's blind.

It can't see past shadow DOM. It guesses CSS selectors. It doesn't know if it's logged in. It clicks the wrong button because "submit" matched a random UI element.

And the anti-detect browsers that solve half of this? Closed-source, per-seat pricing, or hosted — meaning your agent's cookies and identities flow through someone else's cloud.

So I built Ghostfox:

→ Own Firefox engine fork (C++-level fingerprint spoofing — navigator, fonts, WebGL, WebRTC, timezone, audio)
→ Rust MCP runtime (single binary, zero Python/Node deps at runtime)
→ Semantic vision — page_a11y sees every element with role, name, and live value, piercing shadow DOM and iframes
→ Session awareness — login_state in every snapshot, agents check before acting
→ Evidence recording — every session writes an append-only event log, screenshots, and the identity used
→ Android personas with touch points patched at the C++ level

19 MCP tools. 6 platforms (Linux, Windows, macOS — x86_64 + arm64). 500/500 identity coherence.

Open source. Self-hosted. Yours.

It's live on npm, PyPI, Docker Hub, and the official MCP Registry.

🔗 github.com/autokeren/ghostfox
📄 autokeren.github.io/ghostfox

What would you build if your agent could actually see the web?

#AI #OpenSource #WebScraping #BrowserAutomation #MCP #Rust #AIAgents #AutoKeren

---

## Option B (Shorter, punchy):

---

AI agents are blind on the web. I built them eyes. 🦊

Ghostfox = self-hosted stealth browser for AI agents:
- Own Firefox fork (C++-level anti-detect)
- Rust runtime (single binary, no Python/Node)
- page_a11y: sees every element semantically, pierces shadow DOM
- login_state detection: knows if session is alive before acting
- Evidence recording: replay every action your agent took

19 tools. 6 platforms. Open source. Self-hosted.

pip install ghostfox | npx ghostfox | docker run ghcr.io/autokeren/ghostfox

🔗 github.com/autokeren/ghostfox

#AI #OpenSource #MCP #Rust #AIAgents

---

## Option C (Technical, for dev audience):

---

Ghostfox v0.4.0: the agent browser that checks if it's logged in before acting. 🦊

New in this release: login_state detection in every page_a11y snapshot. Agents see "logged-in", "logged-out", or "unknown" — no more discovering a dead session mid-action.

Also shipped this week:
- page_a11y: semantic snapshot with refs, roles, accessible names, live values (pierces shadow DOM + same-origin iframes)
- page_click_ref / page_type_ref: act by ref, no CSS selectors
- page_wait_for: poll until visible (no more sleep(8))
- page_upload_file: synthetic DataTransfer upload
- page_read_ref: full untruncated values
- Rich editor support (Lexical, Draft, ProseMirror) via editor-native paste paths

Under the hood: our own fork of the Camoufox Firefox patches + a Rust MCP runtime speaking Juggler natively.

Try it: pip install ghostfox

🔗 github.com/autokeren/ghostfox
📄 autokeren.github.io/ghostfox

#Rust #Firefox #MCP #AIAgents #OpenSource #WebAutomation

---

## Option D (Company Page — "We" tone):

---

🦊 Today we're launching Ghostfox — the self-hosted stealth browser for AI agents.

Most agent browsers are blind. They can't see past shadow DOM. They guess CSS selectors. They don't know if they're logged in.

Ghostfox gives agents eyes:

👁️ Semantic vision — sees every element with role, name, and value, piercing shadow DOM and iframes
🧠 Session awareness — detects login_state before acting, not after failing
🥷 Engine-level anti-detect — C++ fingerprint spoofing in our own Firefox fork
📹 Evidence recording — replay every action your agent took
📱 Android personas — touch points, coarse pointer, portrait viewport

Built with our own Firefox engine fork + Rust MCP runtime. Single binary. Zero Python/Node at runtime.

19 tools. 6 platforms. Open source (MPL-2.0 + MIT/Apache-2.0).

Available now: npm, PyPI, Docker Hub, MCP Registry.

🔗 Link in comments.

#AI #OpenSource #MCP #Rust #AIAgents #BrowserAutomation #AutoKeren
