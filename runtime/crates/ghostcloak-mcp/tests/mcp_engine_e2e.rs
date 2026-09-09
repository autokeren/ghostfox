use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use serde_json::{json, Value};

fn start_server() -> Child {
    let binary = env!("CARGO_BIN_EXE_ghostcloak-mcp");
    Command::new(binary)
        .env("RUST_LOG", "off")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostcloak-mcp")
}

fn request(child: &mut Child, id: u64, method: &str, params: Value) -> Value {
    let stdin = child.stdin.as_mut().expect("server stdin");
    let request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
    writeln!(stdin, "{request}").expect("write request");
    stdin.flush().expect("flush request");

    let stdout = child.stdout.as_mut().expect("server stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    for _ in 0..100 {
        line.clear();
        let read = reader.read_line(&mut line).expect("read response");
        assert!(read > 0, "server closed before response");
        if let Ok(response) = serde_json::from_str::<Value>(&line) {
            if response.get("id") == Some(&json!(id)) {
                return response;
            }
        }
    }
    panic!("no response for {method}");
}

fn call(child: &mut Child, id: u64, name: &str, arguments: Value) -> Value {
    request(
        child,
        id,
        "tools/call",
        json!({"name": name, "arguments": arguments}),
    )
}

fn text(response: &Value) -> &str {
    if let Some(result) = response
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
    {
        return result;
    }
    panic!("tool error response: {response}")
}

#[test]
#[ignore = "requires GHOSTFOX_HOME and a built Ghostfox engine"]
fn mcp_engine_e2e_opens_and_interacts_with_local_page() {
    let Some(home) = std::env::var_os("GHOSTFOX_HOME") else {
        return;
    };
    assert!(
        Path::new(&home).join("ghostfox").is_file(),
        "GHOSTFOX_HOME does not contain ghostfox engine"
    );

    let fixture = tempfile::Builder::new()
        .prefix("ghostfox-e2e-")
        .suffix(".html")
        .tempfile()
        .expect("temp fixture");
    std::fs::write(
        &fixture,
        r#"<html><head><title>Ghostfox E2E</title></head><body>
            <label>Name<input id="name" name="name"></label>
            <button id="submit" onclick="document.title='submitted'">Submit</button>
        </body></html>"#,
    )
    .expect("write fixture");
    let url = format!("file://{}", fixture.path().display());

    let mut child = start_server();
    let _ = request(
        &mut child,
        1,
        "initialize",
        json!({
            "protocolVersion":"2025-06-18",
            "capabilities":{},
            "clientInfo":{"name":"engine-e2e","version":"1.0.0"}
        }),
    );
    let stdin = child.stdin.as_mut().expect("server stdin");
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .expect("write initialized");
    stdin.flush().expect("flush initialized");

    let session = call(&mut child, 2, "session_create", json!({}));
    let session_id = text(&session).trim().to_string();
    assert!(!session_id.is_empty());

    let page = call(
        &mut child,
        3,
        "page_open",
        json!({"session_id": session_id, "url": url}),
    );
    let page_id = text(&page).trim().to_string();
    assert!(!page_id.is_empty());
    std::thread::sleep(std::time::Duration::from_millis(500));

    let a11y = call(
        &mut child,
        4,
        "page_a11y",
        json!({"session_id": session_id, "page_id": page_id}),
    );
    let a11y_text = text(&a11y);
    assert!(a11y_text.contains("Ghostfox E2E"));
    assert!(a11y_text.contains("textbox"));
    assert!(a11y_text.contains("button"));

    let typed = call(
        &mut child,
        5,
        "page_type",
        json!({"session_id": session_id, "page_id": page_id, "selector":"#name", "text":"ghostfox"}),
    );
    assert_eq!(text(&typed), "ok");

    let value = call(
        &mut child,
        6,
        "page_eval",
        json!({"session_id": session_id, "page_id": page_id, "expression":"document.querySelector('#name').value"}),
    );
    assert_eq!(
        value
            .pointer("/result/content/0/text")
            .and_then(Value::as_str),
        Some("\"ghostfox\"")
    );

    let evidence = call(
        &mut child,
        7,
        "session_evidence",
        json!({"session_id": session_id}),
    );
    assert!(text(&evidence).contains("page_open"));
    assert!(text(&evidence).contains("page_a11y"));

    let _ = child.kill();
    let _ = child.wait();
}
