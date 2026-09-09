//! Identity → CAMOU_CONFIG translation.
//!
//! Maps our `Identity` (TOML, coherent by construction) onto Camoufox's
//! fingerprint config keys, using the schema the engine validates against
//! (`camoucfg.jvv` in the binary distribution).

use std::collections::BTreeMap;

use ghostcloak_fingerprint::identity::{Identity, Platform, WebRtcPolicy};

/// Build the CAMOU_CONFIG JSON map for an identity.
///
/// Keys left unset here are auto-filled by the engine with its own defaults,
/// so we only set what our identity actually decides.
pub fn identity_to_config(identity: &Identity) -> BTreeMap<String, serde_json::Value> {
    let mut cfg: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    // Navigator / UA — Firefox-style values derived from our identity.
    let ua = firefox_ua(identity);
    set(&mut cfg, "navigator.userAgent", ua.clone());
    set(&mut cfg, "navigator.appVersion", app_version(&ua));
    set(
        &mut cfg,
        "navigator.platform",
        platform_str(identity.platform),
    );
    set(&mut cfg, "navigator.oscpu", oscpu(identity.platform, &ua));
    set(
        &mut cfg,
        "navigator.hardwareConcurrency",
        identity.hardware.cpu_cores,
    );
    set(
        &mut cfg,
        "navigator.languages",
        vec![identity.locale.clone()],
    );
    set(&mut cfg, "navigator.language", identity.locale.clone());
    set(&mut cfg, "headers.User-Agent", ua);
    set(
        &mut cfg,
        "headers.Accept-Language",
        accept_language(&identity.locale),
    );

    // Android personas: touch points consistent with a real phone. The
    // engine patch reads this key in Navigator::MaxTouchPoints; combined
    // with the Juggler touch override (coarse pointer) the persona reads
    // like one coherent device.
    if identity.platform == Platform::Android {
        set(&mut cfg, "navigator.maxTouchPoints", 5);
    }

    // Screen.
    set(&mut cfg, "screen.width", identity.screen.width);
    set(&mut cfg, "screen.height", identity.screen.height);
    set(&mut cfg, "screen.availWidth", identity.screen.width);
    set(
        &mut cfg,
        "screen.availHeight",
        identity.screen.height.saturating_sub(40),
    );
    set(&mut cfg, "window.devicePixelRatio", identity.screen.dpr);

    // WebGL.
    set(&mut cfg, "webGl:vendor", "Mozilla");
    set(
        &mut cfg,
        "webGl:renderer",
        identity.hardware.gpu_renderer.as_str(),
    );

    // Locale/timezone/geo — one coherent unit.
    if let Some((lang, region)) = identity.locale.split_once('-') {
        set(&mut cfg, "locale:language", lang);
        set(&mut cfg, "locale:region", region);
    }
    set(&mut cfg, "timezone", identity.timezone.as_str());
    if let Some(geo) = &identity.geo {
        set(&mut cfg, "geolocation:latitude", geo.latitude);
        set(&mut cfg, "geolocation:longitude", geo.longitude);
        set(&mut cfg, "geolocation:accuracy", geo.accuracy_m);
    }

    // Fonts.
    set(&mut cfg, "fonts", identity.hardware.fonts.as_slice());
    // Deterministic per-identity font spacing seed.
    set(&mut cfg, "fonts:spacing_seed", spacing_seed(identity));

    // Canvas: per-identity anti-fingerprinting offsets, Camoufox-style.
    set(&mut cfg, "canvas:aaOffset", aa_offset(identity));
    set(&mut cfg, "canvas:aaCapOffset", true);

    // WebRTC policy.
    match identity.webrtc {
        WebRtcPolicy::PublicOnly => {}
        WebRtcPolicy::Proxied => {
            // Caller sets webrtc:ipv4 / ipv6 from the proxy exit IP.
        }
        WebRtcPolicy::Disabled => {
            // Handled at the prefs level by the engine adapter.
        }
    }

    cfg
}

fn set(
    cfg: &mut BTreeMap<String, serde_json::Value>,
    key: &str,
    value: impl Into<serde_json::Value>,
) {
    cfg.insert(key.to_string(), value.into());
}

