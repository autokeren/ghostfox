//! Manual probe: launch Camoufox with a generated identity, dump the JS
//! surface it presents. Usage: cargo run -p ghostcloak-eval --bin camoufox_probe

use std::sync::Arc;

use ghostcloak_core::engine::{Engine, LaunchOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "ghostcloak=trace".into()))
        .with_writer(std::io::stderr)
        .init();

    let opts = LaunchOptions {
        headless: true,
        ..Default::default()
    };
    let engine = ghostcloak_camoufox::launch(&opts).await?;

    // Report which identity we're presenting.
    let identity = engine.identity();
    println!(
        "identity: {} [{}] ua-ish platform {:?}, {}x{}@{:.1}, {} cores",
        identity.label,
        identity.fingerprint_hash(),
        identity.platform,
        identity.screen.width,
        identity.screen.height,
        identity.screen.dpr,
        identity.hardware.cpu_cores,
    );

    let page = engine.new_page(&Default::default()).await?;
    page.navigate("about:blank").await?;
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let exprs: &[(&str, &str)] = &[
        ("userAgent", "navigator.userAgent"),
        ("appVersion", "navigator.appVersion"),
        ("platform", "navigator.platform"),
        ("oscpu", "navigator.oscpu"),
        ("hardwareConcurrency", "navigator.hardwareConcurrency"),
        ("languages", "JSON.stringify(navigator.languages)"),
        ("screen", "JSON.stringify([screen.width, screen.height, screen.availWidth, screen.availHeight])"),
        ("devicePixelRatio", "window.devicePixelRatio"),
        ("timezone", "Intl.DateTimeFormat().resolvedOptions().timeZone"),
        ("webdriver", "String(navigator.webdriver)"),
        ("gpu", "(function(){try{var c=document.createElement('canvas');var g=c.getContext('webgl');var d=g.getExtension('WEBGL_debug_renderer_info');return g.getParameter(d.UNMASKED_RENDERER_WEBGL)}catch(e){return 'n/a'}})()"),
        ("plugins", "navigator.plugins.length"),
        ("hardwareConcurrencyCheck", "navigator.hardwareConcurrency"),
    ];

    let mut out = serde_json::Map::new();
    for (name, expr) in exprs {
        let val = page.evaluate(expr).await.unwrap_or(serde_json::Value::Null);
        out.insert((*name).to_string(), val);
    }
    println!("{}", serde_json::to_string_pretty(&out)?);

    let _ = engine.shutdown().await;
    let _ = Arc::downcast::<ghostcloak_camoufox::CamoufoxEngine>(Arc::new(())).is_ok();
    Ok(())
}
