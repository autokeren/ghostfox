//! MCP server implementation: tools mapped 1:1 onto core operations.
//!
//! Every tool is narrow and typed — no god-tools, no "smart" behavior an
//! attacker could steer through page content.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use rmcp::handler::server::tool::Parameters;
use rmcp::{tool, tool_router};
use rmcp::model::CallToolResult;
use serde::Deserialize;
use schemars::JsonSchema;

use ghostcloak_core::engine::EngineKind;
use ghostcloak_core::session::Session;

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
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SessionEvidenceParams {
    session_id: String,
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
struct IdentityAuditParams {
    identity_toml: String,
}

#[derive(Clone, Default)]
pub struct GhostcloakServer {
    state: Arc<tokio::sync::RwLock<ServerState>>,
    recorder: crate::recording::Recorder,
}

#[derive(Default)]
pub(crate) struct ServerState {
    pub(crate) sessions: HashMap<String, Arc<Session>>,
}

impl GhostcloakServer {
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
impl GhostcloakServer {    #[tool(description = "Create a new browsing session: launches the engine with a fresh coherent identity. Returns session_id.")]
    async fn session_create(
        &self,
        Parameters(SessionCreateParams { platform, profile_dir, proxy }): Parameters<SessionCreateParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let gen_opts = ghostcloak_fingerprint::GenerateOptions {
            platform: match platform.as_deref() {
                Some("windows") => Some(ghostcloak_fingerprint::Platform::Windows),
                Some("macos") => Some(ghostcloak_fingerprint::Platform::MacOS),
                Some("linux") => Some(ghostcloak_fingerprint::Platform::Linux),
                Some("android") => Some(ghostcloak_fingerprint::Platform::Android),
                _ => None,
            },
            webrtc: None,
        };
        let identity = ghostcloak_fingerprint::generate(&gen_opts);

        let mut launch = ghostcloak_core::engine::LaunchOptions::default();
        launch.profile_dir = profile_dir;
        launch.proxy = proxy;
        launch.headless = true;

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
                let _ = identity.to_toml().map(|t| std::fs::write(&identity_file, t));
            }
        }
        let engine = ghostcloak_camoufox::launch(&launch)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let session = Session::from_engine(
            ghostcloak_core::util::short_id(),
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
            let _ = self.recorder.record_identity(&id, &identity.to_toml().unwrap_or_default());
            let _ = self.recorder.record(&id, "session_create", None, ev);
        }
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

    #[tool(description = "Open a page (navigate) in a session. Returns page_id.")]
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
        let _ = self
            .recorder
            .record(&session_id, "page_open", Some(&new_id), serde_json::json!({ "url": url }));
        Ok(text_result(new_id))
    }

    #[tool(description = "Get a token-friendly snapshot of a page (url, title, extracted text).")]
    async fn page_snapshot(
        &self,
        Parameters(PageRefParams { session_id, page_id }): Parameters<PageRefParams>,
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

    #[tool(description = "Get recorded evidence for a session: event log, snapshot files, identity used. Recordings live under ~/.ghostfox/recordings/.")]
    async fn session_evidence(
        &self,
        Parameters(SessionEvidenceParams { session_id }): Parameters<SessionEvidenceParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        match self.recorder.evidence(&session_id) {
            Ok(ev) => Ok(text_result(serde_json::to_string_pretty(&ev).unwrap_or_default())),
            Err(e) => Err(rmcp::model::ErrorData::internal_error(
                format!("no evidence for session `{session_id}`: {e}"),
                None,
            )),
        }
    }

    #[tool(description = "Capture a PNG screenshot of a page (viewport by default, full page with full_page=true). Saved under the session recordings dir; returns the file path. Feeds the live view when enabled.")]
    async fn page_screenshot(
        &self,
        Parameters(PageScreenshotParams { session_id, page_id, full_page }): Parameters<PageScreenshotParams>,
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

    #[tool(description = "Solve a CAPTCHA through the configured provider (env GHOSTFOX_CAPTCHA_PROVIDER=2captcha + GHOSTFOX_CAPTCHA_KEY). Turnstile/hcaptcha: pass sitekey + pageurl; image captchas: pass image_base64. Stealth-first: prefer not being challenged at all.")]
    async fn captcha_solve(
        &self,
        Parameters(CaptchaSolveParams { session_id, sitekey, pageurl, image_base64 }): Parameters<CaptchaSolveParams>,
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

    #[tool(description = "Click an element by CSS selector.")]
    async fn page_click(
        &self,
        Parameters(PageClickParams { session_id, page_id, selector }): Parameters<PageClickParams>,
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
        let _ = self.recorder.record(&session_id, "page_click", Some(&page_id), serde_json::json!({ "selector": selector }));
        Ok(text_result("ok"))
    }

    #[tool(description = "Type text into an element by CSS selector.")]
    async fn page_type(
        &self,
        Parameters(PageTypeParams { session_id, page_id, selector, text }): Parameters<PageTypeParams>,
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
        let _ = self.recorder.record(&session_id, "page_type", Some(&page_id), serde_json::json!({ "selector": selector, "chars": text.chars().count() }));
        Ok(text_result("ok"))
    }

    #[tool(description = "Set an input's value directly (form fill). Works where key-event typing hits engine bugs; fires input/change events like real edits.")]
    async fn page_fill(
        &self,
        Parameters(PageFillParams { session_id, page_id, selector, text }): Parameters<PageFillParams>,
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
            let _ = self.recorder.record(&session_id, "page_fill", Some(&page_id), serde_json::json!({ "selector": selector, "chars": text.chars().count() }));
            Ok(text_result("ok"))
        } else {
            Err(rmcp::model::ErrorData::internal_error(
                "selector not found".to_string(),
                None,
            ))
        }
    }

    #[tool(description = "Press a named key (Enter, Tab, Escape, ArrowDown, ...) — e.g. Enter to submit a search box.")]
    async fn page_press(
        &self,
        Parameters(PagePressParams { session_id, page_id, key }): Parameters<PagePressParams>,
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
        let _ = self.recorder.record(&session_id, "page_press", Some(&page_id), serde_json::json!({ "key": key }));
        Ok(text_result("ok"))
    }

    #[tool(description = "Generate a new coherent browser identity, returned as TOML.")]
    async fn identity_generate(&self) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let id = ghostcloak_fingerprint::generate(&ghostcloak_fingerprint::GenerateOptions::default());
        let toml_str = id
            .to_toml()
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(toml_str))
    }

    #[tool(description = "Audit an identity TOML for coherence violations (contradictory signals a detector would flag).")]
    async fn identity_audit(
        &self,
        Parameters(IdentityAuditParams { identity_toml }): Parameters<IdentityAuditParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let id: ghostcloak_fingerprint::Identity = toml::from_str(&identity_toml)
            .map_err(|e| rmcp::model::ErrorData::invalid_params(format!("bad TOML: {e}"), None))?;
        let violations = ghostcloak_fingerprint::audit(&id);
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

#[rmcp::tool_handler(router = Self::tool_router())]
impl rmcp::ServerHandler for GhostcloakServer {
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
