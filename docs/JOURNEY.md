# Ghostfox Journey

The dev-log, todo list, and the road ahead — one document to see how far
we've come and where we're going. (Maintained from real runs — when a wall
gets solved, it gets written down here; the next agent inherits our eyes.)

Per-version user-facing changes: [`runtime/CHANGELOG.md`](../runtime/CHANGELOG.md).

---

## The journey (what's behind us)

### v0.1.0 — 2026-09-03 · Born
First Rust runtime + Camoufox engine fork. MCP round-trip E2E working.

### v0.5.x — early Sep · "Behave like a human"
- Human mouse: bezier paths + tremor + overshoot, landing scatter.
- Humanized typing cadence (keystroke-dynamics profile, not machine-gun).
- `confirm_action` gate, `suspicious_elements` (prompt-injection flagging).
- Identity generator: coherent device presets (platform/screen/GPU/fonts
  that actually ship together) + `identity_audit` — 500/500 pass.
- Android personas: touch points patched at the C++ level (dead upstream).

### v0.6.x — mid Sep · Eyes & evidence
- `page_a11y` semantic walker: shadow DOM + iframes, refs, live values,
  `login_state`, per-session evidence recording.
- Juggler internals mapped: event names ≠ CDP (`Runtime.console`,
  `Page.uncaughtError`), hooks must live in the permanent pump.

### v0.6.7 — 2026-09-21 · The all-native captcha toolset
- 7 captcha families E2E in production: GeeTest slide/rotate/icon-click,
  normal OCR, Turnstile, TikTok.
- **bilibili icon-click 6/6 "Verification Succeeded"** — YOLOv8s +
  dequantized siamese running in rten (CPU).
- ddddocr CRNN ported to onnxruntime (zero Python at solve time).
- Key lesson: dynamically-quantized models fail SILENTLY in rten (all-1.0
  outputs) — always dequantize.

### v0.7.0 — 2026-09-25 · Debug cortex + distribution
- `page_console` / `page_errors` / `page_network_start/read/body` — the
  DevTools trio over MCP.
- Four distribution channels: PyPI, npm, Docker (GHCR), and the official
  **MCP Registry** (`io.github.autokeren/ghostfox`).
- **Ghostfox posted its own launch announcement** on the ghostfox LinkedIn
  page — session portability from a Camoufox-lineage browser (cookie
  injection + the Firefox 152 cookies.sqlite schema: `expiry` is
  MILLISECONDS, `schemeMap` 256).
- THE MOST EXPENSIVE LESSON: the identity must match the session's origin
  device. Using a valid token behind a different fingerprint from a distant
  IP = "impossible login" → LinkedIn revoked ALL sessions (including the
  origin browser). Fix: identity matching, documented in AGENTS.md §8.
- Upstream: PR #782 to daijro/camoufox (zombie-target fix in
  TargetRegistry.js) — MERGEABLE, waiting on maintainer beta.31 build assets.

### v0.8.0 — 2026-09-27 · arm64 + landscape check
- Linux arm64 end-to-end: `ghostfox-mcp-arm64` asset (native arm runner),
  arch-aware installers in pip/npm/install.sh. The `lin.arm64` engine zip
  has shipped since v0.7.0.
- Market survey: the space EXPLODED (browser-use 116k★, Vercel
  agent-browser 43k★, obscura 28k★...). Our moat: engine ownership + proof
  corpus + agent UX + canonical distribution. README gained "The moat".
