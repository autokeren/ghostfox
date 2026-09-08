//! ghostcloak-fingerprint: declarative browser identities.
//!
//! One TOML file = one coherent persona (screen, GPU, fonts, locale, TZ,
//! navigator, WebRTC policy). The generator never produces contradictory
//! signals (`4 cores on a MacBook Pro M3` is a fingerprinting own-goal), and
//! the auditor validates identities against the same rules before launch.

pub mod generator;
pub mod identity;
pub mod presets;
pub mod auditor;

pub use auditor::audit;
pub use generator::{generate, GenerateOptions};
pub use identity::{Hardware, Identity, Platform, Screen, WebRtcPolicy};
