# Ghostfox Journey

Dev-log, todolist dan arah jalan. Satu dokumen buat lihat sudah sejauh apa dan mau ke mana.
Per-version user-facing changes: [`runtime/CHANGELOG.md`](../runtime/CHANGELOG.md).
(Dipelihara dari run nyata — kalau ada wall yang kepecahain, catat di sini; agent berikutnya mewarisi mata kita.)

---

## Perjalanan (yang sudah terlewati)

### v0.1.0 — 2026-09-03 · Lahir
Runtime Rust pertama + engine fork Camoufox. MCP round-trip E2E jalan.

### v0.5.x — awal Sep · "Behave like a human"
- Human mouse: jalur bezier + tremor + overshoot, landing scatter.
- Humanized typing cadence (profil keystroke dynamics, bukan machine-gun).
- `confirm_action` gate, `suspicious_elements` (prompt-injection flagging).
- Identity generator: coherent presets (platform/screen/GPU/fonts yang emang sepaket) + `identity_audit` — 500/500 lolos.
- Android personas: touch points di-patch level C++ (dead di upstream).

### v0.6.x — pertengahan Sep · Mata & bukti
- `page_a11y` semantic walker: shadow DOM + iframe, refs, live values, `login_state`, evidence recording per sesi.
- Juggler dipelajari dalam-dalam: nama event ≠ CDP (`Runtime.console`, `Page.uncaughtError`), hook harus di permanent pump.

### v0.6.7 — 2026-09-21 · Captcha toolset all-native
- 7 family captcha E2E di tempat nyata: GeeTest slide/rotate/icon-click, normal-OCR, Turnstile, TikTok.
- **bilibili icon-click 6/6** "Verification Succeeded" — YOLOv8s + siamese ter-dekuantisasi jalan di rten (CPU).
- ddddocr CRNN di-port ke onnxruntime (tanpa Python).
- Pelajaran penting: model quantized dinamis rusak DIAM-DIAM di rten (semua output 1.0) — selalu dekuantisasi.

### v0.7.0 — 2026-09-25 · Debug cortex + distribusi
- `page_console` / `page_errors` / `page_network_start/read/body` — DevTools trio via MCP.
- Rilis 4 kanal: PyPI, npm, Docker (GHCR), **MCP Registry resmi** (`io.github.autokeren/ghostfox`).
- **Ghostfox posting sendiri** launch announcement-nya di page LinkedIn ghostfox — session portability dari Camoufox-lineage (cookie inject + skema cookies.sqlite Firefox 152: `expiry` itu MILIDETIK, `schemeMap` 256).
- PELAJARAN PALING MAHAL: identitas HARUS match device asal session. Pakai token valid dengan fingerprint beda dari jarak jauh = "impossible login" → LinkedIn revoke SEMUA session (termasuk browser asal). Fix: identity match, documented di AGENTS.md §8.
- Upstream: PR #782 ke daijro/camoufox (zombie-target fix di TargetRegistry.js) — MERGEABLE, nunggu aset build beta.31 maintainer.

### v0.8.0 — 2026-09-27 · arm64 + landscape check
- Linux arm64 end-to-end: asset `ghostcloak-mcp-arm64` (runner native arm), installer arch-aware di pip/npm/install.sh. Engine `lin.arm64` udah ada sejak v0.7.0.
- Survey pasar: space-nya MELEDAJ (browser-use 116k★, vercel agent-browser 43k★, obscura 28k★...). Moat kita: engine ownership + proof corpus + agent UX + canonical. README dapat section "The moat".
- Riset: WebGPU atomic fingerprinting = frontier deteksi 2026. Ditemukan: `dom.webgpu.enabled` compile-time — persona Windows/Mac-ARM kita LEAK (FF152 asli punya WebGPU, kita gak). Ini PR engine berikutnya.

### v0.8.1 — 2026-09-28 · Typing fix + arah baru
- **Typing diperbaiki total**: akar masalah `code` DOM palsu ("Key0"/"Key." → TextInputProcessor anggap non-printable → digit & tanda baca diam-diam dimakan di Lexical). Fix: port usKeyboardLayout Playwright 1:1 (91 entri) + `Page.insertText` buat unicode. 223/223 di composer X.
- **Arsitektur baru disepakati**: Ghostfox = Gecko-native agent browser — observe/act/verify di level engine, receipts semua aksi, AI tetap di luar C++.
- **Local engine build loop** jalan: full build 26 menit di mesin dev (`/home/ubuntu/gf-engine/`). Iterasi patch dari 2 jam (CI) jadi menit-an.
- v0.8.2-in-progress (commits): `page_extract` (tool 43), recipes record/replay (44-47), universal receipts + `page_diff` (48), **native a11y observation** (`page_a11y native=true` via `Page.getFullAXTree` — trusted source, bawaan engine!), 120s juggler bootstrap, headless X-isolation.

### Pelajaran lapangan (hard-won, jangan diulang)
- **CPU contention palsu**: engine "mati" Browser.enable → ternyata render berat di host yang sama + userns probe fork hang under load. Fix: bootstrap 120s + knob `MOZ_ASSUME_USER_NS=0`.
- **LD_PRELOAD jangan di-strip** dari child: lib yang dibutuhin bisa kelewat (mesin NoMachine). `DISPLAY`/`WAYLAND_DISPLAY` boleh (headless = desktop-free).
- Juggler valid types cuma `keydown`/`keyup` lowercase — payload "keyDown"/"char" dibuang diam-diam (`Unknown type`).
- Display itu wilayah orang: jangan spawn window headful di display yang dipake manusia, jangan pkill sembarangan — tanya dulu.

