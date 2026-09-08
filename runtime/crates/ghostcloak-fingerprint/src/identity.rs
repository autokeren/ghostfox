//! The identity schema. Everything an engine needs to present one coherent
//! persona to the web, serializable to/from TOML.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Identity {
    pub id: String,
    /// Human label, e.g. "work-laptop-chrome".
    pub label: String,
    pub platform: Platform,
    pub user_agent: String,
    pub locale: String,
    /// IANA timezone, must agree with `geo`.
    pub timezone: String,
    #[serde(default)]
    pub geo: Option<Geo>,
    pub screen: Screen,
    pub hardware: Hardware,
    #[serde(default)]
    pub webrtc: WebRtcPolicy,
    /// Extra JS-injectable values (canvas noise seed, audio offset, ...).
    #[serde(default)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Geo {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_m: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    #[default]
    Windows,
    MacOS,
    Linux,
    /// Android phones (Firefox on Android personas: portrait screens,
    /// ARM GPUs, mobile UAs, Android font stacks).
    Android,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    /// Device pixel ratio; must be plausible for `platform`.
    pub dpr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Hardware {
    /// navigator.hardwareConcurrency.
    pub cpu_cores: u32,
    /// navigator.deviceMemory (Chrome caps at 8).
    pub device_memory_gb: u32,
    /// GPU renderer string, e.g. "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 ...)".
    pub gpu_renderer: String,
    /// Platform-appropriate font list.
    pub fonts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebRtcPolicy {
    /// Leave WebRTC alone; real local IPs surface.
    #[default]
    PublicOnly,
    /// Spoof to a single candidate consistent with the proxy.
    Proxied,
    /// Disable WebRTC entirely.
    Disabled,
}

impl Identity {
    /// Load from a TOML file on disk.
    pub fn load_toml(path: &std::path::Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&raw)?)
    }

    /// Load the identity stored next to a profile dir (`identity.toml`), or
    /// generate + persist a fresh coherent one for brand-new profiles.
    pub fn load_or_generate(profile_dir: Option<&str>) -> std::io::Result<Self> {
        let dir = profile_dir.map(std::path::PathBuf::from);
        if let Some(dir) = &dir {
            let file = dir.join("identity.toml");
            if file.exists() {
                if let Ok(raw) = std::fs::read_to_string(&file) {
                    if let Ok(id) = toml::from_str::<Identity>(&raw) {
                        return Ok(id);
                    }
                }
            }
        }
        let fresh = crate::generator::generate(&crate::generator::GenerateOptions::default());
        if let Some(dir) = &dir {
            std::fs::create_dir_all(dir)?;
            if let Ok(toml_str) = toml::to_string_pretty(&fresh) {
                let _ = std::fs::write(dir.join("identity.toml"), toml_str);
            }
        }
        Ok(fresh)
    }

    /// Serialize to TOML (stable key order via BTreeMap extras).
    pub fn to_toml(&self) -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Stable content hash: two identities with the same persona collide.
    pub fn fingerprint_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let canon = serde_json::to_string(self).unwrap_or_default();
        let mut h = Sha256::new();
        h.update(canon.as_bytes());
        let out = h.finalize();
        out.iter().take(8).map(|b| format!("{b:02x}")).collect()
    }
}
