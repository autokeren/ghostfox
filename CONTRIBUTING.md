# Contributing to Ghostfox

Thanks for helping make the agent-native stealth browser better.

## Repo layout

- `engine/` — Firefox fork (C++, MPL-2.0): stealth patches, branding, builds.
- `runtime/` — Rust (MIT OR Apache-2.0): MCP server, identity generator, sessions.
- `docs/` — landing page, demo assets, benchmark reports.

## Workflow

1. Open an issue first for features; small fixes can go straight to a PR.
2. Fork, create a branch, and keep changes focused.
3. Run the checks locally before pushing:

```bash
cd runtime
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

4. Add tests for new behavior when practical.
5. Open a pull request and describe what changed and why.

## Code style

- Follow existing Rust style; `cargo fmt` is authoritative.
- Keep MCP tools narrow and typed. No "god tools".
- Prefer receipts and verification for mutations (see existing `page_fill`).
- Do not add user-facing changes without updating README/docs.

## Good first issues

Look for the `good first issue` label. Maintainers are happy to help first-time
contributors get a working build.
