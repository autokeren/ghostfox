//! The engine abstraction: every browser backend (Chromium/CDP, patched
//! Firefox, Servo, ...) implements this one trait.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineKind {
    Chromium,
    Firefox,
    Servo,
}

impl EngineKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EngineKind::Chromium => "chromium",
            EngineKind::Firefox => "firefox",
            EngineKind::Servo => "servo",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaunchOptions {
    /// Persistent profile directory (empty = ephemeral).
    pub profile_dir: Option<String>,
    /// Proxy URL, e.g. `socks5://user:pass@host:port`.
    pub proxy: Option<String>,
    /// Extra command-line switches passed to the engine binary.
    pub extra_args: Vec<String>,
    /// Run headless.
    pub headless: bool,
    /// Engine binary override; autodetected when absent.
    pub executable: Option<String>,
}

/// The full a11y snapshot result: elements plus page metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11ySnapshot {
    pub elements: Vec<A11yElement>,
    /// "logged-in" | "logged-out" | "unknown" — detected from login/user-menu
    /// signals so agents don't act blind on a dead session.
    pub login_state: String,
    pub page_url: String,
    pub page_title: String,
    /// v0.5: "financial" | "medical" | "legal" | "authentication" | null
    /// — agents slow down on sensitive pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub danger_zone: Option<String>,
    /// v0.5: number of elements flagged as containing suspicious content
    /// (prompt injection patterns, hidden text).
    #[serde(default)]
    pub suspicious_elements: usize,
    /// v0.5.3: page is archived (read-only, no new interactions possible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_archived: Option<bool>,
    /// v0.5.3: number of elements that belong to the logged-in user.
    #[serde(default)]
    pub own_elements: usize,
    /// v0.5.3: logged-in username (from own-content signals / user menu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// v0.5.3: interactive elements below the visible viewport (scroll to reach).
    #[serde(default)]
    pub below_viewport: usize,
    /// v0.5.3: how many viewport-pages down the lowest element is.
    #[serde(default)]
    pub max_scroll_pages: usize,
}

/// A semantic element from the accessibility walk: what an agent needs to
/// understand and act on a page without knowing any CSS selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11yElement {
    /// Stable handle for click_ref / type_ref ("e12").
    pub r#ref: String,
    /// ARIA-ish role: button, link, textbox, heading, combobox...
    pub role: String,
    /// Accessible name (label, aria-label, placeholder or text).
    pub name: String,
    /// Current value for inputs/editors (reads live form state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// v0.5: element contains suspicious content (prompt injection patterns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspicious: Option<bool>,
    /// v0.5.3: HTML tag name (button, input, a, shreddit-*, facepile-*...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// v0.5.3: expandable element state (aria-expanded / open attr).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// v0.5.3: form field is required (required / aria-required).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// v0.5.3: "visible" | "below" (scroll down) | "hidden" (off-screen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    /// v0.5.3: if visibility == "below", how many viewport-pages down.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_pages: Option<f64>,
    /// v0.5.3: element is OUR own content (matches logged-in username).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own: Option<bool>,
    /// v0.5.3: aria-description / title text for extra context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A captured page state, cheap to hand to an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSnapshot {
    pub url: String,
    pub title: Option<String>,
    /// Accessibility-tree derived text, token-friendly.
    pub content: String,
    pub captured_at: chrono::DateTime<chrono::Utc>,
}

/// A live handle to one page (tab) inside an engine instance.
#[async_trait]
pub trait PageHandle: Send + Sync {
    async fn navigate(&self, url: &str) -> Result<()>;
    async fn snapshot(&self) -> Result<PageSnapshot>;
    async fn click(&self, selector: &str) -> Result<()>;
    async fn type_text(&self, selector: &str, text: &str) -> Result<()>;
    async fn evaluate(&self, expression: &str) -> Result<serde_json::Value>;
    async fn url(&self) -> Result<String>;
    async fn close(&self) -> Result<()>;
    /// Press a named key (Enter, Tab, Escape, arrows...). Engines without
    /// keyboard-event support return an error naming the limitation.
    async fn press_key(&self, key: &str) -> Result<()> {
        let _ = key;
        Err(crate::error::GhostError::PageOp(
            "press_key not supported by this engine".into(),
        ))
    }
    /// Capture the current viewport (full_page=false) or the whole scrollable
    /// document as PNG bytes. Engines without screenshot support return an
    /// error naming the limitation.
    async fn screenshot(&self, full_page: bool) -> Result<Vec<u8>> {
        let _ = full_page;
        Err(crate::error::GhostError::PageOp(
            "screenshot not supported by this engine".into(),
        ))
    }
    /// Semantic snapshot: walk the page (including shadow roots), return
    /// interactive elements with stable refs an agent can act on.
    async fn a11y_snapshot(&self) -> Result<A11ySnapshot> {
        Err(crate::error::GhostError::PageOp(
            "a11y_snapshot not supported by this engine".into(),
        ))
    }
    /// Act on an element by its ref from a11y_snapshot.
    async fn click_ref(&self, r: &str) -> Result<()> {
        let _ = r;
        Err(crate::error::GhostError::PageOp(
            "click_ref not supported by this engine".into(),
        ))
    }
    /// Read the FULL value of the element a ref points at (no truncation).
    async fn read_ref_full(&self, r: &str) -> Result<String> {
        let _ = r;
        Err(crate::error::GhostError::PageOp(
            "read_ref_full not supported by this engine".into(),
        ))
    }
    /// Wait until a CSS selector becomes visible (or timeout). Returns true if visible.
    async fn wait_for(&self, selector: &str, timeout_ms: u64) -> Result<bool> {
        let _ = (selector, timeout_ms);
        Err(crate::error::GhostError::PageOp(
            "wait_for not supported by this engine".into(),
        ))
    }
    /// Type text into the element a ref points at (inputs, editors).
    async fn type_ref(&self, r: &str, text: &str) -> Result<()> {
        let _ = (r, text);
        Err(crate::error::GhostError::PageOp(
            "type_ref not supported by this engine".into(),
        ))
    }
}

/// A running engine process managing pages.
#[async_trait]
pub trait Engine: Send + Sync {
    fn kind(&self) -> EngineKind;
    async fn new_page(
        &self,
        opts: &HashMap<String, serde_json::Value>,
    ) -> Result<Arc<dyn PageHandle>>;
    async fn pages(&self) -> Result<Vec<String>>;
    async fn shutdown(&self) -> Result<()>;
}
