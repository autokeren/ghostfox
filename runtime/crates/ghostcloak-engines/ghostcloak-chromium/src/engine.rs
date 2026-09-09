//! CDP engine implementation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use tokio::sync::Mutex;

use ghostcloak_core::engine::{Engine, EngineKind, LaunchOptions, PageHandle, PageSnapshot};
use ghostcloak_core::error::{GhostError, Result};
use ghostcloak_fingerprint::Identity;

use crate::stealth;

pub struct ChromiumEngine {
    browser: Browser,
    identity: Identity,
    /// Keeps the CDP handler pump task alive for the engine's lifetime.
    _keepalive: Arc<Mutex<()>>,
}

impl ChromiumEngine {
    pub async fn launch(opts: &LaunchOptions) -> Result<Arc<Self>> {
        let identity = Identity::load_or_generate(opts.profile_dir.as_deref())?;

        let mut config = BrowserConfig::builder()
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-features=AutomationControlled")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-infobars")
            .arg("--password-store=basic")
            .arg("--use-mock-keychain");

        if let Some(exe) = &opts.executable {
            config = config.chrome_executable(exe);
        } else if let Some(found) = autodetect_chrome() {
            config = config.chrome_executable(found);
        }

        if let Some(dir) = &opts.profile_dir {
            let p = PathBuf::from(dir);
            std::fs::create_dir_all(&p)?;
            config = config.user_data_dir(p);
        }

        if let Some(proxy) = &opts.proxy {
            // Chrome's --proxy-server accepts socks5://user:pass@host:port and
            // host:port forms; pass through as given.
            config = config
                .arg(format!("--proxy-server={proxy}"))
                .arg("--proxy-bypass-list=<-loopback>");
        }

        for extra in &opts.extra_args {
            config = config.arg(extra.as_str());
        }

        if opts.headless {
            // --headless=new is the modern headless: full engine, not the old
            // headless shell that detectors fingerprint instantly.
            config = config.arg("--headless=new").arg("--window-size=1920,1080");
        }

        let config = config
            .build()
            .map_err(|e| GhostError::Protocol(format!("browser config: {e}")))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| GhostError::Protocol(format!("chromium launch: {e}")))?;

        // Keep the CDP handler pump alive on a background task.
        let keepalive = Arc::new(Mutex::new(()));
        let guard = keepalive.clone();
        tokio::spawn(async move {
            let _g = guard;
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });

        Ok(Arc::new(Self {
            browser,
            identity,
            _keepalive: keepalive,
        }))
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }
}

fn autodetect_chrome() -> Option<PathBuf> {
    for candidate in [
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ] {
        if std::path::Path::new(candidate).exists() {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}

fn map_cdp_err<E: std::fmt::Display>(e: E) -> GhostError {
    GhostError::PageOp(e.to_string())
}

#[async_trait]
impl Engine for ChromiumEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Chromium
    }

    async fn new_page(
        &self,
        _opts: &HashMap<String, serde_json::Value>,
    ) -> Result<Arc<dyn PageHandle>> {
        let page = self
            .browser
            .new_page("about:blank")
            .await
            .map_err(|e| GhostError::Protocol(format!("new page: {e}")))?;

        // Apply the identity's stealth init script before any navigation.
        stealth::apply_identity(&page, &self.identity)
            .await
            .map_err(|e| GhostError::PageOp(format!("stealth init: {e}")))?;

        Ok(Arc::new(ChromiumPage {
            page: tokio::sync::Mutex::new(page),
        }))
    }

    async fn pages(&self) -> Result<Vec<String>> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| GhostError::Protocol(e.to_string()))?;
        Ok(pages
            .into_iter()
            .map(|_| ghostcloak_core::util::short_id())
            .collect())
    }

    async fn shutdown(&self) -> Result<()> {
        // Browser::close takes &mut self; we interior-mutate through a lock.
        // (The engine is shared as Arc, so shutdown goes through the page
        // handles' browser instead; here we simply drop, letting the child
        // process terminate with the pipe.)
        let _ = &self.browser;
        Ok(())
    }
}

pub struct ChromiumPage {
    pub(crate) page: tokio::sync::Mutex<Page>,
}

#[async_trait]
impl PageHandle for ChromiumPage {
    async fn navigate(&self, url: &str) -> Result<()> {
        let guard = self.page.lock().await;
        guard.goto(url).await.map_err(map_cdp_err)?;
        Ok(())
    }

    async fn snapshot(&self) -> Result<PageSnapshot> {
        let guard = self.page.lock().await;
        let url = guard.url().await.map_err(map_cdp_err)?.unwrap_or_default();
        let title = guard.get_title().await.map_err(map_cdp_err)?;
        let content = guard.content().await.map_err(map_cdp_err)?;
        Ok(PageSnapshot {
            url,
            title,
            // Inner text only: token-friendly, no markup soup.
            content: html_to_text(&content),
            captured_at: chrono::Utc::now(),
        })
    }

    async fn click(&self, selector: &str) -> Result<()> {
        let guard = self.page.lock().await;
        guard
            .find_element(selector)
            .await
            .map_err(map_cdp_err)?
            .click()
            .await
            .map_err(map_cdp_err)?;
        Ok(())
    }

    async fn type_text(&self, selector: &str, text: &str) -> Result<()> {
        let guard = self.page.lock().await;
        let el = guard.find_element(selector).await.map_err(map_cdp_err)?;
        el.click().await.map_err(map_cdp_err)?;
        el.type_str(text).await.map_err(map_cdp_err)?;
        Ok(())
    }

    async fn evaluate(&self, expression: &str) -> Result<serde_json::Value> {
        let guard = self.page.lock().await;
        let val = guard.evaluate(expression).await.map_err(map_cdp_err)?;
        val.into_value()
            .map_err(|e| GhostError::PageOp(e.to_string()))
    }

    async fn url(&self) -> Result<String> {
        let guard = self.page.lock().await;
        Ok(guard.url().await.map_err(map_cdp_err)?.unwrap_or_default())
    }

    async fn close(&self) -> Result<()> {
        let guard = self.page.lock().await;
        // Page::close consumes self; clone handles the ownership dance.
        guard.clone().close().await.map_err(map_cdp_err)
    }
}

/// Very small HTML -> text extractor for snapshots; good enough for LLM
/// consumption, cheap enough for a hot path. Real a11y-tree snapshots are the
/// planned replacement (tree built from the DOM, not string munging).
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 4);
    let mut in_tag = false;
    let mut last_space = true;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => {
                let is_ws = c.is_whitespace();
                if !is_ws || !last_space {
                    out.push(if is_ws { ' ' } else { c });
                }
                last_space = !is_ws;
            }
            _ => {}
        }
    }
    out
}
