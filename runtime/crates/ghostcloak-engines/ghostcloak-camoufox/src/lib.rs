//! ghostcloak-camoufox: Camoufox (patched Firefox) engine adapter.
//!
//! Launch strategy — 100% Rust, zero Python/Node at runtime:
//! 1. Spawn the Camoufox binary with `--juggler-pipe` (JSON over stdio).
//! 2. Inject the fingerprint via `CAMOU_CONFIG_1` env var (chunked JSON) and
//!    `FONTCONFIG_PATH` (platform font set), exactly like the reference
//!    Python launcher does — but from our identity TOML, not BrowserForge.
//! 3. Drive pages over the Juggler pipe protocol.
//!
//! The spoofing happens inside the engine (C++ level): canvas, WebGL,
//! fonts, navigator. Nothing is injected from JS, so nothing leaks.

pub mod a11y;
pub mod config;
pub mod engine;
pub mod juggler;

pub use engine::CamoufoxEngine;

use ghostcloak_core::engine::LaunchOptions;
use ghostcloak_core::error::Result;
use std::sync::Arc;

pub const KIND: ghostcloak_core::engine::EngineKind = ghostcloak_core::engine::EngineKind::Firefox;

/// Launch a Camoufox engine instance.
pub async fn launch(opts: &LaunchOptions) -> Result<Arc<CamoufoxEngine>> {
    CamoufoxEngine::launch(opts).await
}
