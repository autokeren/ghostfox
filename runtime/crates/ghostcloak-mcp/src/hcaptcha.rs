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

/// Parse the expected click count from instruction words ("TWO icons").
fn count_from_instruction(s: &str) -> usize {
    let low = s.to_lowercase();
    for (word, n) in [
        ("two", 2usize),
        ("three", 3),
        ("four", 4),
        ("2", 2),
        ("3", 3),
        ("4", 4),
    ] {
        if low.contains(word) {
            return n;
        }
    }
    0 // unknown -> accept whatever the model returns
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

/// A solved challenge round: either a list of clicks, or a DRAG gesture
/// (press at `from`, release at `to`), plus the verify button.
#[derive(Debug, Default)]
pub struct Solved {
    pub clicks: Vec<(f64, f64)>,
    pub drag: Option<((f64, f64), (f64, f64))>,
    pub verify: Option<(f64, f64)>,
}

impl Glm {
    pub fn new(account: String, key: String) -> Self {
        Self { account, key }
    }

    /// Ask the vision model ONLY for the instruction text (its strength),
    /// then let COCO YOLO do detection+localization (its strength).
    /// Returns (click_points, verify_point) in the crop's coordinates.
    pub async fn solve_challenge(&self, png: &[u8], w: u32, h: u32) -> Result<Solved> {
        // ROUTER: detect the challenge's real layout, then use the strategy
        // that fits it — numbered-cell classification for tile grids, direct
        // object pointing for single-image / reference-panel variants.
        match detect_layout(png) {
            Ok(Layout::Grid { x, y, w: gw, h: gh, cols }) => {
                let rows = cols; // square grids
                if let Ok((grid_png, _rect)) =
                    grid_overlay(png, cols, Some((x as u32, y as u32, gw as u32, gh as u32)))
                {
                    if let Ok(cells) = self.pick_cells(&grid_png, w, h, cols).await {
                        if !cells.is_empty() {
                            let cw = gw / cols as f64;
                            let chh = gh / rows as f64;
                            let clicks: Vec<(f64, f64)> = cells
                                .iter()
                                .map(|&(r, c)| (x + (c as f64 + 0.5) * cw, y + (r as f64 + 0.5) * chh))
                                .collect();
                            tracing::info!(target: "ghostcloak::mcp", "hcaptcha GRID path: layout=({:?},{:?},{:?},{:?},cols={}) cells={:?} clicks={:?}", x, y, gw, gh, cols, cells, clicks);
                            return Ok(Solved { clicks, verify: Some((w as f64 - 50.0, h as f64 - 60.0)), drag: None });
                        }
                    }
                }
            }
            Ok(Layout::Image { y: cy, h: chh }) => {
                // Read the instruction first: drag challenges need a
                // gesture, not clicks.
                let inst = self.read_instruction_raw(png).await.unwrap_or_default();
                if inst.to_lowercase().contains("drag") {
                    // GLM consensus FIRST: live-tested it finds the matching
                    // shape + slot reliably ((95,213)->(248,249) repeatedly),
                    // while llama point guesses are noise. Llama is the
                    // fallback.
                    if let Ok((from, to)) = self.glm_drag_consensus(png).await {
                        let dist = ((from.0 - to.0).powi(2) + (from.1 - to.1).powi(2)).sqrt();
                        if dist >= 60.0 {
                            tracing::info!(target: "ghostcloak::mcp", "hcaptcha DRAG-GLM consensus: {from:?} -> {to:?} dist={dist}");
                            return Ok(Solved {
                                clicks: vec![],
                                verify: Some((w as f64 - 50.0, h as f64 - 60.0)),
                                drag: Some((from, to)),
                            });
                        }
                    }
                    for attempt in 0..3 {
                        if let Ok((from, to)) = self.point_drag(png).await {
                            let dist = ((from.0 - to.0).powi(2) + (from.1 - to.1).powi(2)).sqrt();
                            if dist >= 60.0 {
                                tracing::info!(target: "ghostcloak::mcp", "hcaptcha DRAG path (attempt {attempt}): {from:?} -> {to:?} dist={dist}");
                                return Ok(Solved {
                                    clicks: vec![],
                                    verify: Some((w as f64 - 50.0, h as f64 - 60.0)),
                                    drag: Some((from, to)),
                                });
                            }
                        }
                    }
                    // A click cannot solve a drag — fail the round rather
                    // than wasting it on the wrong gesture type.
                    return Err(anyhow!("drag targets not found (instruction: {inst})"));
                }
                // Two-step reasoning: llama RESOLVES the instruction
                // to concrete objects (e.g. "animals that eat the shown
                // food" + grass photo -> cow, sheep), then COCO YOLO
                // localizes them precisely.
                if let Ok(words) = self.resolve_targets(png).await {
                    let idxs = coco_indices(&words);
                    if !idxs.is_empty() {
                        if let Ok(boxes) = coco_detect(png, &idxs, 0.28) {
                            if !boxes.is_empty() {
                                let clicks: Vec<(f64, f64)> = boxes
                                    .iter()
                                    .take(6)
                                    .map(|&(x, y2, _)| (x, y2.max(cy + 6.0).min(cy + chh - 6.0)))
                                    .collect();
                                tracing::info!(target: "ghostcloak::mcp", "hcaptcha REASON path: words={:?} clicks={:?}", words, clicks);
                                return Ok(Solved { clicks, verify: Some((w as f64 - 50.0, h as f64 - 60.0)), drag: None });
                            }
                        }
                    }
                }
                // GLM pointing ensemble: GLM reasons about icon fields
                // (live-tested: its picks cluster; llama's are noise at this
                // icon size). 4 calls, cluster 40px, consensus >= 2 votes.
                let want = count_from_instruction(&inst);
                let mut all: Vec<(f64, f64)> = Vec::new();
                for _ in 0..4 {
                    if let Ok(mut solved) = self.glm_point(png).await {
                        if solved.drag.is_some() {
                            return Ok(solved);
                        }
                        for (x, y2) in solved.clicks {
                            all.push((x, y2.max(cy + 6.0).min(cy + chh - 6.0)));
                        }
                    }
                }
                let mut clusters: Vec<((f64, f64), usize)> = Vec::new();
                for p in all {
                    if let Some(c) = clusters
                        .iter_mut()
                        .find(|((cx, cy2), _)| (cx - p.0).abs() < 40.0 && (cy2 - p.1).abs() < 40.0)
                    {
                        let k = c.1 as f64;
                        c.0 = (((c.0).0 * (k - 1.0) + p.0) / k, ((c.0).1 * (k - 1.0) + p.1) / k);
                        c.1 += 1;
                    } else {
                        clusters.push((p, 1));
                    }
                }
                let mut cons: Vec<((f64, f64), usize)> =
                    clusters.iter().filter(|(_, v)| *v >= 2).cloned().collect();
                if cons.is_empty() {
                    cons = clusters;
                }
                cons.sort_by(|a, b| b.1.cmp(&a.1));
                let take = if want > 0 { want.min(cons.len()) } else { cons.len().min(4) };
                let clicks: Vec<(f64, f64)> = cons.iter().take(take).map(|(p, _)| *p).collect();
                if !clicks.is_empty() {
                    tracing::info!(target: "ghostcloak::mcp", "hcaptcha GLM-ENSEMBLE: want={want} clusters={:?}", cons);
                    return Ok(Solved {
                        clicks,
                        verify: Some((w as f64 - 50.0, h as f64 - 60.0)),
                        drag: None,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(target: "ghostcloak::mcp", "layout detect failed: {e}");
            }
        }
        // LAST RESORT: COCO split-labor (GLM words -> YOLO boxes).
        if let Ok(words) = self.read_instruction(png).await {
            let idxs = coco_indices(&words);
            if !idxs.is_empty() {
                if let Ok(boxes) = coco_detect(png, &idxs, 0.30) {
                    if !boxes.is_empty() {
                        let clicks: Vec<(f64, f64)> = boxes.iter().take(6).map(|(x, y, _)| (*x, *y)).collect();
                        tracing::info!(target: "ghostcloak::mcp", "hcaptcha COCO path: words={:?} clicks={:?}", words, clicks);
                        return Ok(Solved { clicks, verify: Some((w as f64 - 50.0, h as f64 - 60.0)), drag: None });
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
        // PRIMARY: llama-3.2-11b vision — decisive on grid classification
        // (live-tested: "3, 7" vs GLM's rambling). GLM as fallback.
        let llama_payload = serde_json::json!({
            "image": b64,
            "prompt": prompt
        });
        let mut text = self.llama_call(&llama_payload).await.unwrap_or_default();
        if text.trim().is_empty() || text.trim().eq_ignore_ascii_case("none") {
            let payload = serde_json::json!({
                "messages": [{"role": "user", "content": [
                    {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                    {"type": "text", "text": prompt}
                ]}],
                "max_tokens": 3000
            });
            text = self.glm_call(&payload).await?;
        }
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

    /// Drag challenges: locate the shape to move and its destination.
    async fn point_drag(&self, png: &[u8]) -> Result<((f64, f64), (f64, f64))> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let prompt = "This is a drag-the-shape captcha. There is a shape/object that must be dragged, and an empty spot (outline, hole, or matching slot) somewhere ELSE in the image where it must go. Reply with exactly two lines: first the CENTER pixel of the draggable shape as 'x1 y1', then the CENTER of the destination slot as 'x2 y2'. The two points are usually FAR apart. Nothing else.";
        let payload = serde_json::json!({ "image": b64, "prompt": prompt });
        let text = self.llama_call(&payload).await?;
        // take the first two coordinate pairs
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for line in text.lines().take(4) {
            let nums: Vec<f64> = line
                .split_whitespace()
                .filter_map(|t| t.parse::<f64>().ok())
                .collect();
            if nums.len() >= 2 {
                pts.push((nums[0].clamp(2.0, 518.0), nums[1].clamp(2.0, 568.0)));
            }
        }
        if pts.len() < 2 {
            return Err(anyhow!("drag: fewer than two points"));
        }
        Ok((pts[0], pts[1]))
    }

    /// One GLM pointing call on the full challenge crop.
    async fn glm_point(&self, png: &[u8]) -> Result<Solved> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let prompt = "Read the instruction at the top of this captcha challenge. Then give the exact pixel coordinates (x, y) of the element(s) that satisfy it. Reply ONLY coordinate pairs, one per line: x y. If the instruction asks to DRAG something, reply with ONE line: D x1 y1 x2 y2 (source then destination).";
        let payload = serde_json::json!({
            "messages": [{"role": "user", "content": [
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                {"type": "text", "text": prompt}
            ]}],
            "max_tokens": 5000
        });
        let text = self.glm_call(&payload).await?;
        let (clicks, verify, drag) = parse_clicks_full(&text, 520.0, 570.0);
        Ok(Solved { clicks, drag, verify })
    }

    /// GLM drag ensemble: 3 calls, parse D-lines, cluster source and
    /// destination separately, return the 2/3-majority pair.
    async fn glm_drag_consensus(&self, png: &[u8]) -> Result<((f64, f64), (f64, f64))> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        // Focused 2-part: shorter reasoning per call beats one long one
        // (the 408 ceiling punishes deep single-shot analysis).
        let _ = serde_json::json!({
            "messages": [{"role": "user", "content": [
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                {"type": "text", "text": "Where is the EMPTY SLOT / target outline in this drag captcha? Reply with ONLY its center: x y"}
            ]}],
            "max_tokens": 6000
        });
        let prompt = "This is a drag-the-shape captcha. TWO QUESTIONS, answer each on its own line: 1) SLOT x y (center of the empty target slot/outline) 2) SHAPE x y (center of the draggable shape that matches the slot's pattern). Be concise, no long analysis.";
        let payload = serde_json::json!({
            "messages": [{"role": "user", "content": [
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
                {"type": "text", "text": prompt}
            ]}],
            "max_tokens": 11000
        });
        let mut sources: Vec<(f64, f64)> = Vec::new();
        let mut dests: Vec<(f64, f64)> = Vec::new();
        for _ in 0..4 {
            let Ok(text) = self.glm_call(&payload).await else { continue };
            // Parse "SLOT x y" and "SHAPE x y" lines (also tolerates a
            // combined "D x1 y1 x2 y2" line). Prose numbers are ignored:
            // only lines that START with the keywords count.
            let mut slot: Option<(f64, f64)> = None;
            let mut shape: Option<(f64, f64)> = None;
            for line in text.lines() {
                let trimmed = line.trim().trim_start_matches("1)").trim_start_matches("2)").trim();
                let upper = trimmed.to_uppercase();
                let is_slot = upper.starts_with("SLOT");
                let is_shape = upper.starts_with("SHAPE");
                let is_d = trimmed.starts_with('D') && trimmed.split_whitespace().count() == 5;
                let nums: Vec<f64> = trimmed
                    .split(|c: char| c.is_whitespace())
                    .filter_map(|t| t.trim_matches(|c: char| !c.is_ascii_digit() && c != '.').parse::<f64>().ok())
                    .collect();
                if is_slot && nums.len() >= 2 {
                    slot = Some((nums[0], nums[1]));
                } else if is_shape && nums.len() >= 2 {
                    shape = Some((nums[0], nums[1]));
                } else if is_d && nums.len() >= 4 {
                    slot = Some((nums[2], nums[3]));
                    shape = Some((nums[0], nums[1]));
                }
            }
            if let Some(s) = slot {
                dests.push(s);
            }
            if let Some(s) = shape {
                sources.push(s);
            }
        }
        if sources.is_empty() {
            return Err(anyhow!("glm drag: no answers"));
        }
        let consensus = |pts: &[(f64, f64)]| -> (f64, f64) {
            let mut clusters: Vec<((f64, f64), usize)> = Vec::new();
            for p in pts {
                if let Some(c) = clusters
                    .iter_mut()
                    .find(|((cx, cy), _)| (cx - p.0).abs() < 50.0 && (cy - p.1).abs() < 50.0)
                {
                    let k = c.1 as f64;
                    c.0 = (((c.0).0 * (k - 1.0) + p.0) / k, ((c.0).1 * (k - 1.0) + p.1) / k);
                    c.1 += 1;
                } else {
                    clusters.push((*p, 1));
                }
            }
            clusters.sort_by(|a, b| b.1.cmp(&a.1));
            clusters.first().map(|(p, _)| *p).unwrap_or(pts[0])
        };
        // source: require 2+ votes when possible (it's the critical pick)
        let mut s_clusters: Vec<((f64, f64), usize)> = Vec::new();
        for p in &sources {
            if let Some(c) = s_clusters
                .iter_mut()
                .find(|((cx, cy), _)| (cx - p.0).abs() < 50.0 && (cy - p.1).abs() < 50.0)
            {
                c.0 = (c.0 .0, c.0 .1);
                c.1 += 1;
            } else {
                s_clusters.push((*p, 1));
            }
        }
        s_clusters.sort_by(|a, b| b.1.cmp(&a.1));
        let src = s_clusters.first().map(|(p, _)| *p).unwrap_or(sources[0]);
        Ok((src, consensus(&dests)))
    }

    /// Read just the instruction text (raw).
    async fn read_instruction_raw(&self, png: &[u8]) -> Result<String> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let prompt = "Read the instruction in this captcha challenge. Reply with ONLY the instruction text, nothing else.";
        let payload = serde_json::json!({ "image": b64, "prompt": prompt });
        let text = self.llama_call(&payload).await?;
        if text.trim().is_empty() {
            return Err(anyhow!("empty instruction"));
        }
        Ok(text)
    }

    /// Two-step reasoning: resolve the instruction (which may reference a
    /// shown example/food) into CONCRETE object names. E.g. "tap animals
    /// that rely on the shown food source" + a grass photo -> cow, sheep.
    async fn resolve_targets(&self, png: &[u8]) -> Result<Vec<String>> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let prompt = "Look at this captcha challenge. Read the instruction at the top. It may reference a reference image (a food source, an example object, etc). RESOLVE which concrete objects satisfy the instruction — for example, if the instruction asks for animals that eat the shown food, answer with the names of animals that eat that food. Reply with ONLY the English object name(s), comma separated, singular form.";
        let payload = serde_json::json!({ "image": b64, "prompt": prompt });
        let text = self.llama_call(&payload).await?;
        if text.trim().is_empty() {
            return Err(anyhow!("resolve: empty answer"));
        }
        let words: Vec<String> = text
            .split(|c: char| c == ',' || c == '\n' || c == '.' || c.is_whitespace())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.len() < 30)
            .take(6)
            .collect();
        if words.is_empty() {
            return Err(anyhow!("resolve: no words"));
        }
        Ok(words)
    }

    /// Single-image challenges: one llama call — read the instruction and
    /// list the pixel coordinates of every element to click.
    async fn point_targets(&self, png: &[u8]) -> Result<Solved> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let prompt = "Read the instruction in this captcha challenge. If it asks to DRAG something somewhere, reply with ONE line: D x1 y1 x2 y2 (source then destination centers). Otherwise list the pixel coordinates (x, y) of EVERY element that must be clicked, one per line, format: x y. If there is a Verify/Submit button, output its center as the final line: V x y. Nothing else.";
        let payload = serde_json::json!({ "image": b64, "prompt": prompt });
        let text = self.llama_call(&payload).await?;
        let (clicks, verify, drag) = parse_clicks_full(&text, 520.0, 570.0);
        let solved = Solved { clicks, drag, verify };
        Ok(solved)
    }


    /// Instrumented twin of solve_challenge: also returns the detected
    /// layout and the raw model answer for debugging.
    pub async fn solve_challenge_dbg(
        &self,
        png: &[u8],
        w: u32,
        h: u32,
    ) -> Result<(Solved, String, String)> {
        let layout = detect_layout(png).map(|l| match l {
            Layout::Grid { x, y, w: gw, h: gh, cols } =>
                format!("grid x={x:.0} y={y:.0} w={gw:.0} h={gh:.0} cols={cols}"),
            Layout::Image { y, h: chh } => format!("image y={y:.0} h={chh:.0}"),
        }).unwrap_or_else(|e| format!("detect-failed: {e}"));
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let raw = if let Ok(l) = detect_layout(png) {
            match l {
                Layout::Grid { .. } => {
                    let prompt = "Read the instruction in this captcha challenge. What single object or category must be selected? Reply with ONLY the object name(s).";
                    self.llama_call(&serde_json::json!({"image": b64, "prompt": prompt})).await.unwrap_or_default()
                }
                Layout::Image { .. } => {
                    let prompt = "Read the instruction in this captcha challenge and reply with ONLY the instruction text, nothing else.";
                    self.llama_call(&serde_json::json!({"image": b64, "prompt": prompt})).await.unwrap_or_default()
                }
            }
        } else { String::new() };
        let solved = self.solve_challenge(png, w, h).await?;
        Ok((solved, layout, raw.chars().take(180).collect::<String>()))
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

    /// One llama-3.2-11b vision call: {image, prompt} -> result.response.
    async fn llama_call(&self, payload: &serde_json::Value) -> Result<String> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/@cf/meta/llama-3.2-11b-vision-instruct",
            self.account
        );
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.key))
            .json(payload)
            .send()
            .await
            .context("llama request failed")?;
        let d: serde_json::Value = resp.json().await.context("llama response parse")?;
        Ok(d
            .get("result")
            .and_then(|r| r.get("response"))
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string())
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
    async fn solve_by_coords(&self, png: &[u8], w: u32, h: u32) -> Result<Solved> {
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
            Some((w as f64 - 50.0, h as f64 - 60.0))
        };
        tracing::info!(target: "ghostcloak::mcp", "hcaptcha glm: clicks={:?} verify={:?}", clicks, verify);
        Ok(Solved { clicks, verify, drag: None })
    }
}

