
## v0.9.4 — Coherence Fixes (Nov 12–13, 2026)
- Persona continuity: session_create reuses the profile identity.toml on implicit launches (persona matches persisted cookies)
- Headful window coherence: --width/--height from identity screen (fixes 10x10 degenerate windows on bare X11)
- Display coherence: headful X11 sessions pin screen dims+dpr to real xrandr geometry
- navigator.buildID synthesized from the engine major
- Geo coherence guidance: identity timezone/geo should match the exit IP location
- user.js hardening prefs are idempotent-merged on every launch (a pre-existing user.js no longer pins defaults)