/// Translate a Chromium-style identity UA to the Firefox equivalent, keeping
/// the platform truthful (a Firefox UA on the identity's platform class).
pub fn firefox_ua(identity: &Identity) -> String {
    let ff_ver = firefox_major(identity);
    match identity.platform {
        Platform::Windows => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:{ff_ver}.0) Gecko/20100101 Firefox/{ff_ver}.0"
        ),
        Platform::MacOS => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:{ff_ver}.0) Gecko/20100101 Firefox/{ff_ver}.0"
        ),
        Platform::Linux => format!(
            "Mozilla/5.0 (X11; Linux x86_64; rv:{ff_ver}.0) Gecko/20100101 Firefox/{ff_ver}.0"
        ),
        // The Android version tracks the desktop Firefox version in the
        // identity's UA; keep the engine's version in sync via
        // `firefox_major` so UA and `navigator.buildID`-adjacent signals agree.
        Platform::Android => {
            let android_ver = identity
                .user_agent
                .split("Android ")
                .nth(1)
                .and_then(|rest| rest.split(';').next())
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(14);
            format!(
                "Mozilla/5.0 (Android {android_ver}; Mobile; rv:{ff_ver}.0) Gecko/{ff_ver}.0 Firefox/{ff_ver}.0"
            )
        }
    }
}

/// The installed engine's Firefox major, read once from the binary's
/// `application.ini` (`Version=152.0.4-beta.30` → 152). Fallback when the
/// engine can't be probed.
pub const DEFAULT_FIREFOX_MAJOR: u32 = 135;

/// Read the engine's Firefox major from `<home>/application.ini`.
pub fn firefox_major_from_home(home: &std::path::Path) -> Option<u32> {
    let raw = std::fs::read_to_string(home.join("application.ini")).ok()?;
    let version = raw.lines().find(|l| l.starts_with("Version="))?;
    let major = version.trim_start_matches("Version=").split('.').next()?;
    major.parse().ok()
}

fn firefox_major(_identity: &Identity) -> u32 {
    // Prefer the engine's real version so UA, headers and buildID-adjacent
    // signals all agree with the binary in GHOSTFOX_HOME.
    if let Some(home) =
        std::env::var_os("GHOSTFOX_HOME").or_else(|| std::env::var_os("CAMOUFOX_HOME"))
    {
        if let Some(v) = firefox_major_from_home(std::path::Path::new(&home)) {
            return v;
        }
    }
    DEFAULT_FIREFOX_MAJOR
}

pub fn app_version(ua: &str) -> String {
    ua.trim_start_matches("Mozilla/").to_string()
}

pub fn platform_str(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "Win32",
        Platform::MacOS => "MacIntel",
        Platform::Linux => "Linux x86_64",
        // Firefox on Android reports a Linux ARM platform string.
        Platform::Android => "Linux aarch64",
    }
}

pub fn oscpu(platform: Platform, ua: &str) -> String {
    // navigator.oscpu drops the Gecko/Firefox trailer.
    let base = ua
        .split(") ")
        .next()
        .unwrap_or(ua)
        .trim_start_matches("Mozilla/5.0 (")
        .to_string();
    let base = format!("({base}");
    match platform {
        Platform::Windows => base,
        _ => base,
    }
}

pub fn accept_language(locale: &str) -> String {
    match locale.split_once('-') {
        Some((lang, _)) => format!("{locale},{lang};q=0.8,en;q=0.5"),
        None => format!("{locale},en;q=0.5"),
    }
}

/// Deterministic 31-bit seed from the identity's fingerprint hash.
fn spacing_seed(identity: &Identity) -> i64 {
    let h = identity.fingerprint_hash();
    let mut acc: i64 = 0;
    for ch in h.chars() {
        acc = acc.wrapping_mul(31).wrapping_add(ch as i64);
    }
    acc.rem_euclid(1_073_741_823)
}

fn aa_offset(identity: &Identity) -> i64 {
    spacing_seed(identity).rem_euclid(101) - 50
}

/// Env vars to pass to the Camoufox process for one identity.
pub fn env_for_identity(
    identity: &Identity,
    camoufox_home: &std::path::Path,
) -> std::collections::BTreeMap<String, String> {
    let mut env = std::collections::BTreeMap::new();

    let cfg = identity_to_config(identity);
    let json = serde_json::to_string(&cfg).unwrap_or_else(|_| "{}".into());
    // Linux chunk limit is 32767 chars per env var (same as the reference).
    let chunk_size = 32767;
    for (i, chunk) in json.as_bytes().chunks(chunk_size).enumerate() {
        env.insert(
            format!("CAMOU_CONFIG_{}", i + 1),
            String::from_utf8_lossy(chunk).to_string(),
        );
    }

    // Platform fontconfig shipped with the binary. Android personas reuse the
    // Linux font stack (the spoofed font list carries the persona).
    let ua_os = match identity.platform {
        Platform::Windows => "win",
        Platform::MacOS => "mac",
        Platform::Linux | Platform::Android => "lin",
    };
    let fc = camoufox_home.join("fontconfig").join(ua_os);
    env.insert(
        "FONTCONFIG_PATH".to_string(),
        fc.to_string_lossy().to_string(),
    );

    env
}
