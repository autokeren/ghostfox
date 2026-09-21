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
use std::path::PathBuf;
use std::sync::OnceLock;
use image::imageops::FilterType;
use rten::Model;
use rten_tensor::Layout as _;

/// COCO-80 classes. hCaptcha's object categories map onto these almost
/// 1:1 (bus, car, boat, traffic light, animals...) — the split-labor
/// trick: GLM reads the instruction (text), YOLO does the detection.
const COCO: [&str; 80] = [
    "person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck",
    "boat", "traffic light", "fire hydrant", "stop sign", "parking meter", "bench",
    "bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra",
    "giraffe", "backpack", "umbrella", "handbag", "tie", "suitcase", "frisbee",
    "skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove",
    "skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup",
    "fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange",
    "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "couch",
    "potted plant", "bed", "dining table", "toilet", "tv", "laptop", "mouse",
    "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink",
    "refrigerator", "book", "clock", "vase", "scissors", "teddy bear",
    "hair drier", "toothbrush",
];

fn coco_path() -> PathBuf {
    let home = std::env::var("GHOSTFOX_HOME").unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|d| d.join(".ghostfox"))
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".ghostfox".into())
    });
    PathBuf::from(home).join("models").join("coco").join("yolov8n.onnx")
}

static COCO_MODEL: OnceLock<Option<Model>> = OnceLock::new();

fn coco_model() -> Option<&'static Model> {
    COCO_MODEL
        .get_or_init(|| {
            let p = coco_path();
            if !p.exists() {
                tracing::warn!(target: "ghostcloak::mcp", "COCO model missing at {}", p.display());
                return None;
            }
            std::fs::read(&p).ok().and_then(|b| Model::load(b).ok())
        })
        .as_ref()
}

fn singular(w: &str) -> &str {
    w.strip_suffix("es").unwrap_or_else(|| w.strip_suffix('s').unwrap_or(w))
}

/// Map instruction words to COCO class indices (plural-tolerant).
pub fn coco_indices(words: &[String]) -> Vec<usize> {
    let mut out = Vec::new();
    for w in words {
        let lw = w.trim().to_lowercase();
        let s = singular(&lw);
        for (i, c) in COCO.iter().enumerate() {
            if *c == lw || *c == s || lw.contains(c) {
                if !out.contains(&i) {
                    out.push(i);
                }
            }
        }
    }
    out
}

/// Detect target COCO classes in a PNG. Returns box centers in the
/// ORIGINAL image's pixel coordinates, best-confidence first.
pub fn coco_detect(png: &[u8], targets: &[usize], min_conf: f32) -> Result<Vec<(f64, f64, f64)>> {
    let model = coco_model().ok_or_else(|| anyhow!("COCO model unavailable"))?;
    let img = image::load_from_memory(png).context("decoding for COCO")?;
    let rgb = img.to_rgb8();
    let (ow, oh) = (rgb.width() as f64, rgb.height() as f64);
    let resized = image::imageops::resize(&rgb, 640, 640, FilterType::Triangle);
    let (w, h) = (640usize, 640usize);
    let pixels = resized.as_raw();
    let mut chw = vec![0f32; 3 * w * h];
    for (i, px) in pixels.chunks(3).enumerate() {
        let x = i % w;
        let y = i / w;
        let base = w * h;
        chw[0 * base + y * w + x] = px[0] as f32 / 255.0;
        chw[1 * base + y * w + x] = px[1] as f32 / 255.0;
        chw[2 * base + y * w + x] = px[2] as f32 / 255.0;
    }
    let input = rten::Value::from_shape(&[1usize, 3, h, w], chw).context("coco input tensor")?;
    let in_id = model.input_ids()[0];
    let out_id = model.output_ids()[0];
    let outputs = model
        .run(vec![(in_id, input.into())], &[out_id], None)
        .context("COCO inference")?;
    let out = outputs[0].as_view();
    let rten::ValueView::FloatTensor(tv) = &out else {
        return Err(anyhow!("COCO output not float"));
    };
    let n = tv.size(2); // 8400 anchors
    let data = tv.data().context("coco output data")?;
    let xf = ow / 640.0;
    let yf = oh / 640.0;
    // collect (cx, cy, score)
    let mut hits: Vec<(f64, f64, f64)> = Vec::new();
    for i in 0..n {
        let x = data[i];
        let y = data[n + i];
        let bw = data[2 * n + i];
        let bh = data[3 * n + i];
        let mut best = 0f32;
        let mut best_cls = 0usize;
        for (k, t) in targets.iter().enumerate() {
            let s = data[(4 + t) * n + i];
            if s > best {
                best = s;
                best_cls = k;
            }
        }
        let _ = best_cls;
        if best >= min_conf {
            let cx = (x as f64) * xf;
            let cy = (y as f64) * yf;
            hits.push((cx, cy, best as f64));
        }
    }
    // greedy NMS
    hits.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let mut kept: Vec<(f64, f64, f64)> = Vec::new();
    for h3 in hits {
        if kept
            .iter()
            .all(|(kx, ky, _): &(f64, f64, f64)| ((kx - h3.0).powi(2) + (ky - h3.1).powi(2)).sqrt() > 50.0)
        {
            kept.push(h3);
        }
    }
    Ok(kept)
}

