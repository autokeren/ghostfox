# Ghostfox Roadmap — v0.4.0 → v1.0.0

> Built from real-world battle scars. Every feature exists because we hit the
> wall ourselves and built the fix. The order is: reliability first, then
> autonomy, then platform.

> **Where we actually are (2026-09):** version numbers diverged from this
> plan — things shipped earlier than scheduled. Shipped so far: v0.5
> behavioral layer + safety gates (v0.6.x: human mouse bezier/tremor,
> humanized typing, `confirm_action`, `page_a11y` suspicious-elements
> flagging), **v0.6.7 native captcha toolset** (8 families incl. YOLO
> icon-click, ddddocr OCR, hCaptcha zoo — E2E on production sites),
> **v0.7 debug cortex** (console/errors/network capture), and the
> distribution layer (PyPI, npm, Docker/GHCR, MCP Registry listing).
> Sections below keep the original plan; done items are checked off
> where they landed.

---

## v0.5.0 — "Behave Like a Human" (Q4 2026)
**Theme: Anti-detect that goes beyond fingerprints — into behavior.**

### Behavioral Authenticity Engine
- [x] **Keystroke dynamics** — randomized typing speed per character (80-200ms),
       occasional pauses between words, thinking delays before form submission
       *(shipped v0.6.x: humanized typing)*
- [x] **Mouse path organicity** — curved movement between points (not teleport),
       overshoot + correction, idle drift
       *(shipped v0.6.x: bezier + tremor + landing scatter)*
- [ ] **Reading rhythm** — scroll pauses at content blocks, back-scroll,
       variable dwell time per element before acting
- [x] **Session cadence** — random micro-delays between actions (500ms-3s),
       occasional tab switches, natural reading patterns
       *(shipped v0.6.x: session cadence pass)*

### Safety Gates
- [x] **Confirmation gates** — irreversible actions ("Submit Order", "Delete",
       "Send Email") require explicit `confirm_action` tool call
       *(shipped: `confirm_action` tool)*
- [ ] **Danger zone detection** — `page_a11y` flags financial/medical/legal
       pages: `"sensitivity": "high"` → agent knows to slow down
- [ ] **Rate-limit recovery** — detect 429/Cloudflare/captcha interstitials,
       auto-backoff with exponential + jitter, notify agent with context
       *(playbook exists in AGENTS.md; auto-backoff not yet a runtime feature)*

### Prompt Injection Defense
- [ ] **Hidden content detection** — flag `display:none`, `opacity:0`,
       `font-size:0`, off-screen text in `page_a11y` output
