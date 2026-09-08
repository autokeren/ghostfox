//! Touch coherence probe: launch the engine with an Android persona and
//! verify the touch surface matches a real phone — maxTouchPoints, pointer
//! media queries, and the UA/platform class.

use std::time::Duration;

use ghostcloak_core::engine::{Engine, LaunchOptions};
use ghostcloak_fingerprint::{GenerateOptions, Platform};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let identity = ghostcloak_fingerprint::generate(&GenerateOptions {
        platform: Some(Platform::Android),
        webrtc: None,
    });

    // Persist the identity so the engine reads it back via profile dir.
    let profile = std::env::temp_dir().join("ghostfox-touch-probe");
    std::fs::create_dir_all(&profile)?;
    std::fs::write(
        profile.join("identity.toml"),
        identity.to_toml()?,
    )?;

    let opts = LaunchOptions {
        headless: true,
        profile_dir: Some(profile.to_str().unwrap().to_string()),
        ..Default::default()
    };
    // Debug: exactly what the engine will receive.
    let loaded = ghostcloak_fingerprint::Identity::load_or_generate(Some(
        profile.to_str().unwrap(),
    ))?;
    for (k, v) in ghostcloak_camoufox::config::env_for_identity(&loaded, std::path::Path::new(
        std::env::var("GHOSTFOX_HOME").as_deref().unwrap_or_default(),
    )) {
        if k.starts_with("CAMOU_CONFIG") {
            let cfg: serde_json::Value = serde_json::from_str(&v).unwrap_or_default();
            println!("ENV {k} navigator.maxTouchPoints = {:?}", cfg.get("navigator.maxTouchPoints"));
            println!("ENV {k} navigator.platform = {:?}", cfg.get("navigator.platform"));
        }
    }
    let engine = ghostcloak_camoufox::launch(&opts).await?;
    println!(
        "identity: {} [{:?}] {}x{} dpr={}",
        identity.label, identity.platform, identity.screen.width, identity.screen.height, identity.screen.dpr
    );

    let page = engine.new_page(&Default::default()).await?;
    page.navigate("https://example.com").await?;
    tokio::time::sleep(Duration::from_secs(4)).await;

    let probes = [
        ("maxTouchPoints", "navigator.maxTouchPoints"),
        ("pointer_coarse", "matchMedia('(pointer: coarse)').matches"),
        ("pointer_fine", "matchMedia('(pointer: fine)').matches"),
        ("hover_hover", "matchMedia('(hover: hover)').matches"),
        ("any_pointer_coarse", "matchMedia('(any-pointer: coarse)').matches"),
        ("touch_events", "'ontouchstart' in window || typeof TouchEvent !== 'undefined'"),
        ("ua", "navigator.userAgent"),
        ("platform", "navigator.platform"),
        ("screen", "JSON.stringify([screen.width, screen.height, window.devicePixelRatio])"),
    ];
    for (name, expr) in probes {
        let v = page.evaluate(expr).await?;
        println!("{name:18} = {}", serde_json::to_string(&v)?);
    }

    engine.shutdown().await?;
    let _ = std::fs::remove_dir_all(&profile);
    Ok(())
}