/// Is the challenge content rendered yet? Low center-variance = blank.
pub fn is_rendered(png: &[u8]) -> bool {
    let img = match image::load_from_memory(png) {
        Ok(i) => i,
        Err(_) => return false,
    };
    let g = img.to_luma8();
    let (w, h) = g.dimensions();
    if w < 10 || h < 10 {
        return false;
    }
    // center patch
    let x0 = w / 4;
    let y0 = h / 3;
    let patch = image::imageops::crop_imm(&g, x0, y0, w / 2, h / 3).to_image();
    let vals: Vec<f64> = patch.pixels().map(|p| p.0[0] as f64).collect();
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64;
    var > 250.0 // std > ~16
}

pub struct Glm {
    account: String,
    key: String,
}

impl Glm {
    pub fn new(account: String, key: String) -> Self {
        Self { account, key }
    }

    /// Ask the vision model ONLY for the instruction text (its strength),
    /// then let COCO YOLO do detection+localization (its strength).
    /// Returns (click_points, verify_point) in the crop's coordinates.
    pub async fn solve_challenge(&self, png: &[u8], w: u32, h: u32) -> Result<(Vec<(f64, f64)>, Option<(f64, f64)>)> {
        // PRIMARY: numbered-grid classification. Overlay a 4x4 labeled grid
        // and ask the model for CELL NUMBERS — classification, not grounding.
        // Handles every click variant including non-COCO objects (screw,
        // chimney...) and reasoning challenges.
        // hCaptcha layout in the 520x570 iframe: header ~0..130, content
        // ~130..(h-50), buttons at the bottom. Grid 3x3 over the content.
        let cont_y0 = (h as f64 * 0.23) as u32;
        let cont_h = (h as f64 * 0.87) as u32 - cont_y0;
        if let Ok((grid_png, rect)) = grid_overlay(png, 3, Some((0, cont_y0, w, cont_h))) {
            if let Ok(cells) = self.pick_cells(&grid_png, w, h, 3).await {
                if !cells.is_empty() {
                    let cw = rect.2 / 3.0;
                    let chh = rect.3 / 3.0;
                    let clicks: Vec<(f64, f64)> = cells
                        .iter()
                        .map(|&(r, c)| (rect.0 + (c as f64 + 0.5) * cw, rect.1 + (r as f64 + 0.5) * chh))
                        .collect();
                    tracing::info!(target: "ghostcloak::mcp", "hcaptcha GRID path: cells={:?} clicks={:?}", cells, clicks);
                    return Ok((clicks, Some((w as f64 - 50.0, h as f64 - 30.0))));
                }
            }
        }
        // SECONDARY: COCO split-labor (GLM words -> YOLO boxes).
        if let Ok(words) = self.read_instruction(png).await {
            let idxs = coco_indices(&words);
            if !idxs.is_empty() {
                if let Ok(boxes) = coco_detect(png, &idxs, 0.30) {
                    if !boxes.is_empty() {
                        let clicks: Vec<(f64, f64)> = boxes.iter().take(6).map(|(x, y, _)| (*x, *y)).collect();
                        tracing::info!(target: "ghostcloak::mcp", "hcaptcha COCO path: words={:?} clicks={:?}", words, clicks);
                        return Ok((clicks, Some((w as f64 - 50.0, h as f64 - 30.0))));
                    }
                }
            }
        }
        // Stage 2 fallback: GLM coordinates (non-COCO categories, drag, pattern-break).
        self.solve_by_coords(png, w, h).await
    }

