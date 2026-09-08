//! Sessions: a persistent identity (fingerprint + cookies + storage) that an
//! agent can pause and resume across runs.

use std::collections::HashMap;
use std::sync::Arc;

use crate::engine::{Engine, EngineKind, LaunchOptions, PageHandle};
use crate::error::{GhostError, Result};

pub struct Session {
    pub id: String,
    pub identity: String,
    pub engine_kind: EngineKind,
    engine: Arc<dyn Engine>,
    pages: tokio::sync::Mutex<HashMap<String, Arc<dyn PageHandle>>>,
}

impl Session {
    /// Assemble a session around an already-launched engine.
    pub fn from_engine(
        id: String,
        identity: impl Into<String>,
        engine_kind: EngineKind,
        engine: Arc<dyn Engine>,
    ) -> Self {
        Self {
            id,
            identity: identity.into(),
            engine_kind,
            engine,
            pages: Default::default(),
        }
    }

    pub fn engine(&self) -> &Arc<dyn Engine> {
        &self.engine
    }

    pub async fn new_page(&self, url: Option<&str>) -> Result<Arc<dyn PageHandle>> {
        let page = self.engine.new_page(&HashMap::new()).await?;
        if let Some(url) = url {
            page.navigate(url).await?;
        }
        let id = crate::util::short_id();
        self.pages.lock().await.insert(id, page.clone());
        Ok(page)
    }

    pub async fn page(&self, id: &str) -> Result<Arc<dyn PageHandle>> {
        self.pages
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| GhostError::PageNotFound(id.into()))
    }

    pub async fn page_ids(&self) -> Vec<String> {
        self.pages.lock().await.keys().cloned().collect()
    }

    pub async fn close_page(&self, id: &str) -> Result<()> {
        match self.pages.lock().await.remove(id) {
            Some(p) => p.close().await,
            None => Err(GhostError::PageNotFound(id.into())),
        }
    }
}

/// Builder-style sugar for [`SessionVault`].
pub struct SessionBuilder {
    engine_kind: EngineKind,
    launch: LaunchOptions,
}

impl Default for SessionBuilder {
    fn default() -> Self {
        Self {
            engine_kind: EngineKind::Chromium,
            launch: LaunchOptions::default(),
        }
    }
}

impl SessionBuilder {
    pub fn engine(mut self, kind: EngineKind) -> Self {
        self.engine_kind = kind;
        self
    }

    pub fn headless(mut self, yes: bool) -> Self {
        self.launch.headless = yes;
        self
    }

    pub fn proxy(mut self, url: impl Into<String>) -> Self {
        self.launch.proxy = Some(url.into());
        self
    }

    pub fn profile_dir(mut self, dir: impl Into<String>) -> Self {
        self.launch.profile_dir = Some(dir.into());
        self
    }

    pub fn build(self) -> SessionVault {
        SessionVault::new(self.engine_kind, self.launch)
    }
}

/// Builds and runs sessions over a fleet of engines.
pub struct SessionVault {
    default_engine: EngineKind,
    launch: LaunchOptions,
}

impl SessionVault {
    pub fn new(default_engine: EngineKind, launch: LaunchOptions) -> Self {
        Self {
            default_engine,
            launch,
        }
    }

    pub async fn spawn(&self, identity: &str) -> Result<Session> {
        let engine =
            crate::engine_registry::try_acquire(self.default_engine, &self.launch).await?;
        Ok(Session {
            id: crate::util::short_id(),
            identity: identity.into(),
            engine_kind: self.default_engine,
            engine,
            pages: Default::default(),
        })
    }
}
