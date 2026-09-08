//! Random-but-coherent identity generation.
//!
//! The rule the generator never breaks: every field of an identity is drawn
//! from *one* DevicePreset, then jittered only in ways that a real machine
//! could also vary (locale, timezone, exact screen size within the same
//! monitor class, WebRTC policy). Contradictions are impossible by
//! construction; the auditor double-checks anyway.

use rand::seq::IndexedRandom;
use rand::Rng;

use crate::identity::{Geo, Hardware, Identity, Platform, Screen, WebRtcPolicy};
use crate::presets::PRESETS;

/// Locale/timezone pairs that actually co-occur.
const LOCALE_ZONES: &[(&str, &str, f64, f64)] = &[
    ("en-US", "America/New_York", 40.7128, -74.0060),
    ("en-US", "America/Chicago", 41.8781, -87.6298),
    ("en-US", "America/Los_Angeles", 34.0522, -118.2437),
    ("en-US", "America/Denver", 39.7392, -104.9903),
    ("en-GB", "Europe/London", 51.5074, -0.1278),
    ("de-DE", "Europe/Berlin", 52.5200, 13.4050),
    ("fr-FR", "Europe/Paris", 48.8566, 2.3522),
    ("nl-NL", "Europe/Amsterdam", 52.3676, 4.9041),
    ("en-CA", "America/Toronto", 43.6532, -79.3832),
    ("en-AU", "Australia/Sydney", -33.8688, 151.2093),
];

pub struct GenerateOptions {
    /// Restrict to one platform (default: any).
    pub platform: Option<Platform>,
    /// Force a WebRTC policy (default: mostly PublicOnly, sometimes Proxied).
    pub webrtc: Option<WebRtcPolicy>,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self { platform: None, webrtc: None }
    }
}

/// Generate a fresh, coherent identity.
pub fn generate(opts: &GenerateOptions) -> Identity {
    let mut rng = rand::thread_rng();

    let pool: Vec<_> = PRESETS
        .iter()
        .filter(|p| opts.platform.map_or(true, |want| p.platform == want))
        .collect();
    let preset = pool.choose(&mut rng).expect("presets non-empty");

    // Locale/timezone/geo drawn as one unit so they cannot disagree.
    let (locale, tz, lat, lon) = LOCALE_ZONES.choose(&mut rng).copied().unwrap();

    let mut screen: Screen = preset.screen();
    if preset.platform == Platform::Android {
        // Mobile: system bars shave a few CSS pixels; never flip portrait.
        let height_delta = rng.random_range(0..=24);
        screen.height = screen.height.saturating_sub(height_delta);
    } else {
        // Taskbar/dock/multi-monitor variation: shrink the *available* viewport a
        // bit without touching the panel resolution class.
        let height_delta = rng.random_range(0..=40);
        screen.height = screen.height.saturating_sub(height_delta);
    }

    let hardware: Hardware = preset.hardware();

    let webrtc = opts.webrtc.unwrap_or(match rng.random_range(0..10) {
        0..=7 => WebRtcPolicy::PublicOnly,
        _ => WebRtcPolicy::Proxied,
    });

    Identity {
        id: format!("id-{}", util::short_id()),
        label: preset_label(preset.platform),
        platform: preset.platform,
        user_agent: preset.ua.to_string(),
        locale: locale.to_string(),
        timezone: tz.to_string(),
        geo: Some(Geo {
            latitude: lat,
            longitude: lon,
            accuracy_m: rng.random_range(20.0..120.0),
        }),
        screen,
        hardware,
        webrtc,
        extra: Default::default(),
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn preset_label(platform: Platform) -> String {
    let slug = match platform {
        Platform::Windows => "win-desktop",
        Platform::MacOS => "macbook",
        Platform::Linux => "linux-desktop",
        Platform::Android => "android-phone",
    };
    format!("{}-{}", slug, util::short_id())
}

mod util {
    use rand::Rng;
    /// Short id, mirrored from core to keep this crate self-contained.
    pub fn short_id() -> String {
        let mut rng = rand::thread_rng();
        (0..8)
            .map(|_| {
                let i = rng.random_range(0..36);
                char::from_digit(i, 36).unwrap_or('0')
            })
            .collect()
    }
}
