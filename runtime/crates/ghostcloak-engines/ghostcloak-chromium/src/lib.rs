//! ghostcloak-chromium: CDP-driven Chromium adapter with stealth init scripts.
//!
//! Launch strategy: real Chrome binary (`--headless=new` when asked), a fresh
//! or persistent user-data-dir, and an init script applied per-page that
//! patches the JS surface (navigator, screen, WebGL, canvas, permissions,
//! plugins) to match the identity. Engine-level patches (the Firefox fork
//! route) are a separate, deeper layer; this one must at minimum never *leak*
//! the default values.

pub mod engine;
pub mod stealth;

pub use engine::ChromiumEngine;

use ghostcloak_core::engine::{EngineKind, LaunchOptions};
use ghostcloak_core::error::Result;
use std::sync::Arc;

/// Launch a Chromium engine instance and register nothing — caller owns it.
pub async fn launch(opts: &LaunchOptions) -> Result<Arc<ChromiumEngine>> {
    ChromiumEngine::launch(opts).await
}

/// Convenience: the EngineKind this adapter implements.
pub const KIND: EngineKind = EngineKind::Chromium;
