//! Camoufox engine implementation: launches the patched Firefox binary with
//! our identity injected via CAMOU_CONFIG env vars, drives pages over the
//! Juggler pipe.

use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use ghostcloak_core::engine::{Engine, EngineKind, LaunchOptions, PageHandle, PageSnapshot};
use ghostcloak_core::error::{GhostError, Result};
use ghostcloak_fingerprint::Identity;

use crate::config;
use crate::juggler::JugglerConnection;

pub struct CamoufoxEngine {
    conn: Arc<JugglerConnection>,
    identity: Identity,
    #[allow(dead_code)]
    home: PathBuf,
    /// Ephemeral profile dir to remove on shutdown (None = user-provided,
    /// persistent).
    ephemeral_profile: Option<PathBuf>,
}

impl CamoufoxEngine {
    pub async fn launch(opts: &LaunchOptions) -> Result<Arc<Self>> {
        let identity = Identity::load_or_generate(opts.profile_dir.as_deref())?;
        let (home, bin_name) = autodetect_engine()
            .ok_or_else(|| GhostError::EngineUnavailable("ghostfox/camoufox binary not found".into()))?;

        // Ephemeral profiles live under a sweepable root so shutdown can
        // clean them and a crashed run can't strand them all over /tmp.
        let (profile, ephemeral) = match &opts.profile_dir {
            Some(dir) => {
                let mut p = PathBuf::from(dir);
                if p.is_dir() {
                    p = p.join("camoufox-profile");
                }
                (p, false)
            }
            None => {
                let p = std::env::temp_dir()
                    .join("ghostcloak-profiles")
                    .join(ghostcloak_core::util::short_id());
                (p, true)
            }
        };
        // Camoufox refuses to boot without an existing profile directory.
        std::fs::create_dir_all(&profile)?;

        // NOTE: the launcher (`ghostfox`/`camoufox`) re-execs `-bin` WITHOUT
        // preserving the juggler pipes. Always spawn the -bin binary.
        let mut cmd = std::process::Command::new(home.join(bin_name));
        cmd.arg("--juggler-pipe")
            .arg("-silent")
            .arg("-profile")
            .arg(&profile)
            .arg("-no-remote")
            // Run from the install dir: the engine resolves helper binaries
            // (glxtest etc.) relative to its working directory.
            .current_dir(&home);

        // Juggler pipe convention (from Playwright's FirefoxConnection):
        // stdio = [ignore, pipe, pipe, pipe, pipe] — the juggler channel is
        // fd 3 (browser reads) + fd 4 (browser writes), NOT stdin/stdout.
        // os_pipe::pipe() returns (reader, writer).
        let (cmd_rx, cmd_tx) = os_pipe::pipe().map_err(|e| GhostError::Protocol(e.to_string()))?;
        let (resp_rx, resp_tx) = os_pipe::pipe().map_err(|e| GhostError::Protocol(e.to_string()))?;
        // Child fd 3 <- cmd_rx (we write commands into cmd_tx).
        // Child fd 4 -> resp_tx (we read responses from resp_rx).
        let cmd_rx_fd = cmd_rx.as_raw_fd();
        let resp_tx_fd = resp_tx.as_raw_fd();
        // Leak the child-bound ends: their fds must stay open for the
        // child's lifetime (this mirrors the proven fd_probe flow).
        std::mem::forget(cmd_rx);
        std::mem::forget(resp_tx);
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(move || {
                // Own process group: contentprocs must die with the parent.
                libc::setsid();
                // dup the pipe ends onto the juggler fds...
                if libc::dup2(cmd_rx_fd, 3) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(resp_tx_fd, 4) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // ...and clear O_CLOEXEC: os_pipe creates pipes with
                // CLOEXEC set, and dup2 inherits the flag, which would
                // close fds 3/4 at exec and kill the juggler channel.
                let flags = libc::fcntl(3, libc::F_GETFD);
                if flags >= 0 {
                    libc::fcntl(3, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
                }
                let flags4 = libc::fcntl(4, libc::F_GETFD);
                if flags4 >= 0 {
                    libc::fcntl(4, libc::F_SETFD, flags4 & !libc::FD_CLOEXEC);
                }
                Ok(())
            });
        }

        if opts.headless {
            cmd.arg("--headless");
        }
        if let Some(proxy) = &opts.proxy {
            // Firefox-style: each proxy type is its own flag on the command
            // line. Accept host:port or socks5://host:port.
            let (flag, rest) = if let Some(r) = proxy.strip_prefix("socks5://") {
                ("--socks-proxy", r)
            } else if let Some(r) = proxy.strip_prefix("http://") {
                ("--proxy-server", r)
            } else {
                ("--proxy-server", proxy.as_str())
            };
            cmd.arg(format!("{flag}={rest}")).arg("--proxy-bypass-list=<-loopback>");
        }
        for extra in &opts.extra_args {
            cmd.arg(extra);
        }

        // Identity injection — the whole point of this engine.
        for (k, v) in config::env_for_identity(&identity, &home) {
            cmd.env(k, v);
        }

        // Spawn via std::process::Command (command-fds hooks pre_exec on
        // std). Track the child through tokio's reaper so it doesn't zombie.
        let child = cmd
            .spawn()
            .map_err(|e| GhostError::Protocol(format!("camoufox spawn: {e}")))?;
        let pid = child.id();
        let conn = JugglerConnection::spawn_std(child, pid, cmd_tx, resp_rx)?;

        // Mobile personas: enable the engine's touch override for the default
        // context — (pointer: coarse) media queries + touch event dispatch,
        // the same mechanism Playwright's `hasTouch` uses. maxTouchPoints
        // is handled through CAMOU_CONFIG by the engine patch.
        if identity.platform == ghostcloak_fingerprint::identity::Platform::Android {
            let _ = conn
                .request(
                    "Browser.setTouchOverride",
                    serde_json::json!({ "hasTouch": true }),
                )
                .await;
        }

        Ok(Arc::new(Self {
            conn,
            identity,
            home,
            ephemeral_profile: ephemeral.then_some(profile),
        }))
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }
}

