//! MCP server implementation: tools mapped 1:1 onto core operations.
//!
//! Every tool is narrow and typed — no god-tools, no "smart" behavior an
//! attacker could steer through page content.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use rmcp::handler::server::tool::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use ghostfox_core::engine::EngineKind;
use ghostfox_core::session::Session;

#[derive(Debug, Deserialize, JsonSchema)]
struct SessionCreateParams {
    /// Restrict identity platform: "windows" | "macos" | "linux" | "android" (optional).
    #[serde(default)]
    platform: Option<String>,
    /// Reuse a persistent profile directory (optional).
    #[serde(default)]
    profile_dir: Option<String>,
    /// Proxy URL, e.g. socks5://user:pass@host:port (optional).
    #[serde(default)]
    proxy: Option<String>,
    /// Run with a visible browser window instead of headless (optional) —
    /// for humans who want to watch the agent work.
    #[serde(default)]
    headful: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SessionEvidenceParams {
    session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RefParams {
    session_id: String,
    page_id: String,
    /// Element ref from page_a11y (e.g. "e12").
    r#ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DragParams {
    session_id: String,
    page_id: String,
    /// Ref of the element to grab (from page_a11y).
    from_ref: String,
    /// Ref of the drop target. Omit to drag by offset instead (sliders).
    to_ref: Option<String>,
    /// Horizontal pixels to drag when to_ref is omitted (positive = right).
    offset_x: Option<f64>,
    /// Vertical pixels to drag when to_ref is omitted (positive = down).
    offset_y: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct A11ySetTextParams {
    session_id: String,
    page_id: String,
    /// Native a11y role of the field (e.g. "entry", "textbox").
    role: String,
    /// Native a11y name of the field (e.g. "Nomor Anggota").
    name: String,
    /// The text to set.
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WaitStableParams {
    session_id: String,
    page_id: String,
    /// Cap on waiting in ms (default 5000, min 500, max 20000).
    max_ms: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MutationsParams {
    session_id: String,
    page_id: String,
    /// Reset the whisper buffer after reading.
    clear: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PixelsParams {
    session_id: String,
    page_id: String,
    /// Ref of the element to render (canvas / img / background-image).
    r#ref: Option<String>,
    /// M2.5 semantic mode: capture the element matching role+name via
    /// the native a11y bounds (trusted rect).
    role: Option<String>,
    name: Option<String>,
    /// M2.5 region mode: raw rect in content-viewport CSS pixels.
    x: Option<i64>,
    y: Option<i64>,
    width: Option<u32>,
    height: Option<u32>,
    /// M2.9 Layer X-Ray: also paint visibility:hidden content.
    include_hidden: Option<bool>,
    /// Grid width in cells (default 32).
    grid_w: Option<u32>,
    /// Grid height in cells (default 21).
    grid_h: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ContrastParams {
    session_id: String,
    page_id: String,
    /// Ref of the element to analyze (canvas / img / background-image).
    r#ref: String,
    /// Grid width in cells (default 100).
    grid_w: Option<u32>,
    /// Grid height in cells (default 44).
    grid_h: Option<u32>,
    /// Gaussian blur radius in px (default 4).
    blur_radius: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct InitScriptParams {
    session_id: String,
    page_id: String,
    /// JavaScript source to run at document start on every navigation.
    source: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CaptchaVisionParams {
    session_id: String,
    page_id: String,
    /// Element ref from page_a11y (the captcha img/canvas). Omit for the
    /// whole viewport.
    r#ref: Option<String>,
    /// Optional override: the exact question to ask the model. Defaults
    /// to the captcha-text prompt.
    instruction: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CookieEventsParams {
    session_id: String,
    page_id: String,
    clear: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NetParams {
    session_id: String,
    page_id: String,
    /// Optional URL substring filter (e.g. "geetest").
    filter: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NetBodyParams {
    session_id: String,
    page_id: String,
    /// requestId from page_network_read.
    request_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MatchImageParams {
    session_id: String,
    page_id: String,
    /// Ref of the NEEDLE element (canvas / img / background-image).
    needle_ref: String,
    /// Optional crop of the needle image (x, y, w, h in needle-image px).
    /// Omit to use the whole image. Use this to match a sub-region
    /// (e.g. an instruction glyph band inside the same image).
    needle_rect: Option<Vec<i64>>,
    /// Ref of the HAYSTACK element to search in.
    hay_ref: String,
    /// Optional search-region crop of the haystack (x, y, w, h in
    /// haystack-image px). Omit for the full image. USE THIS to
    /// exclude the needle's own area (self-match guard).
    hay_rect: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct OcrParams {
    session_id: String,
    page_id: String,
    /// Ref of the element to OCR. Omit for the whole viewport.
    r#ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct VisionParams {
    session_id: String,
    page_id: String,
    /// Ref of the element to analyze. Omit for the whole viewport.
    r#ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GeetestClickParams {
    session_id: String,
    page_id: String,
    /// Ref of the .geetest_item_wrap element (carries the challenge background image).
    r#ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GeetestSlideParams {
    session_id: String,
    page_id: String,
    /// Optional ref of the slider handle (.geetest_slider_button). If omitted
    /// the tool locates it by selector and registers its own ref.
    slider_ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CaptchaOcrParams {
    session_id: String,
    page_id: String,
    /// Ref of the captcha <img> element. Omit for the whole viewport.
    r#ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HcaptchaParams {
    session_id: String,
    page_id: String,
    /// Optional ref of the hCaptcha anchor iframe / checkbox. If omitted
    /// the tool auto-locates the anchor iframe by src pattern.
    checkbox_ref: Option<String>,
    /// Cloudflare API key for the host vision model (defaults to CLOUDFLARE_API_KEY env).
    cf_api_key: Option<String>,
    /// Cloudflare account id (defaults to CLOUDFLARE_ACCOUNT_ID env).
    cf_account_id: Option<String>,
    /// Max challenge rounds (hCaptcha chains 2-4). Default 4.
    max_rounds: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ConsoleParams {
    session_id: String,
    page_id: String,
    /// Drain the buffer after reading (default: keep entries).
    clear: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ErrorsParams {
    session_id: String,
    page_id: String,
    /// Drain the buffer after reading (default: keep entries).
    clear: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RotateParams {
    session_id: String,
    page_id: String,
    /// Ref of the rotate-right button (15deg per click in the standard demos).
    rot_right_ref: String,
    /// Ref of the rotate-left button.
    rot_left_ref: String,
    /// Ref of the check/verify button.
    check_ref: String,
    /// Ref of the reset button.
    reset_ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PagesParams {
    session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageCommentParams {
    session_id: String,
    page_id: String,
    /// The comment text to submit.
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SessionMeParams {
    session_id: String,
    #[serde(default)]
    page_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ConfirmActionParams {
    session_id: String,
    page_id: String,
    /// What the agent is about to do (for the evidence log).
    action: String,
    /// The ref of the element that will be clicked/activated.
    r#ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DismissModalParams {
    session_id: String,
    page_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadRefParams {
    session_id: String,
    page_id: String,
    /// Element ref from page_a11y (e.g. "e12").
    r#ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WaitForParams {
    session_id: String,
    page_id: String,
    /// CSS selector to wait for.
    selector: String,
    /// Timeout in milliseconds (default 10000).
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct UploadParams {
    session_id: String,
    page_id: String,
    /// CSS selector for the file input (input[type=file]).
    selector: String,
    /// Absolute path to the file to upload.
    file_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TypeRefParams {
    session_id: String,
    page_id: String,
    /// Element ref from page_a11y (e.g. "e12").
    r#ref: String,
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageEvalParams {
    session_id: String,
    page_id: String,
    /// JavaScript expression; the JSON-ified result is returned.
    expression: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageScreenshotParams {
    session_id: String,
    page_id: String,
    /// Capture the whole scrollable document instead of the viewport (optional).
    #[serde(default)]
    full_page: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CaptchaSolveParams {
    session_id: String,
    /// Turnstile/hcaptcha-style: the site's sitekey.
    #[serde(default)]
    sitekey: Option<String>,
    /// Turnstile/hcaptcha-style: the page URL the challenge lives on.
    #[serde(default)]
    pageurl: Option<String>,
    /// Image captcha: the challenge image as base64 PNG.
    #[serde(default)]
    image_base64: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageOpenParams {
    session_id: String,
    url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageRefParams {
    session_id: String,
    page_id: String,
}

/// Filter for one extraction field. All specified conditions are ANDed.
/// Matching is case-insensitive; `name` matches by substring.
#[derive(Debug, Deserialize, JsonSchema)]
struct ExtractSpec {
    /// Exact ARIA-ish role: "textbox", "button", "link", "heading", ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    /// Substring match against the accessible name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// Substring match against the element's live value (inputs/editors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// Substring match against the HTML tag name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    /// true (default): return the first match in DOM order. false: all matches.
    #[serde(default = "default_true")]
    first: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageClickNativeParams {
    session_id: String,
    page_id: String,
    /// Accessible role: "link", "button", "textbox", ...
    role: String,
    /// Substring match against the accessible name (case-insensitive).
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageA11yParams {
    session_id: String,
    page_id: String,
    /// Source: false (default) = the JS DOM walk (actionable refs via
    /// click_ref/type_ref). true = the engine's NATIVE accessibility tree
    /// (trusted — page scripts cannot tamper; shadow DOM, iframes and
    /// ARIA semantics handled by Gecko itself; richer states). Native refs
    /// are observation handles: identify elements / diff / anchor recipes —
    /// act via role+name anchors (page_extract) or the walk source.
    #[serde(default)]
    native: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RecipeRecordParams {
    session_id: String,
    /// Recipe name (a-zA-Z0-9-_). Overwrites an existing recipe with the same name on save.
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RecipeSaveParams {
    session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RecipeReplayParams {
    session_id: String,
    page_id: String,
    name: String,
    /// strict (default): stop with a rich error when an anchor fails to
    /// resolve (page changed). false: skip unresolvable steps and report them.
    #[serde(default = "default_true")]
    strict: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageExtractParams {
    session_id: String,
    page_id: String,
    /// Map of output key -> filter spec, e.g.
    /// {"search": {"role":"textbox","name":"Search"}, "signin": {"role":"button","name":"Sign in"}}
    fields: std::collections::BTreeMap<String, ExtractSpec>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageClickParams {
    session_id: String,
    page_id: String,
    /// CSS selector.
    selector: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageTypeParams {
    session_id: String,
    page_id: String,
    /// CSS selector.
    selector: String,
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageFillParams {
    session_id: String,
    page_id: String,
    /// CSS selector.
    selector: String,
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PagePressParams {
    session_id: String,
    page_id: String,
    /// Key name: Enter, Tab, Escape, Backspace, Delete, ArrowUp/Down/Left/Right, Home, End, PageUp, PageDown.
    key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdentityMorphParams {
    session_id: String,
    /// Optional target platform: windows / macos / linux / android.
    /// Omit for a fully random persona.
    platform: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdentityAuditParams {
    identity_toml: String,
}

#[derive(Clone, Default)]
pub struct GhostfoxServer {
    state: Arc<tokio::sync::RwLock<ServerState>>,
    recorder: crate::recording::Recorder,
}

#[derive(Default)]
pub(crate) struct ServerState {
    pub(crate) sessions: HashMap<String, Arc<Session>>,
    /// M5 taste: the launch recipe per session — the morph relaunches
    /// the engine with the same profile (cookies survive) but a fresh
    /// identity (fingerprint changes coherently).
    pub(crate) launches: HashMap<String, ghostfox_core::engine::LaunchOptions>,
    /// Recipe drafts in progress: session_id -> (name, steps).
    pub(crate) recipe_drafts: HashMap<String, (String, Vec<crate::recipes::RecipeStep>)>,
    /// Last a11y snapshot per page — the anchor source at record time.
    pub(crate) a11y_cache: HashMap<String, ghostfox_core::engine::A11ySnapshot>,
}

impl GhostfoxServer {
    /// Record a step into the session's recipe draft (if one is active).
    /// `full_params` are the original tool args; for `*_ref` tools pass
    /// Some(ref) so the anchor (role+name) is captured from the cached
    /// a11y snapshot.
    async fn note_recipe(
        &self,
        session_id: &str,
        tool: &str,
        full_params: serde_json::Value,
        used_ref: Option<&str>,
    ) {
        let (active, anchor) = {
            let state = self.state.read().await;
            if !state.recipe_drafts.contains_key(session_id) {
                return;
            }
            let anchor = used_ref.and_then(|r| {
                state
                    .a11y_cache
                    .values()
                    .find_map(|snap| crate::recipes::anchor_from_ref(snap, r))
            });
            (true, anchor)
        };
        if !active {
            return;
        }
        let step = crate::recipes::RecipeStep {
            tool: tool.to_string(),
            anchor,
            params: full_params,
        };
        let mut state = self.state.write().await;
        if let Some((_, steps)) = state.recipe_drafts.get_mut(session_id) {
            steps.push(step);
        }
    }

    /// Compose an action receipt: cached-before snapshot (if any) diffed
    /// against a fresh after-snapshot. Degrades gracefully when no cached
    /// before exists (the receipt then reports state, not diff).
    async fn action_receipt(
        &self,
        page: &Arc<dyn ghostfox_core::engine::PageHandle>,
        page_id: &str,
        action: &str,
        target_ref: Option<&str>,
    ) -> serde_json::Value {
        let before = {
            let state = self.state.read().await;
            state.a11y_cache.get(page_id).cloned()
        };
        // Settle: a click on a link/form submit starts a navigation that the
        // immediate walk would miss. Half a second covers the common cases
        // without turning every action into a slow round trip — agents
        // re-snapshot after actions anyway; this returns evidence instead.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let after = page.a11y_snapshot().await.ok();
        let Some(after) = after else {
            return serde_json::json!({
                "action": action,
                "receipt": "degraded",
                "reason": "post-action snapshot unavailable (page navigating?)",
            });
        };
        self.cache_a11y(page_id, after.clone()).await;
        let target = target_ref
            .and_then(|r| after.elements.iter().find(|e| e.r#ref == r))
            .map(|e| serde_json::json!({ "ref": e.r#ref, "role": e.role, "name": e.name }));
        let (changes, url_changed) = match &before {
            Some(b) => (
                crate::receipts::diff(b, &after, 40),
                after.page_url != b.page_url,
            ),
            None => (
                serde_json::json!({ "changed": null, "note": "no cached before-snapshot — run page_a11y before acting to enable diffs" }),
                false,
            ),
        };
        serde_json::json!({
            "action": action,
            "target": target,
            "url": after.page_url,
            "url_changed": url_changed,
            "login_state": after.login_state,
            "changes": changes,
        })
    }

    async fn cache_a11y(&self, page_id: &str, snap: ghostfox_core::engine::A11ySnapshot) {
        let mut state = self.state.write().await;
        state.a11y_cache.insert(page_id.to_string(), snap);
        // Bound the cache: keep the 8 most recent pages.
        if state.a11y_cache.len() > 8 {
            let victim = state.a11y_cache.keys().next().cloned();
            if let Some(v) = victim {
                state.a11y_cache.remove(&v);
            }
        }
    }

    pub fn new() -> Self {
        Self::default()
    }

    async fn session(&self, id: &str) -> Result<Arc<Session>, String> {
        self.state
            .read()
            .await
            .sessions
            .get(id)
            .cloned()
            .ok_or_else(|| format!("session `{id}` not found"))
    }
}

fn text_result(s: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![rmcp::model::Content::text(s.into())])
}

#[tool_router]
impl GhostfoxServer {
    #[tool(
        description = "Create a new browsing session: launches the engine with a fresh coherent identity. Returns session_id."
    )]
    async fn session_create(
        &self,
        Parameters(SessionCreateParams {
            platform,
            profile_dir,
            proxy,
            headful,
        }): Parameters<SessionCreateParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let gen_opts = ghostfox_fingerprint::GenerateOptions {
            platform: match platform.as_deref() {
                Some("windows") => Some(ghostfox_fingerprint::Platform::Windows),
                Some("macos") => Some(ghostfox_fingerprint::Platform::MacOS),
                Some("linux") => Some(ghostfox_fingerprint::Platform::Linux),
                Some("android") => Some(ghostfox_fingerprint::Platform::Android),
                _ => None,
            },
            webrtc: None,
        };
        let identity = ghostfox_fingerprint::generate(&gen_opts);

        let launch = ghostfox_core::engine::LaunchOptions {
            profile_dir,
            proxy,
            headless: !headful.unwrap_or(false),
            ..Default::default()
        };

        // Camoufox is the primary engine: patched-Firefox spoofing at the
        // C++ level. Identity is injected via CAMOU_CONFIG env at launch,
        // so generate-then-launch isn't a race — launch reads the identity
        // it finds in the (optional) profile dir or generates its own.
        //
        // Write identity.toml only when it doesn't exist yet: overwriting
        // it on every session_create would re-randomize the fingerprint of
        // a persistent profile, leaving cookies from device A behind a
        // fingerprint from device B — an incoherent identity.
        if let Some(dir) = &launch.profile_dir {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir)
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            let identity_file = dir.join("identity.toml");
            if !identity_file.exists() {
                let _ = identity
                    .to_toml()
                    .map(|t| std::fs::write(&identity_file, t));
            }
        }
        let engine = ghostfox_camoufox::launch(&launch)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let session = Session::from_engine(
            ghostfox_core::util::short_id(),
            identity.label.clone(),
            EngineKind::Firefox,
            engine,
        );
        let id = session.id.clone();
        // Evidence: record the persona this run uses, plus the launch facts.
        {
            let ev = serde_json::json!({
                "label": identity.label,
                "platform": format!("{:?}", identity.platform),
                "identity_hash": identity.fingerprint_hash(),
                "profile_dir": launch.profile_dir,
                "proxy": launch.proxy.is_some(),
                "engine": "ghostfox",
            });
            let _ = self
                .recorder
                .record_identity(&id, &identity.to_toml().unwrap_or_default());
            let _ = self.recorder.record(&id, "session_create", None, ev);
        }
        self.state.write().await.launches.insert(id.clone(), launch);
        self.state
            .write()
            .await
            .sessions
            .insert(id.clone(), Arc::new(session));
        // Live view (opt-in via GHOSTFOX_LIVE_VIEW_PORT): starts once, on the
        // first session.
        crate::liveview::start_if_configured(self.state.clone());
        Ok(text_result(id))
    }

    #[tool(
        description = "Navigate to a URL in an existing session. Returns a page_id (string) that must be passed to all subsequent page tools. Waits for the page to load. If the page has iframes or shadow DOM, use page_a11y instead of guessing CSS selectors. Example: page_open(session_id, 'https://example.com') returns a page_id like 'abc123'."
    )]
    async fn page_open(
        &self,
        Parameters(PageOpenParams { session_id, url }): Parameters<PageOpenParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        // The session's page registry is the id authority; new_page returns
        // the handle and registers it — we surface its id via the registry.
        let page_ids_before = session.page_ids().await;
        let _page = session
            .new_page(Some(&url))
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let page_ids_after = session.page_ids().await;
        let new_id = page_ids_after
            .into_iter()
            .find(|id| !page_ids_before.contains(id))
            .unwrap_or_default();
        let _ = self.recorder.record(
            &session_id,
            "page_open",
            Some(&new_id),
            serde_json::json!({ "url": url }),
        );
        self.note_recipe(
            &session_id,
            "page_open",
            serde_json::json!({ "url": url }),
            None,
        )
        .await;
        Ok(text_result(new_id))
    }

    #[tool(
        description = "Extract the visible text content of a page as plain text (token-friendly). Returns: url, title, and content (all visible text, no HTML). For semantic element data with refs and values, use page_a11y instead — it gives you interactive elements with roles and names. Use this when you just need to READ page content without needing to interact with elements."
    )]
    async fn page_snapshot(
        &self,
        Parameters(PageRefParams {
            session_id,
            page_id,
        }): Parameters<PageRefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let snap = page
            .snapshot()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        // Evidence: persist the full snapshot for replay/audit.
        let _ = self.recorder.record_snapshot(&session_id, &page_id, &snap);
        Ok(text_result(
            serde_json::to_string_pretty(&snap).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "READ-ONLY: Retrieve the complete audit trail for a session. No side effects — does not modify the session or browser. Returns JSON with: session_id, dir (evidence directory path), identity_toml (path to the identity file used), event_count (total tool calls recorded), events (array of all events with timestamps), and snapshots (list of page snapshot file paths). The session_id must come from a previous session_create call. For a nonexistent session, returns an error. Evidence persists after the session ends and includes: append-only events.jsonl, page snapshots (markdown), screenshots (PNG), and identity.toml. Use session_create first if you need a session."
    )]
    async fn session_evidence(
        &self,
        Parameters(SessionEvidenceParams { session_id }): Parameters<SessionEvidenceParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        match self.recorder.evidence(&session_id) {
            Ok(ev) => Ok(text_result(
                serde_json::to_string_pretty(&ev).unwrap_or_default(),
            )),
            Err(e) => Err(rmcp::model::ErrorData::internal_error(
                format!("no evidence for session `{session_id}`: {e}"),
                None,
            )),
        }
    }

    #[tool(
        description = "Semantic snapshot of the page: every visible interactive element with a stable ref, role (button/link/textbox/...), accessible name and CURRENT value — pierces shadow DOM, so web-component UIs (Reddit, modern frameworks) are fully visible. Use this instead of guessing CSS selectors."
    )]
    async fn page_a11y(
        &self,
        Parameters(PageA11yParams {
            session_id,
            page_id,
            native,
        }): Parameters<PageA11yParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let snap = if native {
            let tree = page
                .a11y_tree_native()
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            native_a11y_flatten(tree)
        } else {
            page.a11y_snapshot()
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?
        };
        self.cache_a11y(&page_id, snap.clone()).await;
        let _ = self.recorder.record(
            &session_id,
            "page_a11y",
            Some(&page_id),
            serde_json::json!({
                "elements": snap.elements.len(),
                "login_state": snap.login_state,
            }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&snap).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Extract specific elements into a compact typed JSON — the token-efficient alternative to dumping the full page_a11y. Pass a fields map: {output_key: {role, name, text, tag, first}}. Each filter ANDs its conditions (case-insensitive substring). Runs the same semantic walk as page_a11y (shadow DOM piercing, live values) but returns ONLY what you asked for: {output_key: {ref, role, name, value} | [matches...] | null} plus a tiny meta block (login_state, page_url). Example: page_extract(fields={'search': {'role':'textbox','name':'Search'}, 'post': {'role':'button','name':'Post'}}) → act on the returned refs with page_click_ref/page_type_ref."
    )]
    async fn page_extract(
        &self,
        Parameters(PageExtractParams {
            session_id,
            page_id,
            fields,
        }): Parameters<PageExtractParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let snap = page
            .a11y_snapshot()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        self.cache_a11y(&page_id, snap.clone()).await;

        let eq_ci = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let contains_ci =
            |haystack: &str, needle: &str| haystack.to_lowercase().contains(&needle.to_lowercase());

        let mut out = serde_json::Map::new();
        for (key, spec) in &fields {
            let matches: Vec<&ghostfox_core::engine::A11yElement> = snap
                .elements
                .iter()
                .filter(|el| {
                    spec.role
                        .as_deref()
                        .map(|r| eq_ci(&el.role, r))
                        .unwrap_or(true)
                        && spec
                            .name
                            .as_deref()
                            .map(|n| contains_ci(&el.name, n))
                            .unwrap_or(true)
                        && spec
                            .text
                            .as_deref()
                            .map(|t| contains_ci(el.value.as_deref().unwrap_or(""), t))
                            .unwrap_or(true)
                        && spec
                            .tag
                            .as_deref()
                            .map(|t| contains_ci(el.tag.as_deref().unwrap_or(""), t))
                            .unwrap_or(true)
                })
                .collect();
            let hits: Vec<_> = if spec.first {
                matches.into_iter().take(1).collect()
            } else {
                matches
            };
            let render = |el: &ghostfox_core::engine::A11yElement| {
                serde_json::json!({
                    "ref": el.r#ref,
                    "role": el.role,
                    "name": el.name,
                    "value": el.value,
                    "disabled": el.disabled,
                })
            };
            let val = if spec.first {
                hits.first()
                    .map(|el| render(el))
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Array(hits.iter().map(|el| render(el)).collect())
            };
            out.insert(key.clone(), val);
        }
        let result = serde_json::json!({
            "meta": {
                "login_state": snap.login_state,
                "page_url": snap.page_url,
            },
            "fields": serde_json::Value::Object(out),
        });
        let _ = self.recorder.record(
            &session_id,
            "page_extract",
            Some(&page_id),
            serde_json::json!({ "fields": fields.len() }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Click an element by its ref from page_a11y. Scrolls it into view first. No selectors needed."
    )]
    async fn page_click_ref(
        &self,
        Parameters(RefParams {
            session_id,
            page_id,
            r#ref,
        }): Parameters<RefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        page.click_ref(&r#ref)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        self.note_recipe(
            &session_id,
            "page_click_ref",
            serde_json::json!({ "ref": r#ref }),
            Some(&r#ref),
        )
        .await;
        let _ = self.recorder.record(
            &session_id,
            "page_click_ref",
            Some(&page_id),
            serde_json::json!({ "ref": r#ref }),
        );
        // Act -> Observe -> Compare: diff the cached before-snapshot against
        // a fresh walk so the agent gets EVIDENCE, not "clicked".
        let receipt = self
            .action_receipt(&page, &page_id, "click", Some(&r#ref))
            .await;
        Ok(text_result(
            serde_json::to_string_pretty(&receipt).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Move the mouse onto an element (hover) along a HUMAN-LIKE path: bezier arc, ease-in-out velocity, sub-pixel tremor, overshoot+correction. Triggers hover menus and tooltips. Pass the ref from page_a11y."
    )]
    async fn page_move_to(
        &self,
        Parameters(RefParams {
            session_id,
            page_id,
            r#ref,
        }): Parameters<RefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        page.mouse_move_to(&r#ref)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_move_to",
            Some(&page_id),
            serde_json::json!({ "ref": r#ref }),
        );
        Ok(text_result("moved"))
    }

    #[tool(
        description = "Drag with a HUMAN-LIKE movement profile: approaches the source, presses, drags along a bezier arc with ease-in-out velocity and micro-pauses, settles, releases. Pass from_ref + to_ref to drag element onto element, or from_ref + offset_x/offset_y to drag by pixels (slider captchas, resize handles). All from page_a11y refs."
    )]
    async fn page_drag(
        &self,
        Parameters(DragParams {
            session_id,
            page_id,
            from_ref,
            to_ref,
            offset_x,
            offset_y,
        }): Parameters<DragParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let to = to_ref.unwrap_or_default();
        page.drag_ref(
            &from_ref,
            &to,
            offset_x.unwrap_or(0.0),
            offset_y.unwrap_or(0.0),
        )
        .await
        .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_drag",
            Some(&page_id),
            serde_json::json!({
                "from": from_ref,
                "to": to,
                "offset_x": offset_x.unwrap_or(0.0),
                "offset_y": offset_y.unwrap_or(0.0),
            }),
        );
        Ok(text_result("dragged"))
    }

    #[tool(
        description = "SUPERMAN GLASSES v2 (M2.5): render a screen region / semantic element / walk-ref into a compact luminance grid the agent READS as numbers. Pixels come STRAIGHT from the compositor (Page.captureSurface) — no page-realm canvas, no toDataURL, no getImageData: pages cannot hook, poison or even observe the read. Modes (pick one): {x,y,width,height} raw viewport rect · {role,name} semantic element (native a11y bounds, trusted) · {ref} walk-ref from page_a11y (privileged rect). Returns {w, h, grid} — 0-9 luminance rows, and {width,height} in CSS px."
    )]
    async fn page_pixels(
        &self,
        Parameters(PixelsParams {
            session_id,
            page_id,
            r#ref,
            role,
            name,
            x,
            y,
            width,
            height,
            grid_w,
            grid_h,
            include_hidden,
        }): Parameters<PixelsParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let gw = grid_w.unwrap_or(32).clamp(4, 128);
        let gh = grid_h.unwrap_or(21).clamp(4, 128);

        // Resolve the capture rect by mode.
        let rect: Option<ghostfox_core::engine::Bounds> =
            if let (Some(x), Some(y), Some(w), Some(h)) = (x, y, width, height) {
                Some(ghostfox_core::engine::Bounds {
                    x,
                    y,
                    width: w as i64,
                    height: h as i64,
                })
            } else if let (Some(role), Some(name)) = (role, name) {
                // Scroll-first (M2): below-fold targets must be visible or
                // the compositor snapshot shows the wrong content.
                let _ = page.scroll_accessible_into_view(&role, &name).await;
                let raw = page
                    .a11y_tree_native()
                    .await
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                let snap = native_a11y_flatten(raw);
                let anchor = crate::recipes::Anchor { role, name };
                crate::recipes::resolve_anchor(&snap, &anchor).and_then(|el| el.bounds.clone())
            } else if let Some(r) = r#ref.as_deref() {
                page.get_ref_rect(r)
                    .await
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?
            } else {
                None
            };

        // Canvas refs read the DRAWING BUFFER (M2.9): the buffer is the
        // source of truth — hidden canvases, WebGL and worker-transferred
        // OffscreenCanvas all render nothing to the compositor.
        let ref_is_canvas = if let Some(r) = r#ref.as_ref() {
            let js = format!(
                r#"JSON.stringify({{ tag: ((window.__gfxRefs || new Map()).get('{r}') || {{}}).tagName || '' }})"#,
                r = r
            );
            page.evaluate(&js)
                .await
                .ok()
                .and_then(|v| v.as_str().map(|s| s.contains("CANVAS")))
                .unwrap_or(false)
        } else {
            false
        };
        let (grid, width, height, source) = if ref_is_canvas {
            let r = r#ref.as_deref().unwrap_or_default();
            // Page-realm expandos are invisible to the frame script (Xray
            // AND waiveXrays) — mark the element with an attribute and
            // resolve it as a SELECTOR on the privileged side instead.
            let mark = format!("data-gfx-px-{r}");
            let mark_js = format!(
                r#"(function() {{ var el = (window.__gfxRefs || new Map()).get('{r}'); if (!el) return 'STALE-REF'; el.setAttribute('{mark}', '1'); return 'ok'; }})()"#
            );
            match page.evaluate(&mark_js).await {
                Ok(v) if v.as_str() == Some("STALE-REF") => {
                    return Err(rmcp::model::ErrorData::internal_error(
                        format!("ref {r} is stale — rerun page_a11y"),
                        None,
                    ))
                }
                Ok(_) => {}
                Err(e) => return Err(rmcp::model::ErrorData::internal_error(e.to_string(), None)),
            }
            let cb = page
                .capture_canvas_buffer(&format!("canvas[{mark}]"))
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?
                .ok_or_else(|| {
                    rmcp::model::ErrorData::internal_error("canvas buffer read failed", None)
                })?;
            let rgba = if cb.raw {
                cb.bytes
            } else {
                image::load_from_memory(&cb.bytes)
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?
                    .to_rgba8()
                    .into_raw()
            };
            let grid = crate::vision::luminance_grid(&rgba, cb.width, cb.height, gw, gh);
            (grid, cb.width, cb.height, "canvas-buffer")
        } else {
            let Some(b) = rect else {
                return Err(rmcp::model::ErrorData::invalid_params(
                    "no capture target: pass {x,y,width,height}, {role,name} or {ref}",
                    None,
                ));
            };
            let (bw, bh) = (
                b.width.clamp(1, 2048) as u32,
                b.height.clamp(1, 2048) as u32,
            );
            if bw * bh > 4_194_304 {
                return Err(rmcp::model::ErrorData::invalid_params(
                    format!("capture region {}x{} exceeds 4M pixels — crop it", bw, bh),
                    None,
                ));
            }
            let surface = page
                .capture_surface(b.x, b.y, bw, bh, include_hidden.unwrap_or(false))
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            (
                crate::vision::luminance_grid(&surface.rgba, surface.width, surface.height, gw, gh),
                surface.width,
                surface.height,
                "compositor",
            )
        };
        let _ = self.recorder.record(
            &session_id,
            "page_pixels",
            Some(&page_id),
            serde_json::json!({ "mode": "page_pixels", "source": source, "grid_w": gw, "grid_h": gh }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "w": gw,
                "h": gh,
                "width": width,
                "height": height,
                "source": source,
                "grid": grid,
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "M5/FLUTTER: set a text field's content through the ACCESSIBILITY protocol (nsIAccessibleEditableText.setTextContents) — the AT-native route. Flutter web in semantics mode edits via the a11y layer, not DOM input events, so this is the route that syncs the Dart controllers. Pass the role+name as seen in page_a11y(native=true). Returns ok/error."
    )]
    async fn page_a11y_set_text(
        &self,
        Parameters(A11ySetTextParams {
            session_id,
            page_id,
            role,
            name,
            text,
        }): Parameters<A11ySetTextParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let ok = page
            .a11y_set_text(&role, &name, &text)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({ "ok": ok })).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "M4 proprioception: the browser's honest body state — load state, focused element (a11y tree), native selection/caret (the typing receipt: where the caret actually landed), scrollers, viewport. Read from the privileged frame script, pages cannot fake it."
    )]
    async fn page_proprio(
        &self,
        Parameters(PageRefParams {
            session_id,
            page_id,
            ..
        }): Parameters<PageRefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let page = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let state = page
            .proprio_state()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(
            serde_json::to_string_pretty(&state).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "M4 proprioception: the cookie/session heartbeat — every cookie change (added/changed/deleted/cleared) since the page opened, as {kind, host, name, path, flags}. Values are NEVER recorded. An auth cookie being deleted = the earliest session-death signal (silent revocations can't hide). clear=true resets the buffer."
    )]
    async fn page_cookie_events(
        &self,
        Parameters(CookieEventsParams {
            session_id,
            page_id,
            clear,
        }): Parameters<CookieEventsParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let page = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let events = page
            .read_cookie_events(clear.unwrap_or(false))
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(
            serde_json::to_string_pretty(&events).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "M4.5 interoception: the engine process-group health — per-process CPU ticks + memory (RSS/VmSize). A leaking tab shows as a growing content process; diff successive reads to feel the leak. Linux-only for now."
    )]
    async fn session_vitals(
        &self,
        Parameters(PageRefParams {
            session_id,
            page_id,
            ..
        }): Parameters<PageRefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let page = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let v = page
            .session_vitals()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(
            serde_json::to_string_pretty(&v).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "M3.5 SMELL: wait until the page is VISUALLY ready — the render settled (no DOM mutations for two consecutive polls AND the layout rects are unchanged). The true successor to arbitrary sleep(): instead of guessing '3 seconds', wait for the signal that the page actually stopped moving. Returns {stable, waited_ms, reason}. Polls up to max_ms (default 5000)."
    )]
    async fn page_wait_stable(
        &self,
        Parameters(WaitStableParams {
            session_id,
            page_id,
            max_ms,
        }): Parameters<WaitStableParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let max_ms = max_ms.unwrap_or(5000).clamp(500, 20000);
        let started = std::time::Instant::now();
        let mut reason = "timeout";
        let mut stable = false;
        let mut prev_rects: Vec<ghostfox_core::engine::UiRect> = Vec::new();
        let mut quiet_polls = 0u32;
        while started.elapsed().as_millis() < max_ms as u128 {
            let whispers = page
                .read_mutation_whispers(true)
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            let rects = page
                .collect_all_rects()
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            let rects_same = rects.len() == prev_rects.len()
                && rects
                    .iter()
                    .zip(prev_rects.iter())
                    .all(|(a, b)| a.x == b.x && a.y == b.y && a.w == b.w && a.h == b.h);
            if whispers.is_empty() && rects_same {
                quiet_polls += 1;
                if quiet_polls >= 2 {
                    stable = true;
                    reason = "render-settled";
                    break;
                }
            } else {
                quiet_polls = 0;
            }
            prev_rects = rects;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "stable": stable,
                "waited_ms": started.elapsed().as_millis() as u64,
                "reason": reason,
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "M5 THE CRITIC: audit the CURRENT page's visual quality the way a human eye judges a design — run this after any UI change (vibe-coded layouts love clipped captions, missing padding, overlapping elements). Computed from the engine's native a11y bounds (trusted geometry) plus the viewport. Returns {issues: [{type, severity, text/a/b, bounds, detail}], counts: {errors, warns}}. Types: text-clipped (teks keluar container), no-padding (teks nempel tepi), viewport-overflow, overlap (elemen tabrakan), crowded (elemen interaktif kelewat dekat)."
    )]
    async fn page_ui_audit(
        &self,
        Parameters(PageRefParams {
            session_id,
            page_id,
        }): Parameters<PageRefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let raw = page
            .a11y_tree_native()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let viewport = page
            .evaluate("JSON.stringify([window.innerWidth, window.innerHeight])")
            .await
            .ok()
            .and_then(|v| v.as_str().map(|s| s.trim_matches('"').to_string()))
            .and_then(|s| {
                let parts: Vec<i64> = s
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .filter_map(|p| p.trim().parse().ok())
                    .collect();
                if parts.len() == 2 {
                    Some((parts[0], parts[1]))
                } else {
                    None
                }
            })
            .unwrap_or((1280, 800));
        let full_rects = page.collect_all_rects().await.unwrap_or_default();
        let issues = ui_audit_from_raw(&raw, viewport, &full_rects);
        let (mut errors, mut warns) = (0usize, 0usize);
        for i in &issues {
            match i.get("severity").and_then(|v| v.as_str()) {
                Some("error") => errors += 1,
                _ => warns += 1,
            }
        }
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "issues": issues,
                "counts": { "errors": errors, "warns": warns },
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "M3 HEARING: drain the engine's privileged DOM-mutation whispers — every childList/attributes/characterData change the page made since the observer started, observed from the UNHOOKABLE frame-script realm (the page cannot poison or hide its own mutations from this observer). Returns {whispers: [{type, tag, attr, text, added, removed}]} capped at 300. Use after any action to see WHAT the page changed (feedback toasts, injected forms, anti-bot DOM churn) without paying for a full snapshot. clear=true resets the buffer; the observer auto-starts on first call."
    )]
    async fn page_mutations(
        &self,
        Parameters(MutationsParams {
            session_id,
            page_id,
            clear,
        }): Parameters<MutationsParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        page.start_mutation_whispers()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let whispers = page
            .read_mutation_whispers(clear.unwrap_or(false))
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "whispers": whispers,
                "count": whispers.len(),
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "INIT SCRIPTS: run your JS at DOCUMENT START on every navigation — BEFORE any page script loads. The deepest interception layer in Ghostfox: hooks installed here are what page libraries (gt.js, analytics, frameworks) capture, so their stored references are YOURS. Use for: API response interception (JSONP callback swaps), state capture, anti-detection instrumentation, environment patches. Applies to subsequent navigations."
    )]
    async fn page_init_script(
        &self,
        Parameters(InitScriptParams {
            session_id,
            page_id,
            source,
        }): Parameters<InitScriptParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        page.add_init_script(&source)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_init_script",
            Some(&page_id),
            serde_json::json!({ "chars": source.chars().count() }),
        );
        Ok(text_result("init script registered"))
    }

    #[tool(
        description = "PROTOCOL-LEVEL NETWORK CAPTURE: start recording every HTTP response for this page, BELOW the page (invisible to page JS, unpatchable). The answer data of any captcha/API travels here. Start before triggering the flow you want to see."
    )]
    async fn page_network_start(
        &self,
        Parameters(NetParams {
            session_id,
            page_id,
            ..
        }): Parameters<NetParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        page.net_capture_start()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result("capture started"))
    }

    #[tool(
        description = "List captured HTTP responses [{url, requestId}] since capture start. Optionally filter by URL substring (e.g. 'geetest' or 'api.'). Pair with page_network_body to read the response content BY PROTOCOL."
    )]
    async fn page_network_read(
        &self,
        Parameters(NetParams {
            session_id,
            page_id,
            filter,
        }): Parameters<NetParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let entries = page
            .net_read(false)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let filtered: Vec<serde_json::Value> = entries
            .into_iter()
            .filter(|e| {
                filter.as_deref().is_none_or(|f| {
                    e.get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .contains(f)
                })
            })
            .collect();
        Ok(text_result(
            serde_json::to_string_pretty(&filtered).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Fetch the BODY of a captured response by requestId — Network.getResponseBody BY PROTOCOL. The JSONP/API answer data that page JS can't see, served from the engine. THIS is the 0s and 1s layer."
    )]
    async fn page_network_body(
        &self,
        Parameters(NetBodyParams {
            session_id,
            page_id,
            request_id,
        }): Parameters<NetBodyParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let body = page
            .net_get_body(&request_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(body))
    }

    #[tool(
        description = "REAL TEMPLATE MATCHING: multi-scale normalized cross-correlation of a needle against a haystack, computed IN-PAGE at full grayscale resolution (no grid loss). Returns top matches [{x, y, score, scale}] — needle-center positions in haystack-image pixels. needle_rect optionally crops the needle (e.g. an instruction glyph band from the same image — GeeTest icon-click pattern). Images are cached per URL: single-use challenge URLs fetch exactly once. THE tool for: captcha piece->gap, glyph->character, logo->page."
    )]
    async fn page_match_image(
        &self,
        Parameters(MatchImageParams {
            session_id,
            page_id,
            needle_ref,
            needle_rect,
            hay_ref,
            hay_rect,
        }): Parameters<MatchImageParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let rect = needle_rect.unwrap_or_default();
        let (nx, ny, nw, nh) = match rect.as_slice() {
            [x, y, w, h] => (*x, *y, *w, *h),
            _ => (0, 0, 0, 0),
        };
        let hrect = hay_rect.unwrap_or_default();
        let (hx, hy, hw, hh) = match hrect.as_slice() {
            [x, y, w, h] => (*x, *y, *w, *h),
            _ => (0, 0, 0, 0),
        };
        let result = page
            .match_image_ref(&needle_ref, nx, ny, nw, nh, &hay_ref, hx, hy, hw, hh)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_match_image",
            Some(&page_id),
            serde_json::json!({ "needle": needle_ref, "hay": hay_ref }),
        );
        Ok(text_result(result))
    }

    #[tool(
        description = "HIGH-PASS VISION: local-contrast grid of an element's image (|gray - gaussian_blur|). Makes ANYTHING blended into a background VISIBLE — captcha characters on photos, watermarks, hidden strokes. 0 = flat area, 9 = strong edge. This is the native tool born from solving GeeTest icon-click with pure math. Works on canvas / img / background-image (one fetch per challenge)."
    )]
    async fn page_contrast(
        &self,
        Parameters(ContrastParams {
            session_id,
            page_id,
            r#ref,
            grid_w,
            grid_h,
            blur_radius,
        }): Parameters<ContrastParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let gw = grid_w.unwrap_or(100).clamp(4, 256);
        let gh = grid_h.unwrap_or(44).clamp(4, 256);
        let radius = blur_radius.unwrap_or(4).clamp(1, 16);
        let grid = page
            .contrast_ref(&r#ref, gw, gh, radius)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_contrast",
            Some(&page_id),
            serde_json::json!({ "ref": r#ref, "grid_w": gw, "grid_h": gh }),
        );
        Ok(text_result(grid))
    }

    /// Shared helper: produce PNG bytes of the whole viewport or the
    /// element a ref points at (screenshot + crop).
    async fn element_png(
        &self,
        session_id: &str,
        page_id: &str,
        r#ref: Option<String>,
    ) -> Result<Vec<u8>, rmcp::model::ErrorData> {
        let session = self
            .session(session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let png = page
            .screenshot(false)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        match r#ref {
            None => Ok(png),
            Some(r) => {
                // Rect of the element (CSS px) + viewport width + DPR.
                let js = format!(
                    r#"(function() {{
  var el = (window.__gfxRefs || new Map()).get({r});
  if (!el || !el.isConnected) return 'STALE-REF';
  var r = el.getBoundingClientRect();
  return JSON.stringify({{x: r.x, y: r.y, w: r.width, h: r.height, vw: window.innerWidth}});
}})()"#,
                    r = serde_json::to_string(&r).unwrap_or_default()
                );
                let out = page
                    .evaluate(&js)
                    .await
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                let s = out.as_str().unwrap_or("STALE-REF");
                if s == "STALE-REF" {
                    return Err(rmcp::model::ErrorData::internal_error(
                        format!("ref {r} is stale — rerun page_a11y"),
                        None,
                    ));
                }
                let rect: serde_json::Value = serde_json::from_str(s)
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                let img = image::load_from_memory(&png)
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                let vw = rect.get("vw").and_then(|v| v.as_f64()).unwrap_or(1.0);
                let scale = img.width() as f64 / vw.max(1.0);
                let x = (rect.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) * scale) as i64;
                let y = (rect.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) * scale) as i64;
                let w = (rect.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0) * scale) as i64;
                let h = (rect.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0) * scale) as i64;
                let (x, y) = (x.max(0) as u32, y.max(0) as u32);
                let (w, h) = (
                    (w.max(1) as u32).min(img.width().saturating_sub(x)),
                    (h.max(1) as u32).min(img.height().saturating_sub(y)),
                );
                let cropped = image::imageops::crop_imm(&img, x, y, w, h).to_image();
                let mut buf = std::io::Cursor::new(Vec::new());
                cropped
                    .write_to(&mut buf, image::ImageFormat::Png)
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                Ok(buf.into_inner())
            }
        }
    }

    #[tool(
        description = "TIER 2 VISION — LOCAL OCR: extract text from an element (or the whole viewport if no ref). 100% local (pure-Rust ML models auto-download once to GHOSTFOX_HOME/models). Answers 'what text is written there' — for image captchas, canvas text, scanned UI. Models download on first call (~12MB, once)."
    )]
    async fn page_ocr(
        &self,
        Parameters(OcrParams {
            session_id,
            page_id,
            r#ref,
        }): Parameters<OcrParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let png = self.element_png(&session_id, &page_id, r#ref).await?;
        let text = crate::ocr::ocr_png(&png)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_ocr",
            Some(&page_id),
            serde_json::json!({ "chars": text.chars().count() }),
        );
        Ok(text_result(text))
    }

    #[tool(
        description = "TIER 4 VISION — VISUAL CORTEX: detect word bounding boxes in an element (or viewport) via the built-in text-detection ML model. Returns JSON [{x, y, w, h}, ...] in image pixels. The first shipped visual-cortex model — see where text lives in ANY image (click-order captchas, canvas text, scanned layout). 100% local."
    )]
    async fn page_vision(
        &self,
        Parameters(VisionParams {
            session_id,
            page_id,
            r#ref,
        }): Parameters<VisionParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let png = self.element_png(&session_id, &page_id, r#ref).await?;
        let boxes = crate::ocr::detect_text_boxes(&png)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_vision",
            Some(&page_id),
            serde_json::json!({ "words_found": boxes.len() }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&boxes).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "TIER 5 VISION — HOST MODEL CAPTCHA READER: send the captcha element (or the whole viewport when ref is omitted) to the host vision model (Cloudflare Workers AI GLM-5.3-flash) and return what it reads — the OCR tier the local CRNN cannot reach (heavily warped text captchas like Google's). Credentials: CLOUDFLARE_API_KEY + CLOUDFLARE_ACCOUNT_ID env (same as page_hcaptcha). instruction overrides the default text-reading prompt."
    )]
    async fn page_captcha_vision(
        &self,
        Parameters(CaptchaVisionParams {
            session_id,
            page_id,
            r#ref,
            instruction,
        }): Parameters<CaptchaVisionParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let png = self.element_png(&session_id, &page_id, r#ref).await?;
        let account = std::env::var("CLOUDFLARE_ACCOUNT_ID").ok().or_else(|| {
            tracing::warn!(target: "ghostfox::mcp", "no CLOUDFLARE_ACCOUNT_ID");
            None
        });
        let key = std::env::var("CLOUDFLARE_API_KEY").ok();
        let (Some(account), Some(key)) = (account, key) else {
            return Err(rmcp::model::ErrorData::invalid_params(
                "no CLOUDFLARE_ACCOUNT_ID / CLOUDFLARE_API_KEY (env) — the host vision model needs them",
                None,
            ));
        };
        let glm = crate::hcaptcha::Glm::new(account, key);
        let prompt = instruction.unwrap_or_else(|| {
            // Neutral OCR framing on purpose: explicitly asking to "solve a
            // CAPTCHA" trips the model's refusal policy (E2E-observed).
            "Read any text or characters visible in this image. Reply with ONLY the characters, nothing else. If you cannot read it confidently, reply with just the word: UNCLEAR".to_string()
        });
        let text = glm
            .read_text(&png, &prompt)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_captcha_vision",
            Some(&page_id),
            serde_json::json!({ "chars": text.chars().count() }),
        );
        Ok(text_result(text))
    }

    #[tool(
        description = "GEETEST ICON-CLICK SOLVER: solve the GeeTest v3 word-click captcha (characters on a photo, click in the strip's order) from the live page. Fetches the challenge image off the element's background, runs the trained ONNX pair (YOLOv8s char detection + siamese order matching, 100% local CPU, ~0.5s), and returns click targets in page coordinates plus the raw boxes. Call AFTER the challenge popup is open. Then click each 'click' point via page_drag and press the geetest confirm button. Returns JSON {clicks: [{x, y}, ...] in page px, boxes_raw: [[x, y], ...] in image px, rect: {...}, img: {w, h}}."
    )]
    async fn page_geetest_click(
        &self,
        Parameters(GeetestClickParams {
            session_id,
            page_id,
            r#ref,
        }): Parameters<GeetestClickParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        // 1. Background image URL + element rect (CSS px).
        let js = format!(
            r#"(function() {{
  var el = (window.__gfxRefs || new Map()).get({r});
  if (!el || !el.isConnected) return 'STALE-REF';
  var bg = getComputedStyle(el).backgroundImage;
  var m = bg.match(/url\(["']?([^"')]+)["']?\)/);
  var r = el.getBoundingClientRect();
  return JSON.stringify({{url: m ? m[1] : '', x: r.x, y: r.y, w: r.width, h: r.height}});
}})()"#,
            r = serde_json::to_string(&r#ref).unwrap_or_default()
        );
        let out = page
            .evaluate(&js)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let s = out.as_str().unwrap_or_default();
        if s == "STALE-REF" {
            return Err(rmcp::model::ErrorData::invalid_params(
                format!("ref {} is stale — rerun page_a11y", r#ref),
                None,
            ));
        }
        let info: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let url = info.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if url.is_empty() {
            return Err(rmcp::model::ErrorData::internal_error(
                "element has no background image — is the challenge open?",
                None,
            ));
        }
        // 2. Fetch the challenge image.
        let bytes = reqwest::get(url)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?
            .bytes()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?
            .to_vec();
        let img = image::load_from_memory(&bytes)
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let (iw, ih) = (img.width(), img.height());
        // 3. Trained eyes: YOLO detection + siamese order.
        let boxes = crate::geetest::solve_image(&bytes)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        // 4. Map box centers to page coords. The raw image = field (top ih-40 px)
        //    + instruction strip (bottom 40 px); the element shows the field region.
        let rx = info.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let ry = info.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let rw = info.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let rh = info.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let sx = if iw > 0 { rw / iw as f64 } else { 1.0 };
        let sy = if ih > 40 { rh / (ih - 40) as f64 } else { 1.0 };
        let clicks: Vec<serde_json::Value> = boxes
            .iter()
            .map(|b| {
                serde_json::json!({
                    "x": (rx + (b[0] as f64 + 31.0) * sx).round(),
                    "y": (ry + (b[1] as f64 + 31.0) * sy).round(),
                })
            })
            .collect();
        let _ = self.recorder.record(
            &session_id,
            "page_geetest_click",
            Some(&page_id),
            serde_json::json!({ "clicks": boxes.len(), "img": [iw, ih] }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "clicks": clicks,
                "boxes_raw": boxes,
                "rect": { "x": rx, "y": ry, "w": rw, "h": rh },
                "img": { "w": iw, "h": ih },
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "GEETEST SLIDE SOLVER: solve the GeeTest v3-style slide puzzle from the live page, then perform the human drag itself. Captures the three canvases (bg, puzzle slice, full reference bg) STRAIGHT from the compositor (M2.5 — no page-realm toDataURL, no CORS taint, unhookable), finds the hole with |bg-fullbg| diff + largest-blob + morphological closing (the JPEG-noise trap), measures the piece's solid-alpha left edge, and drags the slider by hole_x0 - piece_x0 with the engine's humanized mouse. Returns JSON {drag_x, hole: [x0, x1], piece_x0}. Call AFTER the challenge popup is open. No vision model, pure pixel math."
    )]
    async fn page_geetest_slide(
        &self,
        Parameters(GeetestSlideParams {
            session_id,
            page_id,
            slider_ref,
        }): Parameters<GeetestSlideParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        // 1. Locate the three canvases + register the slider ref, then
        // capture them STRAIGHT from the compositor (M2.5): no page-realm
        // toDataURL, no CORS taint — the pixel path is unhookable.
        let slider_js = match &slider_ref {
            Some(r) => serde_json::to_string(r).unwrap_or_default(),
            None => "null".into(),
        };
        let js = format!(
            r#"(function() {{
  window.__gfxRefs = window.__gfxRefs || new Map();
  var bg = document.querySelector('canvas.geetest_canvas_bg');
  var sl = document.querySelector('canvas.geetest_canvas_slice');
  var fb = document.querySelector('canvas.geetest_canvas_fullbg');
  if (!bg || !sl) return JSON.stringify({{err: 'canvases missing — is the slide challenge open?'}});
  var btn = {slider_js} !== null && (window.__gfxRefs.get({slider_js}) || null);
  if (!btn) {{
    btn = document.querySelector('.geetest_slider_button');
    if (btn) window.__gfxRefs.set('gs_btn', btn);
  }}
  if (!btn) return JSON.stringify({{err: 'slider handle not found'}});
  return JSON.stringify({{ok: true}});
}})()"#
        );
        let out = page
            .evaluate(&js)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap_or_default())
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        if let Some(err) = v.get("err").and_then(|e| e.as_str()).map(|e| e.to_string()) {
            return Err(rmcp::model::ErrorData::internal_error(err, None));
        }
        // M2.5: read the three DRAWING BUFFERS from the privileged frame
        // script (native toDataURL via Xray — page hooks cannot poison or
        // observe). The fullbg canvas is hidden (CSS 0x0), so the
        // compositor never renders it; the buffer is the honest source.
        let buf = async |sel: &str| -> Result<
            ghostfox_core::engine::CanvasBuffer,
            rmcp::model::ErrorData,
        > {
            let bytes = page
                .capture_canvas_buffer(sel)
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?
                .ok_or_else(|| {
                    rmcp::model::ErrorData::internal_error(
                        format!("{sel} buffer read failed"),
                        None,
                    )
                })?;
            Ok(bytes)
        };
        let decode = |cb: &ghostfox_core::engine::CanvasBuffer,
                      name: &str|
         -> Result<image::DynamicImage, rmcp::model::ErrorData> {
            if cb.raw {
                // WebGL readPixels output: plain RGBA, no header.
                let (w, h) = (cb.width, cb.height);
                if w == 0 || h == 0 {
                    return Err(rmcp::model::ErrorData::internal_error(
                        format!("{name} raw buffer missing dims"),
                        None,
                    ));
                }
                let img = image::RgbaImage::from_raw(w, h, cb.bytes.clone()).ok_or_else(|| {
                    rmcp::model::ErrorData::internal_error(
                        format!("{name} raw buffer size mismatch"),
                        None,
                    )
                })?;
                Ok(image::DynamicImage::ImageRgba8(img))
            } else {
                image::load_from_memory(&cb.bytes)
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))
            }
        };
        let bg = decode(&buf("canvas.geetest_canvas_bg").await?, "bg")?.to_luma8();
        let sl = decode(&buf("canvas.geetest_canvas_slice").await?, "sl")?.to_rgba8();
        let fb = decode(&buf("canvas.geetest_canvas_fullbg").await?, "fb")?.to_luma8();
        let drag_x = crate::geetest::slide_gap(&bg, &sl, &fb)
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        // 2. Human drag.
        page.drag_ref("gs_btn", "", drag_x as f64, 0.0)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_geetest_slide",
            Some(&page_id),
            serde_json::json!({ "drag_x": drag_x }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "drag_x": drag_x,
                "dragged": true,
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "CAPTCHA OCR: classify the text of a normal image captcha (the distorted-text family) with the ddddocr model (CRNN+LSTM trained specifically on captcha text, 8210-char charset). 100% local — runs on onnxruntime inside the runtime. Takes the ref of the captcha <img> element (or the whole viewport) and returns the recognized text. Feed it into the answer field and submit. Far stronger than generic OCR on captcha fonts."
    )]
    async fn page_captcha_ocr(
        &self,
        Parameters(CaptchaOcrParams {
            session_id,
            page_id,
            r#ref,
        }): Parameters<CaptchaOcrParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let png = self.element_png(&session_id, &page_id, r#ref).await?;
        let text = crate::ddddocr::classify_png(&png)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_captcha_ocr",
            Some(&page_id),
            serde_json::json!({ "chars": text.chars().count() }),
        );
        Ok(text_result(text))
    }

    #[tool(
        description = "HCAPTCHA SOLVER: solve hCaptcha image challenges from the live page with the host vision model (Cloudflare Workers AI GLM). Handles ALL challenge variants — tile grids, pattern-break icon fields, any image-pick — by asking the model for the exact pixel coordinates of every element to click, then clicking with the humanized mouse. Multi-round: loops until the page's h-captcha-response field receives a token (up to max_rounds, default 4). Credentials come from CLOUDFLARE_API_KEY / CLOUDFLARE_ACCOUNT_ID env or the params. Returns JSON {success, rounds, response_len}."
    )]
    async fn page_hcaptcha(
        &self,
        Parameters(HcaptchaParams {
            session_id,
            page_id,
            checkbox_ref,
            cf_api_key,
            cf_account_id,
            max_rounds,
        }): Parameters<HcaptchaParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let key = cf_api_key
            .or_else(|| std::env::var("CLOUDFLARE_API_KEY").ok())
            .ok_or_else(|| {
                rmcp::model::ErrorData::invalid_params("no CLOUDFLARE_API_KEY (param or env)", None)
            })?;
        let account = cf_account_id
            .or_else(|| std::env::var("CLOUDFLARE_ACCOUNT_ID").ok())
            .ok_or_else(|| {
                rmcp::model::ErrorData::invalid_params(
                    "no CLOUDFLARE_ACCOUNT_ID (param or env)",
                    None,
                )
            })?;
        let glm = crate::hcaptcha::Glm::new(account, key);
        let max_rounds = max_rounds.unwrap_or(6) as usize;

        // 1. Locate + click the anchor checkbox.
        let anchor_js = match &checkbox_ref {
            Some(r) => format!(
                r#"(function() {{
  var el = (window.__gfxRefs || new Map()).get({r});
  if (!el || !el.isConnected) return JSON.stringify({{err: 'stale ref'}});
  var b = el.getBoundingClientRect();
  return JSON.stringify({{x: b.x + b.width/2, y: b.y + b.height/2}});
}})()"#,
                r = serde_json::to_string(r).unwrap_or_default()
            ),
            None => r#"(function() {
  var f = null;
  document.querySelectorAll('iframe').forEach(function(i) {
    var src = (i.src || '');
    if ((src.indexOf('hcaptcha') >= 0 || src.indexOf('newassets') >= 0) && !f) {
      var b = i.getBoundingClientRect();
      if (b.width > 200 && b.width < 400 && b.y > -100) f = i;
    }
  });
  if (!f) return JSON.stringify({err: 'no hCaptcha anchor iframe found'});
  var b = f.getBoundingClientRect();
  return JSON.stringify({x: b.x + b.width/2, y: b.y + b.height/2});
})()"#
                .to_string(),
        };
        let out = page
            .evaluate(&anchor_js)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let av: serde_json::Value = serde_json::from_str(out.as_str().unwrap_or("{}"))
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        if let Some(err) = av
            .get("err")
            .and_then(|e| e.as_str())
            .map(|e| e.to_string())
        {
            return Err(rmcp::model::ErrorData::internal_error(err, None));
        }
        let (ax, ay) = (
            av.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
            av.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
        );
        let mk_js = format!(
            r#"(function() {{
  window.__gfxRefs = window.__gfxRefs || new Map();
  var d = document.createElement('div');
  d.style.cssText = 'position:fixed;left:{ax}px;top:{ay}px;width:2px;height:2px;z-index:99999;pointer-events:none;';
  document.body.appendChild(d);
  window.__gfxRefs.set('hc_anchor', d);
  return 'OK';
}})()"#
        );
        let _ = page.evaluate(&mk_js).await;
        page.drag_ref("hc_anchor", "", 0.0, 0.0)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;

        // 2. Wait for the challenge iframe (large, visible).
        let chall_js = r#"(function() {
  var f = null;
  document.querySelectorAll('iframe').forEach(function(i) {
    var b = i.getBoundingClientRect();
    if (b.width > 400 && b.y > -100 && !f) f = i;
  });
  if (!f) return 'NO';
  var b = f.getBoundingClientRect();
  return JSON.stringify({x: b.x, y: b.y, w: b.width, h: b.height, vw: window.innerWidth});
})()"#;
        let mut chall: Option<serde_json::Value> = None;
        for _ in 0..14 {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            let out = page
                .evaluate(chall_js)
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            if out.as_str() != Some("NO") {
                chall = serde_json::from_str(out.as_str().unwrap_or("")).ok();
                break;
            }
        }
        let chall = chall.ok_or_else(|| {
            rmcp::model::ErrorData::internal_error("hCaptcha challenge iframe never appeared", None)
        })?;
        let (mut cx, mut cy, mut cw, mut ch, vw) = (
            chall.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
            chall.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
            chall.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0),
            chall.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0),
            chall.get("vw").and_then(|v| v.as_f64()).unwrap_or(1.0),
        );

        let token_js = r#"(function() {
  var r = document.querySelector('[name*=h-captcha-response], textarea[name*=h-captcha-response]');
  return r ? (r.value || '').length : 0;
})()"#;

        // 3. Solve rounds.
        let mut rounds = 0usize;
        let mut response_len = 0i64;
        let mut debug_rounds: Vec<serde_json::Value> = Vec::new();
        for round in 0..max_rounds {
            rounds = round + 1;
            // Re-query the challenge iframe each round (it can move/resize).
            let out = page
                .evaluate(chall_js)
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            if out.as_str() == Some("NO") {
                break;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(out.as_str().unwrap_or("")) {
                cx = v.get("x").and_then(|x| x.as_f64()).unwrap_or(cx);
                cy = v.get("y").and_then(|x| x.as_f64()).unwrap_or(cy);
                cw = v.get("w").and_then(|x| x.as_f64()).unwrap_or(cw);
                ch = v.get("h").and_then(|x| x.as_f64()).unwrap_or(ch);
            }
            let png = page
                .screenshot(false)
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            let img = image::load_from_memory(&png)
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            let scale = img.width() as f64 / vw.max(1.0);
            let (bx, by, bw, bh) = (
                (cx * scale) as u32,
                (cy * scale) as u32,
                (cw * scale) as u32,
                (ch * scale) as u32,
            );
            let cropped = image::imageops::crop_imm(
                &img,
                bx,
                by,
                bw.min(img.width().saturating_sub(bx)),
                bh.min(img.height().saturating_sub(by)),
            )
            .to_image();
            let mut buf = std::io::Cursor::new(Vec::new());
            cropped
                .write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            // Render-wait: hCaptcha tiles lazy-load; solve only once content is visible.
            let mut ready_png = buf.into_inner();
            for _ in 0..6 {
                if crate::hcaptcha::is_rendered(&ready_png) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                let png2 = page
                    .screenshot(false)
                    .await
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                let img2 = image::load_from_memory(&png2)
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                let scale2 = img2.width() as f64 / vw.max(1.0);
                let (bx2, by2, bw2, bh2) = (
                    (cx * scale2) as u32,
                    (cy * scale2) as u32,
                    (cw * scale2) as u32,
                    (ch * scale2) as u32,
                );
                let c2 = image::imageops::crop_imm(
                    &img2,
                    bx2,
                    by2,
                    bw2.min(img2.width().saturating_sub(bx2)),
                    bh2.min(img2.height().saturating_sub(by2)),
                )
                .to_image();
                let mut b2 = std::io::Cursor::new(Vec::new());
                c2.write_to(&mut b2, image::ImageFormat::Png)
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                ready_png = b2.into_inner();
            }
            let (solved, layout, model_raw) = glm
                .solve_challenge_dbg(&ready_png, cropped.width(), cropped.height())
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            debug_rounds.push(serde_json::json!({
                "round": round + 1,
                "iframe": { "x": cx, "y": cy, "w": cw, "h": ch },
                "layout": layout,
                "model": model_raw,
                "clicks": solved.clicks,
                "drag": solved.drag,
                "verify": solved.verify,
            }));
            // DRAG challenge: press at source, human-drag to target.
            if let Some((from, to)) = solved.drag {
                let mk = format!(
                    r#"(function() {{
  window.__gfxRefs = window.__gfxRefs || new Map();
  var d = document.createElement('div');
  d.style.cssText = 'position:fixed;left:{fx}px;top:{fy}px;width:2px;height:2px;z-index:99999;pointer-events:none;';
  document.body.appendChild(d);
  window.__gfxRefs.set('hc_drag', d);
  return 'OK';
}})()"#,
                    fx = cx + from.0,
                    fy = cy + from.1
                );
                let _ = page.evaluate(&mk).await;
                page.drag_ref("hc_drag", "", to.0 - from.0, to.1 - from.1)
                    .await
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
                let out = page.evaluate(token_js).await.unwrap_or_default();
                response_len = out.as_i64().unwrap_or(0);
                if response_len > 0 {
                    break;
                }
                continue;
            }
            let clicks = solved.clicks;
            let verify = solved.verify;

            for (i, (px, py)) in clicks.iter().enumerate() {
                let mk = format!(
                    r#"(function() {{
  window.__gfxRefs = window.__gfxRefs || new Map();
  var d = document.createElement('div');
  d.style.cssText = 'position:fixed;left:{x}px;top:{y}px;width:2px;height:2px;z-index:99999;pointer-events:none;';
  document.body.appendChild(d);
  window.__gfxRefs.set('hc_t{i}', d);
  return 'OK';
}})()"#,
                    x = cx + px,
                    y = cy + py,
                    i = i
                );
                let _ = page.evaluate(&mk).await;
                page.drag_ref(&format!("hc_t{i}"), "", 0.0, 0.0)
                    .await
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            }
            if let Some((vx, vy)) = verify {
                let mk = format!(
                    r#"(function() {{
  window.__gfxRefs = window.__gfxRefs || new Map();
  var d = document.createElement('div');
  d.style.cssText = 'position:fixed;left:{x}px;top:{y}px;width:2px;height:2px;z-index:99999;pointer-events:none;';
  document.body.appendChild(d);
  window.__gfxRefs.set('hc_v', d);
  return 'OK';
}})()"#,
                    x = cx + vx,
                    y = cy + vy
                );
                let _ = page.evaluate(&mk).await;
                page.drag_ref("hc_v", "", 0.0, 0.0)
                    .await
                    .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            }
            tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
            let out = page.evaluate(token_js).await.unwrap_or_default();
            response_len = out.as_i64().unwrap_or(0);
            if response_len > 0 {
                break;
            }
        }
        let _ = self.recorder.record(
            &session_id,
            "page_hcaptcha",
            Some(&page_id),
            serde_json::json!({ "rounds": rounds, "response_len": response_len }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "success": response_len > 0,
                "rounds": rounds,
                "response_len": response_len,
                "debug": debug_rounds,
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "ROTATE CAPTCHA SOLVER: brute-force sweep + human replay. Rotate challenges ask to turn an image upright; standard demos rotate 15deg per button click and answer with a check button. The tool first sweeps every angle programmatically (instant JS clicks, ~30s), reads the visible feedback after each check (success keywords: pass/通过/正确/成功/verif/succeeded), then REPLAYS the winning rotation with real human mouse drags and the final check. Returns JSON {clicks, angle, status}. For fixed-image demos the sweep converges every time."
    )]
    async fn page_captcha_rotate(
        &self,
        Parameters(RotateParams {
            session_id,
            page_id,
            rot_right_ref,
            rot_left_ref,
            check_ref,
            reset_ref,
        }): Parameters<RotateParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let js = format!(
            r#"(async function() {{
  window.__gfxRefs = window.__gfxRefs || new Map();
  var rr = window.__gfxRefs.get({rr});
  var rl = window.__gfxRefs.get({rl});
  var ck = window.__gfxRefs.get({ck});
  var rs = window.__gfxRefs.get({rs});
  if (!rr || !rl || !ck || !rs) return JSON.stringify({{err: 'stale refs — rerun page_a11y and re-set refs'}});
  var read = function() {{
    var out = [];
    document.querySelectorAll('[class*=alert],[class*=status],[class*=result],[class*=success],[class*=error],[class*=message],[class*=tip],[role=alert],[role=status]').forEach(function(el){{
      var t = (el.innerText||'').trim();
      var st = getComputedStyle(el);
      if (t && t.length < 70 && st.display !== 'none' && el.getBoundingClientRect().width > 0) out.push(t.slice(0,55));
    }});
    return Array.from(new Set(out)).join(' | ');
  }};
  var ok = function(t) {{ return /pass|通过|正确|成功|success|verif|solved/i.test(t) && !/错误|wrong|incorrect/i.test(t); }};
  var history = [];
  var winner = -1;
  for (var k = 0; k < 24; k++) {{
    rs.click();
    await new Promise(function(r){{ setTimeout(r, 120); }});
    for (var i = 0; i < k; i++) {{ rr.click(); }}
    await new Promise(function(r){{ setTimeout(r, 120); }});
    ck.click();
    await new Promise(function(r){{ setTimeout(r, 500); }});
    var t = read();
    history.push({{k: k, text: t.slice(0,60)}});
    if (t && ok(t) && winner < 0) winner = k;
  }}
  return JSON.stringify({{winner: winner, history: history.slice(-24)}});
}})()"#,
            rr = serde_json::to_string(&rot_right_ref).unwrap_or_default(),
            rl = serde_json::to_string(&rot_left_ref).unwrap_or_default(),
            ck = serde_json::to_string(&check_ref).unwrap_or_default(),
            rs = serde_json::to_string(&reset_ref).unwrap_or_default(),
        );
        let out = page
            .evaluate(&js)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let s = out.as_str().unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        if let Some(err) = v.get("err").and_then(|e| e.as_str()).map(|e| e.to_string()) {
            return Err(rmcp::model::ErrorData::internal_error(err, None));
        }
        let winner = v.get("winner").and_then(|w| w.as_i64()).unwrap_or(-1);
        if winner < 0 {
            return Ok(text_result(
                serde_json::to_string_pretty(&serde_json::json!({
                    "solved": false,
                    "history": v.get("history"),
                }))
                .unwrap_or_default(),
            ));
        }
        // Human replay of the winning rotation. Re-resolve refs first:
        // the JS sweep mutates the page (React re-renders), stale DOM nodes
        // fail the engine's a11y ref validation.
        let rejs = format!(
            r#"(function() {{
  window.__gfxRefs = window.__gfxRefs || new Map();
  var btns = document.querySelectorAll('button');
  btns.forEach(function(b) {{
    var t = (b.innerText||'').trim();
    if (t.indexOf('向右') >= 0 || t.toLowerCase().indexOf('rotate right') >= 0) window.__gfxRefs.set({rr}, b);
    if (t.indexOf('向左') >= 0 || t.toLowerCase().indexOf('rotate left') >= 0) window.__gfxRefs.set({rl}, b);
    if (t.indexOf('检查') >= 0 || t.toLowerCase().indexOf('check') >= 0) window.__gfxRefs.set({ck}, b);
    if (t.indexOf('重置') >= 0 || t.toLowerCase().indexOf('reset') >= 0) window.__gfxRefs.set({rs}, b);
  }});
  return 'OK';
}})()"#,
            rr = serde_json::to_string(&rot_right_ref).unwrap_or_default(),
            rl = serde_json::to_string(&rot_left_ref).unwrap_or_default(),
            ck = serde_json::to_string(&check_ref).unwrap_or_default(),
            rs = serde_json::to_string(&reset_ref).unwrap_or_default(),
        );
        let _ = page.evaluate(&rejs).await;
        page.drag_ref(&reset_ref, "", 0.0, 0.0)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        for _ in 0..winner {
            page.drag_ref(&rot_right_ref, "", 0.0, 0.0)
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        }
        page.drag_ref(&check_ref, "", 0.0, 0.0)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_captcha_rotate",
            Some(&page_id),
            serde_json::json!({ "clicks": winner, "angle": winner * 15 }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "solved": true,
                "clicks": winner,
                "angle": winner * 15,
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "DEBUG CORTEX: read the page's console output (log/warning/error) captured at the PROTOCOL level — the page cannot hide or patch it. Invisible debugger for AI agents: reproduce the bug, read what the page logged. Optionally filter noise by clearing after read. Returns JSON [{kind, text, url, line, ts}]. Auto-starts capture on first call."
    )]
    async fn page_console(
        &self,
        Parameters(ConsoleParams {
            session_id,
            page_id,
            clear,
        }): Parameters<ConsoleParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let entries = page
            .console_read(clear.unwrap_or(false))
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_console",
            Some(&page_id),
            serde_json::json!({ "entries": entries.len() }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&entries).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "DEBUG CORTEX: read the page's UNCAUGHT JS EXCEPTIONS with stack traces, captured at the protocol level. The error console a developer opens in DevTools — as a tool. Returns JSON [{text, url, line, stack: [frames], ts}]. Auto-starts capture on first call."
    )]
    async fn page_errors(
        &self,
        Parameters(ErrorsParams {
            session_id,
            page_id,
            clear,
        }): Parameters<ErrorsParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let entries = page
            .errors_read(clear.unwrap_or(false))
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_errors",
            Some(&page_id),
            serde_json::json!({ "errors": entries.len() }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&entries).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "POPUP/TAB VISION: list EVERY browser target — pages you opened AND popups the site opened (OAuth windows, payment flows). Popups are auto-attached and given a page_id usable with every page_* tool immediately. Returns JSON [{page_id, target_id, url}]."
    )]
    async fn session_pages(
        &self,
        Parameters(PagesParams { session_id }): Parameters<PagesParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let overview = session.pages_overview().await;
        let _ = self.recorder.record(
            &session_id,
            "session_pages",
            None,
            serde_json::json!({ "targets": overview.len() }),
        );
        let items: Vec<serde_json::Value> = overview
            .into_iter()
            .map(|(page_id, target_id, url)| {
                serde_json::json!({ "page_id": page_id, "target_id": target_id, "url": url })
            })
            .collect();
        Ok(text_result(
            serde_json::to_string_pretty(&items).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Type text into the element a page_a11y ref points at (inputs and rich editors). Returns landed chars as a receipt."
    )]
    async fn page_type_ref(
        &self,
        Parameters(TypeRefParams {
            session_id,
            page_id,
            r#ref,
            text,
        }): Parameters<TypeRefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        page.type_ref(&r#ref, &text)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        self.note_recipe(
            &session_id,
            "page_type_ref",
            serde_json::json!({ "text": text }),
            Some(&r#ref),
        )
        .await;
        let _ = self.recorder.record(
            &session_id,
            "page_type_ref",
            Some(&page_id),
            serde_json::json!({ "ref": r#ref, "chars": text.chars().count() }),
        );
        let receipt = self
            .action_receipt(&page, &page_id, "type", Some(&r#ref))
            .await;
        Ok(text_result(
            serde_json::to_string_pretty(&receipt).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Read the FULL value of an element by its page_a11y ref — no truncation. Use when the snapshot's 200-char preview isn't enough (body text, long input fields)."
    )]
    async fn page_read_ref(
        &self,
        Parameters(ReadRefParams {
            session_id,
            page_id,
            r#ref,
        }): Parameters<ReadRefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let value = page
            .read_ref_full(&r#ref)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_read_ref",
            Some(&page_id),
            serde_json::json!({ "ref": r#ref, "chars": value.len() }),
        );
        Ok(text_result(value))
    }

    #[tool(
        description = "Wait until a CSS selector becomes visible on the page (replaces manual sleeps). Returns true if found, false on timeout."
    )]
    async fn page_wait_for(
        &self,
        Parameters(WaitForParams {
            session_id,
            page_id,
            selector,
            timeout_ms,
        }): Parameters<WaitForParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let timeout = timeout_ms.unwrap_or(10_000);
        let found = page
            .wait_for(&selector, timeout)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_wait_for",
            Some(&page_id),
            serde_json::json!({ "selector": selector, "timeout_ms": timeout, "found": found }),
        );
        Ok(text_result(if found { "true" } else { "false" }))
    }

    #[tool(
        description = "Upload a file to an input[type=file] by CSS selector. The file must exist on the machine running the engine."
    )]
    async fn page_upload_file(
        &self,
        Parameters(UploadParams {
            session_id,
            page_id,
            selector,
            file_path,
        }): Parameters<UploadParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        // Verify the file exists, then set it on the input via a DataTransfer.
        let content = std::fs::read(&file_path).map_err(|e| {
            rmcp::model::ErrorData::invalid_params(format!("cannot read {file_path}: {e}"), None)
        })?;
        let b64 = {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD.encode(&content)
        };
        let sel = serde_json::to_string(&selector).unwrap_or_default();
        let expr = format!(
            r#"(function() {{
  var el = document.querySelector({sel});
  if (!el) return 'NOT-FOUND';
  if (el.tagName !== 'INPUT' || el.type !== 'file') return 'NOT-FILE-INPUT';
  var b64 = {b64};
  var bin = atob(b64);
  var bytes = new Uint8Array(bin.length);
  for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  var name = {name}.split('/').pop() || 'upload';
  var mime = 'application/octet-stream';
  var file = new File([bytes], name, {{ type: mime }});
  var dt = new DataTransfer();
  dt.items.add(file);
  el.files = dt.files;
  el.dispatchEvent(new Event('input', {{ bubbles: true }}));
  el.dispatchEvent(new Event('change', {{ bubbles: true }}));
  return 'UPLOADED:' + el.files.length;
}})()"#,
            sel = sel,
            b64 = serde_json::to_string(&b64).unwrap_or_default(),
            name = serde_json::to_string(&file_path).unwrap_or_default(),
        );
        let result = page
            .evaluate(&expr)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_upload_file",
            Some(&page_id),
            serde_json::json!({ "selector": selector, "file": file_path }),
        );
        Ok(text_result(result.as_str().unwrap_or("unknown")))
    }

    #[tool(
        description = "Detect the currently logged-in username on the page (Reddit, X, GitHub, HN). Returns username or 'unknown'. Use before commenting to avoid duplicates."
    )]
    async fn session_me(
        &self,
        Parameters(SessionMeParams {
            session_id,
            page_id,
        }): Parameters<SessionMeParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page =
            match page_id {
                Some(pid) => session
                    .page(&pid)
                    .await
                    .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?,
                None => {
                    let ids = session.page_ids().await;
                    if let Some(first) = ids.first() {
                        session.page(first).await.map_err(|e| {
                            rmcp::model::ErrorData::invalid_params(e.to_string(), None)
                        })?
                    } else {
                        let pg = session.new_page(Some("about:blank")).await.map_err(|e| {
                            rmcp::model::ErrorData::internal_error(e.to_string(), None)
                        })?;
                        pg
                    }
                }
            };
        // Use a11y_snapshot (pierces shadow DOM) to find username
        let snap =
            page.a11y_snapshot()
                .await
                .unwrap_or_else(|_| ghostfox_core::engine::A11ySnapshot {
                    elements: vec![],
                    login_state: "unknown".into(),
                    page_url: String::new(),
                    page_title: String::new(),
                    danger_zone: None,
                    suspicious_elements: 0,
                    page_archived: None,
                    own_elements: 0,
                    username: None,
                    below_viewport: 0,
                    max_scroll_pages: 0,
                    notifications: vec![],
                    rate_limit_seconds: None,
                });
        // v0.5.3: page_a11y now detects the username from header profile
        // links (Reddit /user/X, X /@handle, HN logout link) — far more
        // reliable than scraping "Comment from X" which matches other users.
        let mut username = snap
            .username
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        if username == "unknown" {
            for e in &snap.elements {
                let name = &e.name;
                // Reddit: "Comment from [username]"
                if let Some(rest) = name.strip_prefix("Comment from ") {
                    let uname = rest.split_whitespace().next().unwrap_or("unknown");
                    if uname.len() >= 3 {
                        username = uname.to_string();
                        break;
                    }
                }
                // Reddit: "Expand user menu" → check for @username nearby
                if name.contains("user menu") || name.contains("User menu") {
                    if let Some(at) = name.find('@') {
                        let rest = &name[at + 1..];
                        let uname = rest.split_whitespace().next().unwrap_or("unknown");
                        if uname.len() >= 3 {
                            username = uname.to_string();
                            break;
                        }
                    }
                }
                // Reddit: profile link with /user/username
                if name.contains("/user/") || name.contains("u/") {
                    let parts: Vec<&str> = name.split('/').collect();
                    for (i, part) in parts.iter().enumerate() {
                        if (*part == "user" || *part == "u") && i + 1 < parts.len() {
                            let uname = parts[i + 1];
                            if uname.len() >= 3
                                && uname
                                    .chars()
                                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                            {
                                username = uname.to_string();
                                break;
                            }
                        }
                    }
                    if username != "unknown" {
                        break;
                    }
                }
            }
        }
        let username = username;
        Ok(text_result(username))
    }

    #[tool(
        description = "Submit a comment on the current page. Automatically: 1) detects your username via session_me, 2) checks if you already commented (skips if duplicate), 3) finds the comment editor (Reply or Join conversation), 4) opens it, 5) types your text, 6) clicks submit, 7) verifies. Returns result JSON."
    )]
    async fn page_comment(
        &self,
        Parameters(PageCommentParams {
            session_id,
            page_id,
            text,
        }): Parameters<PageCommentParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;

        // STEP 1: Get own username via a11y (fast, uses shadow DOM piercing)
        let snap = page
            .a11y_snapshot()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;

        let mut own_username = String::new();
        for e in &snap.elements {
            if let Some(rest) = e.name.strip_prefix("Comment from ") {
                let uname = rest.split_whitespace().next().unwrap_or("");
                if uname.len() >= 3 {
                    own_username = uname.to_string();
                    break;
                }
            }
        }

        // STEP 2: Check for duplicate comments
        if !own_username.is_empty() {
            let has_own = snap.elements.iter().any(|e| {
                e.name.contains(&format!("Comment from {}", own_username))
                    || (e.name.contains(&own_username) && e.name.contains("ago"))
            });
            if has_own {
                let _ = self.recorder.record(
                    &session_id,
                    "page_comment",
                    Some(&page_id),
                    serde_json::json!({
                        "action": "skipped", "reason": "duplicate", "username": own_username
                    }),
                );
                return Ok(text_result(
                    serde_json::to_string_pretty(&serde_json::json!({
                        "action": "skipped",
                        "reason": "already_commented",
                        "username": own_username,
                    }))
                    .unwrap_or_default(),
                ));
            }
        }

        // STEP 3: Find comment trigger (Reply preferred, Join conversation fallback)
        let trigger = page
            .evaluate(
                r#"(() => {
                var btns = document.querySelectorAll('button');
                for (var i=0;i<btns.length;i++) {
                    if (btns[i].offsetParent && btns[i].textContent.trim() === 'Reply') {
                        return 'reply';
                    }
                }
                var tbs = document.querySelectorAll('[role=textbox]');
                for (var j=0;j<tbs.length;j++) {
                    var label = tbs[j].getAttribute('aria-label')||tbs[j].textContent||'';
                    if (label.includes('Join the conversation')) return 'join';
                }
                return 'none';
            })()"#,
            )
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;

        let trigger = trigger.as_str().unwrap_or("none");
        if trigger == "none" {
            return Err(rmcp::model::ErrorData::internal_error(
                "No comment box or Reply button found on this page".to_string(),
                None,
            ));
        }

        // STEP 4: Click the trigger
        let click_js = if trigger == "reply" {
            r#"(() => {
                var btns = document.querySelectorAll('button');
                for (var i=0;i<btns.length;i++) {
                    if (btns[i].offsetParent && btns[i].textContent.trim() === 'Reply') {
                        btns[i].click(); return 'clicked-reply';
                    }
                }
                return 'not-found';
            })()"#
        } else {
            r#"(() => {
                var els = document.querySelectorAll('[role=textbox]');
                for (var i=0;i<els.length;i++) {
                    if ((els[i].getAttribute('aria-label')||'').includes('Join the conversation')) {
                        els[i].focus(); els[i].click(); return 'clicked-join';
                    }
                }
                return 'not-found';
            })()"#
        };

        page.evaluate(click_js)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;

        // Wait for editor to render
        tokio::time::sleep(std::time::Duration::from_millis(3000)).await;

        // STEP 5: Find editor and type
        let typed = page
            .evaluate(&format!(
                r#"(() => {{
                    var els = document.querySelectorAll('[contenteditable=true][role=textbox]');
                    for (var i=0;i<els.length;i++) {{
                        var r = els[i].getBoundingClientRect();
                        if (r.width > 50 && r.height > 20 && els[i].offsetParent) {{
                            var el = els[i];
                            el.focus();
                            var text = {text};
                            try {{
                                var dt = new DataTransfer();
                                dt.setData('text/plain', text);
                                el.dispatchEvent(new ClipboardEvent('paste', {{clipboardData: dt, bubbles: true, cancelable: true}}));
                                if (el.textContent.length > 0) return 'TYPED:' + el.textContent.length;
                            }} catch(e) {{}}
                            el.textContent = text;
                            el.dispatchEvent(new InputEvent('input', {{bubbles: true, data: text, inputType: 'insertText'}}));
                            return 'TYPED:' + el.textContent.length;
                        }}
                    }}
                    return 'NO-EDITOR';
                }})()"#,
                text = serde_json::to_string(&text).unwrap_or_default(),
            ))
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;

        let typed_str = typed.as_str().unwrap_or("");
        if !typed_str.starts_with("TYPED:") {
            return Err(rmcp::model::ErrorData::internal_error(
                format!("Comment editor not found after clicking {}", trigger),
                None,
            ));
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // STEP 6: Find and click submit
        let submitted = page
            .evaluate(
                r#"(() => {
                var btns = document.querySelectorAll('button');
                for (var i=0;i<btns.length;i++) {
                    if (btns[i].offsetParent && btns[i].textContent.trim() === 'Comment') {
                        btns[i].click(); return 'SUBMITTED';
                    }
                }
                return 'NO-SUBMIT';
            })()"#,
            )
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;

        // STEP 7: Verify
        tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
        let verify_snippet: String = text.chars().take(25).collect();
        let verified = page
            .evaluate(&format!(
                r#"(() => document.body.innerText.includes({snippet}) ? 'LIVE' : 'PENDING')()"#,
                snippet = serde_json::to_string(&verify_snippet).unwrap_or_default(),
            ))
            .await
            .unwrap_or_default();

        let result = serde_json::json!({
            "action": "commented",
            "method": trigger,
            "username": own_username,
            "duplicate_check": "passed",
            "typed": typed_str.trim_start_matches("TYPED:"),
            "submitted": submitted.as_str().unwrap_or(""),
            "verified": verified.as_str().unwrap_or(""),
        });

        let _ = self
            .recorder
            .record(&session_id, "page_comment", Some(&page_id), result.clone());
        Ok(text_result(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "SAFETY GATE: confirm a dangerous action before executing it. Call this BEFORE clicking refs on pages flagged with danger_zone (financial/medical/legal/auth)."
    )]
    async fn confirm_action(
        &self,
        Parameters(ConfirmActionParams {
            session_id,
            page_id,
            action,
            r#ref,
        }): Parameters<ConfirmActionParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let snap = page
            .snapshot()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "confirm_action",
            Some(&page_id),
            serde_json::json!({
                "action": action, "ref": r#ref, "url": snap.url, "confirmed": true,
            }),
        );
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "confirmed": true, "action": action, "ref": r#ref,
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Detect and dismiss common modals: cookie banners, consent dialogs, popups, overlays. Returns what was dismissed."
    )]
    async fn page_dismiss_modal(
        &self,
        Parameters(DismissModalParams {
            session_id,
            page_id,
        }): Parameters<DismissModalParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let result = page.evaluate(r#"(() => {
            const patterns = ['accept','agree','got it','ok','close','dismiss','allow all','continue','not now','no thanks'];
            const selectors = ['[aria-label*="close"]','[aria-label*="dismiss"]','[aria-label*="accept"]','[class*="cookie"] button','[class*="consent"] button','[role="dialog"] button'];
            const clicked = [];
            for (const sel of selectors) {
                try {
                    const els = document.querySelectorAll(sel);
                    for (const el of els) {
                        if (!el.offsetParent) continue;
                        const text = (el.textContent || '').toLowerCase().trim();
                        if (text && patterns.some(p => text.includes(p)) && text.length < 30) {
                            el.click(); clicked.push({selector: sel, text: text.slice(0,20)});
                            if (clicked.length >= 3) break;
                        }
                    }
                    if (clicked.length >= 3) break;
                } catch(e) {}
            }
            return JSON.stringify({dismissed: clicked.length, details: clicked});
        })()"#).await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(result.as_str().unwrap_or("no modals found")))
    }

    #[tool(
        description = "Evaluate a JavaScript expression in the page's main frame and return its JSON value. Read-only introspection is safest; treat results of mutations with care."
    )]
    async fn page_eval(
        &self,
        Parameters(PageEvalParams {
            session_id,
            page_id,
            expression,
        }): Parameters<PageEvalParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let value = page
            .evaluate(&expression)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_eval",
            Some(&page_id),
            serde_json::json!({ "len": expression.len() }),
        );
        Ok(text_result(
            serde_json::to_string(&value).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Capture a PNG screenshot of a page (viewport by default, full page with full_page=true). Saved under the session recordings dir; returns the file path. Feeds the live view when enabled."
    )]
    async fn page_screenshot(
        &self,
        Parameters(PageScreenshotParams {
            session_id,
            page_id,
            full_page,
        }): Parameters<PageScreenshotParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let png = page
            .screenshot(full_page.unwrap_or(false))
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let path = self
            .recorder
            .record_screenshot(&session_id, &page_id, &png)
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let url = page.url().await.unwrap_or_default();
        crate::liveview::update(&session_id, &page_id, &url, png);
        Ok(text_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "file": path.to_string_lossy(),
                "bytes": std::path::Path::new(&path).metadata().map(|m| m.len()).unwrap_or_default(),
            }))
            .unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Solve a CAPTCHA through the configured provider (env GHOSTFOX_CAPTCHA_PROVIDER=2captcha + GHOSTFOX_CAPTCHA_KEY). Turnstile/hcaptcha: pass sitekey + pageurl; image captchas: pass image_base64. Stealth-first: prefer not being challenged at all."
    )]
    async fn captcha_solve(
        &self,
        Parameters(CaptchaSolveParams {
            session_id,
            sitekey,
            pageurl,
            image_base64,
        }): Parameters<CaptchaSolveParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let _ = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let started = std::time::Instant::now();
        let result = if let Some(b64) = image_base64 {
            crate::captcha::solve_image(&b64).await
        } else if let (Some(sitekey), Some(pageurl)) = (sitekey, pageurl) {
            crate::captcha::solve_turnstile(&sitekey, &pageurl).await
        } else {
            return Err(rmcp::model::ErrorData::invalid_params(
                "pass image_base64, or sitekey + pageurl".to_string(),
                None,
            ));
        };
        match result {
            Ok(token) => {
                let _ = self.recorder.record(
                    &session_id,
                    "captcha_solve",
                    None,
                    serde_json::json!({ "ok": true, "seconds": started.elapsed().as_secs() }),
                );
                Ok(text_result(token))
            }
            Err(e) => {
                let _ = self.recorder.record(
                    &session_id,
                    "captcha_solve",
                    None,
                    serde_json::json!({ "ok": false, "error": e }),
                );
                Err(rmcp::model::ErrorData::internal_error(e, None))
            }
        }
    }

    #[tool(
        description = "Click an element by CSS selector. For form controls (buttons, inputs), uses a JS click; for links and other elements, dispatches real mouse events at coordinates. Prefer page_click_ref when you have a page_a11y ref — it scrolls into view first and is more reliable on web-component UIs. Returns 'ok' on success."
    )]
    async fn page_click(
        &self,
        Parameters(PageClickParams {
            session_id,
            page_id,
            selector,
        }): Parameters<PageClickParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        page.click(&selector)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_click",
            Some(&page_id),
            serde_json::json!({ "selector": selector }),
        );
        Ok(text_result("ok"))
    }

    #[tool(
        description = "Type text character-by-character into an element by CSS selector (human-like key events). Prefer page_type_ref when you have a page_a11y ref — it handles rich editors (Lexical/Draft/ProseMirror) and returns a verified receipt. Use this only when you only have a CSS selector and don't need rich editor support. Returns 'ok' on success."
    )]
    async fn page_type(
        &self,
        Parameters(PageTypeParams {
            session_id,
            page_id,
            selector,
            text,
        }): Parameters<PageTypeParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        page.type_text(&selector, &text)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_type",
            Some(&page_id),
            serde_json::json!({ "selector": selector, "chars": text.chars().count() }),
        );
        Ok(text_result("ok"))
    }

    #[tool(
        description = "Set an input's value directly (form fill). Works where key-event typing hits engine bugs; fires input/change events like real edits."
    )]
    async fn page_fill(
        &self,
        Parameters(PageFillParams {
            session_id,
            page_id,
            selector,
            text,
        }): Parameters<PageFillParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let sel = serde_json::to_string(&selector).unwrap_or_default();
        let expr = format!(
            "(() => {{ const el = document.querySelector({sel}); if (!el) return 'MISSING'; \
             el.value = {val}; el.dispatchEvent(new Event('input', {{bubbles: true}})); \
             el.dispatchEvent(new Event('change', {{bubbles: true}})); return 'OK'; }})()",
            sel = sel,
            val = serde_json::to_string(&text).unwrap_or_default(),
        );
        let res = page
            .evaluate(&expr)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        if res.as_str() == Some("OK") {
            // Receipt: confirm what landed where (guard against silent
            // page swaps eating the fill).
            let check = format!(
                "(() => {{ const el = document.querySelector({sel}); if (!el) return 'GONE'; \
                 return el.value === null ? 'NOTFIELD' : String(el.value.length); }})()",
                sel = serde_json::to_string(&selector).unwrap_or_default()
            );
            let receipt = page
                .evaluate(&check)
                .await
                .unwrap_or(serde_json::Value::Null);
            let landed = receipt.as_str().and_then(|v| v.parse::<usize>().ok());
            let _ = self.recorder.record(&session_id, "page_fill", Some(&page_id), serde_json::json!({ "selector": selector, "chars": text.chars().count(), "landed": landed }));
            Ok(text_result(
                serde_json::to_string_pretty(&serde_json::json!({
                    "filled": selector,
                    "landed_chars": landed,
                    "requested_chars": text.chars().count(),
                }))
                .unwrap_or_default(),
            ))
        } else {
            Err(rmcp::model::ErrorData::internal_error(
                "selector not found".to_string(),
                None,
            ))
        }
    }

    #[tool(
        description = "Press a named key (Enter, Tab, Escape, ArrowDown, ...) — e.g. Enter to submit a search box."
    )]
    async fn page_press(
        &self,
        Parameters(PagePressParams {
            session_id,
            page_id,
            key,
        }): Parameters<PagePressParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        // The core PageHandle trait has no press; the camoufox page does.
        // Downcast through the engine-specific handle.
        page.press_key(&key)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_press",
            Some(&page_id),
            serde_json::json!({ "key": key }),
        );
        Ok(text_result("ok"))
    }

    #[tool(description = "Generate a new coherent browser identity, returned as TOML.")]
    async fn identity_generate(&self) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let id = ghostfox_fingerprint::generate(&ghostfox_fingerprint::GenerateOptions::default());
        let toml_str = id
            .to_toml()
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(toml_str))
    }

    #[tool(
        description = "M5 taste: PERSONA METAMORPHOSIS — change the session's identity mid-flight. Regenerates the coherent fingerprint (platform/locale/tz/geo/screen/UA/fonts together), relaunches the engine against the SAME profile dir (cookies + logins survive), and closes the old pages. Reopen with page_open after. Use when a site flagged the current persona: morph and retry from the same session."
    )]
    async fn identity_morph(
        &self,
        Parameters(IdentityMorphParams {
            session_id,
            platform,
        }): Parameters<IdentityMorphParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let launch = {
            let state = self.state.read().await;
            state.launches.get(&session_id).cloned().ok_or_else(|| {
                rmcp::model::ErrorData::internal_error(
                    "no launch recipe for this session".to_string(),
                    None,
                )
            })?
        };
        // Fresh coherent identity — the whole persona regenerates together.
        let id = ghostfox_fingerprint::generate(&ghostfox_fingerprint::GenerateOptions {
            platform: match platform.as_deref() {
                Some("windows") => Some(ghostfox_fingerprint::Platform::Windows),
                Some("macos") => Some(ghostfox_fingerprint::Platform::MacOS),
                Some("linux") => Some(ghostfox_fingerprint::Platform::Linux),
                Some("android") => Some(ghostfox_fingerprint::Platform::Android),
                _ => None,
            },
            webrtc: None,
        });
        // Overwrite the profile identity — cookies stay, the fingerprint
        // moves as one coherent unit (the session_create rule is inverted
        // here ON PURPOSE: this is the metamorphosis).
        if let Some(dir) = &launch.profile_dir {
            let file = std::path::PathBuf::from(dir).join("identity.toml");
            let toml_str = id
                .to_toml()
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            std::fs::write(&file, toml_str)
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        }
        // Stop the OLD engine FIRST (shutdown + reap). Launching the new
        // engine before the old one releases the profile lock deadlocks
        // it at Browser.enable — the swap race found in E2E.
        session
            .stop_engine()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        // Now the profile is free: relaunch with the new identity.
        let engine = ghostfox_camoufox::launch(&launch)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        session.set_engine(engine).await;
        let toml_str = id
            .to_toml()
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(format!(
            "metamorphosis complete — pages closed, cookies preserved. Reopen with page_open.\n{}",
            toml_str
        )))
    }

    #[tool(
        description = "M2 semantic click: resolve an element by role+name against the NATIVE accessibility tree and click its center with a human-like mouse path. The coordinates come from the a11y tree's layout bounds — pages CANNOT poison them (JS getBoundingClientRect hooks are bypassed entirely). Returns an action receipt. Prefer this for anything a JS-rect poison could break: login buttons, captcha sliders, anti-bot traps."
    )]
    async fn page_click_native(
        &self,
        Parameters(PageClickNativeParams {
            session_id,
            page_id,
            role,
            name,
        }): Parameters<PageClickNativeParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        // Scroll the target into view FIRST via the engine's native
        // a11y scrollToPoint (no JS rects involved), then take the tree
        // and resolve the anchor against it. Best-effort: errors are
        // ignored, the anchor resolution below reports the truth.
        let _ = page.scroll_accessible_into_view(&role, &name).await;
        let raw = page
            .a11y_tree_native()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let snap = native_a11y_flatten(raw);
        self.cache_a11y(&page_id, snap.clone()).await;
        let anchor = crate::recipes::Anchor {
            role: role.clone(),
            name: name.clone(),
        };
        let el = crate::recipes::resolve_anchor(&snap, &anchor)
            .ok_or_else(|| {
                rmcp::model::ErrorData::internal_error(format!(
                    "no native element matches role={role:?} name={name:?} — run page_a11y(native=true) to inspect"
                ), None)
            })?;
        let Some(b) = &el.bounds else {
            return Err(rmcp::model::ErrorData::internal_error(
                "native element has no layout bounds".to_string(),
                None,
            ));
        };
        let (cx, cy) = (
            b.x as f64 + b.width as f64 / 2.0,
            b.y as f64 + b.height as f64 / 2.0,
        );
        page.click_coords(cx, cy)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let debug_target = serde_json::json!({
            "bounds": b,
            "click": { "x": cx, "y": cy }
        });
        self.note_recipe(
            &session_id,
            "page_click_native",
            serde_json::json!({ "role": role, "name": name }),
            None,
        )
        .await;
        let _ = self.recorder.record(
            &session_id,
            "page_click_native",
            Some(&page_id),
            serde_json::json!({ "role": role, "name": name, "x": cx, "y": cy }),
        );
        let receipt = self
            .action_receipt(&page, &page_id, "click_native", None)
            .await;
        let mut out = receipt;
        out["target"] = debug_target;
        Ok(text_result(
            serde_json::to_string_pretty(&out).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Observe what changed since the last a11y snapshot: diffs the page's cached snapshot (from page_a11y/page_extract/the last action receipt) against a FRESH walk. Returns {changed, adds, removes, updates, changes:[{op, ref, role, name, value?, prev?}]}. Capped at 40 entries. This is the observeDiff primitive: act, then page_diff to see consequences without paying for a full snapshot."
    )]
    async fn page_diff(
        &self,
        Parameters(PageRefParams {
            session_id,
            page_id,
        }): Parameters<PageRefParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session
            .page(&page_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let before = {
            let state = self.state.read().await;
            state.a11y_cache.get(&page_id).cloned()
        };
        let after = page
            .a11y_snapshot()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        self.cache_a11y(&page_id, after.clone()).await;
        let _ = self.recorder.record(
            &session_id,
            "page_diff",
            Some(&page_id),
            serde_json::json!({ "had_base": before.is_some() }),
        );
        match before {
            Some(b) => {
                let d = crate::receipts::diff(&b, &after, 40);
                Ok(text_result(
                    serde_json::to_string_pretty(&d).unwrap_or_default(),
                ))
            }
            None => Ok(text_result(
                "no cached snapshot to diff against — run page_a11y first",
            )),
        }
    }

    #[tool(
        description = "Start recording a deterministic recipe on a session. While recording, action tools (page_open, page_click_ref, page_type_ref, ...) append steps to the draft; ref-based actions capture SEMANTIC anchors (role + accessible name) so the recipe survives DOM churn. Call recipe_save to persist. Replay later with recipe_replay — no LLM needed for the happy path."
    )]
    async fn recipe_record(
        &self,
        Parameters(RecipeRecordParams { session_id, name }): Parameters<RecipeRecordParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        self.session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let safe: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(64)
            .collect();
        if safe.is_empty() {
            return Err(rmcp::model::ErrorData::invalid_params(
                "recipe name must contain a-zA-Z0-9-_".to_string(),
                None,
            ));
        }
        {
            let mut state = self.state.write().await;
            state
                .recipe_drafts
                .insert(session_id.clone(), (safe.clone(), Vec::new()));
        }
        Ok(text_result(format!(
            "recording `{safe}` — act now; recipe_save when done"
        )))
    }

    #[tool(
        description = "Persist the in-progress recipe of a session to ~/.ghostfox/recipes/<name>.json and stop recording. Returns the step summary."
    )]
    async fn recipe_save(
        &self,
        Parameters(RecipeSaveParams { session_id }): Parameters<RecipeSaveParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let (name, steps) = {
            let mut state = self.state.write().await;
            state.recipe_drafts.remove(&session_id).ok_or_else(|| {
                rmcp::model::ErrorData::invalid_params(
                    format!(
                        "session `{session_id}` has no recipe draft — call recipe_record first"
                    ),
                    None,
                )
            })?
        };
        let recipe = crate::recipes::Recipe {
            name: name.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            steps: steps.clone(),
        };
        let path = crate::recipes::save(&recipe)
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "recipe_save",
            None,
            serde_json::json!({ "recipe": name, "steps": steps.len() }),
        );
        let summary: Vec<String> = steps
            .iter()
            .map(|s| {
                format!(
                    "{}{}",
                    s.tool,
                    s.anchor
                        .as_ref()
                        .map(|a| format!(" -> ({}, {:?})", a.role, a.name))
                        .unwrap_or_default()
                )
            })
            .collect();
        Ok(text_result(format!(
            "saved `{name}` ({} steps) to {}\n{}",
            steps.len(),
            path.display(),
            summary.join("\n")
        )))
    }

    #[tool(description = "List saved recipes (name, step count, created).")]
    async fn recipe_list(&self) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let items: Vec<serde_json::Value> = crate::recipes::list()
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "steps": r.steps.len(),
                    "created_at": r.created_at,
                })
            })
            .collect();
        Ok(text_result(
            serde_json::to_string_pretty(&items).unwrap_or_default(),
        ))
    }

    #[tool(
        description = "Replay a saved recipe deterministically on a page: each step re-resolves its semantic anchor against a FRESH a11y snapshot (anchors: role+name), then executes. strict=true (default): a missing anchor stops with a rich error (step index + anchor + hint) so the LLM can take over; strict=false skips unresolvable steps. Returns {played, skipped, failed_at?}."
    )]
    async fn recipe_replay(
        &self,
        Parameters(RecipeReplayParams {
            session_id,
            page_id,
            name,
            strict,
        }): Parameters<RecipeReplayParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let recipe = crate::recipes::load(&name)
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let mut played = 0usize;
        let mut skipped: Vec<serde_json::Value> = Vec::new();
        // page_open steps create a NEW page; subsequent steps must act on
        // the page the previous step left behind, not the replay target.
        let mut current_page = page_id.clone();
        for (idx, step) in recipe.steps.iter().enumerate() {
            // Fresh snapshot per step: anchors must re-resolve against the
            // page the previous step left behind.
            let page = session
                .page(&current_page)
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            let snap = page
                .a11y_snapshot()
                .await
                .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
            self.cache_a11y(&current_page, snap.clone()).await;
            match step.tool.as_str() {
                "page_open" => {
                    let url = step
                        .params
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let before = session.page_ids().await;
                    session
                        .new_page(Some(url))
                        .await
                        .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                    let after = session.page_ids().await;
                    if let Some(new_id) = after.into_iter().find(|id| !before.contains(id)) {
                        current_page = new_id;
                    }
                    played += 1;
                    continue;
                }
                "page_click_ref" | "page_type_ref" => {
                    let Some(anchor) = &step.anchor else {
                        skipped.push(serde_json::json!({
                            "step": idx, "tool": step.tool, "reason": "no anchor recorded"
                        }));
                        if strict {
                            return Err(rmcp::model::ErrorData::internal_error(format!(
                                "step {idx} ({}) has no anchor — record the recipe from ref-based actions",
                                step.tool
                            ), None));
                        }
                        continue;
                    };
                    let Some(el) = crate::recipes::resolve_anchor(&snap, anchor) else {
                        let msg = serde_json::json!({
                            "failed_at": idx,
                            "tool": step.tool,
                            "anchor": anchor,
                            "hint": "page changed — run page_a11y/page_extract and continue manually",
                            "played": played,
                        });
                        if strict {
                            return Ok(text_result(format!(
                                "ESCALATE: {}",
                                serde_json::to_string_pretty(&msg).unwrap_or_default()
                            )));
                        }
                        skipped.push(msg);
                        continue;
                    };
                    if step.tool == "page_click_ref" {
                        page.click_ref(&el.r#ref).await.map_err(|e| {
                            rmcp::model::ErrorData::internal_error(e.to_string(), None)
                        })?;
                    } else {
                        let text = step
                            .params
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        page.type_ref(&el.r#ref, text).await.map_err(|e| {
                            rmcp::model::ErrorData::internal_error(e.to_string(), None)
                        })?;
                    }
                    played += 1;
                    continue;
                }
                other => {
                    skipped.push(serde_json::json!({
                        "step": idx, "tool": other, "reason": "tool not replayable in v1"
                    }));
                    if strict {
                        return Err(rmcp::model::ErrorData::internal_error(
                            format!("step {idx}: tool `{other}` is not replayable in v1"),
                            None,
                        ));
                    }
                    continue;
                }
            }
        }
        let _ = self.recorder.record(
            &session_id,
            "recipe_replay",
            Some(&page_id),
            serde_json::json!({ "recipe": name, "played": played, "skipped": skipped.len() }),
        );
        Ok(text_result(format!(
            "{{\"played\": {played}, \"skipped\": {}}}",
            serde_json::to_string(&skipped).unwrap_or_default()
        )))
    }

    #[tool(
        description = "Audit an identity TOML for coherence violations (contradictory signals a detector would flag)."
    )]
    async fn identity_audit(
        &self,
        Parameters(IdentityAuditParams { identity_toml }): Parameters<IdentityAuditParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let id: ghostfox_fingerprint::Identity = toml::from_str(&identity_toml)
            .map_err(|e| rmcp::model::ErrorData::invalid_params(format!("bad TOML: {e}"), None))?;
        let violations = ghostfox_fingerprint::audit(&id);
        if violations.is_empty() {
            Ok(text_result("clean: no violations"))
        } else {
            Ok(text_result(
                violations
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ))
        }
    }
}

