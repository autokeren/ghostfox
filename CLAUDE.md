# Ghostfox monorepo

## Layout

- `engine/` — the browser itself: Camoufox-derived build system (patches,
  additions, branding). **MPL-2.0.** Build with `make dir && make build`
  from that directory. Agent guidance: see `engine/CLAUDE.md`.
- `runtime/` — the Rust MCP runtime (`ghostcloak-*` crates). **MIT OR
  Apache-2.0.** Build with `cargo build --release` from that directory.

## Wiring

The runtime locates the engine via `GHOSTFOX_HOME` (fallback:
`CAMOUFOX_HOME`, then `~/.cache/camoufox`). The MCP server binary is
`runtime/target/release/ghostcloak-mcp`.

## Conventions

- Commits touching `engine/` follow the engine conventions (see
  `engine/CONTRIBUTING.md`); commits touching `runtime/` follow
  conventional commits.
- Never edit generated trees (`engine/camoufox-*/`) — persist engine changes
  as patches (`make edits`).
