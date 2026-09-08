//! ghostcloak-mcp: exposes the browser runtime to AI agents over MCP (stdio).

use rmcp::service::serve_server;
use rmcp::transport::stdio;
use rmcp::ServerHandler;

mod server;

use server::GhostcloakServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .with_writer(std::io::stderr)
        .init();

    let service = serve_server(GhostcloakServer::new(), stdio()).await?;
    service.waiting().await?;
    Ok(())
}
