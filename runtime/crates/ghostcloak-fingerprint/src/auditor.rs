//! The auditor: rejects identities that would embarrass us in front of a
//! detector. Every rule here encodes a *coherence* requirement — signals that
//! contradict each other are the #1 giveaway of a spoofed browser.

use crate::identity::{Identity, Platform, WebRtcPolicy};

/// One validation finding.
#[derive(Debug)]
pub struct Violation {
    pub field: &'static str,
    pub reason: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.reason)
    }
}

/// Audit an identity; empty vec = clean.
pub fn audit(identity: &Identity) -> Vec<Violation> {
    let mut v = Vec::new();

    // UA string must agree with the platform.
    let ua = &identity.user_agent;
    let ua_platform = if ua.contains("Windows NT") {
        Some(Platform::Windows)
    } else if ua.contains("Macintosh") {
        Some(Platform::MacOS)
    } else if ua.contains("X11; Linux") || ua.contains("Linux x86_64") {
        Some(Platform::Linux)
    } else {
        None
    };
    match ua_platform {
        Some(p) if p != identity.platform => v.push(Violation {
            field: "user_agent",
            reason: format!("UA says {p:?} but identity platform is {:?}", identity.platform),
        }),
        None => v.push(Violation {
            field: "user_agent",
            reason: "UA platform substring not recognized".into(),
        }),
        _ => {}
    }

    // Cores must be plausible for the platform class.
    if !(2..=24).contains(&identity.hardware.cpu_cores) {
        v.push(Violation {
            field: "hardware.cpu_cores",
            reason: format!("{} cores is not a plausible consumer machine", identity.hardware.cpu_cores),
        });
    }

    // Chrome caps navigator.deviceMemory at 8; anything else is a tell.
    if identity.hardware.device_memory_gb > 8 {
        v.push(Violation {
            field: "hardware.device_memory_gb",
            reason: "Chrome reports deviceMemory capped at 8; higher values are spoofed-looking".into(),
        });
    }

    // DPR and resolutions per platform class.
    let (w, h, dpr) = (
        identity.screen.width,
        identity.screen.height,
        identity.screen.dpr,
    );
    if identity.platform == Platform::MacOS && dpr < 2.0 {
        v.push(Violation {
            field: "screen.dpr",
            reason: "modern MacBooks report dpr >= 2.0".into(),
        });
    }
    if identity.platform == Platform::Windows && w == 1920 && dpr != 1.0 && dpr != 1.25 {
        v.push(Violation {
            field: "screen.dpr",
            reason: "1080p Windows desktops are overwhelmingly dpr 1.0 or 1.25".into(),
        });
    }
    if w == 0 || h == 0 || h >= w {
        v.push(Violation {
            field: "screen",
            reason: "landscape screens only; w must exceed h and both be nonzero".into(),
        });
    }

    // GPU string must mention the platform-appropriate backend.
    let gpu_ok = match identity.platform {
        Platform::Windows => identity.hardware.gpu_renderer.contains("Direct3D11") || identity.hardware.gpu_renderer.contains("D3D11"),
        Platform::MacOS => identity.hardware.gpu_renderer.contains("Metal") || identity.hardware.gpu_renderer.contains("ANGLE (Apple"),
        Platform::Linux => identity.hardware.gpu_renderer.contains("OpenGL") || identity.hardware.gpu_renderer.contains("Mesa") || identity.hardware.gpu_renderer.contains("ANGLE ("),
    };
    if !gpu_ok {
        v.push(Violation {
            field: "hardware.gpu_renderer",
            reason: "GPU renderer string inconsistent with platform".into(),
        });
    }

    // Timezone must resolve; geo must exist when WebRTC is proxied (we need
    // coordinates consistent with the network path).
    if identity.webrtc == WebRtcPolicy::Proxied && identity.geo.is_none() {
        v.push(Violation {
            field: "webrtc",
            reason: "Proxied WebRTC requires geo coordinates".into(),
        });
    }

    // Fonts must be non-empty and platform-flavored.
    if identity.hardware.fonts.is_empty() {
        v.push(Violation {
            field: "hardware.fonts",
            reason: "empty font list enumerates as headless/robotic".into(),
        });
    }

    v
}