- [x] **Injection pattern matching** — known attack phrases ("ignore previous
       instructions", "download from", "enter your") marked as
       `"suspicious_content": true`
       *(shipped: `suspicious_elements` in `page_a11y`)*
- [ ] **Content sanitization** — `page_a11y` strips hidden/invisible text from
       `name` and `value` fields, reports what was stripped

**Milestone: Ghostfox agents are behaviorally indistinguishable from humans.**

---

## v0.6.0 — "Never Break" (Q1 2027)
**Theme: Recovery, resilience, and identity lifecycle.**

### Self-Healing Sessions
- [ ] **Auto re-auth** — detect session expiry → attempt re-login from stored
       credentials → resume task (requires credentials vault)
- [ ] **State checkpointing** — every N actions, save page state snapshot +
       task position; on crash, restore from checkpoint
- [ ] **Graceful degradation** — if page_a11y fails, fall back to page_snapshot;
       if that fails, fall back to screenshot; agent always has SOME input

### Identity Lifecycle
- [ ] **Identity aging** — built-in "warm-up" mode: generate identity → let it
       age (browse, read, scroll) for N hours/days before first real action
- [ ] **Reputation tracking** — per-identity activity log; sites where identity
       has karma/history = lower block rate
- [ ] **Identity vault** — encrypted storage (AES-256) for identity + credential
       pairs; exportable between machines
- [ ] **Identity rotation** — automatic rotation policies (by time, by site,
       by block-detection) with continuity guard (don't rotate mid-session)

### Modal & Interruption Handling
- [x] **Auto-dismissal** — detect cookie banners, newsletter popups, "Are you
       18+" overlays → dismiss or accept based on session config
       *(shipped: `page_dismiss_modal`)*
- [ ] **Interruption recovery** — if a modal appears mid-action, snapshot it,
       record in evidence, dismiss, verify original action state preserved

**Milestone: Ghostfox agents survive the chaotic web without human intervention.**

---

## v0.7.0 — "Think Bigger" (Q2 2027)
**Theme: Intelligence at the browser layer.**

### Universal API Adapter
- [ ] **API detection** — scan page for public API hints (meta tags, JSON-LL,
       OpenAPI specs, `/.well-known/`)
- [ ] **API preference** — when API exists, offer agent direct API call option
       (faster, cheaper, more reliable than browser automation)
- [ ] **Hybrid routing** — start with browser, auto-switch to API when
       available; fall back to browser on API failure

### Multi-Page Orchestration
- [ ] **Page graphs** — session maintains a directed graph of visited pages,
       relationships, and state; agent can navigate back semantically
- [ ] **Cross-page state** — "add item to cart on page A, check cart on page B"
       with automatic context transfer
- [ ] **Parallel sessions** — multiple Ghostfox instances with different
       identities, coordinated via MCP (one agent, many personas)

### Enhanced Vision
- [ ] **DOM diffing** — page_a11y returns diff from last snapshot; agent sees
       WHAT CHANGED, not just current state
- [ ] **Layout awareness** — spatial relationships in a11y output
       ("button X is below form Y") for better reasoning
- [ ] **Table extraction** — structured table data as JSON (not just text)

**Milestone: Ghostfox is smarter than the pages it visits.**

---

## v0.8.0 — "Go Everywhere" (Q3 2027)
**Theme: Platform expansion.**

### Ghostfox Mobile (Android APK)
- [ ] **GeckoView integration** — Camoufox patches applied to GeckoView
       (Firefox for Android as a library)
- [ ] **Rust runtime via JNI** — ghostcloak-mcp compiled for aarch64-linux-android
- [ ] **MCP over HTTP** — WebSocket/HTTP transport instead of stdio (Android
       can't do stdio MCP)
- [ ] **Edge model support** — llama.cpp integration for on-device LLM
       (Llama 3.2 1B/3B, Phi-3, Qwen 2.5)
- [ ] **Chat UI** — Kotlin Compose interface: chat + live view + evidence replay
- [ ] **Real mobile advantage** — carrier IP (not proxy), real sensors, real
       touch, real GPS, camera/mic for vision agents

### Advanced Engine Features
- [ ] **TLS fingerprint customization** — JA3/JA4 per identity
- [ ] **Network layer observability** — request/response logging to evidence
- [ ] **WebSocket interception** — for sites that communicate via WS
- [ ] **Service worker support** — for PWAs that depend on service workers

**Milestone: Ghostfox runs on desktop AND mobile, fully self-contained.**

---

## v0.9.0 — "Prove It" (Q4 2027)
**Theme: Enterprise-grade trust and verification.**

### Compliance & Audit
- [ ] **SOC 2 preparation** — access controls, audit logging, data retention
       policies
- [ ] **GDPR mode** — automatic PII detection and redaction in evidence
- [ ] **Compliance export** — session evidence exportable as signed PDF/JSON
       with hash chain (tamper-proof)
- [ ] **Policy engine** — org-level rules: "never visit .gov sites", "always
       use EU proxy for EU sites", "block all file downloads"

### Performance
- [ ] **Cold start < 2 seconds** — profile pre-warming, lazy identity loading
- [ ] **Memory optimization** — target < 500MB per session (currently ~1.2GB)
- [ ] **Concurrent sessions** — 10+ simultaneous sessions on one machine
- [ ] **Token efficiency** — page_a11y returns only CHANGED elements (delta
       snapshots) → 5-10x token reduction

### Benchmark Suite
- [ ] **Patterson-style live benchmark** — automated 31-target suite, results
       published per release
- [ ] **Comparison mode** — benchmark Ghostfox vs playwright-mcp vs
       browser-use on same targets, side-by-side report
- [ ] **Regression detection** — benchmark score drops >5% = CI failure

**Milestone: Ghostfox is enterprise-ready, verified, and trusted.**

---

## v1.0.0 — "The Agent Browser" (2028)
**Theme: The platform that won the browser layer.**

### Agent-Native Web Layer
- [ ] **Structured data endpoints** — Ghostfox exposes per-site "APIs" it has
       learned; agent says `ghostfox.get("reddit.com/user/me")` → structured
       JSON, no browser needed for known sites
- [ ] **Site knowledge base** — community-contributed site profiles (selectors,
       flows, API endpoints, known pitfalls) — like a CDN for site knowledge
- [ ] **Agent-to-agent protocol** — Ghostfox instances communicate directly:
       "I'm agent A on site X, can you check site Y for me?"

### Ecosystem
- [ ] **Plugin system** — community plugins for specific sites, workflows,
       and integrations (like browser extensions but for agents)
- [ ] **Identity marketplace** — pre-warmed, aged identities with history
       (opt-in, privacy-preserving)
- [ ] **Ghostfox Cloud** (optional hosted tier) — managed sessions for teams
       that don't want to self-host (revenue model)
- [ ] **Enterprise support** — SLA, custom builds, on-premise deployment

### Community
- [ ] **1,000+ GitHub stars** (realistic with consistent shipping)
- [ ] **100+ contributors** (engine patches, site profiles, plugins)
- [ ] **Discord community** with 500+ active members
- [ ] **Published benchmarks** that the industry quotes

**Milestone: Ghostfox is THE browser platform for AI agents. The one the
Echofold prediction said would "capture an enormous share of this market."**

---

## Metrics We Track

| Metric | v0.4.0 (now) | v0.7.0 target | v1.0.0 target |
|--------|-------------|---------------|-------------|
| MCP tools | 20 | 35 | 50+ |
| Platforms | 6 (desktop) | 7 (+Android) | 8 (+iOS?) |
| Task completion rate | ~40% (est) | 70% | 85%+ |
| Token efficiency (vs MCP avg) | baseline | 3x better | 10x better |
| Identity coherence | 500/500 | 1000/1000 | 10,000/10,000 |
| Cold start time | ~8s | <3s | <2s |
| Evidence recording | always-on | delta mode | signed + exportable |
| GitHub stars | 0 | 100 | 1,000+ |

---

## What We Don't Do (and why)

- **No native engine from scratch** — Firefox fork is the sane path; building
  an engine from zero is a decade of work
- **No hosted-only model** — self-hosted sovereignty is our core value
- **No closed-source core** — open source is our moat, not our weakness
- **No "general AI"** — we do browser automation, not AGI
- **No Chrome/Chromium** — Firefox is harder to detect (20% of web vs 80%+),
  easier to patch at C++ level, and Mozilla-aligned

*Last updated: 2026-09-09. This roadmap is ambitious but every feature has a
technical path. Priorities shift based on user feedback and market reality.*