/// Locate the engine install: Ghostfox (preferred) with legacy Camoufox
/// fallback. Returns (install_dir, binary_name).
fn autodetect_engine() -> Option<(PathBuf, &'static str)> {
    let home = std::env::var("GHOSTFOX_HOME")
        .or_else(|_| std::env::var("CAMOUFOX_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
            let mut p = PathBuf::from(home);
            p.push(".cache");
            p.push("camoufox");
            p
        });
    for bin in ["ghostfox-bin", "camoufox-bin"] {
        if home.join(bin).exists() {
            return Some((home, bin));
        }
    }
    None
}

pub struct CamoufoxPage {
    conn: Arc<JugglerConnection>,
    target_id: String,
    /// Juggler session for this target; all Page/Runtime commands must be
    /// routed through it, not the root session.
    session_id: Mutex<Option<String>>,
    /// The page's MAIN frame. Only this frame's contexts and navigations are
    /// tracked for evaluate(); iframe contexts must never hijack them.
    main_frame_id: Mutex<Option<String>>,
    frame_id: Mutex<Option<String>>,
    execution_context_id: Mutex<Option<String>>,
}

use tokio::sync::Mutex;

impl CamoufoxPage {
    fn new(conn: Arc<JugglerConnection>, target_id: String) -> Arc<Self> {
        Arc::new(Self {
            conn,
            target_id,
            session_id: Mutex::new(None),
            main_frame_id: Mutex::new(None),
            frame_id: Mutex::new(None),
            execution_context_id: Mutex::new(None),
        })
    }

    async fn session_id(&self) -> Result<String> {
        let guard = self.session_id.lock().await;
        guard
            .clone()
            .ok_or_else(|| GhostError::PageOp("target session not attached".into()))
    }

    async fn frame_id(&self) -> Result<String> {
        let guard = self.frame_id.lock().await;
        guard
            .clone()
            .ok_or_else(|| GhostError::PageOp("frame not yet attached".into()))
    }

    async fn execution_context(&self) -> Result<String> {
        let guard = self.execution_context_id.lock().await;
        guard
            .clone()
            .ok_or_else(|| GhostError::PageOp("execution context not established".into()))
    }

    async fn snapshot_inner(&self) -> Result<PageSnapshot> {
        let url = self
            .evaluate("location.href")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string();
        let title = self
            .evaluate("document.title")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string();
        let body = self
            .evaluate("document.body ? document.body.innerText : ''")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string();
        Ok(PageSnapshot {
            url,
            title: Some(title),
            content: body,
            captured_at: chrono::Utc::now(),
        })
    }
}

