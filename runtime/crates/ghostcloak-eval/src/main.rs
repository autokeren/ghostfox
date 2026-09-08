//! ghostcloak-eval: the stealth referee.
//!
//! Modes:
//!   identity  — generate N identities, audit each, report violations (pure,
//!               offline, fast — this is the CI smoke test).
//!   web       — launch the engine, hit detector pages, capture signals.
//!
//! Scores are JSON-lines so they can be diffed between commits: the core
//! loop of the project is "change something -> run evals -> compare".

use std::sync::Arc;

use clap::{Parser, Subcommand};
use ghostcloak_core::engine::{Engine, LaunchOptions};
use ghostcloak_fingerprint::GenerateOptions;

#[derive(Parser, Debug)]
#[command(name = "ghostcloak-eval", version, about = "Stealth eval harness for ghostcloak")]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand, Debug)]
enum Mode {
    /// Generate N identities and audit them for coherence.
    Identity {
        #[arg(long, default_value_t = 20)]
        count: usize,
    },
    /// Launch the engine against a URL and dump observed JS signals.
    Web {
        url: String,
        /// Evaluate this JS expression and report its JSON result.
        #[arg(long)]
        expr: Option<String>,
        #[arg(long, default_value_t = true)]
        headless: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.mode {
        Mode::Identity { count } => eval_identity(count),
        Mode::Web { url, expr, headless } => eval_web(url, expr, headless).await,
    }
}

fn eval_identity(count: usize) -> anyhow::Result<()> {
    let mut violations_total = 0usize;
    for i in 0..count {
        let id = ghostcloak_fingerprint::generate(&GenerateOptions::default());
        let violations = ghostcloak_fingerprint::audit(&id);
        let score = if violations.is_empty() { "PASS" } else { "FAIL" };
        if !violations.is_empty() {
            violations_total += violations.len();
            eprintln!("[{i}] {score} {}", id.label);
            for v in &violations {
                eprintln!("    - {v}");
            }
        }
        println!(
            "{}",
            serde_json::json!({
                "i": i,
                "label": id.label,
                "platform": format!("{:?}", id.platform),
                "hash": id.fingerprint_hash(),
                "violations": violations.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
                "pass": violations.is_empty(),
            })
        );
    }
    eprintln!("— {count} identities, {violations_total} total violations —");
    if violations_total > 0 {
        std::process::exit(1);
    }
    Ok(())
}

async fn eval_web(url: String, expr: Option<String>, headless: bool) -> anyhow::Result<()> {
    let opts = LaunchOptions {
        headless,
        ..Default::default()
    };
    let engine = ghostcloak_chromium::launch(&opts).await?;
    let page = engine.new_page(&Default::default()).await?;

    page.navigate(&url).await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let default_exprs: Vec<(&str, String)> = vec![
        ("user_agent", "navigator.userAgent".into()),
        ("platform", "navigator.platform".into()),
        ("cores", "navigator.hardwareConcurrency".into()),
        ("memory", "navigator.deviceMemory".into()),
        ("languages", "JSON.stringify(navigator.languages)".into()),
        ("webdriver", "String(navigator.webdriver)".into()),
        ("screen", "JSON.stringify([screen.width, screen.height, screen.availHeight])".into()),
        ("dpr", "window.devicePixelRatio".into()),
        ("gpu", "(function(){try{var c=document.createElement('canvas');var g=c.getContext('webgl');var d=g.getExtension('WEBGL_debug_renderer_info');return g.getParameter(d.UNMASKED_RENDERER_WEBGL)}catch(e){return 'n/a'}})()".into()),
        ("tz", "Intl.DateTimeFormat().resolvedOptions().timeZone".into()),
        ("plugins", "navigator.plugins.length".into()),
    ];

    let exprs: Vec<(&str, String)> = if let Some(e) = expr {
        vec![("custom", e)]
    } else {
        default_exprs
    };

    let mut out = serde_json::Map::new();
    out.insert("url".into(), serde_json::json!(page.url().await?));
    for (name, e) in exprs {
        let val = page.evaluate(&e).await.unwrap_or(serde_json::Value::Null);
        out.insert(name.into(), val);
    }
    println!("{}", serde_json::to_string_pretty(&out)?);

    if let Some(engine) = Arc::downcast::<ghostcloak_chromium::ChromiumEngine>(engine.clone())
        .ok()
    {
        let _ = engine.shutdown().await;
    }
    Ok(())
}
