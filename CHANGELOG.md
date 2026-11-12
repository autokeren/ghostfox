
## v0.9.4 — Coherence & Multi-Engine (Nov 12–13, 2026)
- Persona continuity: session_create reuses the profile identity.toml on implicit launches (persona matches persisted cookies)
- Headful window coherence: --width/--height from identity screen (fixes 10x10 degenerate windows on bare X11)
- Display coherence: headful X11 sessions pin screen dims+dpr to real xrandr geometry
- navigator.buildID synthesized from the engine major
- Geo coherence guidance: identity timezone/geo should match the exit IP location
- user.js hardening prefs are idempotent-merged on every launch (a pre-existing user.js no longer pins defaults)
- Chromium CDP backend completed: session_create(engine: "chromium") drives the system Chrome on the same Engine/PageHandle abstraction — a11y walker (shadow-piercing), trusted input (click_ref/click_coords/type_ref/press_key), screenshots, wait_for. Chrome user-data isolated under <profile>/chromium-profile.
