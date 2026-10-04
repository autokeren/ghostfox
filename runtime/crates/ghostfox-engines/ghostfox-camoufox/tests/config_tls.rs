use ghostfox_camoufox::config::identity_to_config;
use ghostfox_fingerprint::identity::TlsProfile;
use ghostfox_fingerprint::GenerateOptions;

#[test]
fn chrome_persona_emits_tls_keys() {
    let id = ghostfox_fingerprint::generate(&GenerateOptions {
        platform: Some(ghostfox_fingerprint::Platform::Windows),
        ..Default::default()
    });
    let cfg = identity_to_config(&id);
    if id.hardware.tls == TlsProfile::Chrome141 {
        assert!(cfg.contains_key("tls:groups"));
        assert!(cfg.contains_key("tls:cipherSuites"));
        eprintln!(
            "groups={:?} ciphers={:?}",
            cfg.get("tls:groups"),
            cfg.get("tls:cipherSuites")
        );
    }
}
