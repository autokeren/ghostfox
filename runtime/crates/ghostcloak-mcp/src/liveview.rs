//! Optional live view: an opt-in HTTP server for humans watching agents work.
//!
//! Enable with `GHOSTFOX_LIVE_VIEW_PORT=<port>` on the MCP server. Each page
//! keeps its latest screenshot in memory; the server serves a small index
//! plus per-page PNGs, and a background ticker re-captures every 5 seconds
//! while at least one viewer page is open is NOT tracked (keep it simple:
//! capture while the server runs).
//!
//! Routes:
//!   GET /                      — index of sessions/pages (auto-refresh)
//!   GET /shot/<session>/<page> — latest PNG for that page

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use ghostcloak_core::session::Session;

#[derive(Clone)]
#[allow(dead_code)]
struct Shot {
    png: Arc<Vec<u8>>,
    ts: String,
    url: String,
}

fn store() -> &'static Mutex<HashMap<String, Shot>> {
    static STORE: OnceLock<Mutex<HashMap<String, Shot>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(session: &str, page: &str) -> String {
    format!("{session}/{page}")
}

/// Record the newest screenshot for a page (from the `page_screenshot` tool
/// or the auto-capture ticker).
pub fn update(session: &str, page: &str, url: &str, png: Vec<u8>) {
    store().lock().unwrap().insert(
        key(session, page),
        Shot {
            png: Arc::new(png),
            ts: chrono::Utc::now().to_rfc3339(),
            url: url.to_string(),
        },
    );
}

/// Start the live view server + auto-capture ticker (idempotent).
pub fn start_if_configured(state: Arc<tokio::sync::RwLock<crate::server::ServerState>>) {
    let Some(port) = std::env::var("GHOSTFOX_LIVE_VIEW_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
    else {
        return;
    };
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    tokio::spawn(async move {
        // Auto-capture ticker: every 5s, refresh the latest shot of every
        // page of every session.
        let ticker_state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let sessions: Vec<Arc<Session>> = {
                    ticker_state
                        .read()
                        .await
                        .sessions
                        .values()
                        .cloned()
                        .collect()
                };
                for session in sessions {
                    let pages = session.page_ids().await;
                    for pid in pages {
                        if let Ok(page) = session.page(&pid).await {
                            if let Ok(png) = page.screenshot(false).await {
                                let url = page.url().await.unwrap_or_default();
                                update(&session.id, &pid, &url, png);
                            }
                        }
                    }
                }
            }
        });

        let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("live view disabled: cannot bind {port}: {e}");
                return;
            }
        };
        tracing::info!("live view listening on http://127.0.0.1:{port}");
        loop {
            let (mut stream, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => continue,
            };
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();

                let (status, ctype, body): (&str, &str, Vec<u8>) = if path == "/" {
                    let mut html = String::from(
                        "<!doctype html><html><head><meta charset=utf-8><meta http-equiv=refresh content=2>\
                         <title>Ghostfox live view</title><style>body{background:#111;color:#ddd;font-family:sans-serif}\
                         img{max-width:100%;border:1px solid #444;margin:8px}\
                         a{color:#a78bfa;text-decoration:none;display:block;padding:4px}</style></head><body><h2>Ghostfox live view</h2>",
                    );
                    let keys: Vec<String> = store().lock().unwrap().keys().cloned().collect();
                    for k in keys {
                        html.push_str(&format!("<a href=\"/shot/{k}\">{k}</a><br>",));
                    }
                    html.push_str("</body></html>");
                    ("200 OK", "text/html; charset=utf-8", html.into_bytes())
                } else if let Some(rest) = path.strip_prefix("/shot/") {
                    match store().lock().unwrap().get(rest).cloned() {
                        Some(shot) => ("200 OK", "image/png", (*shot.png).clone()),
                        None => ("404 Not Found", "text/plain", b"no such page".to_vec()),
                    }
                } else {
                    ("404 Not Found", "text/plain", b"not found".to_vec())
                };
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.write_all(&body).await;
            });
        }
    });
}