/// Map a Gecko native role string (nsAccessibilityService::GetStringRole)
/// to our simplified agent roles; None = skip (structural/static).
fn map_native_role(role: &str) -> Option<String> {
    Some(match role {
        "pushbutton" | "toggle button" => "button".to_string(),
        "entry" | "password text" => "textbox".to_string(),
        "link" => "link".to_string(),
        "heading" => "heading".to_string(),
        "check button" => "checkbox".to_string(),
        "radio button" => "radio".to_string(),
        "combobox" => "combobox".to_string(),
        "slider" => "slider".to_string(),
        "listitem" => "listitem".to_string(),
        "menu item" => "menuitem".to_string(),
        "pagetab" => "tab".to_string(),
        "option" => "option".to_string(),
        "treeitem" => "treeitem".to_string(),
        "gridcell" => "gridcell".to_string(),
        "switch" => "switch".to_string(),
        "progress bar" => "progressbar".to_string(),
        "spin button" => "spinbutton".to_string(),
        "text leaf" | "text container" | "document" | "paragraph" | "section" | "region"
        | "group" | "unknown" | "panel" | "root frame" | "root pane" | "caption" | "label"
        | "menu bar" | "menubar" | "status bar" => return None,
        other => other.to_string(),
    })
}

/// Flatten the native AX tree into the agent snapshot shape. Refs are
/// assigned in deterministic DFS order (n1..n) — observation handles for
/// native mode: identify elements, diff snapshots, anchor recipes. Acting
/// still goes through role+name anchors or the walk source's refs.
/// M5 the Critic: an audit-oriented walk over the RAW a11y tree — keeps
/// TEXT leaves and every box (not just interactive nodes), so geometry
/// mistakes (clipped captions, missing padding, overlaps) can be judged
/// the way a human eye judges them.
#[derive(Debug, Clone, serde::Serialize)]
struct UiBox {
    role: String,
    name: String,
    bounds: Option<ghostfox_core::engine::Bounds>,
}

