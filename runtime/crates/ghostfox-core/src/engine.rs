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
    /// Identity TOML to launch with (overrides profile identity.toml
    /// and the fallback regeneration — the caller's generated persona).
    pub identity_toml: Option<String>,
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
    /// v0.5.3: visible notifications (toasts, alerts, errors) — the agent
    /// MUST read these after every action (self health).
    #[serde(default)]
    pub notifications: Vec<String>,
    /// v0.5.3: rate limit seconds remaining, parsed from notifications
    /// ("try again in 381 seconds"). Agent waits instead of retrying blind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_seconds: Option<u64>,
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
    /// M2: native layout bounds (CSS pixels) from the accessibility tree —
    /// trusted geometry for semantic clicks (JS getBoundingClientRect can
    /// be hooked to poison coordinates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Bounds>,
    /// Extra engine-side keys (debug telemetry etc.), preserved verbatim.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Native layout rectangle (CSS pixels), from the a11y tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

/// M2.5: a canvas drawing buffer read from the privileged frame script.
/// `raw` = plain RGBA from a WebGL readPixels (no image header);
/// otherwise `bytes` = an encoded image (PNG/JPEG data URL payload).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasBuffer {
    pub bytes: Vec<u8>,
    pub raw: bool,
    pub width: u32,
    pub height: u32,
}

/// M5 the Critic: a plain DOM element's layout rect (tag + geometry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiRect {
    pub tag: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

