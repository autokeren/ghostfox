//! ghostcloak-core: the runtime heart.
//!
//! An engine-agnostic browser runtime for AI agents: sessions, identities,
//! proxies, and the stealth layers that sit between an agent and the web.

pub mod engine;
pub mod engine_registry;
pub mod error;
pub mod session;
pub mod util;

pub use engine::{Engine, EngineKind, LaunchOptions, PageHandle, PageSnapshot};
pub use error::{GhostError, Result};
pub use session::{Session, SessionBuilder, SessionVault};