fn collect_ui_boxes(
    node: &serde_json::Value,
    parent: Option<&ghostfox_core::engine::Bounds>,
    texts: &mut Vec<(
        String,
        ghostfox_core::engine::Bounds,
        Option<ghostfox_core::engine::Bounds>,
    )>,
    boxes: &mut Vec<UiBox>,
) {
    let role = node.get("role").and_then(|v| v.as_str()).unwrap_or("");
    let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let bounds = node.get("bounds").and_then(|b| {
        Some(ghostfox_core::engine::Bounds {
            x: b.get("x")?.as_i64()?,
            y: b.get("y")?.as_i64()?,
            width: b.get("width")?.as_i64()?,
            height: b.get("height")?.as_i64()?,
        })
    });
    let is_text_leaf = matches!(role, "text leaf" | "text" | "statictext" | "text_leaf");
    if is_text_leaf {
        if let (Some(b), true) = (&bounds, !name.trim().is_empty()) {
            texts.push((name.to_string(), b.clone(), parent.cloned()));
        }
    } else if let Some(b) = &bounds {
        if b.width > 0 && b.height > 0 && role != "document" && role != "root" {
            boxes.push(UiBox {
                role: role.to_string(),
                name: name.to_string(),
                bounds: Some(b.clone()),
            });
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            collect_ui_boxes(child, bounds.as_ref(), texts, boxes);
        }
    }
}