/// M2.5: raw RGBA pixels of a viewport rectangle, straight from the
/// compositor (no page-realm canvas, no toDataURL, no PNG round-trip).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfacePixels {
    pub width: u32,
    pub height: u32,
    /// RGBA, row-major, 4 bytes per pixel.
    pub rgba: Vec<u8>,
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
    /// v0.6.2: The engine-level target id (tab), if this handle wraps one.
    fn target_id(&self) -> Option<String> {
        None
    }
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
    /// NATIVE observation: the engine's own accessibility tree (trusted —
    /// page scripts cannot tamper), raw JSON from the Juggler
    /// Page.getFullAXTree protocol. Engines without native a11y error.
    async fn a11y_tree_native(&self) -> Result<serde_json::Value> {
        Err(crate::error::GhostError::PageOp(
            "native a11y tree not supported by this engine".into(),
        ))
    }
    /// M2.5: read a canvas's DRAWING BUFFER (data URL) from the
    /// privileged frame script — the Xray wrapper calls the NATIVE
    /// toDataURL, which page hooks cannot poison or observe. For
    /// hidden canvases the compositor never renders (GeeTest fullbg).
    async fn capture_canvas_buffer(&self, target: &str) -> Result<Option<CanvasBuffer>> {
        let _ = target;
        Err(crate::error::GhostError::PageOp(
            "capture_canvas_buffer not supported by this engine".into(),
        ))
    }
    /// M5/Flutter: set a text field's content via the AT-native
    /// a11y editable-text action (the route Flutter semantics listens on).
    async fn a11y_set_text(&self, role: &str, name: &str, text: &str) -> Result<bool> {
        let _ = (role, name, text);
        Err(crate::error::GhostError::PageOp(
            "a11y_set_text not supported by this engine".into(),
        ))
    }
    /// M4 Proprioception: the honest body state — load state, a11y focus,
    /// native selection/caret, scrollers, viewport (frame-script read).
    async fn proprio_state(&self) -> Result<serde_json::Value> {
        Err(crate::error::GhostError::PageOp(
            "proprio_state not supported by this engine".into(),
        ))
    }
    /// M4 Proprioception: the cookie/session heartbeat — {kind, host,
    /// name, path, flags} for every cookie change since the page opened.
    /// Values are NEVER recorded. An auth cookie deletion = the earliest
    /// session-death signal.
    async fn read_cookie_events(&self, clear: bool) -> Result<Vec<serde_json::Value>> {
        let _ = clear;
        Err(crate::error::GhostError::PageOp(
            "read_cookie_events not supported by this engine".into(),
        ))
    }
    /// M3 Hearing: the accessibility EVENT STREAM — compacted
    /// {type, role, name, + detail} entries from the engine's own
    /// "accessible-event" topic since the last read. The incremental
    /// diff of the a11y tree; read instead of full snapshots.
    async fn read_acc_events(&self, clear: bool) -> Result<Vec<serde_json::Value>> {
        let _ = clear;
        Err(crate::error::GhostError::PageOp(
            "read_acc_events not supported by this engine".into(),
        ))
    }
    /// M3.5 Smell: the frame sampler — rAF cadence from the privileged
    /// frame script. Frame deltas ARE the jank signal (a busy main
    /// thread shows as a delta spike). reset=true restarts the window.
    async fn read_frame_stats(&self, reset: bool) -> Result<serde_json::Value> {
        let _ = reset;
        Err(crate::error::GhostError::PageOp(
            "read_frame_stats not supported by this engine".into(),
        ))
    }
    /// M3.5 Smell: timing sense — Navigation Timing (DNS/TLS/connect/
    /// TTFB/domInteractive/loadEvent) + paint entries for the current
    /// document.
    async fn read_timing_report(&self) -> Result<serde_json::Value> {
        Err(crate::error::GhostError::PageOp(
            "read_timing_report not supported by this engine".into(),
        ))
    }
    /// M3.5 Smell: visual stability — block until the refresh driver
    /// has delivered frames at normal cadence for quiet_ms consecutive.
    /// Returns (stable, elapsed_ms). The successor to arbitrary sleeps.
    async fn wait_visual_stable(&self, quiet_ms: u64, timeout_ms: u64) -> Result<bool> {
        let _ = (quiet_ms, timeout_ms);
        Err(crate::error::GhostError::PageOp(
            "wait_visual_stable not supported by this engine".into(),
        ))
    }
    /// M3 hearing: the WebSocket stream — socket lifecycle records
    /// {kind: socket, wsid, url, opened, closed, error} and frame records
    /// {kind: frame, wsid, direction, opcode, data, ts} since the page
    /// opened (or the last clear). Binary frames arrive base64-encoded.
    async fn read_ws_frames(&self, clear: bool) -> Result<Vec<serde_json::Value>> {
        let _ = clear;
        Err(crate::error::GhostError::PageOp(
            "read_ws_frames not supported by this engine".into(),
        ))
    }
    /// Network interception (the spike verified the Juggler route):
    /// enable/disable the interception on the page's session. While
    /// enabled, every request HOLDS and waits for resume/abort/fulfill —
    /// the agent decides per requestId (from the netcap entries).
    async fn network_set_interception(&self, enabled: bool) -> Result<()> {
        let _ = enabled;
        Err(crate::error::GhostError::PageOp(
            "network_set_interception not supported by this engine".into(),
        ))
    }
    /// Resume a held intercepted request — url/method/headers/postData
    /// optional overrides (the request MODIFIER).
    async fn network_resume(
        &self,
        request_id: &str,
        url: Option<&str>,
        method: Option<&str>,
        headers: Option<serde_json::Value>,
        post_data: Option<&str>,
    ) -> Result<()> {
        let _ = (request_id, url, method, headers, post_data);
        Err(crate::error::GhostError::PageOp(
            "network_resume not supported by this engine".into(),
        ))
    }
    /// Abort a held intercepted request (the blocker).
    async fn network_abort(&self, request_id: &str, error_code: &str) -> Result<()> {
        let _ = (request_id, error_code);
        Err(crate::error::GhostError::PageOp(
            "network_abort not supported by this engine".into(),
        ))
    }
    /// Fulfill a held intercepted request with a mocked response
    /// (the response FORGER — status/statusText/headers/body).
    async fn network_fulfill(
        &self,
        request_id: &str,
        status: u32,
        status_text: &str,
        headers: Option<serde_json::Value>,
        body: Option<&str>,
    ) -> Result<()> {
        let _ = (request_id, status, status_text, headers, body);
        Err(crate::error::GhostError::PageOp(
            "network_fulfill not supported by this engine".into(),
        ))
    }
    /// M4.5 Interoception: the engine process-group health — per-process
    /// CPU ticks + memory (Linux /proc; graceful degradation elsewhere).
    async fn session_vitals(&self) -> Result<serde_json::Value> {
        Err(crate::error::GhostError::PageOp(
            "session_vitals not supported by this engine".into(),
        ))
    }
    /// M5 the Critic: every element's honest layout rect (frame-script
    /// Xray walk — includes elements the a11y tree prunes).
    async fn collect_all_rects(&self) -> Result<Vec<UiRect>> {
        Err(crate::error::GhostError::PageOp(
            "collect_all_rects not supported by this engine".into(),
        ))
    }
    /// M3 Hearing: start the privileged mutation-whisper observer.
    async fn start_mutation_whispers(&self) -> Result<()> {
        Err(crate::error::GhostError::PageOp(
            "start_mutation_whispers not supported by this engine".into(),
        ))
    }
    /// M3 Hearing: drain the collected DOM-change whispers (clear=true
    /// resets the buffer).
    async fn read_mutation_whispers(&self, clear: bool) -> Result<Vec<serde_json::Value>> {
        let _ = clear;
        Err(crate::error::GhostError::PageOp(
            "read_mutation_whispers not supported by this engine".into(),
        ))
    }
    /// M2: scroll an accessible into view via the engine's native
    /// a11y scrollToPoint, returning the resulting layout bounds.
    async fn scroll_accessible_into_view(&self, role: &str, name: &str) -> Result<Option<Bounds>> {
        let _ = (role, name);
        Err(crate::error::GhostError::PageOp(
            "scroll_accessible_into_view not supported by this engine".into(),
        ))
    }
    /// M2: click at raw viewport coordinates with a human-like mouse
    /// path (bezier + tremor + landing). Coordinates come from trusted
    /// sources (native a11y bounds), NOT from JS-measured rects that
    /// pages can poison.
    async fn click_coords(&self, _x: f64, _y: f64) -> Result<()> {
        Err(crate::error::GhostError::PageOp(
            "click_coords not supported by this engine".into(),
        ))
    }
    /// M2.5: read RAW RGBA pixels of a content-viewport rectangle
    /// straight from the compositor — no page-realm canvas, no
    /// toDataURL, no PNG round-trip. Page scripts cannot hook, poison
    /// or observe this path (the hole every wrapper-based competitor
    /// has in their captcha pipelines).
    async fn capture_surface(
        &self,
        x: i64,
        y: i64,
        width: u32,
        height: u32,
        include_hidden: bool,
    ) -> Result<SurfacePixels> {
        let _ = (x, y, width, height, include_hidden);
        Err(crate::error::GhostError::PageOp(
            "capture_surface not supported by this engine".into(),
        ))
    }
    /// Act on an element by its ref from a11y_snapshot.
    async fn click_ref(&self, r: &str) -> Result<()> {
        let _ = r;
        Err(crate::error::GhostError::PageOp(
            "click_ref not supported by this engine".into(),
        ))
    }
    /// Move the mouse onto the element a ref points at, along a
    /// human-like path (hover). Engines without mouse pathing return
    /// an error naming the limitation.
    async fn mouse_move_to(&self, r: &str) -> Result<()> {
        let _ = r;
        Err(crate::error::GhostError::PageOp(
            "mouse_move_to not supported by this engine".into(),
        ))
    }
    /// Drag the element `from` (ref) onto the element `to` (ref), or by
    /// an offset when `to` is empty, with a human-like movement profile
    /// (bezier arc, ease-in-out velocity, jitter, overshoot+correction).
    async fn drag_ref(&self, from: &str, to: &str, dx: f64, dy: f64) -> Result<()> {
        let _ = (from, to, dx, dy);
        Err(crate::error::GhostError::PageOp(
            "drag_ref not supported by this engine".into(),
        ))
    }
    /// v0.6.3: Register a script that runs at DOCUMENT START on every
    /// navigation — before any page script. The deepest hook layer.
    async fn add_init_script(&self, source: &str) -> Result<()> {
        let _ = source;
        Err(crate::error::GhostError::PageOp(
            "add_init_script not supported by this engine".into(),
        ))
    }

    /// v0.7 DEBUG CORTEX: buffered console messages (log/warning/error)
    /// captured at the PROTOCOL level (Runtime.consoleAPICalled) — the
    /// page cannot hide or patch it. `clear` drains the buffer.
    async fn console_read(&self, clear: bool) -> Result<Vec<serde_json::Value>> {
        let _ = clear;
        Err(crate::error::GhostError::PageOp(
            "console capture not supported by this engine".into(),
        ))
    }

    /// v0.7 DEBUG CORTEX: buffered uncaught JS exceptions with stack
    /// traces (Runtime.exceptionThrown). `clear` drains the buffer.
    async fn errors_read(&self, clear: bool) -> Result<Vec<serde_json::Value>> {
        let _ = clear;
        Err(crate::error::GhostError::PageOp(
            "error capture not supported by this engine".into(),
        ))
    }

    /// v0.7 DEBUG CORTEX: structured network entries
    /// [{requestId, url, method, status}] — request metadata + response
    /// status, captured passively below the page. `clear` drains.
    async fn net_read(&self, clear: bool) -> Result<Vec<serde_json::Value>> {
        let _ = clear;
        Err(crate::error::GhostError::PageOp(
            "net capture not supported by this engine".into(),
        ))
    }

    /// v0.6.3: PROTOCOL-LEVEL network capture — start collecting every
    /// HTTP response for this page, BELOW the page (invisible to page JS).
    async fn net_capture_start(&self) -> Result<()> {
        Err(crate::error::GhostError::PageOp(
            "net_capture not supported by this engine".into(),
        ))
    }
    /// List captured responses (url, requestId) since capture start.
    async fn net_capture_list(&self) -> Result<Vec<(String, String)>> {
        Err(crate::error::GhostError::PageOp(
            "net_capture not supported by this engine".into(),
        ))
    }
    /// Fetch a captured response body BY PROTOCOL (Network.getResponseBody).
    async fn net_get_body(&self, request_id: &str) -> Result<String> {
        let _ = request_id;
        Err(crate::error::GhostError::PageOp(
            "net_capture not supported by this engine".into(),
        ))
    }

    /// v0.6.2 SUPERMAN GLASSES: render the element a ref points at
    /// (canvas / img / background-image) as a compact luminance grid
    /// the agent READS as digits — a text-model-friendly way to see
    /// shapes without a vision model.
    /// M2.5: the honest layout rect of a walk-ref, resolved in the
    /// privileged frame-script compartment (pages cannot hook
    /// getBoundingClientRect there).
    async fn get_ref_rect(&self, r: &str) -> Result<Option<Bounds>> {
        let _ = r;
        Err(crate::error::GhostError::PageOp(
            "get_ref_rect not supported by this engine".into(),
        ))
    }
    async fn pixels_ref(&self, r: &str, gw: u32, gh: u32) -> Result<String> {
        let _ = (r, gw, gh);
        Err(crate::error::GhostError::PageOp(
            "pixels_ref not supported by this engine".into(),
        ))
    }
    /// v0.6.3: HIGH-PASS VISION — local-contrast grid of an element's
    /// image. |gray - blur| makes blended content visible (captcha
    /// characters on photos, watermarks). The technique that solved the
    /// captcha family we believed needed a vision model.
    async fn contrast_ref(&self, r: &str, gw: u32, gh: u32, radius: u32) -> Result<String> {
        let _ = (r, gw, gh, radius);
        Err(crate::error::GhostError::PageOp(
            "contrast_ref not supported by this engine".into(),
        ))
    }
    /// v0.6.3: REAL template matching — multi-scale NCC of a needle
    /// (element image, optional sub-rect crop) against a haystack image,
    /// computed in-page at full grayscale resolution. Returns top match
    /// positions (needle centers) in haystack pixels.
    #[allow(clippy::too_many_arguments)]
    async fn match_image_ref(
        &self,
        needle_ref: &str,
        nx: i64,
        ny: i64,
        nw: i64,
        nh: i64,
        hay_ref: &str,
        hx: i64,
        hy: i64,
        hw: i64,
        hh: i64,
    ) -> Result<String> {
        let _ = (needle_ref, nx, ny, nw, nh, hay_ref, hx, hy, hw, hh);
        Err(crate::error::GhostError::PageOp(
            "match_image_ref not supported by this engine".into(),
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
    /// v0.6.2: All browser targets (tabs AND site-opened popups) with
    /// URLs where known. Engines without popup vision return their
    /// known page ids with unknown URLs.
    async fn list_targets(&self) -> Result<Vec<(String, Option<String>)>> {
        Ok(self
            .pages()
            .await?
            .into_iter()
            .map(|id| (id, None))
            .collect())
    }
    /// v0.6.2: Attach to an existing target (a popup the SITE opened —
    /// OAuth windows, payment flows). Returns a live page handle.
    async fn attach_target(&self, target_id: &str) -> Result<Arc<dyn PageHandle>> {
        let _ = target_id;
        Err(crate::error::GhostError::PageOp(
            "attach_target not supported by this engine".into(),
        ))
    }
}
