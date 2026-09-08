//! Optional CAPTCHA solving hook.
//!
//! Ghostfox's philosophy is stealth-first (don't get challenged), but when a
//! flow hits a gate anyway, an external solver can be wired in. Configure:
//!
//!   GHOSTFOX_CAPTCHA_PROVIDER=2captcha
//!   GHOSTFOX_CAPTCHA_KEY=<api key>
//!
//! Nothing is enabled without both variables; no key material is recorded
//! in session evidence.

const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_WAIT: std::time::Duration = std::time::Duration::from_secs(150);

fn provider() -> Option<(&'static str, String)> {
    let key = std::env::var("GHOSTFOX_CAPTCHA_KEY").ok()?;
    let provider = std::env::var("GHOSTFOX_CAPTCHA_PROVIDER").unwrap_or_else(|_| "2captcha".into());
    match provider.as_str() {
        "2captcha" => Some(("2captcha", key)),
        _ => None,
    }
}

async fn http_get_json(url: &str) -> Result<serde_json::Value, String> {
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

/// Solve a Cloudflare Turnstile (or compatible) challenge for `sitekey` on
/// `pageurl`; returns the `cf-turnstile-response` token.
pub async fn solve_turnstile(sitekey: &str, pageurl: &str) -> Result<String, String> {
    let (_, key) = provider().ok_or("captcha solving not configured (set GHOSTFOX_CAPTCHA_PROVIDER + GHOSTFOX_CAPTCHA_KEY)")?;
    let client = reqwest::Client::new();

    // Submit.
    let submit: serde_json::Value = client
        .get("https://2captcha.com/in.php")
        .query(&[
            ("key", key.as_str()),
            ("method", "turnstile"),
            ("sitekey", sitekey),
            ("pageurl", pageurl),
            ("json", "1"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let request_id = submit
        .get("request")
        .and_then(|r| r.as_str())
        .ok_or(format!("solver rejected the task: {submit}"))?
        .to_string();

    // Poll.
    let deadline = std::time::Instant::now() + MAX_WAIT;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(POLL_INTERVAL).await;
        let res = http_get_json(&format!(
            "https://2captcha.com/res.php?key={key}&action=get&id={request_id}&json=1"
        ))
        .await?;
        match res.get("status").and_then(|s| s.as_i64()) {
            Some(1) => {
                return Ok(res
                    .get("request")
                    .and_then(|r| r.as_str())
                    .unwrap_or_default()
                    .to_string())
            }
            Some(0) => {
                // CAPCHA_NOT_READY → keep polling; anything else is fatal.
                let msg = res.get("request").and_then(|r| r.as_str()).unwrap_or("");
                if msg != "CAPCHA_NOT_READY" {
                    return Err(format!("solver error: {msg}"));
                }
            }
            _ => return Err(format!("unexpected solver response: {res}")),
        }
    }
    Err("solver timed out".into())
}

/// Solve an image captcha (base64 PNG) and return the text.
pub async fn solve_image(image_b64: &str) -> Result<String, String> {
    let (_, key) = provider().ok_or("captcha solving not configured (set GHOSTFOX_CAPTCHA_PROVIDER + GHOSTFOX_CAPTCHA_KEY)")?;
    let client = reqwest::Client::new();
    let submit: serde_json::Value = client
        .post("https://2captcha.com/in.php")
        .form(&[
            ("key", key.as_str()),
            ("method", "base64"),
            ("body", image_b64),
            ("json", "1"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let request_id = submit
        .get("request")
        .and_then(|r| r.as_str())
        .ok_or(format!("solver rejected the task: {submit}"))?
        .to_string();

    let deadline = std::time::Instant::now() + MAX_WAIT;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(POLL_INTERVAL).await;
        let res = http_get_json(&format!(
            "https://2captcha.com/res.php?key={key}&action=get&id={request_id}&json=1"
        ))
        .await?;
        match res.get("status").and_then(|s| s.as_i64()) {
            Some(1) => {
                return Ok(res
                    .get("request")
                    .and_then(|r| r.as_str())
                    .unwrap_or_default()
                    .to_string())
            }
            Some(0) => {
                let msg = res.get("request").and_then(|r| r.as_str()).unwrap_or("");
                if msg != "CAPCHA_NOT_READY" {
                    return Err(format!("solver error: {msg}"));
                }
            }
            _ => return Err(format!("unexpected solver response: {res}")),
        }
    }
    Err("solver timed out".into())
}
