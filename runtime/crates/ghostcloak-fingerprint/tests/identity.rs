use ghostcloak_fingerprint::{audit, generate, GenerateOptions, Identity, Platform, WebRtcPolicy};
use std::path::Path;

fn assert_clean(identity: &Identity) {
    let violations = audit(identity);
    assert!(
        violations.is_empty(),
        "unexpected violations: {:?}",
        violations
    );
}

#[test]
fn generates_coherent_identities_for_every_platform() {
    for platform in [
        Platform::Windows,
        Platform::MacOS,
        Platform::Linux,
        Platform::Android,
    ] {
        let identity = generate(&GenerateOptions {
            platform: Some(platform),
            webrtc: Some(WebRtcPolicy::PublicOnly),
        });
        assert_eq!(identity.platform, platform);
        assert!(!identity.id.is_empty());
        assert!(!identity.label.is_empty());
        assert!(!identity.user_agent.is_empty());
        assert!(!identity.hardware.fonts.is_empty());
        assert_clean(&identity);

        if platform == Platform::Android {
            assert!(identity.screen.height > identity.screen.width);
            assert!(identity.screen.dpr >= 2.0);
        } else {
            assert!(identity.screen.width > identity.screen.height);
        }
    }
}

#[test]
fn identity_toml_round_trips_and_fingerprints_stably() {
    let identity = generate(&GenerateOptions::default());
    let raw = identity.to_toml().expect("serialize identity");
    let decoded: Identity = toml::from_str(&raw).expect("deserialize identity");

    assert_eq!(decoded.id, identity.id);
    assert_eq!(decoded.label, identity.label);
    assert_eq!(decoded.platform, identity.platform);
    assert_eq!(decoded.fingerprint_hash(), identity.fingerprint_hash());
    assert_clean(&decoded);
}

#[test]
fn load_or_generate_persists_and_reuses_identity() {
    let dir = tempfile::tempdir().expect("temp dir");
    let profile_dir = dir.path().join("profile");
    let profile = profile_dir.to_string_lossy().to_string();

    let first = Identity::load_or_generate(Some(&profile)).expect("generate identity");
    assert!(Path::new(&profile).join("identity.toml").is_file());

    let second = Identity::load_or_generate(Some(&profile)).expect("load identity");
    assert_eq!(second.id, first.id);
    assert_eq!(second.fingerprint_hash(), first.fingerprint_hash());
    assert_clean(&first);
}

#[test]
fn auditor_rejects_contradictory_identities() {
    let mut identity = generate(&GenerateOptions {
        platform: Some(Platform::Windows),
        webrtc: Some(WebRtcPolicy::PublicOnly),
    });
    identity.user_agent = identity
        .user_agent
        .replace("Windows NT", "Android; Mobile;");
    identity.hardware.cpu_cores = 999;
    identity.hardware.device_memory_gb = 64;
    identity.screen.width = 0;
    identity.screen.height = 0;
    identity.hardware.gpu_renderer = "Metal (Apple M3)".into();
    identity.hardware.fonts.clear();

    let violations = audit(&identity);
    let fields: Vec<_> = violations.iter().map(|v| v.field).collect();
    assert!(fields.contains(&"user_agent"), "fields: {fields:?}");
    assert!(fields.contains(&"hardware.cpu_cores"), "fields: {fields:?}");
    assert!(
        fields.contains(&"hardware.device_memory_gb"),
        "fields: {fields:?}"
    );
    assert!(fields.contains(&"screen"), "fields: {fields:?}");
    assert!(
        fields.contains(&"hardware.gpu_renderer"),
        "fields: {fields:?}"
    );
    assert!(fields.contains(&"hardware.fonts"), "fields: {fields:?}");
}
