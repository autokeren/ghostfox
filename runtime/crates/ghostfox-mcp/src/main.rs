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
