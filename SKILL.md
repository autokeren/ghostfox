# Ghostfox — the browser that can SEE

Drive a Gecko-native stealth browser (engine-owned, unhookable) through
51 MCP tools. Everything below is engine-level: page scripts cannot
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

## Gotchas

- Refs are ephemeral (e12, n3) — re-walk after DOM churn; recipes use
  semantic anchors (role+name) instead.
- Persistent profiles restore scroll positions — test with fresh
  profiles when clicks "miss" mysteriously.
- Sessions are ephemeral by default: pass `profile_dir` to
  `session_create` for a persistent identity/cookies.