- Research: WebGPU atomic fingerprinting is 2026's detection frontier.
  Found: `dom.webgpu.enabled` is compile-time — our Windows/Mac-ARM
  personas LEAK (real FF152 has WebGPU, we don't). That's the next engine PR.

### v0.8.1 — 2026-09-28 · Typing fix + the new direction
- **Typing fixed at the root**: fake DOM `code` values ("Key0"/"Key." —
  Firefox's TextInputProcessor reads them as non-printable, so digits and
  punctuation were silently eaten in Lexical editors). Fix: Playwright's
  usKeyboardLayout ported 1:1 (91 entries) + `Page.insertText` for
  unicode. 223/223 in X's composer.
- **New architecture agreed**: Ghostfox = a Gecko-native agent browser —
  observe/act/verify at the engine level, receipts on every action, AI
  reasoning stays OUT of C++.
- **Local engine build loop** achieved: full build in 26 minutes on the
  dev box. Patch iteration went from 2h (CI round-trips) to minutes.
- v0.8.2-in-progress (committed): `page_extract` (tool 43), recipes
  record/replay (44-47), universal receipts + `page_diff` (48), **native
  a11y observation** (`page_a11y native=true` via `Page.getFullAXTree` —
  the trusted source, already in the engine!), 120s juggler bootstrap,
  headless X-isolation.

### Field lessons (hard-won — don't repeat)
- **THE 8-HOUR HOST WEDGE (2026-09-29)**: every host-spawned engine hung
  at Browser.enable while Docker on the same kernel worked instantly.
  Every theory died (env ×6, uid, netns, cgroup, inotify, /dev/shm,
  mounts) — root cause: 7272 DANGLING SYMLINKS in the dev engine dist,
  pointing into a workspace deleted during disk cleanup. Chrome
  components silently fail to load when their files are dead links.
  Lesson: after deleting anything, `find -type l -xtype l` before
  blaming the kernel.
- **a11y lazy-init blocks on the ATK bridge**: on hosts without an
  AT-SPI bus, atk_bridge_adaptor_init blocks the main thread forever
  (socket read). AND Firefox force-sets NO_AT_BRIDGE=0, ignoring the
  env. Fix shipped: atk-bridge-env.patch (respect the env) + runtime
  sets NO_AT_BRIDGE=1.
- **Fake engine deaths from CPU contention**: Browser.enable "timeouts"
  were actually heavy renders on the same host + the userns fork-probe
  hanging under load. Fix: 120s bootstrap + the `MOZ_ASSUME_USER_NS=0`
  operator knob.
- **Never strip LD_PRELOAD from children**: required libs can route
  through it (a NoMachine host died from this). `DISPLAY`/`WAYLAND_DISPLAY`
  are safe to strip (headless means desktop-free).
- Juggler's valid key event types are lowercase `keydown`/`keyup` only —
  "keyDown"/"char" payloads are silently discarded (`Unknown type`).
- A display belongs to a human: never spawn headful windows on a display
  someone is using, never pkill broadly — ask first.

### 8-family E2E field notes (2026-09-30, headful on the host)

| # | Family | Target | Result |
|---|---|---|---|
| 1 | GeeTest v3 slide | demos.geetest.com/slide-popup.html | ✅ 验证成功 (drag_x=133) |
| 2 | Rotate | 2captcha.com/demo/rotatecaptcha | ✅ solved:true (after 2 fixes) |
| 3 | Normal OCR | 2captcha.com/demo/normal | ✅ "Captcha is passed successfully!" (w9h5k, first try) |
| 4 | Turnstile | demo.turnstile.workers.dev | ✅ XXXX.DUMMY.TOKEN.XXXX + sign-in |
| 5 | Icon-click | passport.bilibili.com (REAL) | 🟡 rounds advance (clicks accepted) but next-round image throttled on our flagged session; full pass proven 2026-09-21 |
| 6 | TikTok OTP | tiktok.com | prior evidence (2026-09-21, logged in) |
| 7 | GeeTest v4 radar | demos.geetest.com/fullpage.html | ✅ 验证成功 (radar→slide escalation, drag_x=131) |
| 8 | hCaptcha | accounts.hcaptcha.com/demo | ⏸ ran out of time in this session (production pass proven earlier) |

Lessons from THIS run:
- **geetest demo URLs moved**: `demos.geetest.com/<x>` 404s now — the
  working paths end in `.html` (slide-popup.html, fullpage.html,
  click-popup.html). Index: demos.geetest.com/ lists them all.
- **2captcha demos are SVG mocks now**: rotate/normal/turnstile pages
  re-rendered as marketing mockups; the rotate demo's feedback lives in
  `[role=alert]` ("Incorrect captcha angle...") and the replay labels
  are ENGLISH ("Check"/"Reset") — solver updated bilingual. The normal
  demo's captcha img = `captcha-*.jpg` (find it, the walk won't list it).
  The turnstile demo widget never renders — use
  demo.turnstile.workers.dev (Cloudflare's official dummy) instead.
- **Persistent-profile startupCache serves STALE engine JS** after an
  omni.ja swap — clear `profile/startupCache/` when deploying engine
  changes to a reused profile.
- **Hidden-canvas rule**: any canvas the compositor never renders
  (CSS 0x0, display:none) must be read from its BUFFER, not
  captureSurface. That's why Page.captureCanvasBuffer exists.
- **Human-like E2E rig**: keep the browser ALIVE across the tour — a
  persistent MCP bridge (unix socket, /tmp/opencode/gf_bridge.py +
  gf.py) + one session + tool-by-tool interaction, like an agent seeing
  the page for the first time. The user watches headful on the desktop.

---

## Status today

- **48 MCP tools** (CI green, clippy 0-0, test suite green)
- Channels: PyPI 0.8.1 · npm 0.8.1 · GHCR 0.8.0 · MCP Registry — all live
- Production proofs: bilibili 6/6 · hCaptcha on 2 real signups · TikTok
  OAuth+OTP · the LinkedIn self-post · Docker E2E
- Early traction: ~215 installs/day (PyPI+npm) before any real campaign;
  5 stars, 2 forks, 129 unique cloners

## Active todo (execution order)

- [x] M1 native a11y E2E → shipped in v0.8.2
- [x] M2 native click broker → shipped (page_click_native, E2E verified)
- [x] M2.5 compositor pixel capture → shipped (captureSurface + buffer reads)
- [ ] **M2.9 the sixth sense** → Layer X-Ray + native framebuffer reads first
  (see the M2.9 section — closes the hidden-canvas hole the E2E fell into)
- [ ] M3 hearing: a11y AccEvent diff + mutation whispers + WebSocket frames
- [ ] M3.5 smell: visual-stability events + timing sense + jank detector
- [ ] M4/M4.5 proprioception + interoception (body state, session health)
- [ ] M5 taste: persona metamorphosis (session state + fingerprint coherence)
- [ ] Streamable HTTP transport (rmcp has it; the fork proved demand)
- [ ] Network interception spike (verify Juggler route/setInterception — 1 day, then 3-5 days to build)
- [ ] WebGPU v1 persona spoofing (close the Win/Mac-ARM leak; 2h per build cycle)
- [ ] PR #782: watch for upstream beta.31 assets
- [ ] MCP Registry: publish 0.8.2+ (needs user device authorization)

## Horizon — the C++ arc: an AI's five senses at the Gecko level

**Principle**: AI never goes into C++. C++ is the deterministic sensor +
actuator. The brain lives in the runtime/MCP.
Framing: every milestone unlocks one more "sense" for the agent.

### M1 — 👁 Structured sight ✅ SHIPPED
`Accessibility.getFullAXTree` (note: the Accessibility domain, not Page —
v0.7 engines had it under Page) + flatten → `page_a11y native=true` — the
trusted a11y tree. E2E verified on the host: 230 elements in 0s (richer
than the 199-element JS walker; real hrefs in `value`, elements the walker
misses). Engine-side shipped: atk-bridge-env.patch + NO_AT_BRIDGE=1 at
spawn. Remaining: actionable native refs (a backendNodeId-style registry).

### M2 — ✋ Touch: the native click/type broker
**SHIPPED (in-development, E2E verified).** `page_click_native(role, name)`
resolves the accessible → the NATIVE layout rect (JS can lie about
`getBoundingClientRect`; sites hook rects to poison clicks — mouse
coordinates that cannot be fooled) → scroll-first → trusted events.
Drag captchas benefit most: precise gap→target geometry.

Field lessons (this one fought hard):
- **The 57px chrome offset**: a11y bounds are window-relative, page CSS
  is content-viewport-relative; the delta is the engine's hidden chrome
  strip. Fix: `mozInnerScreenY - screenY` — window properties, always
  fresh, zero staleness (the doc-accessible's own bounds lag scrolls).
- **`scrollToPoint` is a misnomer**: it moves the VIEWPORT so a screen
  point tops the window — using it to "scroll an element into view"
  shoves the page the wrong way. Replaced with deterministic instant
  `win.scrollTo` computed from the a11y rect (smooth `scrollIntoView`
  raced the click dispatch mid-animation).
- **Scroll restoration poisons tests**: persistent profiles restore the
  previous scroll position on revisit — a "click missed at y=364"
  turned out to be the page arriving pre-scrolled. Test with fresh
  profiles; the restore never came from our tools.

### M2.5 — 🥷 Stealth pixel extraction
**SHIPPED (core + GeeTest migrated).** `Page.captureSurface`: raw RGBA
from the compositor/backing store WITHOUT `toDataURL` (which sites can
hook to detect exfiltration) → the captcha suite becomes the only one
that's stealth at the data-acquisition layer. A combination no
wrapper-based competitor can have — they ALL go through hookable
toDataURL/CDP screenshots. `page_pixels` v2 (region / semantic / ref
modes, grids in Rust) + the GeeTest slide solver now read pixels this way.

Field lessons:
- **drawSnapshot is black for offset rects**: a rect whose origin is not
  (0,0) returns a BLACK snapshot (Firefox quirk, invisible to every
  screenshot path because screenshots always use the full viewport).
  Fix: snap the full content viewport and crop via
  `drawImage(snapshot, -x, -y)`.
- **Xray Maps don't cross from frame scripts**: the page-realm
  `__gfxRefs` Map is unreliable through the frame script compartment —
  resolve walk-ref rects via page-realm evaluate instead; the PIXELS
  stay compositor-native either way.

### M2.9 — 🔮 The Sixth Sense: see what the page HIDES
Born from the live 8-family E2E (2026-09-30): every wound found maps to
something the page hides from the JS agent. We own the house — the sixth
sense is planted in Gecko/C++ bones, not page JS.

Wounds that named this milestone:
- **GeeTest fullbg canvas is CSS 0x0** — hidden canvases never reach the
  compositor. Current fix reads the buffer via Xray; the sixth sense
  reads it at the ENGINE level.
- **Bilibili throttles the next round's image** — we couldn't tell
  "loading" from "strangled". The page's nervous system is visible from
  the profiler.
- **Feedback appears as [role=alert]** — the rotate sweep missed it. We
  need a "what just appeared" stream (also M3's whisper).
- **A11y tree showed 7 elements on a page with hundreds** — frames,
  canvas internals and widget guts are invisible to both walkers.

The six (engine-level, C++ not JS):
1. **Layer X-Ray**: patch/own `drawSnapshot` to capture ANY layer —
   `display:none`, `opacity:0`, `translateX(-9999)`, hidden canvases.
   The compositor refuses them today; we stop refusing.
2. **Native framebuffer reads**: canvas + WebGL backing stores read in
   C++, including OffscreenCanvas inside Web Workers (GeeTest renders
   puzzles in workers).
3. **Mutation whispers**: hook nsIMutationObserver natively — every DOM
   change as an event stream, unhookable (JS MutationObserver is
   detectable and the page can lie).
4. **Profiler nervous system**: after each action, which JS ran, how
   long, what it touched — anti-bot scripts light up.
5. **Truth vs Lie detector**: compositor-real innerWidth/scroll/DPR vs
   the page-claimed (spoofed) values — the delta is the trap map.
6. **Ghost frame**: our own shadow frame that renders hidden content
   INTERACTIVELY (not just pixels — clickable), plus storage
   archaeology (what the page learned about us).

Priority: 1+2 first (they close the exact hole the E2E fell into), then
3+4 fold into M3.

Progress (2026-09-30):
- **SHIPPED: GfxXray** — an XPCOM service reading canvas DRAWING BUFFERS
  at the C++ level (`HTMLCanvasElement::GetImageBuffer`, Unrestricted
  extraction): 2D, WebGL and transferred OffscreenCanvas. Zero JS in the
  pixel path. E2E: a display:none canvas read natively (RGB stripes
  exact); geetest slide regression passed. WebGL trap covered too
  (readPixels beats the preserveDrawingBuffer:false blank).
- **Known gap (upstream)**: 2D OffscreenCanvas on WORKERS — the
  compositor-side snapshot relies on GetFrontBufferSnapshot, which 2D
  contexts never implement. Fix = dispatch our own runnable calling
  OffscreenCanvas::GetSurfaceSnapshot on the owning worker thread.
- **Found: page-realm expandos are INVISIBLE to frame scripts** (Xray
  AND waiveXrays) — every 'stale ref' mystery traced to this. Ref
  resolution now uses a marker attribute + privileged querySelector.
- Next: worker-2D readback runnable, then the CrossProcessPaint
  'PaintAllLayers' flag for the Layer X-Ray.

Progress (2026-09-30, continued):
- **SHIPPED: Layer X-Ray** — RenderDocumentFlags::ForceVisibleContent
  threaded from drawSnapshot through CrossProcessPaint to
  PresShell::RenderDocument with a scoped nsIFrame override: hidden
  (visibility:hidden/collapsed) content paints in the privileged
  snapshot. E2E: a hidden green strip renders white normally, GREEN
  with include_hidden. Persisted: m2.9-layer-xray.patch.
- **SHIPPED: page_mutations (M3 hearing)** — privileged MutationObserver
  whispers: every DOM change as pullable records, unhookable.
- **SHIPPED: the ONE-NAME rename** — ghostcloak* -> ghostfox everywhere
  (crates, binary ghostfox-mcp, RUST_LOG targets, CI assets, docs).
  Two names were historical baggage; the repo is single-brand now.

### M3 — 👂 Hearing: incremental updates + whisper streams
- AccEvent diff (`Agent.observeDiff`) — the a11y tree as a stream, not
  full snapshots.
- **DOM mutation whispers**: every DOM change as an event (no polling).
- **WebSocket frames**: full-duplex network (HTTP done; WS carries
  challenge/config more and more).

### M3.5 — 👃 Smell: paint & timing sense
- **Visual-stability events** (RefreshDriver/compositor observer):
  "wait until VISUALLY ready" — kills every arbitrary `sleep(3)` in the
  playbook. The true successor to page_wait_for.
- **Timing sense**: DNS/TLS/TTFB per request, long-task events.
- **Jank detector**: main thread busy → don't click yet (the page
  "feels" heavy).

### M4 — 🧍 Proprioception: browser body state
- Load state, dialogs, downloads, **focus/selection**.
- **Caret/selection/IME state**: the agent "feels" where its fingers are
  in an editor — typing receipts become real (where did the caret land
  after 1000 chars? native selection range, JS can't lie about it).
- **Cookie/session heartbeat**: an event when auth cookies change/get
  deleted → session death detected EARLY. The LinkedIn lesson (silent
  revocation) can't repeat — we'd know the second it happens.
- **Frame stream + visual diff** (screencast exposure): the agent "sees"
  whether an animation is still running or actually finished.

### M4.5 — 🩺 Self-health
- **Per-tab memory/CPU**: a multi-tab agent "feels" the leaking tab → closes it.
- Content-process crash → event → auto-recovery (self-healing sessions).

### M5 — 🛡 Policy layer
Origin allowlists, enforced confirmations, redacted evidence.

**Execution order**: M1 E2E → v0.8.2 → M2 → M2.5 → M2.9 (sixth sense) →
M3/M3.5 → M4/M4.5 → M5.
**The most "ghostfox" ones first**: cookie heartbeat + visual stability —
both connect directly to field experience (the silently-revoked session,
sleep-guessing in the playbook).

## Progressive independence from upstream (strategy)

Formalize the patch stack → do one full Firefox update cycle ourselves →
cherry-pick Camoufox while it stays open → hard-fork criteria agreed
upfront (internal).

## What we don't do

- Embed AI reasoning in C++ (deterministic sensors/actuators only)
- God-tools that "do anything" — small primitives on solid natives
- De-prioritize anti-detect (it's the moat — two legs: reliability +
  survival)
