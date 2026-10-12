//! ghostfox-mcp: exposes the browser runtime to AI agents over MCP.
//!
//! Transport: stdio by default; set `GHOSTFOX_HTTP_PORT` (e.g. "5000")
//! to serve the MCP Streamable HTTP transport on 127.0.0.1:<port> —
//! the mode remote agents + web MCP hosts connect to.

use rmcp::service::serve_server;
use rmcp::transport::stdio;
mod captcha;
mod ddddocr;
mod geetest;
mod hcaptcha;
mod liveview;
mod ocr;
mod receipts;
mod recipes;
mod recording;
mod server;
mod vision;

use server::GhostfoxServer;

/// Ghostfox Android gateway (no Termux/adb): the app DIALS OUT to
/// GHOSTFOX_ANDROID_GATEWAY (a `host:port` listen address); each inbound
/// phone connection is held in a FIFO and paired with the next local
/// connection (the runtime's spawn_tcp to the same port). A shared token
/// (GHOSTFOX_GATEWAY_TOKEN, sent as the first \0-terminated line by the
/// app) rejects strangers.
mod android_gateway {
    use std::sync::Arc;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    pub async fn run(bind: &str, token: Option<String>) -> anyhow::Result<()> {
        let listener = TcpListener::bind(bind).await?;
        tracing::info!(target: "ghostfox::gateway", "android gateway on {bind}");
        let pending: Arc<Mutex<Vec<tokio::net::TcpStream>>> = Arc::new(Mutex::new(vec![]));
        let token = Arc::new(token);
        loop {
            let (stream, peer) = listener.accept().await?;
            let pending2 = pending.clone();
            let token2 = token.clone();
            tokio::spawn(async move {
                let mut stream = stream;
                // Read the first \0-terminated line: "phone" or "runtime".
                let mut buf = Vec::new();
                let mut byte = [0u8; 1];
                while buf.len() < 256 {
                    if stream.read(&mut byte).await.unwrap_or(0) == 0 {
                        return;
                    }
                    if byte[0] == 0 {
                        break;
                    }
                    buf.push(byte[0]);
                }
                let greeting = String::from_utf8_lossy(&buf).to_string();
                let mut parts = greeting.splitn(3, ':');
                let role = parts.next().unwrap_or("");
                let sent_token = parts.next().unwrap_or("");
                if let Some(want) = token2.as_ref() {
                    if sent_token != want.as_str() {
                        tracing::warn!(target: "ghostfox::gateway", "rejected {peer}: bad token");
                        return;
                    }
                }
                if role == "phone" {
                    pending2.lock().await.push(stream);
                    tracing::info!(target: "ghostfox::gateway", "phone connected from {peer}, queued");
                } else if role == "runtime" {
                    let phone = pending2.lock().await.pop();
                    if phone.is_none() {
                        tracing::warn!(target: "ghostfox::gateway", "runtime connected but no phone queued");
                        return;
                    }
                    let phone = phone.unwrap();
                    tracing::info!(target: "ghostfox::gateway", "paired runtime <-> phone");
                    let (mut a_r, mut a_w) = stream.into_split();
                    let (mut b_r, mut b_w) = phone.into_split();
                    let ab = tokio::spawn(async move { tokio::io::copy(&mut a_r, &mut b_w).await });
                    let ba = tokio::spawn(async move { tokio::io::copy(&mut b_r, &mut a_w).await });
                    let _ = ab.await;
                    let _ = ba.await;
                }
            });
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .with_writer(std::io::stderr)
        .init();

    if let Ok(port) = std::env::var("GHOSTFOX_HTTP_PORT") {
        serve_http(&port).await?;
        return Ok(());
    }

    // Ghostfox Android gateway: hold inbound phone connections and pair
    // them with runtime connections (replaces Termux ssh -R entirely).
    if let Ok(bind) = std::env::var("GHOSTFOX_ANDROID_GATEWAY") {
        let token = std::env::var("GHOSTFOX_GATEWAY_TOKEN").ok();
        android_gateway::run(&bind, token).await?;
        return Ok(());
    }

    let service = serve_server(GhostfoxServer::new(), stdio()).await?;
    service.waiting().await?;
    Ok(())
}

/// MCP Streamable HTTP transport (stateful sessions, SSE keep-alive) —
/// the fork proved the demand: remote agents + web hosts can't speak
/// stdio. Bound to loopback by default; GHOSTFOX_HTTP_ADDR overrides.
async fn serve_http(port: &str) -> anyhow::Result<()> {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::Request;
    use axum::Router;
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService,
    };

    let addr = std::env::var("GHOSTFOX_HTTP_ADDR").unwrap_or_else(|_| "127.0.0.1".into());
    let listen = format!("{addr}:{port}");

    // Stateful mode: one server per session, kept alive across requests
    // (the browser session must survive the HTTP request/response cycle).
    let svc = StreamableHttpService::new(
        || Ok::<_, std::io::Error>(GhostfoxServer::new()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let app = Router::new().fallback(move |req: Request<Body>| {
        let svc = svc.clone();
        async move { svc.handle(req).await }
    });

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    tracing::info!(target: "ghostfox::http", "MCP Streamable HTTP listening on http://{listen}/mcp");
    axum::serve(listener, app).await?;
    Ok(())
}
