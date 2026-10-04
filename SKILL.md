# Ghostfox — the browser that can SEE

Drive a Gecko-native stealth browser (engine-owned, unhookable) through
67 MCP tools. Everything below is engine-level: page scripts cannot
poison the coordinates, the pixels or the observations.

## The Golden Loop

```
READ    → page_a11y (semantic dashboard) or page_snapshot (plain text)
REASON  → skip? wait? scroll? proceed? — every signal is in the JSON
DECIDE  → one clear next action
ACT     → page_type_ref / page_click_ref / page_click_native / page_type / page_eval
VERIFY  → ALWAYS read again: page_diff (what changed) or
          page_mutations (what the page itself changed) + the action receipt
```

After EVERY action, read again. Never act blind.

## Tool map (the important ones)

| Need | Tool |
|---|---|
| Semantic view of the page | `page_a11y` (also `native: true` for the untamperable engine tree) |
| Only the fields you need | `page_extract` — typed, token-cheap filters |
| Click with UNPOISONABLE coordinates | `page_click_native {role, name}` — a11y bounds, scroll-first |
| Type with verification | `page_type_ref` — fire-then-verify, receipt included |
| Evidence of what happened | receipts on every action; `page_diff` for deltas |
| What the page changed (DOM whispers) | `page_mutations` — unhookable observer stream |
| What the a11y tree did (event stream) | `page_a11y_events` — focus/text/state/caret/live-region diffs |
| Is the main thread busy? | `page_frame_stats` — frame-cadence jank detector |
| Wait until VISUALLY ready | `page_wait_visual` — refresh-driver stability (no sleep guessing) |
| How fast was the load? | `page_timing_report` — nav timing + paint; netcap carries per-request TTFB too |
| Pixels as digits (0-9 grids) | `page_pixels` — compositor-captured, 3 modes (region/semantic/ref) |
| Hidden content (sixth sense) | `page_pixels` canvas refs read drawing buffers; `include_hidden: true` = Layer X-Ray (visibility:hidden content) |
| Visual QA of a rendered UI | `page_ui_audit` — clipped captions, padding, overlap, overflow |
| Repeat a flow without an LLM | `recipe_record` → act → `recipe_save`; `recipe_replay` |
| Captcha families | `page_geetest_slide/click`, `page_captcha_rotate/ocr`, `page_hcaptcha`, `captcha_solve` |
| Console/errors/network | `page_console`, `page_errors`, `page_network_start/read/body` |
| Identity | `identity_generate`, `identity_audit` |

## The receipts rule

`page_click_ref` / `page_type_ref` / `page_click_native` return
{target, url, url_changed, login_state, changes[]} — evidence, not
"clicked". READ `changes` after every action.

## UI work (M5 the Critic)

After any layout/CSS change: `page_ui_audit`. It catches what vibe-coded
designs always get wrong — captions clipped by their container, text
glued to edges, elements off-screen or overlapping. Fix, then audit
again until `counts.errors` is zero.

## Secrets of the senses

- Coordinates from `page_click_native` come from the ENGINE's a11y tree —
  pages that hook `getBoundingClientRect` cannot poison them.
- `page_pixels` reads the COMPOSITOR (and canvas drawing buffers for
  hidden canvases) — no page-realm `toDataURL`, unhookable.
- `page_mutations` observes from the privileged frame-script realm —
  the page cannot hide its own DOM changes.
- `include_hidden: true` on `page_pixels` paints visibility:hidden
  content (the Layer X-Ray) — the compositor's refusal is optional.

## Android E2E via Flutter web (no emulator)

Build the Flutter app for the web and drive it with the senses:

1. `flutter build web --dart-define=API_BASE_URL=http://localhost:8080`
   (the default `10.0.2.2` is the Android emulator's loopback alias).
2. Serve the build dir as the ROOT (Flutter's bootstrap script is
   root-absolute: `/flutter_bootstrap.js`): `python3 -m http.server 8898
   --directory build/web`.
3. Open it, then enable Flutter semantics ONCE (the app must call
   `SemanticsBinding.instance.ensureSemantics()` after
   `WidgetsFlutterBinding.ensureInitialized()`, or click Flutter's
   "Enable accessibility" affordance via the DOM).
4. `page_a11y(native=true)` now reads the Flutter widgets — textboxes,
   buttons, labels, with trusted bounds. `page_click_native` fires
   them. `page_wait_stable` + `page_diff` + receipts close the loop.

Known gaps: typing INTO Flutter text fields (the DOM input receives
the text but the Dart controllers don't sync — WIP); CORS against a
local backend (the engine's CORS bypass mode covers this — set
`GHOSTFOX_INSECURE_CORS=1` for local testing).

## Gotchas

- Refs are ephemeral (e12, n3) — re-walk after DOM churn; recipes use
  semantic anchors (role+name) instead.
- Persistent profiles restore scroll positions — test with fresh
  profiles when clicks "miss" mysteriously.
- Sessions are ephemeral by default: pass `profile_dir` to
  `session_create` for a persistent identity/cookies.