    /// Numbered-grid classification: the model answers with cell indices.
    async fn pick_cells(&self, gridded_png: &[u8], w: u32, h: u32, grid: u32) -> Result<Vec<(u32, u32)>> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(gridded_png);
        let prompt = format!(
            "This challenge image has a {grid}x{grid} numbered grid (labels 0-{} in yellow, top-left of each cell). Read the instruction and reply ONLY with the cell numbers that must be clicked, comma separated (e.g. 3,7), or none. Cell numbering: left to right, top to bottom starting at 0.",
            grid * grid - 1
        );
        let payload = serde_json::json!({
            "messages": [{"role": "user", "content": [
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                {"type": "text", "text": prompt}
            ]}],
            "max_tokens": 3000
        });
        let text = self.glm_call(&payload).await?;
        let mut cells = Vec::new();
        for tok in text.split(|c: char| c == ',' || c == '\n' || c.is_whitespace()) {
            if let Ok(n) = tok.trim().parse::<u32>() {
                if n < grid * grid {
                    cells.push((n / grid, n % grid));
                }
            }
        }
        cells.dedup();
        let _ = (w, h);
        Ok(cells)
    }

    /// GLM reads the instruction; returns the target object words.
    async fn read_instruction(&self, png: &[u8]) -> Result<Vec<String>> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let prompt = "Read the instruction text in this captcha challenge image. Reply with ONLY the English object name(s) that must be selected, comma separated, singular form. Example: bus. Example: cow, sheep. If the instruction is not about selecting a kind of object, reply only: NONE";
        let payload = serde_json::json!({
            "messages": [{"role": "user", "content": [
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                {"type": "text", "text": prompt}
            ]}],
            "max_tokens": 3000
        });
        let text = self.glm_call(&payload).await?;
        if text.trim().eq_ignore_ascii_case("none") || text.is_empty() {
            return Err(anyhow!("no object category"));
        }
        let words: Vec<String> = text
            .split(|c: char| c == ',' || c == '\n' || c.is_whitespace())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.len() < 30)
            .take(6)
            .collect();
        if words.is_empty() {
            return Err(anyhow!("no words parsed"));
        }
        Ok(words)
    }

    async fn glm_call(&self, payload: &serde_json::Value) -> Result<String> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/@cf/zai-org/glm-5.3-flash",
            self.account
        );
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.key))
            .json(payload)
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
        Ok(msg
            .get("content")
            .and_then(|c| c.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .or_else(|| {
                msg.get("reasoning_content")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default())
    }

    /// Legacy path: GLM enumerates click coordinates directly.
    async fn solve_by_coords(&self, png: &[u8], w: u32, h: u32) -> Result<(Vec<(f64, f64)>, Option<(f64, f64)>)> {
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

/// Overlay a numbered grid on the challenge crop: red cell borders +
/// yellow index labels. Turns pixel-grounding into cell-classification
/// (the trick from the big players: VLMs classify far better than they
/// ground coordinates).
///
/// `content` = Some((x0, y0, w, h)) restricts the grid to the content
/// area (hCaptcha: header ~130px + footer ~50px are NOT tiles). Returns
/// the PNG plus the content rect used, so callers can map cell centers
/// back to full-crop coordinates.
pub fn grid_overlay(png: &[u8], grid: u32, content: Option<(u32, u32, u32, u32)>) -> Result<(Vec<u8>, (f64, f64, f64, f64))> {
    use image::ImageFormat;
    let img = image::load_from_memory(png).context("grid overlay decode")?;
    let mut rgb = img.to_rgb8();
    let (fw, fh) = rgb.dimensions();
    let (ox, oy, w, h) = content.map(|(x, y, ww, hh)| (x, y, ww, hh)).unwrap_or((0, 0, fw, fh));
    let (cw, ch) = (w / grid, h / grid);
    for r in 0..grid {
        for c in 0..grid {
            let x0 = ox + c * cw;
            let y0 = oy + r * ch;
            // red border
            for x in x0..x0 + cw {
                if y0 < fh { rgb.put_pixel(x, y0, image::Rgb([255, 0, 0])); }
                if y0 + ch.saturating_sub(1) < fh && y0 + ch > 0 { rgb.put_pixel(x, y0 + ch - 1, image::Rgb([255, 0, 0])); }
            }
            for y in y0..y0 + ch {
                if x0 < fw { rgb.put_pixel(x0, y, image::Rgb([255, 0, 0])); }
                if x0 + cw.saturating_sub(1) < fw && x0 + cw > 0 { rgb.put_pixel(x0 + cw - 1, y, image::Rgb([255, 0, 0])); }
            }
            // yellow label box + number (blocky 3x5 font digits)
            let idx = r * grid + c;
            let label = idx.to_string();
            let (lx, ly) = (x0 + 6, y0 + 6);
            for yy in ly..(ly + 22).min(fh) {
                for xx in lx..(lx + 4 + 10 * label.len() as u32).min(fw) {
                    rgb.put_pixel(xx, yy, image::Rgb([255, 255, 0]));
                }
            }
            // draw the digits with simple block glyphs
            const GLYPHS: [&str; 10] = [
                "111 101 101 101 111", "010 110 010 010 111", "111 001 111 100 111",
                "111 001 111 001 111", "101 101 111 001 001", "111 100 111 001 111",
                "111 100 111 101 111", "111 001 001 001 001", "111 101 111 101 111",
                "111 101 111 001 111",
            ];
            let mut dx = lx + 3;
            for ch_ in label.chars() {
                let d = ch_.to_digit(10).unwrap_or(0) as usize;
                let glyph = GLYPHS[d];
                for (gy, row) in glyph.split("  ").enumerate() {
                    for (gx_, bit) in row.chars().enumerate() {
                        if bit == '1' {
                            let (px, py) = (dx + gx_ as u32 * 2, ly + 3 + gy as u32 * 4);
                            for oy2 in 0..3 {
                                for ox2 in 0..2 {
                                    if px + ox2 < fw && py + oy2 < fh {
                                        rgb.put_pixel(px + ox2, py + oy2, image::Rgb([0, 0, 0]));
                                    }
                                }
                            }
                        }
                    }
                }
                dx += 10;
            }
        }
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    rgb.write_to(&mut buf, ImageFormat::Png).context("grid encode")?;
    Ok((buf.into_inner(), (ox as f64, oy as f64, w as f64, h as f64)))
}
