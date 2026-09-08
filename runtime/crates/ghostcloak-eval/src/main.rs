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
    /// Live-target probe: hit a fixed target set, classify OK / GATED /
    /// BLOCKED, and check the JS surface against the identity. Results are
    /// JSONL (diffable between releases). Honesty note: run from a clean IP;
    /// datacenter IPs bias gate rates up.
    Targets {
        #[arg(long, default_value_t = false)]
        headless: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.mode {
        Mode::Identity { count } => eval_identity(count),
        Mode::Web { url, expr, headless } => eval_web(url, expr, headless).await,
        Mode::Targets { headless } => eval_targets(headless).await,
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

/// Live-target probe set: a mix of sanity pages, a classic detector panel
/// and a fingerprint referee (the same external judge the Web Scraping Club
/// benchmark uses).
const TARGETS: &[(&str, &str)] = &[
    ("example", "https://example.com"),
    ("httpbin", "https://httpbin.org/html"),
    ("sannysoft", "https://bot.sannysoft.com"),
    // Headless-detection referee (Vastel's areyouheadless).
    ("areyouheadless", "https://arh.antoinevastel.com/bots/areyouheadless"),
];

async fn eval_targets(headless: bool) -> anyhow::Result<()> {
    use ghostcloak_camoufox::CamoufoxEngine;

    let opts = LaunchOptions {
        headless,
        ..Default::default()
    };
    let engine = ghostcloak_camoufox::launch(&opts).await?;
    let engine = engine as Arc<dyn Engine>;

    let mut ok = 0;
    let mut gated = 0;
    let mut blocked = 0;

    for (name, url) in TARGETS {
        let page = engine.new_page(&Default::default()).await?;
        let nav = page.navigate(url).await;
        // Detectors and JSON viewers need a beat to render; 4s proved too
        // tight for bot.sannysoft.com from a datacenter IP.
        tokio::time::sleep(std::time::Duration::from_secs(9)).await;
        let title = String::new();
        // Body text: the main document first; iframe unwrapping only when
        // the main body is nearly empty (some sites render into a srcdoc
        // wrapper); JSON viewer pages fall back to textContent.
        let body = page
            .evaluate(
                "(function(){ var t = (document.body && document.body.innerText) || ''; \
                 if (t.trim().length < 40) { \
                   var f = document.querySelector('iframe'); \
                   if (f && f.contentDocument && f.contentDocument.body) \
                     t = f.contentDocument.body.innerText || ''; \
                 } \
                 if (t.trim().length < 40) t = (document.documentElement && document.documentElement.textContent) || ''; \
                 return t.slice(0, 8000); })()",
            )
            .await
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let final_url = page.url().await.unwrap_or_default();
        let nav_err = nav.err().map(|e| e.to_string());

        // Classify.
        let lower = body.to_lowercase();
        let status = if lower.contains("just a moment")
            || lower.contains("checking your browser")
            || lower.contains("attention required")
            || final_url.contains("/sorry/")
        {
            "gated"
        } else if body.trim().is_empty() && nav_err.is_some() {
            "blocked"
        } else if body.trim().len() < 40 {
            // Empty-ish body without a challenge → treat as blocked.
            "blocked"
        } else {
            "ok"
        };

        // Target-specific detail.
        let detail: serde_json::Value = match *name {
            "sannysoft" => {
                // Rows read "passed"/"failed"/"missing (passed)"; count both
                // cases (the site styles cells but innerText is lowercase).
                let lower = body.to_lowercase();
                let pass = lower.matches("passed").count();
                let fail = lower.matches("failed").count();
                let webdriver_leak = !lower.contains("webdriver") || lower.contains("missing (passed)");
                serde_json::json!({ "pass_rows": pass, "fail_rows": fail, "webdriver_clean": webdriver_leak })
            }
            "areyouheadless" => {
                let lower = body.to_lowercase();
                let headless = lower.contains("headless");
                serde_json::json!({
                    "says_headless": headless,
                    "snippet": body.lines().find(|l| !l.trim().is_empty()).unwrap_or_default(),
                })
            }
            _ => serde_json::Value::Null,
        };

        match status {
            "ok" => ok += 1,
            "gated" => gated += 1,
            _ => blocked += 1,
        }
        println!(
            "{}",
            serde_json::json!({
                "target": name,
                "url": url,
                "status": status,
                "final_url": final_url,
                "nav_err": nav_err,
                "title": title,
                "detail": detail,
            })
        );
        let _ = page.close().await;
    }

    eprintln!("— {ok} ok / {gated} gated / {blocked} blocked of {} targets —", TARGETS.len());
    let _ = engine.shutdown().await;
    Ok(())
}

async fn eval_web(url: String, expr: Option<String>, headless: bool) -> anyhow::Result<()> {    let opts = LaunchOptions {
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
