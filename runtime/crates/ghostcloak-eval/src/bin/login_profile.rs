//! login_profile: open a headful Ghostfox window with a persistent, coherent
//! identity — the same identity the MCP runtime will use later when driving
//! this profile, so login sessions and automation share one fingerprint.
//!
//! Usage:
//!   login_profile <profile_dir> [url]      (GHOSTFOX_HOME must be set)

use std::path::PathBuf;

use ghostcloak_fingerprint::identity::Identity;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let profile = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("usage: login_profile <profile_dir> [url]"))?;
    let url = args
        .next()
        .unwrap_or_else(|| "https://news.ycombinator.com".into());
    let home = std::env::var("GHOSTFOX_HOME")
        .map(PathBuf::from)
        .map_err(|_| anyhow::anyhow!("GHOSTFOX_HOME must point at the engine"))?;

    // Coherent identity, persisted at the profile root — the runtime picks
    // the very same one back up on session_create(profile_dir=...).
    let identity = Identity::load_or_generate(Some(profile.to_str().unwrap()))?;
    println!(
        "persona: {} [{:?}] {}x{} (identity.toml in {})",
        identity.label,
        identity.platform,
        identity.screen.width,
        identity.screen.height,
        profile.display()
    );

    // The runtime nests the browser profile under <dir>/camoufox-profile/
    // (identity.toml lives at the root, browser state in the subfolder) —
    // the login window must write to the exact same location.
    let browser_profile = profile.join("camoufox-profile");
    std::fs::create_dir_all(&browser_profile)?;
    println!("browser profile: {}", browser_profile.display());

    // Same CAMOU_CONFIG the runtime injects — the login session and every
    // later automated session present one device.
    let env = ghostcloak_camoufox::config::env_for_identity(&identity, &home);

    let bin = home.join("ghostfox-bin");
    let status = std::process::Command::new(&bin)
        .arg("-no-remote")
        .arg("-profile")
        .arg(&browser_profile)
        .arg(&url)
        .envs(env)
        .status()?;
    println!("engine exited: {status}");
    Ok(())
}
