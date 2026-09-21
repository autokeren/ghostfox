//! v0.6.8 HCAPTCHA SOLVER — the generic image-challenge eyes.
//!
//! hCaptcha challenges live in a cross-origin iframe (our coordinate
//! clicks pass through). The challenge may be a 3x3 tile grid, a
//! pattern-break icon field, or any image-pick variant — so instead of
//! hardcoding layouts we ask a host vision model (Cloudflare Workers
//! AI GLM-5.3-flash) for the EXACT pixel coordinates of every element
//! to click, then click them with the humanized mouse. Works for any
//! image-challenge family (hCaptcha, reCAPTCHA hard mode, Arkose).
//!
//! Multi-round: hCaptcha chains 2-4 challenges; each round is solved
//! the same way and verified by the appearance of the response token
//! in the page's `h-captcha-response` field.

use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use std::collections::HashSet;

pub struct Glm {
    account: String,
    key: String,
}

impl Glm {
    pub fn new(account: String, key: String) -> Self {
        Self { account, key }
    }

    /// Ask the vision model: which pixels must be clicked?
    /// Returns (click_points, verify_point) in the crop's coordinates.
    pub async fn solve_challenge(&self, png: &[u8], w: u32, h: u32) -> Result<(Vec<(f64, f64)>, Option<(f64, f64)>)> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let prompt = format!(
            "Solve this captcha challenge ({w}x{h} pixels).\n\nRULE: your entire answer must be ONLY coordinate lines. No descriptions, no sentences.\n\nFor every element that must be clicked, output one line with its center pixel coordinates:\n<x> <y>\n\nIf there is a Verify or Submit button, output its center as the FINAL line:\nV <x> <y>\n\nExample answer:\n120 150\n240 320\nV 460 540"
        );
        let payload = serde_json::json!({
            "messages": [{"role": "user", "content": [
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                {"type": "text", "text": prompt}
            ]}],
            "max_tokens": 6000
        });
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/@cf/zai-org/glm-5.3-flash",
            self.account
        );
        let mut answers: Vec<String> = Vec::new();
        // Ensemble: 3 votes. The model misses tiles on individual calls,
        // so we cluster the points and keep those with >= 2 votes.
        for _ in 0..3 {
            let resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.key))
                .json(&payload)
                .send()
                .await
                .context("GLM request failed")?;
            let d: serde_json::Value = resp.json().await.context("GLM response parse")?;
            let msg = d
                .get("result")
                .and_then(|r| r.get("choices"))
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let content = msg
                .get("content")
                .and_then(|c| c.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string())
                .or_else(|| {
                    msg.get("reasoning_content")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            if !content.is_empty() {
                answers.push(content);
            }
        }
        if answers.is_empty() {
            return Err(anyhow!("vision model returned no answer"));
        }
        // Cluster votes: union of all parsed points, then majority (>=2)
        // within a 45px radius.
        let mut all_pts: Vec<(f64, f64)> = Vec::new();
        let mut all_verify: Vec<(f64, f64)> = Vec::new();
        for ans in &answers {
            let (c, v) = parse_clicks(ans, w as f64, h as f64);
            all_pts.extend(c);
            if let Some(vv) = v {
                all_verify.push(vv);
            }
        }
        let mut clusters: Vec<((f64, f64), usize)> = Vec::new();
        for p in all_pts {
            if let Some(c) = clusters.iter_mut().find(|((cx, cy), _)| (cx - p.0).abs() < 45.0 && (cy - p.1).abs() < 45.0) {
                let n = c.1 + 1;
                let k = n as f64;
                c.0 = (((c.0).0 * (k - 1.0) + p.0) / k, ((c.0).1 * (k - 1.0) + p.1) / k);
                c.1 = n;
            } else {
                clusters.push((p, 1));
            }
        }
        let clicks: Vec<(f64, f64)> = clusters
            .iter()
            .filter(|(_, n)| *n >= 2)
            .map(|(p, _)| *p)
            .collect();
        if clicks.is_empty() {
            return Err(anyhow!(
                "no click target got >=2 votes; answers: {}",
                answers.join(" || ")
            ));
        }
        let verify = if !all_verify.is_empty() {
            let (sx, sy): (f64, f64) = all_verify.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
            Some((sx / all_verify.len() as f64, sy / all_verify.len() as f64))
        } else {
            Some((w as f64 - 50.0, h as f64 - 30.0))
        };
        tracing::info!(target: "ghostcloak::mcp", "hcaptcha glm: clicks={:?} verify={:?}", clicks, verify);
        Ok((clicks, verify))
    }
}

/// Parse "<x> <y>" lines and a trailing "V <x> <y>" verify point.
/// Coordinates are clamped to the image bounds. Near-duplicate points
/// (within 30px) collapse into one.
pub fn parse_clicks(s: &str, w: f64, h: f64) -> (Vec<(f64, f64)>, Option<(f64, f64)>) {
    let mut clicks: Vec<(f64, f64)> = Vec::new();
    let mut verify: Option<(f64, f64)> = None;
    for line in s.lines() {
        let mut parts = line.split_whitespace();
        let (a, b, c) = (parts.next(), parts.next(), parts.next());
        let is_verify = matches!(a, Some(t) if t.eq_ignore_ascii_case("v") || t.contains("erif") || t.contains("erify") || t.contains("ubmit"));
        let (x_tok, y_tok) = if is_verify {
            (b, c)
        } else {
            (a, b)
        };
        let (Some(x_tok), Some(y_tok)) = (x_tok, y_tok) else { continue };
        let (Ok(x_f), Ok(y_f)) = (x_tok.parse::<f64>(), y_tok.parse::<f64>()) else { continue };
        let clamp = |v: f64, max: f64| v.clamp(2.0, max - 2.0);
        let pt = (clamp(x_f, w), clamp(y_f, h));
        if is_verify {
            verify = Some(pt);
        } else {
            let dup = clicks.iter().any(|(x, y)| (x - pt.0).abs() < 30.0 && (y - pt.1).abs() < 30.0);
            if !dup {
                clicks.push(pt);
            }
        }
    }
    (clicks, verify)
}

/// Distinct challenge-round fingerprint helper (unused placeholder for
/// future round-diff detection; kept for API stability).
pub fn _round_set(clicks: &[(f64, f64)]) -> HashSet<(i64, i64)> {
    clicks.iter().map(|(x, y)| (*x as i64 / 10, *y as i64 / 10)).collect()
}