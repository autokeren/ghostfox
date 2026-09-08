//! End-to-end probe: navigate the Camoufox engine to a real URL, take a
//! snapshot, and dump both the JS surface and the extracted page text.

use std::time::Duration;

use ghostcloak_core::engine::{Engine, LaunchOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".into()))
        .with_writer(std::io::stderr)
        .init();

    let url = std::env::args().nth(1).unwrap_or_else(|| "https://example.com".into());

    let opts = LaunchOptions {
        headless: true,
        ..Default::default()
    };
    let engine = ghostcloak_camoufox::launch(&opts).await?;

    let identity = engine.identity();
    println!("identity: {} [{}] {:?} {}x{}", identity.label, identity.fingerprint_hash(), identity.platform, identity.screen.width, identity.screen.height);

    let page = engine.new_page(&Default::default()).await?;

    println!("navigating to {url} ...");
    page.navigate(&url).await?;

    // Give the page a moment to settle (load events, redirects).
    tokio::time::sleep(Duration::from_secs(4)).await;

    // 1. JS surface: what the page can see of us.
    let ua = page.evaluate("navigator.userAgent").await?;
    let tz = page.evaluate("Intl.DateTimeFormat().resolvedOptions().timeZone").await?;
    let cores = page.evaluate("navigator.hardwareConcurrency").await?;
    let lang = page.evaluate("navigator.language").await?;
    let webdriver = page.evaluate("String(navigator.webdriver)").await?;
    println!("--- JS surface ---");
    println!("ua:        {}", ua.as_str().unwrap_or("?"));
    println!("timezone:  {}", tz.as_str().unwrap_or("?"));
    println!("cores:     {}", cores.as_i64().unwrap_or(-1));
    println!("language:  {}", lang.as_str().unwrap_or("?"));
    println!("webdriver: {}", webdriver.as_str().unwrap_or("?"));

    // Debug: IIFE + querySelector — the exact shape click() uses.
    let probe1 = page.evaluate("(function(){ return document.title; })()").await?;
    println!("iife title: {:?}", probe1);
    let probe2 = page.evaluate("(function(){ const el = document.querySelector('input[type=text]'); return el ? 'FOUND' : 'NULL'; })()").await?;
    println!("querySelector input: {:?}", probe2);
    let probe3 = page.evaluate("JSON.stringify((function(){ const el = document.querySelector('input[type=text]'); if (!el) return null; const r = el.getBoundingClientRect(); return {x: r.x + r.width/2, y: r.y + r.height/2}; })())").await?;
    println!("click-shape pos: {:?}", probe3);

    // Exact expression shape that click() generates (arrow IIFE + format).
    let selector = "input[name=custname]";
    let expr = format!(
        "(() => {{ const el = document.querySelector({sel}); if (!el) return null; \
         const r = el.getBoundingClientRect(); return JSON.stringify({{x: r.x + r.width/2, y: r.y + r.height/2}}); }})()",
        sel = serde_json::to_string(selector).unwrap_or_default()
    );
    println!("--- generated click expr ---\n{expr}\n---");
    let probe4 = page.evaluate(&expr).await?;
    println!("click-expr result: {:?}", probe4);
    // And call the real click() through the trait.
    use ghostcloak_core::engine::PageHandle;
    match page.click(selector).await {
        Ok(_) => println!("REAL click(): OK"),
        Err(e) => println!("REAL click(): ERR {e}"),
    }

    // 2. Snapshot: what we give an LLM.
    let snap = page.snapshot().await?;
    println!("--- snapshot ---");
    println!("url:   {}", snap.url);
    println!("title: {}", snap.title.as_deref().unwrap_or("?"));
    println!("content[0..300]: {}", snap.content.chars().take(300).collect::<String>());

    engine.shutdown().await?;
    println!("--- clean shutdown ---");
    // Browser tasks (event pumps, pipe readers) hold the tokio runtime
    // open; exit deterministically for CLI use.
    std::process::exit(0);
}