/// Parse "<x> <y>" lines, a trailing "V <x> <y>" verify point, and an
/// optional "D x1 y1 x2 y2" drag line. Coordinates clamped to bounds.
/// Near-duplicate points (within 30px) collapse into one.
pub fn parse_clicks_full(
    s: &str,
    w: f64,
    h: f64,
) -> (Vec<(f64, f64)>, Option<(f64, f64)>, Option<((f64, f64), (f64, f64))>) {
    let mut clicks: Vec<(f64, f64)> = Vec::new();
    let mut verify: Option<(f64, f64)> = None;
    let mut drag: Option<((f64, f64), (f64, f64))> = None;
    for line in s.lines() {
        let mut parts = line.split_whitespace();
        let (a, b, c) = (parts.next(), parts.next(), parts.next());
        let is_drag = matches!(a, Some(t) if t.eq_ignore_ascii_case("d") || t.eq_ignore_ascii_case("drag"));
        if is_drag {
            let mut rest = parts;
            let (Some(x1), Some(y1), Some(x2), Some(y2)) =
                (rest.next(), rest.next(), rest.next(), rest.next())
            else {
                continue;
            };
            let (Ok(x1f), Ok(y1f), Ok(x2f), Ok(y2f)) =
                (x1.parse::<f64>(), y1.parse::<f64>(), x2.parse::<f64>(), y2.parse::<f64>())
            else {
                continue;
            };
            let clamp = |v: f64, max: f64| v.clamp(2.0, max - 2.0);
            drag = Some(((clamp(x1f, w), clamp(y1f, h)), (clamp(x2f, w), clamp(y2f, h))));
            continue;
        }
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
    (clicks, verify, drag)
}

/// Legacy parser wrapper (no drag).
pub fn parse_clicks(s: &str, w: f64, h: f64) -> (Vec<(f64, f64)>, Option<(f64, f64)>) {
    let (clicks, verify, _) = parse_clicks_full(s, w, h);
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

/// Detected challenge layout from pixel band analysis.
#[derive(Debug)]
pub enum Layout {
    /// 3x3-style tile grid with its real rect + column count.
    Grid { x: f64, y: f64, w: f64, h: f64, cols: u32 },
    /// Single image (click-on-object) or reference-panel variant: the
    /// model must point at targets directly.
    Image { y: f64, h: f64 },
}

/// Detect the challenge layout from solid-band pixel analysis:
/// banner (solid row band), sub-banner, buttons, then column
/// separators inside the tile area. Fresh-measured on the 520x570
/// iframe: banner y118-144, tiles y144-465, buttons y501-520.
pub fn detect_layout(png: &[u8]) -> Result<Layout> {
    let img = image::load_from_memory(png).context("layout decode")?;
    let g = img.to_luma8();
    let (w, h) = g.dimensions();
    if w < 20 || h < 20 {
        return Err(anyhow!("challenge image too small"));
    }
    // row std
    let mut row_std = vec![0f64; h as usize];
    for y in 0..h as usize {
        let mut sum = 0f64;
        let mut sumsq = 0f64;
        for x in 0..w as usize {
            let v = g.get_pixel(x as u32, y as u32).0[0] as f64;
            sum += v;
            sumsq += v * v;
        }
        let n = w as f64;
        row_std[y] = (sumsq / n - (sum / n) * (sum / n)).max(0.0).sqrt();
    }
    // solid bands (std < 9, run >= 4 rows)
    let mut bands: Vec<(u32, u32)> = Vec::new();
    let mut y = 0usize;
    while y < h as usize {
        if row_std[y] < 9.0 {
            let start = y;
            while y < h as usize && row_std[y] < 9.0 {
                y += 1;
            }
            if y - start >= 4 {
                bands.push((start as u32, y as u32));
            }
        } else {
            y += 1;
        }
    }
    // banner = first solid band starting >= 40px down, >= 10px tall
    let banner = bands
        .iter()
        .find(|b| b.0 >= 40 && b.1 - b.0 >= 10)
        .ok_or_else(|| anyhow!("no instruction banner band found"))?;
    // sub-banner: next solid band well below the banner (>= 80px under it)
    let sub = bands.iter().find(|b| b.0 > banner.1 + 80 && b.1 - b.0 >= 10);
    let content_top = banner.1 + 2;
    let content_bottom = sub.map(|b| b.0 - 2).unwrap_or(h.saturating_sub(55));
    if content_bottom <= content_top + 20 {
        return Err(anyhow!("degenerate content area"));
    }
    // column std within the content rows
    let band_h = (content_bottom - content_top) as usize;
    let mut col_std = vec![0f64; w as usize];
    for x in 0..w as usize {
        let mut sum = 0f64;
        let mut sumsq = 0f64;
        for y in (content_top as usize)..(content_bottom as usize) {
            let v = g.get_pixel(x as u32, y as u32).0[0] as f64;
            sum += v;
            sumsq += v * v;
        }
        let n = band_h as f64;
        col_std[x] = (sumsq / n - (sum / n) * (sum / n)).max(0.0).sqrt();
    }
    let mut colbands: Vec<(u32, u32)> = Vec::new();
    let mut x = 0usize;
    while x < w as usize {
        if col_std[x] < 8.0 {
            let start = x;
            while x < w as usize && col_std[x] < 8.0 {
                x += 1;
            }
            if x - start >= 3 {
                colbands.push((start as u32, x as u32));
            }
        } else {
            x += 1;
        }
    }
    // reference-panel variant: a wide solid column on the left half
    if colbands.iter().any(|b| b.1 - b.0 >= 80 && b.0 < w / 2) {
        return Ok(Layout::Image { y: content_top as f64, h: (content_bottom - content_top) as f64 });
    }
    // thin vertical separators -> tile grid. Sanity: separators must sit
    // away from the edges, be few (2-3), and leave a wide, tall content
    // rect — otherwise it's an icon field and pointing is the strategy.
    let seps: Vec<(u32, u32)> = colbands
        .iter()
        .filter(|b| b.1 - b.0 <= 12 && b.0 >= 40 && b.1 <= w.saturating_sub(40))
        .cloned()
        .collect();
    if (2..=3).contains(&seps.len()) {
        let gx0 = seps.first().map(|s| s.1).unwrap_or(0);
        let gx1 = seps.last().map(|s| s.0).unwrap_or(w);
        let gw = gx1.saturating_sub(gx0);
        let gh = content_bottom.saturating_sub(content_top);
        let cols = seps.len() as u32 + 1;
        if gw as f64 >= w as f64 * 0.4 && gh as f64 >= h as f64 * 0.25 {
            return Ok(Layout::Grid {
                x: gx0 as f64,
                y: content_top as f64,
                w: gw as f64,
                h: gh as f64,
                cols,
            });
        }
    }
    Ok(Layout::Image { y: content_top as f64, h: (content_bottom - content_top) as f64 })
}