#[async_trait]
impl Engine for CamoufoxEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Firefox
    }

    async fn new_page(&self, _opts: &HashMap<String, serde_json::Value>) -> Result<Arc<dyn PageHandle>> {
        // 1. Enable the browser-side dispatcher (required before anything
        //    else; attachToDefaultContext is mandatory, not optional).
        self.conn
            .request(
                "Browser.enable",
                serde_json::json!({ "attachToDefaultContext": true }),
            )
            .await?;

        // 2. Subscribe to events BEFORE newPage: the attachedToTarget event
        //    fires immediately when the page is created, and we'd otherwise
        //    race the broadcast and miss it.
        let mut events = self.conn.subscribe();

        // 3. Create the page in the default context.
        let result = self
            .conn
            .request("Browser.newPage", serde_json::json!({}))
            .await?;
        let target_id = result
            .get("targetId")
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                GhostError::Protocol(format!(
                    "Browser.newPage returned no targetId: {}",
                    serde_json::to_string(&result).unwrap_or_default()
                ))
            })?
            .to_string();

        let handle = CamoufoxPage::new(self.conn.clone(), target_id);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            if std::time::Instant::now() > deadline {
                // No context event seen; fall back to empty ids — evaluate
                // against the main world by convention and let the caller
                // surface any protocol error.
                *handle.frame_id.lock().await = Some(String::new());
                *handle.execution_context_id.lock().await = Some(String::new());
                break;
            }
            if self.conn.is_closed() {
                return Err(GhostError::EngineCrashed);
            }
            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                events.recv(),
            )
            .await
            {
                Ok(Ok(msg)) => {
                    let method = msg.get("method").and_then(|m| m.as_str());
                    if method == Some("Browser.attachedToTarget") {
                        // targetInfo.targetId names the page this session
                        // belongs to — only claim our own.
                        if let Some(tid) = msg
                            .pointer("/params/targetInfo/targetId")
                            .or_else(|| msg.pointer("/params/targetId"))
                            .and_then(|v| v.as_str())
                        {
                            if tid == handle.target_id {
                                if let Some(sid) = msg.pointer("/params/sessionId").and_then(|v| v.as_str()) {
                                    *handle.session_id.lock().await = Some(sid.to_string());
                                }
                            }
                        }
                    }
                    if method == Some("Browser.detachedFromTarget") {
                        if let Some(tid) = msg.pointer("/params/targetId").and_then(|v| v.as_str()) {
                            if tid == handle.target_id {
                                *handle.session_id.lock().await = None;
                                *handle.execution_context_id.lock().await = None;
                            }
                        }
                    }
                    // Once attached, ignore every other page's events —
                    // they'd otherwise hijack our context id.
                    let my_sid = handle.session_id.lock().await.clone();
                    if let Some(evt_sid) = msg.get("sessionId").and_then(|s| s.as_str()) {
                        if my_sid.is_some() && Some(evt_sid) != my_sid.as_deref() {
                            continue;
                        }
                    }
                    if method == Some("Runtime.executionContextCreated") {
                        if let Some(cx) = msg.pointer("/params/executionContextId").and_then(|v| v.as_str()) {
                            let fid = msg
                                .pointer("/params/auxData/frameId")
                                .and_then(|v| v.as_str())
                                .map(str::to_string);
                            let mut main = handle.main_frame_id.lock().await;
                            let is_main = match (main.clone(), fid.as_deref()) {
                                (Some(m), Some(f)) => m == f,
                                (None, Some(f)) => {
                                    // First frame we ever see is the main one:
                                    // a page cannot host an iframe before its
                                    // main frame exists.
                                    *main = Some(f.to_string());
                                    true
                                }
                                _ => false,
                            };
                            drop(main);
                            if is_main {
                                *handle.frame_id.lock().await = fid;
                                *handle.execution_context_id.lock().await = Some(cx.to_string());
                            }
                        }
                    }
                    if method == Some("Page.navigationCommitted") {
                        if let Some(fid) = msg.pointer("/params/frameId").and_then(|v| v.as_str()) {
                            let mut main = handle.main_frame_id.lock().await;
                            match main.clone() {
                                Some(m) if m == fid => {
                                    drop(main);
                                    *handle.frame_id.lock().await = Some(fid.to_string());
                                }
                                None => {
                                    *main = Some(fid.to_string());
                                    drop(main);
                                    *handle.frame_id.lock().await = Some(fid.to_string());
                                }
                                _ => {}
                            }
                        }
                    }
                    let sid = handle.session_id.lock().await.clone();
                    let cx = handle.execution_context_id.lock().await.clone();
                    if sid.is_some() && cx.is_some() {
                        break;
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    *handle.frame_id.lock().await = Some(String::new());
                    *handle.execution_context_id.lock().await = Some(String::new());
                    break;
                }
                Err(_) => continue, // timeout, keep waiting until deadline
            }
        }

        // 4b. Give the page a real viewport: headless pages default to a
        // degenerate size and mouse-event dispatch needs actual bounds.
        {
            let sid = handle.session_id().await?;
            // Viewport follows the identity's screen class: a phone persona
            // must present a phone viewport, a desktop a desktop one.
            // (Height shaved slightly for browser chrome.)
            let (vw, vh) = match self.identity.platform {
                ghostcloak_fingerprint::identity::Platform::Android => (
                    self.identity.screen.width,
                    self.identity.screen.height.saturating_sub(80),
                ),
                _ => (1280, 800),
            };
            let _ = self
                .conn
                .request_session(
                    "Page.setViewportSize",
                    serde_json::json!({"viewportSize": {"width": vw, "height": vh}}),
                    Some(&sid),
                )
                .await;
        }

        // 4. Keep execution-context and frame ids live: contexts are
        //    recreated on every navigation, so a static snapshot goes stale.
        //    A background pump tracks the newest ids forever.
        {
            let conn = self.conn.clone();
            let handle2 = handle.clone();
            let mut rx = self.conn.subscribe();
            tokio::spawn(async move {
                // Every Juggler event is stamped with the sessionId of the
                // page that emitted it; only listen to ours. Without this,
                // another page's navigation (e.g. a consent screen click)
                // corrupts our context id — the cross-talk bug.
                loop {
                    match rx.recv().await {
                        Ok(msg) => {
                            let method = msg.get("method").and_then(|m| m.as_str());
                            // Root-session events (Browser.*) are broadcast;
                            // per-page events must match our session, read
                            // live so late attachments are covered too.
                            if let Some(evt_sid) = msg.get("sessionId").and_then(|s| s.as_str()) {
                                let my_sid = handle2.session_id.lock().await.clone();
                                if Some(evt_sid) != my_sid.as_deref() {
                                    continue;
                                }
                            }
                            match method {
                                Some("Runtime.executionContextCreated") => {
                                    if let Some(cx) = msg.pointer("/params/executionContextId").and_then(|v| v.as_str()) {
                                        // Only the MAIN frame's context is a
                                        // valid evaluate target; iframe srcdoc
                                        // contexts must not hijack it (e.g.
                                        // bot.sannysoft.com's trailing test
                                        // iframes).
                                        let fid = msg
                                            .pointer("/params/auxData/frameId")
                                            .and_then(|v| v.as_str())
                                            .map(str::to_string);
                                        let mut main = handle2.main_frame_id.lock().await;
                                        let is_main = match (main.clone(), fid.as_deref()) {
                                            (Some(m), Some(f)) => m == f,
                                            (None, Some(f)) => {
                                                *main = Some(f.to_string());
                                                true
                                            }
                                            _ => false,
                                        };
                                        drop(main);
                                        if is_main {
                                            if let Some(f) = fid {
                                                *handle2.frame_id.lock().await = Some(f);
                                            }
                                            *handle2.execution_context_id.lock().await = Some(cx.to_string());
                                        }
                                    }
                                }
                                Some("Runtime.executionContextDestroyed") => {
                                    // If the context we hold just died, clear
                                    // it so evaluate waits for the successor.
                                    let dead = msg.pointer("/params/executionContextId").and_then(|v| v.as_str());
                                    let mut guard = handle2.execution_context_id.lock().await;
                                    if guard.as_deref() == dead {
                                        *guard = None;
                                    }
                                }
                                // Navigation wipes all contexts: drop the
                                // stale id so evaluate waits for the new one.
                                Some("Runtime.executionContextsCleared") => {
                                    *handle2.execution_context_id.lock().await = None;
                                }
                                _ => {}
                            }
                            if method == Some("Page.navigationCommitted") {
                                if let Some(fid) = msg.pointer("/params/frameId").and_then(|v| v.as_str()) {
                                    let mut main = handle2.main_frame_id.lock().await;
                                    match main.clone() {
                                        Some(m) if m == fid => {
                                            drop(main);
                                            *handle2.frame_id.lock().await = Some(fid.to_string());
                                        }
                                        None => {
                                            *main = Some(fid.to_string());
                                            drop(main);
                                            *handle2.frame_id.lock().await = Some(fid.to_string());
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            if method == Some("Browser.attachedToTarget") {
                                if let Some(sid) = msg.pointer("/params/sessionId").and_then(|v| v.as_str()) {
                                    *handle2.session_id.lock().await = Some(sid.to_string());
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Event storm overflowed the broadcast buffer;
                            // lifecycle events were lost. Drop the cached
                            // context so evaluate waits for the next
                            // executionContextCreated instead of firing at
                            // an id that may already be dead.
                            *handle2.execution_context_id.lock().await = None;
                            continue;
                        }
                        Err(_) => break,
                    }
                    let _ = &conn;
                }
            });
        }

        // If we have no context yet, navigate to about:blank to force one.
        if handle.execution_context_id.lock().await.is_none() {
            let _ = handle.navigate("about:blank").await;
            // Wait for the fresh context event.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while handle.execution_context_id.lock().await.is_none() {
                if std::time::Instant::now() > deadline {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        Ok(handle)
    }

    async fn pages(&self) -> Result<Vec<String>> {
        let result = self.conn.request("Browser.targets", serde_json::json!({})).await?;
        let targets = result
            .get("targets")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(targets
            .iter()
            .filter_map(|t| t.get("targetId").and_then(|i| i.as_str()))
            .map(|s| s.to_string())
            .collect())
    }

    async fn shutdown(&self) -> Result<()> {
        // Fire-and-forget close: Camoufox doesn't always send the
        // Browser.close response (known upstream quirk), so we don't wait.
        // kill() does the graceful SIGTERM dance.
        let conn = self.conn.clone();
        tokio::spawn(async move {
            let _ = conn.request("Browser.close", serde_json::json!({})).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        self.conn.kill().await;
        // Remove our ephemeral profile; leave user-provided dirs alone.
        if let Some(dir) = &self.ephemeral_profile {
            if std::fs::remove_dir_all(dir).is_err() {
                tracing::warn!(target: "ghostcloak::camoufox", "failed to clean profile {}", dir.display());
            }
        }
        Ok(())
    }
}

#[async_trait]
impl PageHandle for CamoufoxPage {
    async fn navigate(&self, url: &str) -> Result<()> {
        let sid = self.session_id().await?;
        let frame = self.frame_id.lock().await.clone().unwrap_or_default();
        // Navigating invalidates the execution context; drop it now so any
        // concurrent evaluate waits for the fresh one instead of firing at
        // a dead context id.
        *self.execution_context_id.lock().await = None;
        // Some sites (redirect chains) let the navigate response go missing;
        // the navigation itself still proceeds. Bound the wait and treat a
        // timeout as fire-and-forget rather than an error.
        let nav = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            self.conn.request_session(
                "Page.navigate",
                serde_json::json!({"frameId": frame, "url": url}),
                Some(&sid),
            ),
        )
        .await;
        let out = match nav {
            Ok(Ok(result)) => {
                let _ = result;
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(()), // response lost mid-redirect; navigation continues
        };
        // Wait (bounded) for the new page's context before handing control
        // back — callers (MCP page_open) fire evaluate right after.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while self.execution_context_id.lock().await.is_none() {
            if std::time::Instant::now() > deadline {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        out
    }

    async fn snapshot(&self) -> Result<PageSnapshot> {
        // Snapshot must self-heal: a mid-browse context loss (challenge
        // pages like Google /sorry kill the content channel) shouldn't
        // wedge the session. One retry via re-navigation to the current URL.
        for attempt in 0..2 {
            match self.snapshot_inner().await {
                Ok(s) => return Ok(s),
                Err(e) if attempt == 0 => {
                    tracing::debug!(target: "ghostcloak::camoufox", "snapshot failed ({e}); retrying via reload");
                    // Reload: recover context by re-navigating to the same URL.
                    if let Ok(u) = self.url().await {
                        if !u.is_empty() {
                            let _ = self.navigate(&u).await;
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    async fn click(&self, selector: &str) -> Result<()> {
        // Strategy 0: JS click for form controls. Split into two evaluates:
        // (1) locate the element — this response MUST come back;
        // (2) click it — if the click submits a form, the page starts
        //     navigating and the response can legitimately vanish mid-swap.
        // A vanished step-2 response after a located element = success.
        let locate = format!(
            "(() => {{ const el = document.querySelector({sel}); if (!el) return 'MISSING'; \
             const tag = el.tagName.toLowerCase(); \
             const isForm = tag === 'input' || tag === 'button' || el.closest('form') !== null; \
             return isForm ? 'FORM' : 'SKIP'; }})()",
            sel = serde_json::to_string(selector).unwrap_or_default()
        );
        let res = self.evaluate(&locate).await?;
        match res.as_str() {
            Some("FORM") => {
                let act = format!(
                    "(() => {{ document.querySelector({sel}).click(); return 'OK'; }})()",
                    sel = serde_json::to_string(selector).unwrap_or_default()
                );
                match self.evaluate(&act).await {
                    // Response made it back before the page swap.
                    Ok(_) => return Ok(()),
                    // Click fired a navigation that ate the response — the
                    // click itself landed. Same class as the missing
                    // Browser.close response quirk.
                    Err(e) => {
                        tracing::debug!(target: "ghostcloak::camoufox", "click response lost in nav (treated as success): {e}");
                        return Ok(());
                    }
                }
            }
            Some("SKIP") => { /* fall through to mouse events */ }
            _ => return Err(GhostError::PageOp("selector not found".into())),
        }

        // Strategy 1: real mouse events (best signal for anti-bot). If the
        // page's content channel dies mid-click (navigation), fall through
        // to strategy 2.
        let expr = format!(
            "(() => {{ const el = document.querySelector({sel}); if (!el) return null; \
             el.scrollIntoView({{block: 'center'}}); \
             const r = el.getBoundingClientRect(); \
             return JSON.stringify({{x: r.x + r.width/2, y: r.y + r.height/2, vw: window.innerWidth, vh: window.innerHeight}}); }})()",
            sel = serde_json::to_string(selector).unwrap_or_default()
        );
        let pos = self.evaluate(&expr).await;
        let pos: Option<(f64, f64)> = match pos {
            Ok(v) => v.as_str().and_then(|s| {
                serde_json::from_str::<serde_json::Value>(s)
                    .ok()
                    .and_then(|o| Some((o.get("x")?.as_f64()?, o.get("y")?.as_f64()?)))
            }),
            // Channel death during the coordinate probe = page transition:
            // jump straight to the JS-click fallback.
            Err(_) => None,
        };

        if let Some((x, y)) = pos {
            let mut dispatched = true;
            for ty in ["mousedown", "mouseup"] {
                let sid = {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                    loop {
                        match self.session_id().await {
                            Ok(s) => break s,
                            Err(e) if std::time::Instant::now() < deadline => {
                                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                continue;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                };
                match self
                    .conn
                    .request_session(
                        "Page.dispatchMouseEvent",
                        serde_json::json!({
                            "type": ty,
                            "button": 0,
                            "x": x,
                            "y": y,
                            "modifiers": 0,
                            "clickCount": 1,
                            "buttons": 1,
                        }),
                        Some(&sid),
                    )
                    .await
                {
                    Ok(_) => {}
                    // Click-triggered navigation can kill the channel mid-
                    // response; the click itself landed.
                    Err(_) if ty == "mouseup" => break,
                    Err(e) => {
                        dispatched = false;
                        let _ = e;
                        break;
                    }
                }
            }
            if dispatched {
                return Ok(());
            }
        }

        // Strategy 2: JS click — works through navigation churn and on
        // elements that hide from mouse-event hit testing.
        let js_click = format!(
            "(() => {{ const el = document.querySelector({sel}); if (!el) return 'MISSING'; el.click(); return 'OK'; }})()",
            sel = serde_json::to_string(selector).unwrap_or_default()
        );
        let res = self.evaluate(&js_click).await?;
        match res.as_str() {
            Some("OK") => Ok(()),
            Some("MISSING") | _ => Err(GhostError::PageOp("selector not found".into())),
        }
    }

    async fn type_text(&self, selector: &str, text: &str) -> Result<()> {
        // Focus via JS, not a mouse click: dispatching mouse events into
        // form fields has been observed to kill this Camoufox build's
        // content channel on some pages. (Mouse clicks stay available via
        // `click()` for links and buttons.)
        let focus = format!(
            "(() => {{ const el = document.querySelector({sel}); if (!el) return 'MISSING'; el.focus(); return 'OK'; }})()",
            sel = serde_json::to_string(selector).unwrap_or_default()
        );
        let res = self.evaluate(&focus).await?;
        if res.as_str() != Some("OK") {
            return Err(GhostError::PageOp("selector not found".into()));
        }
        let sid = self.session_id().await?;
        // Type per-character via key events — what a human produces, and
        // the point we can add cadence randomization later.
        let key = |c: char| -> (u32, String, String) {
            // (keyCode, code, key) for printable ASCII.
            let code = format!("Key{}", c.to_ascii_uppercase());
            (c.to_ascii_uppercase() as u32, code, c.to_string())
        };
        for ch in text.chars() {
            let spec = match ch {
                '\n' => Some(("Enter".to_string(), 13u32, "Enter".to_string())),
                '\r' => continue, // normalize CRLF: \n carries the Enter
                '\t' => Some(("Tab".to_string(), 9u32, "Tab".to_string())),
                ' ' => Some((" ".to_string(), 32u32, "Space".to_string())),
                _ => {
                    let (kc, code, k) = key(ch);
                    Some((k, kc, code))
                }
            };
            let Some((k, kc, code)) = spec else { continue };
            for ty in ["keydown", "keyup"] {
                let _ = self
                    .conn
                    .request_session(
                        "Page.dispatchKeyEvent",
                        serde_json::json!({
                            "type": ty,
                            "key": k,
                            "keyCode": kc,
                            "location": 0,
                            "code": code,
                            "repeat": false,
                        }),
                        Some(&sid),
                    )
                    .await;
            }
        }
        Ok(())
    }

    /// Press a named key (Enter, Tab, Escape, ArrowDown, ...) in the page.
    async fn press_key(&self, key_name: &str) -> Result<()> {
        let sid = self.session_id().await?;
        // Common named keys; anything else passes through as typed.
        let (code, keyc): (&str, u32) = match key_name {
            "Enter" => ("Enter", 13),
            "Tab" => ("Tab", 9),
            "Escape" => ("Escape", 27),
            "Backspace" => ("Backspace", 8),
            "Delete" => ("Delete", 46),
            "ArrowUp" => ("ArrowUp", 38),
            "ArrowDown" => ("ArrowDown", 40),
            "ArrowLeft" => ("ArrowLeft", 37),
            "ArrowRight" => ("ArrowRight", 39),
            "Home" => ("Home", 36),
            "End" => ("End", 35),
            "PageUp" => ("PageUp", 33),
            "PageDown" => ("PageDown", 34),
            _ => ("KeyUnknown", 0),
        };
        for ty in ["keydown", "keyup"] {
            self.conn
                .request_session(
                    "Page.dispatchKeyEvent",
                    serde_json::json!({
                        "type": ty,
                        "key": key_name,
                        "keyCode": keyc,
                        "location": 0,
                        "code": code,
                        "repeat": false,
                    }),
                    Some(&sid),
                )
                .await?;
        }
        Ok(())
    }


    async fn a11y_snapshot(&self) -> Result<Vec<ghostcloak_core::engine::A11yElement>> {
        let raw = self.evaluate(crate::a11y::WALK_JS).await?;
        let json: String = raw
            .as_str()
            .ok_or_else(|| GhostError::PageOp("a11y walk returned no data".into()))?
            .to_string();
        let els: Vec<ghostcloak_core::engine::A11yElement> =
            serde_json::from_str(&json).map_err(|e| GhostError::PageOp(format!("a11y parse: {e}")))?;
        Ok(els)
    }

    async fn read_ref_full(&self, r: &str) -> Result<String> {
        let out = self.evaluate(&crate::a11y::read_ref_full_js(r)).await?;
        let s = out.as_str().unwrap_or_default();
        if s == "STALE-REF" {
            return Err(GhostError::PageOp(format!("ref {r} is stale — rerun page_a11y")));
        }
        Ok(s.to_string())
    }

    async fn wait_for(&self, selector: &str, timeout_ms: u64) -> Result<bool> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        let expr = crate::a11y::wait_for_js(selector);
        while std::time::Instant::now() < deadline {
            match self.evaluate(&expr).await {
                Ok(v) if v.as_str() == Some("VISIBLE") => return Ok(true),
                _ => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
            }
        }
        Ok(false)
    }

    async fn click_ref(&self, r: &str) -> Result<()> {
        let out = self.evaluate(&crate::a11y::click_ref_js(r)).await?;
        match out.as_str() {
            Some("CLICKED") => Ok(()),
            Some("STALE-REF") => Err(GhostError::PageOp(format!("ref {r} is stale — rerun page_a11y"))),
            _ => Err(GhostError::PageOp(format!("click_ref({r}) unexpected result"))),
        }
    }

    async fn type_ref(&self, r: &str, text: &str) -> Result<()> {
        // Fire...
        let out = self.evaluate(&crate::a11y::type_ref_action_js(r, text)).await?;
        if out.as_str() == Some("STALE-REF") {
            return Err(GhostError::PageOp(format!("ref {r} is stale — rerun page_a11y")));
        }
        // ...let async editors (Lexical and friends) settle...
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        // ...then verify what actually landed.
        let check = self.evaluate(&crate::a11y::read_ref_js(r)).await?;
        let s = check.as_str().unwrap_or_default();
        if s == "STALE-REF" {
            return Err(GhostError::PageOp(format!("ref {r} went stale mid-type")));
        }
        let len: usize = s.strip_prefix("LEN:").and_then(|v| v.parse().ok()).unwrap_or(0);
        if len < text.chars().count() / 2 {
            return Err(GhostError::PageOp(format!(
                "type_ref({r}) landed {len} of {} chars",
                text.chars().count()
            )));
        }
        Ok(())
    }

    async fn screenshot(&self, full_page: bool) -> Result<Vec<u8>> {        use base64::Engine as _;
        let sid = self.session_id().await?;

        // The Juggler screenshot takes an explicit clip: viewport shots use
        // the window size, full-page shots measure the scrollable document.
        let dims_expr = if full_page {
            "JSON.stringify([Math.max(document.documentElement.scrollWidth, document.body ? document.body.scrollWidth : 0), Math.max(document.documentElement.scrollHeight, document.body ? document.body.scrollHeight : 0)])"
        } else {
            "JSON.stringify([window.innerWidth, window.innerHeight])"
        };
        let dims = self.evaluate(dims_expr).await?;
        let (w, h) = dims
            .as_str()
            .and_then(|s| {
                let inner = s.trim_matches('"');
                let parts: Vec<u32> = inner
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .filter_map(|p| p.trim().parse().ok())
                    .collect();
                if parts.len() == 2 { Some((parts[0], parts[1])) } else { None }
            })
            .unwrap_or((1280, 800));
        // Engine canvas caps: 32767px per side.
        let cap = 32767u32;
        let (w, h) = (w.min(cap), h.min(cap));

        let result = self
            .conn
            .request_session(
                "Page.screenshot",
                serde_json::json!({
                    "mimeType": "image/png",
                    "clip": { "x": 0, "y": 0, "width": w, "height": h },
                    // 1:1 pixels regardless of the identity's spoofed DPR.
                    "omitDeviceScaleFactor": true,
                }),
                Some(&sid),
            )
            .await?;
        let b64 = result
            .get("data")
            .and_then(|d| d.as_str())
            .ok_or_else(|| GhostError::Protocol("Page.screenshot returned no data".into()))?;
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| GhostError::Protocol(format!("screenshot base64: {e}")))
    }

    async fn evaluate(&self, expression: &str) -> Result<serde_json::Value> {
        // In Juggler, executionContextId is the "id-N" context id from
        // Runtime.executionContextCreated — NOT the mainframe-N frame id.
        // Contexts are recreated on navigation, so on a stale-id failure we
        // wait briefly for the pump to report the new one and retry once.
        for attempt in 0..4 {
            // Wait (bounded) for the pump to report a live context after a
            // navigation before firing the evaluate.
            let ctx = {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
                loop {
                    let ctx = self.execution_context().await.ok();
                    if ctx.is_some() || std::time::Instant::now() > deadline {
                        // Juggler also accepts the FRAME id ("mainframe-N")
                        // as the executionContextId — it resolves the
                        // frame's default context itself. Fall back to it
                        // when the "id-N" context never materializes (e.g.
                        // after insertText churn with no fresh event).
                        if ctx.is_some() {
                            break ctx;
                        }
                        break self.frame_id.lock().await.clone();
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
            };
            let ctx = match ctx {
                Some(c) => c,
                None => {
                    if attempt == 3 {
                        return Err(GhostError::PageOp("no execution context".into()));
                    }
                    continue;
                }
            };
            let sid = self.session_id().await?;
            tracing::debug!(target: "ghostcloak::camoufox", "evaluate attempt {attempt} ctx={ctx} sid={sid}");
            let result = match self
                .conn
                .request_session(
                    "Runtime.evaluate",
                    serde_json::json!({
                        "expression": expression,
                        "executionContextId": ctx,
                        "returnByValue": true,
                    }),
                    Some(&sid),
                )
                .await
            {
                Ok(r) => r,
                Err(e) if attempt < 3 => {
                    // Stale context (mid-navigation): clear the cached id so
                    // the next attempt waits for the pump to report the
                    // replacement instead of reusing the dead one.
                    *self.execution_context_id.lock().await = None;
                    tracing::debug!(target: "ghostcloak::camoufox", "evaluate ctx stale ({e}); cleared cache, retrying");
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => return Err(e),
            };
            if let Some(exc) = result.get("exceptionDetails") {
                return Err(GhostError::PageOp(format!(
                    "js exception: {}",
                    serde_json::to_string(exc).unwrap_or_default()
                )));
            }
            let value = result
                .pointer("/result/value")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if value.is_null() && attempt < 3 {
                // A null return on a stale context is indistinguishable from
                // a legitimately-null expression. Nudge: on the next attempt
                // try the frame id directly — Juggler resolves the frame's
                // current default context — before giving up entirely.
                tracing::debug!(target: "ghostcloak::camoufox", "null result on attempt {attempt}, retrying via frame id");
                if attempt >= 1 {
                    let fid = self.frame_id.lock().await.clone();
                    if let Some(f) = fid {
                        *self.execution_context_id.lock().await = Some(f);
                    }
                } else {
                    *self.execution_context_id.lock().await = None;
                }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                continue;
            }
            tracing::debug!(target: "ghostcloak::camoufox", "evaluate done attempt {attempt}: {value:?}");
            return Ok(value);
        }
        Err(GhostError::PageOp("evaluate: context never became ready".into()))
    }

    async fn url(&self) -> Result<String> {
        Ok(self
            .evaluate("location.href")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    async fn close(&self) -> Result<()> {
        // Juggler's page close is a Page.* method on the target session
        // (`Target.close` is not implemented by this juggler version).
        let sid = self.session_id().await?;
        self.conn
            .request_session(
                "Page.close",
                serde_json::json!({ "runBeforeUnload": false }),
                Some(&sid),
            )
            .await?;
        Ok(())
    }
}

impl Drop for CamoufoxEngine {
    fn drop(&mut self) {
        // Last-resort cleanup: if shutdown() never ran (caller panicked or
        // the channel died early), still remove the ephemeral profile and
        // SIGKILL any leftover browser process from our launch.
        if let Some(dir) = &self.ephemeral_profile {
            let _ = std::fs::remove_dir_all(dir);
        }
        let _ = self.conn.kill_now();
    }
}
