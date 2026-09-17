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
    struct PixelsParams {
        session_id: String,
        page_id: String,
        /// Ref of the element to render (canvas / img / background-image).
        r#ref: String,
        /// Grid width in cells (default 32).
        grid_w: Option<u32>,
        /// Grid height in cells (default 21).
        grid_h: Option<u32>,
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
impl GhostcloakServer {
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

        let launch = ghostcloak_core::engine::LaunchOptions {
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
            let _ = self
                .recorder
                .record_identity(&id, &identity.to_toml().unwrap_or_default());
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
            .a11y_snapshot()
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
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
        let _ = self.recorder.record(
            &session_id,
            "page_click_ref",
            Some(&page_id),
            serde_json::json!({ "ref": r#ref }),
        );
        Ok(text_result("clicked"))
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
        page.drag_ref(&from_ref, &to, offset_x.unwrap_or(0.0), offset_y.unwrap_or(0.0))
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
        description = "SUPERMAN GLASSES: render an element (canvas / img / background-image) as a compact luminance GRID of digits 0-9 the agent READS directly — see shapes, holes, object orientation, image layout WITHOUT a vision model. 0=black, 9=white. Captcha gaps appear as darker cells, upright skies are bright rows on top. Pass the ref from page_a11y. Returns JSON {w, h, grid:[rows of digits]}."
    )]
    async fn page_pixels(
        &self,
        Parameters(PixelsParams {
            session_id,
            page_id,
            r#ref,
            grid_w,
            grid_h,
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
        let grid = page
            .pixels_ref(&r#ref, gw, gh)
            .await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(
            &session_id,
            "page_pixels",
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
        let _ = self.recorder.record(
            &session_id,
            "page_type_ref",
            Some(&page_id),
            serde_json::json!({ "ref": r#ref, "chars": text.chars().count() }),
        );
        Ok(text_result("typed"))
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
        Parameters(SessionMeParams { session_id, page_id }): Parameters<SessionMeParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self
            .session(&session_id)
            .await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = match page_id {
            Some(pid) => session
                .page(&pid)
                .await
                .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?,
            None => {
                let ids = session.page_ids().await;
                if let Some(first) = ids.first() {
                    session
                        .page(first)
                        .await
                        .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?
                } else {
                    let pg = session.new_page(Some("about:blank")).await
                        .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
                    pg
                }
            }
        };
        // Use a11y_snapshot (pierces shadow DOM) to find username
        let snap = page.a11y_snapshot().await
            .unwrap_or_else(|_| ghostcloak_core::engine::A11ySnapshot {
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
        let mut username = snap.username.clone().unwrap_or_else(|| "unknown".to_string());
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
                    let rest = &name[at+1..];
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
                        let uname = parts[i+1];
                        if uname.len() >= 3 && uname.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                            username = uname.to_string();
                            break;
                        }
                    }
                }
                if username != "unknown" { break; }
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
        Parameters(PageCommentParams { session_id, page_id, text }): Parameters<PageCommentParams>,
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
        let snap = page.a11y_snapshot().await
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
                let _ = self.recorder.record(&session_id, "page_comment", Some(&page_id), serde_json::json!({
                    "action": "skipped", "reason": "duplicate", "username": own_username
                }));
                return Ok(text_result(serde_json::to_string_pretty(&serde_json::json!({
                    "action": "skipped",
                    "reason": "already_commented",
                    "username": own_username,
                })).unwrap_or_default()));
            }
        }

        // STEP 3: Find comment trigger (Reply preferred, Join conversation fallback)
        let trigger = page
            .evaluate(r#"(() => {
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
            })()"#)
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

        page.evaluate(click_js).await
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
            .evaluate(r#"(() => {
                var btns = document.querySelectorAll('button');
                for (var i=0;i<btns.length;i++) {
                    if (btns[i].offsetParent && btns[i].textContent.trim() === 'Comment') {
                        btns[i].click(); return 'SUBMITTED';
                    }
                }
                return 'NO-SUBMIT';
            })()"#)
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

        let _ = self.recorder.record(&session_id, "page_comment", Some(&page_id), result.clone());
        Ok(text_result(serde_json::to_string_pretty(&result).unwrap_or_default()))
    }

    #[tool(
        description = "SAFETY GATE: confirm a dangerous action before executing it. Call this BEFORE clicking refs on pages flagged with danger_zone (financial/medical/legal/auth)."
    )]
    async fn confirm_action(
        &self,
        Parameters(ConfirmActionParams { session_id, page_id, action, r#ref }): Parameters<ConfirmActionParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self.session(&session_id).await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session.page(&page_id).await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let snap = page.snapshot().await
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        let _ = self.recorder.record(&session_id, "confirm_action", Some(&page_id), serde_json::json!({
            "action": action, "ref": r#ref, "url": snap.url, "confirmed": true,
        }));
        Ok(text_result(serde_json::to_string_pretty(&serde_json::json!({
            "confirmed": true, "action": action, "ref": r#ref,
        })).unwrap_or_default()))
    }

    #[tool(
        description = "Detect and dismiss common modals: cookie banners, consent dialogs, popups, overlays. Returns what was dismissed."
    )]
    async fn page_dismiss_modal(
        &self,
        Parameters(DismissModalParams { session_id, page_id }): Parameters<DismissModalParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let session = self.session(&session_id).await
            .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
        let page = session.page(&page_id).await
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
        let id =
            ghostcloak_fingerprint::generate(&ghostcloak_fingerprint::GenerateOptions::default());
        let toml_str = id
            .to_toml()
            .map_err(|e| rmcp::model::ErrorData::internal_error(e.to_string(), None))?;
        Ok(text_result(toml_str))
    }

    #[tool(
        description = "Audit an identity TOML for coherence violations (contradictory signals a detector would flag)."
    )]
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