fn ui_audit_from_raw(
    raw: &serde_json::Value,
    viewport: (i64, i64),
    full_rects: &[ghostfox_core::engine::UiRect],
) -> Vec<serde_json::Value> {
    let tree = raw.get("tree").cloned().unwrap_or_else(|| raw.clone());
    let mut texts: Vec<(
        String,
        ghostfox_core::engine::Bounds,
        Option<ghostfox_core::engine::Bounds>,
    )> = Vec::new();
    let mut boxes: Vec<UiBox> = Vec::new();
    collect_ui_boxes(&tree, None, &mut texts, &mut boxes);
    let mut issues: Vec<serde_json::Value> = Vec::new();

    let _intersects = |a: &ghostfox_core::engine::Bounds, b: &ghostfox_core::engine::Bounds| {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    };
    let _contains = |a: &ghostfox_core::engine::Bounds, b: &ghostfox_core::engine::Bounds| {
        a.x <= b.x
            && a.y <= b.y
            && a.x + a.width >= b.x + b.width
            && a.y + a.height >= b.y + b.height
    };

    // 1. Clip: text leaves poking out of their container.
    for (name, b, parent) in &texts {
        if let Some(p) = parent {
            let clip_r = b.x + b.width - (p.x + p.width);
            let clip_b = b.y + b.height - (p.y + p.height);
            let clip_l = p.x - b.x;
            let clip_t = p.y - b.y;
            let over_r = clip_r.max(0);
            let over_b = clip_b.max(0);
            if over_r > 0 || over_b > 0 {
                issues.push(serde_json::json!({
                    "type": "text-clipped",
                    "severity": "error",
                    "text": name,
                    "bounds": b,
                    "detail": format!(
                        "teks keluar container: kanan +{over_r}px, bawah +{over_b}px (kiri {clip_l}px, atas {clip_t}px)"
                    ),
                }));
            } else if clip_l < 1 || clip_t < 1 || clip_r.abs() < 1 || clip_b.abs() < 1 {
                // 2. Padding: text touching the container edge.
                let edge = if clip_l < 1 {
                    "kiri"
                } else if clip_t < 1 {
                    "atas"
                } else if clip_r.abs() < 1 {
                    "kanan"
                } else {
                    "bawah"
                };
                issues.push(serde_json::json!({
                    "type": "no-padding",
                    "severity": "warn",
                    "text": name,
                    "bounds": b,
                    "detail": format!("teks nempel tepi {edge} container (<1px)"),
                }));
            }
        }
    }

    // 3. Viewport overflow (texts + boxes).
    let (vw, vh) = viewport;
    for (name, b, _) in &texts {
        if b.x < 0 || b.y < 0 || b.x + b.width > vw || b.y + b.height > vh {
            issues.push(serde_json::json!({
                "type": "viewport-overflow",
                "severity": "warn",
                "text": name,
                "bounds": b,
                "detail": format!("keluar viewport {}x{}", vw, vh),
            }));
        }
    }

    // UiRect geometry helpers (the UiRect fields are x/y/w/h).
    let r_intersects = |a: &ghostfox_core::engine::UiRect, b: &ghostfox_core::engine::UiRect| {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    };
    let r_contains = |a: &ghostfox_core::engine::UiRect, b: &ghostfox_core::engine::UiRect| {
        a.x <= b.x && a.y <= b.y && a.x + a.w >= b.x + b.w && a.y + a.h >= b.y + b.h
    };

    // 4. Overlap: FULL-DOM rects (empty divs included) — computed from
    // the privileged frame-script walk, not the pruned a11y tree.
    // Skip ancestor/descendant pairs via containment; skip tiny noise.
    for i in 0..full_rects.len() {
        let a = &full_rects[i];
        if a.w * a.h < 64 {
            continue;
        }
        for b in full_rects.iter().skip(i + 1) {
            if b.w * b.h < 64 {
                continue;
            }
            if r_intersects(a, b) && !r_contains(a, b) && !r_contains(b, a) {
                let inter_w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
                let inter_h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
                if inter_w > 2 && inter_h > 2 {
                    issues.push(serde_json::json!({
                        "type": "overlap",
                        "severity": "error",
                        "a": { "tag": a.tag, "bounds": a },
                        "b": { "tag": b.tag, "bounds": b },
                        "detail": format!("dua elemen tabrakan ({inter_w}x{inter_h}px)"),
                    }));
                }
            }
        }
    }

    // 5. Crowding: interactive boxes closer than 4px side by side.
    let inter = |a: &UiBox| {
        matches!(
            a.role.as_str(),
            "button" | "textbox" | "link" | "checkbox" | "radio" | "combobox" | "slider" | "tab"
        )
    };
    for i in 0..boxes.len() {
        for j in (i + 1)..boxes.len() {
            let (a, b) = (&boxes[i], &boxes[j]);
            if !inter(a) || !inter(b) {
                continue;
            }
            let (Some(ba), Some(bb)) = (&a.bounds, &b.bounds) else {
                continue;
            };
            let same_row = ba.y.max(bb.y) < ba.y.min(bb.y) + ba.height.max(bb.height);
            if !same_row {
                continue;
            }
            let gap = if ba.x >= bb.x + bb.width {
                ba.x - (bb.x + bb.width)
            } else if bb.x >= ba.x + ba.width {
                bb.x - (ba.x + ba.width)
            } else {
                0
            };
            if gap < 4 {
                issues.push(serde_json::json!({
                    "type": "crowded",
                    "severity": "warn",
                    "a": { "role": a.role, "name": a.name, "bounds": ba },
                    "b": { "role": b.role, "name": b.name, "bounds": bb },
                    "detail": format!("elemen interaktif kelewat deket (jarak {gap}px)"),
                }));
            }
        }
    }

    issues
}