---

## Status hari ini

- **48 MCP tools** (CI hijau, clippy 0-0, test suite green)
- Kanal: PyPI 0.8.1 · npm 0.8.1 · GHCR 0.8.0 · MCP Registry 0.8.0 → semua live
- Bukti nyata: bilibili 6/6 · hCaptcha 2 production site · TikTok OTP · LinkedIn self-post · Docker E2E
- Traction dini: ~215 install/hari (PyPI+npm) sebelum campaign besar; 5 stars, 2 forks, 129 unik cloner

## TODO aktif (urutan eksekusi)

- [ ] **E2E native a11y** (blocked: mesin lagi dipake render) → **cut v0.8.2** (48 tools)
- [ ] **X post v0.8** (draft siap — rate limit keburu cleared?) + repost Show HN timing Senin
- [ ] Streamable HTTP transport (rmcp punya; fork panzx bukti demand — remote use case gulutux)
- [ ] Network interception spike (verifikasi Juggler route/setInterception 1 hari → build 3-5 hari)
- [ ] WebGPU v1 persona spoofing (nutup leak persona Win/Mac-ARM; 2 jam per iterasi build)
- [ ] PR #782: monitor upstream beta.31 assets

## Horizon — C++ arc: panca indra AI di level Gecko

**Prinsip**: AI jangan masuk C++. C++ = sensor + actuator deterministik. Otak di runtime/MCP.
Framing: setiap milestone = satu "indra" yang dibuka untuk agen.

### M1 — 👁 Penglihatan terstruktur ✅ (wiring done)
`Page.getFullAXTree` + flatten → `page_a11y native=true` — trusted a11y tree.
Tinggal: E2E + actionable native refs (ref registry ala backendNodeId).

### M2 — ✋ Peraba: native click/type broker
`semantic_click(ref)` — resolve accessible → **rect layout NATIVE** (JS bisa bohong soal
`getBoundingClientRect`; sites hook rect buat racun klik — koordinat mouse yang gak bisa
dibohongin) → hit-test → trusted events. Drag captcha paling diuntungkan: gap→target presisi.

### M2.5 — 🥷 Stealth pixel extraction
`Agent.captureSurface`: baca piksel dari compositor/backing store TANPA `toDataURL`
(yang bisa di-hook site buat deteksi exfiltrasi) → captcha suite jadi satu-satunya yang
stealth dari sisi akuisisi data. Kombinasi unik yang gak dimiliki kompetitor wrapper-based
(mereka SEMUA lewat toDataURL/CDP screenshot yang bisa di-hook). PR kecil di juggler
additions + compositor hook.

### M3 — 👂 Pendengaran: update inkremental + whisper stream
- AccEvent diff (`Agent.observeDiff`) — a11y tree sebagai stream, bukan full snapshot.
- **DOM mutation whisper**: tiap perubahan DOM = event (bukan polling).
- **WebSocket frames**: network full-duplex (HTTP udah; WS buat challenge/config makin umum).

### M3.5 — 👃 Penciuman: paint & timing sense
- **Visual-stability events** (RefreshDriver/compositor observer): "tunggu sampai
  VISUALLY siap" — bunuh semua `sleep(3)` arbiter di playbook. Ini pengganti
  page_wait_for yang sejati.
- **Timing sense**: DNS/TLS/TTFB per request, long-task events.
- **Jank detector**: main-thread sibuk → jangan klik dulu (page "rasanya" berat).

### M4 — 🧍 Proprioception: state tubuh browser
- Load state, dialogs, downloads, **focus/selection**.
- **Caret/selection/IME state**: agent "merasa" posisi jarinya di editor —
  receipts typing jadi nyata (caret pindah ke mana setelah 1000 chars? selection
  range native, JS gak bisa bohong soal ini).
- **Cookie/session heartbeat**: event "auth cookie berubah/kehapus" → deteksi
  matinya session LEBIH AWAL. Pelajaran LinkedIn (revoked diam-diam) jadi
  gak bakal keulang — kita TAU di detik kejadian.
- **Frame stream + visual diff** (screencast exposure): agent "lihat" animasi
  masih jalan apa udah selesai.

### M4.5 — 🩺 Self-health
- **Memory/CPU per-tab**: agent multi-tab "merasa" tab yang bocor → close.
- Crash content-process → event → auto-recovery (self-healing sessions).

### M5 — 🛡 Policy layer
Origin allowlist, confirmation enforced, redacted evidence.

**Urutan eksekusi**: M1 E2E → v0.8.2 → M2 (+M2.5 bareng) → M3/M3.5 → M4/M4.5 → M5.
**Yang paling "ghostfox banget" dulu**: cookie heartbeat + visual stability — dua-duanya
nyambung langsung ke pengalaman lapangan (session ke-revoke diam-diam, sleep-guessing).

**Kemerdekaan bertahap dari upstream** (strategi): patch stack formalisasi → satu siklus update Firefox kita kerjain sendiri → cherry-pick camoufox selagi open → kriteria hard-fork ditentukan dari awal (internal).

## Yang gak kita lakuin

- Tanam AI reasoning ke C++ (deterministic sensor/actuator doang)
- God-tool "do anything" — primitives kecil di atas native solid
- Ganti prioritas anti-detect (itu moat kita — dua kaki: reliability + survival)
