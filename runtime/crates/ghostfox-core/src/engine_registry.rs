//! Registry mapping EngineKind -> factory. Engines register themselves at
//! startup so the core stays engine-agnostic (Chromium, patched Firefox, Servo).

use std::collections::HashMap;
use std::sync::Arc;

use once_cell::sync::Lazy;

use crate::engine::{Engine, EngineKind, LaunchOptions};
use crate::error::{GhostError, Result};

pub type EngineFactory = Arc<
    dyn Fn(&LaunchOptions) -> futures::future::BoxFuture<'static, Result<Arc<dyn Engine>>>
        + Send
        + Sync,
>;

static REGISTRY: Lazy<parking_lot::Mutex<HashMap<EngineKind, EngineFactory>>> =
    Lazy::new(|| parking_lot::Mutex::new(HashMap::new()));

pub fn register(kind: EngineKind, factory: EngineFactory) {
    REGISTRY.lock().insert(kind, factory);
}

/// Acquire a running engine instance for `kind`, or `None` if unregistered.
pub async fn acquire(kind: EngineKind, opts: &LaunchOptions) -> Option<Arc<dyn Engine>> {
    let factory = REGISTRY.lock().get(&kind).cloned()?;
    factory(opts).await.ok()
}

/// Like [`acquire`] but propagates the error.
pub async fn try_acquire(kind: EngineKind, opts: &LaunchOptions) -> Result<Arc<dyn Engine>> {
    // Take the factory out of the lock *before* awaiting: the parking_lot
    // guard is !Send and must not live across an await point.
    let factory = {
        let registry = REGISTRY.lock();
        registry
            .get(&kind)
            .cloned()
            .ok_or_else(|| GhostError::EngineUnavailable(kind.as_str().into()))?
    };
    factory(opts).await
}