fn native_a11y_flatten(raw: serde_json::Value) -> ghostfox_core::engine::A11ySnapshot {
    use ghostfox_core::engine::A11yElement;
    let tree = raw.get("tree").cloned().unwrap_or(raw);
    let mut elements: Vec<A11yElement> = Vec::new();
    let mut counter: usize = 0;

    fn walk(node: &serde_json::Value, out: &mut Vec<A11yElement>, counter: &mut usize) {
        let role_str = node.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let Some(role) = map_native_role(role_str) else {
            if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
                for child in children {
                    walk(child, out, counter);
                }
            }
            return;
        };
        let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let interactive = matches!(
            role.as_str(),
            "button"
                | "textbox"
                | "link"
                | "checkbox"
                | "radio"
                | "combobox"
                | "slider"
                | "listitem"
                | "menuitem"
                | "tab"
                | "option"
                | "treeitem"
                | "switch"
        ) || node
            .get("focusable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let heading = role == "heading";
        if interactive || heading {
            *counter += 1;
            let checked = node.get("checked").and_then(|v| match v {
                serde_json::Value::Bool(b) => Some(*b),
                _ => None,
            });
            let bounds = node.get("bounds").and_then(|b| {
                Some(ghostfox_core::engine::Bounds {
                    x: b.get("x")?.as_i64()?,
                    y: b.get("y")?.as_i64()?,
                    width: b.get("width")?.as_i64()?,
                    height: b.get("height")?.as_i64()?,
                })
            });
            let el = A11yElement {
                r#ref: format!("n{}", counter),
                role: role.clone(),
                name: name.to_string(),
                value: node
                    .get("value")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                checked,
                disabled: node.get("disabled").and_then(|v| v.as_bool()),
                suspicious: None,
                tag: node.get("tag").and_then(|v| v.as_str()).map(str::to_string),
                expanded: node.get("expanded").and_then(|v| v.as_bool()),
                required: node.get("required").and_then(|v| v.as_bool()),
                visibility: None,
                scroll_pages: None,
                own: None,
                description: node
                    .get("description")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
                bounds,
                extra: node
                    .as_object()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|(k, _)| k != "bounds" && k != "children")
                    .collect(),
            };
            out.push(el);
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                walk(child, out, counter);
            }
        }
    }
    walk(&tree, &mut elements, &mut counter);

    ghostfox_core::engine::A11ySnapshot {
        elements,
        login_state: "unknown".into(),
        page_url: String::new(),
        page_title: String::new(),
        danger_zone: None,
        suspicious_elements: 0,
        page_archived: None,
        own_elements: 0,
        username: None,
        below_viewport: 0,
        max_scroll_pages: 0,
        notifications: Vec::new(),
        rate_limit_seconds: None,
    }
}

#[rmcp::tool_handler(router = Self::tool_router())]
impl rmcp::ServerHandler for GhostfoxServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        use rmcp::model::*;
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            server_info: Implementation::from_build_env(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            instructions: Some(
                "Stealth browser for AI agents. Create a session, open pages, take snapshots, click and type. Identities are coherent by construction; use identity_audit to check any identity TOML.".into(),
            ),
        }
    }
}
