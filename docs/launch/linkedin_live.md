# LinkedIn Post — Live Demo Story

## Post body (copy-paste):

---

I just watched an AI agent post to X using the browser I built.

Not a demo. Not a mockup. It happened on my screen, right now, while I screen-recorded the whole thing.

Last week I started building Ghostfox — a self-hosted stealth browser for AI agents. Firefox fork with C++-level anti-detect patches, plus a Rust MCP runtime I wrote from scratch.

Why? Because every AI agent I saw was BLIND on the web:
→ Can't see past shadow DOM
→ Can't tell if they're still logged in
→ Guesses CSS selectors and clicks the wrong button
→ No record of what they actually did

So I built tools to fix this:

👁️ page_a11y — semantic vision that pierces shadow DOM
🧠 login_state — agent knows if session is alive before acting
🦀 page_type_ref — typing that handles real editors (Lexical, Draft)
📹 session_evidence — every action logged, replayable audit trail

Today I put it to the test.

I gave my AI agent (opencode + GLM 5.3) one job: post to X.

What it did:
→ READ the compose page with page_a11y: found 31 elements
→ TYPED the post with page_type_ref: 278 characters
→ VERIFIED the text landed (not assumed — verified)
→ CLICKED the Post button with page_click_ref
→ VERIFIED the post appeared in the timeline

6 tool calls. No scripts. No pre-recorded actions. The agent read, understood, and acted — each step a separate decision.

The best part? It caught its own mistake. The first draft was too long for X's character limit. It detected the truncation, rewrote a shorter version, and posted that instead.

I didn't intervene. I just watched.

Numbers from this week:
✅ 21 MCP tools (15 rated A by Glama)
✅ 100% Server Quality Checklist score
✅ 6 platforms (Linux, Windows, macOS × x86_64, ARM64)
✅ 5 distribution channels (npm, PyPI, Docker, MCP Registry, GitHub)
✅ 500/500 identity coherence
✅ Live post to X verified

Every feature exists because I hit the wall myself. The shadow DOM piercing? Because Reddit hid a form field inside a web component and my agent couldn't find it for 3 hours. The login detection? Because my agent acted on a dead session without knowing.

Build what you need. The features write themselves.

Ghostfox is open source and free. Link in comments.

Video coming soon — I recorded the whole X post in real-time.

What's the wildest thing you've seen an AI agent do on the web?

#BuildInPublic #OpenSource #Rust #MCP #AIAgents #BrowserAutomation #Firefox #AutoKeren

---

## First comment (link):

Ghostfox is free and open source:

🔗 Repo: github.com/autokeren/ghostfox
🌐 Landing: ghostfox.autokeren.com
📚 Docs: autokeren.github.io/ghostfox

Install:
→ pip install ghostfox
→ npx ghostfox install
→ docker pull ghcr.io/autokeren/ghostfox

Verified by Glama: 100% Server Quality Checklist, 15/19 tools rated A.
