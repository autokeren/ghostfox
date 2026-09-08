# Benchmark results — 2026-09-08

## T0: Identity coherence (offline, `ghostcloak-eval -- identity`)

| Metric | Result |
|---|---|
| Identities generated | 500 |
| Auditor violations | **0** |
| Pass rate | **500/500 (100%)** |
| Platform mix | Windows 135 · Android 161 · MacOS 103 · Linux 101 |

Every generated identity is coherent by construction: UA, platform,
`navigator.platform`, screen/DPR class, GPU string, fonts, timezone, locale
and geo are all drawn from a single device preset; the auditor re-checks
and rejected **zero**.

Run it yourself:

```sh
cargo run -p ghostcloak-eval --bin ghostcloak-eval -- identity --count 500
```

## T1: JS surface vs identity (live engine, Ghostfox 152.0.4-beta.30)

Verified signals on a live session (`session_create` → `page_open`):

| Signal | Check | Result |
|---|---|---|
| `navigator.userAgent` | matches identity platform class | ✅ |
| `navigator.hardwareConcurrency` | matches identity cores | ✅ |
| `screen.width/height` | matches identity screen | ✅ |
| `Intl.DateTimeFormat().timeZone` | matches identity timezone | ✅ |
| `navigator.languages` | matches identity locale | ✅ |
| `navigator.webdriver` | false (not `true`/undefined-leak) | ✅ |

## T2: Full MCP round-trip

`session_create` → `page_open` → `page_snapshot` → `page_click` / `page_type` /
`page_fill` / `page_press` verified end-to-end over MCP stdio, including
Android personas (portrait screens, Android UAs, Adreno/Mali GPU strings)
and evidence recording (`session_evidence`).

## Context: third-party landscape

Independent benchmarking of live-target stealth (Patterson, May 2026) and
JS-fingerprint labs (Web Scraping Club, July 2026) placed the Camoufox
engine family mid-table on live targets (25/31 OK with Firefox 135) and
**top-4 of 15 in JS-fingerprint labs**. Ghostfox tracks upstream + rebases
(currently Firefox 152); we do not yet have our own published live-target
run — that is on the roadmap (see ROADMAP.md).

## T3: Live-target probe (2026-09-08, engine 152.0.4 + touch patch)

`ghostcloak-eval targets --headless`, run from a datacenter IP (biased against
us — gate rates are higher from DC ranges):

| Target | Status | Detail |
|---|---|---|
| example.com | ok | — |
| httpbin.org/html | ok | — |
| bot.sannysoft.com | ok | 3 rows passed · 1 "failed" = "Chrome (New): missing" (expected on a Firefox persona, not a real detection) · webdriver clean |
| areyouheadless (Vastel) | ok | not flagged headless (site itself returned 502 this run) |

**4/4 OK, 0 gated.** Run it yourself:

```sh
cargo run --release -p ghostcloak-eval -- targets --headless
```

## Touch coherence (Android personas, engine patch navigator-touch-spoofing)

| Signal | Value on `session_create{"platform":"android"}` |
|---|---|
| navigator.maxTouchPoints | 5 (config-driven, engine-level) |
| matchMedia('(pointer: coarse)') | true |
| matchMedia('(pointer: fine)') | false |
| matchMedia('(hover: hover)') | false |
| TouchEvent / ontouchstart | present |
| navigator.platform | "Linux aarch64" |
| viewport | identity screen class (e.g. 384x815 portrait, dpr 3) |
