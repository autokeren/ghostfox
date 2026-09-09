use ghostcloak_core::engine::PageSnapshot;
use ghostcloak_mcp::recording::Recorder;
use serde_json::json;

#[test]
fn recorder_writes_append_only_evidence() {
    let root = tempfile::tempdir().expect("temp dir");
    let recorder = Recorder::new(root.path().to_path_buf());

    recorder
        .record(
            "session_1",
            "page_open",
            Some("page_1"),
            json!({"url": "about:blank"}),
        )
        .expect("record page_open");
    recorder
        .record(
            "session_1",
            "page_a11y",
            Some("page_1"),
            json!({"elements": 2}),
        )
        .expect("record page_a11y");
    recorder
        .record_identity("session_1", "id = \"test\"\n")
        .expect("record identity");

    let snapshot = PageSnapshot {
        url: "https://example.test".into(),
        title: Some("Example".into()),
        content: "Example body".into(),
        captured_at: chrono::Utc::now(),
    };
    let snapshot_path = recorder
        .record_snapshot("session_1", "page_1", &snapshot)
        .expect("record snapshot");
    assert!(snapshot_path.is_file());

    let evidence = recorder.evidence("session_1").expect("read evidence");
    assert_eq!(evidence["event_count"], json!(3));
    assert_eq!(evidence["events"][0]["tool"], json!("page_open"));
    assert_eq!(evidence["events"][1]["tool"], json!("page_a11y"));
    assert_eq!(evidence["events"][2]["tool"], json!("page_snapshot"));
    assert_eq!(evidence["events"][0]["page_id"], json!(Some("page_1")));
    assert!(evidence["identity_toml"]
        .as_str()
        .unwrap()
        .contains("identity.toml"));
    assert_eq!(evidence["snapshots"].as_array().unwrap().len(), 1);
}

#[test]
fn recorder_sanitizes_session_and_page_ids() {
    let root = tempfile::tempdir().expect("temp dir");
    let recorder = Recorder::new(root.path().to_path_buf());
    recorder
        .record("../escape", "page_open", None, json!({}))
        .expect("record event");

    let events = root.path().join("escape").join("events.jsonl");
    assert!(events.is_file());
    let evidence = recorder.evidence("../escape").expect("read evidence");
    assert_eq!(evidence["dir"], json!(root.path().join("escape")));
}
