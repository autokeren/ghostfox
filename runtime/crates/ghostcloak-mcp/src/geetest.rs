//! v0.6.5 TRAINED EYES — GeeTest v3 icon-click (文字点选) solver.
//!
//! Two tiny ONNX models, 100% local CPU inference via the RTEN runtime
//! (same engine as ocrs — no C deps, no API):
//!   - `yolov8s.onnx` (44MB): detects char boxes. Trained with 2
//!     classes: "small" (<35px = the instruction strip, left-to-right
//!     = click order) vs "big" (the field chars on the photo).
//!   - `siamese.onnx` (17MB): similarity net. Compares each strip
//!     glyph against each field glyph and emits the click order.
//!
//! ~0.5s total on CPU. Verified 5/5 in-browser runs (2/2 on the real
//! passport.bilibili.com login, "Verification Succeeded").
//!
//! Models ship under `$GHOSTFOX_HOME/models/geetest_click/` and
//! auto-download on first use. AGPL-3.0, attributed:
//! ravizhan/geetest-v3-click-crack (see tools/geetest-click/README.md).

use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use image::imageops::FilterType;
use rten::Model;
use rten_tensor::Layout as _;

const YOLO_URL: &str =
    "https://raw.githubusercontent.com/ravizhan/geetest-v3-click-crack/master/yolov8s.onnx";
/// The siamese ships dequantized (`siamese_float.onnx`) because the upstream
/// model is dynamic-quantized (DynamicQuantizeLinear/ConvInteger/MatMulInteger)
/// which rten's ONNX importer mishandles. Regenerate with
/// `tools/geetest-click/dequant_siamese.py`.
const CONF: f32 = 0.8;
const IOU_THRESH: f32 = 0.8;
const SIAMESE_THRESHOLD: f32 = 0.1;
const YOLO_IN: u32 = 384;
const SIAMESE_IN: u32 = 105;

fn models_dir() -> PathBuf {
    let home = std::env::var("GHOSTFOX_HOME").unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|d| d.join(".ghostfox"))
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".ghostfox".into())
    });
    PathBuf::from(home).join("models").join("geetest_click")
}

async fn download(url: &str, to: &PathBuf) -> Result<()> {
    let tmp = to.with_extension("part");
    let resp = reqwest::get(url)
        .await
        .map_err(|e| anyhow!("model download failed ({url}): {e}"))?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| anyhow!("model download read failed ({url}): {e}"))?;
    tokio::fs::write(&tmp, &bytes)
        .await
        .with_context(|| format!("writing {}", tmp.display()))?;
    tokio::fs::rename(&tmp, to)
        .await
        .with_context(|| format!("moving {} -> {}", tmp.display(), to.display()))?;
    tracing::info!(target: "ghostcloak::mcp", "downloaded geetest model {} ({} KB)", to.display(), bytes.len() / 1024);
    Ok(())
}

async fn ensure_models() -> Result<(PathBuf, PathBuf)> {
    let dir = models_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let yolo = dir.join("yolov8s.onnx");
    let siam = dir.join("siamese_float.onnx");
    if !yolo.exists() {
        download(YOLO_URL, &yolo).await?;
    }
    if !siam.exists() {
        return Err(anyhow!(
            "siamese_float.onnx missing at {} — ship it with the distribution or generate via tools/geetest-click/dequant_siamese.py",
            siam.display()
        ));
    }
    Ok((yolo, siam))
}

struct GtModels {
    yolo: Model,
    siamese: Model,
}

static MODELS: OnceLock<GtModels> = OnceLock::new();

async fn models() -> Result<&'static GtModels> {
    if let Some(m) = MODELS.get() {
        return Ok(m);
    }
    let (yolo_path, siam_path) = ensure_models().await?;
    let yolo_bytes = tokio::fs::read(&yolo_path).await?;
    let siam_bytes = tokio::fs::read(&siam_path).await?;
    let m = GtModels {
        yolo: Model::load(yolo_bytes).context("loading yolov8s.onnx")?,
        siamese: Model::load(siam_bytes).context("loading siamese.onnx")?,
    };
    Ok(MODELS.get_or_init(|| m))
}

/// Box = (x, y, w, h, score). All in ORIGINAL image pixel coords.
#[derive(Debug, Clone)]
struct Box {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    score: f32,
}

fn iou(a: &Box, b: &Box) -> f32 {
    let ax2 = a.x + a.w;
    let ay2 = a.y + a.h;
    let bx2 = b.x + b.w;
    let by2 = b.y + b.h;
    let ix = (ax2.min(bx2) - a.x.max(b.x)).max(0.0);
    let iy = (ay2.min(by2) - a.y.max(b.y)).max(0.0);
    let inter = ix * iy;
    let union = a.w * a.h + b.w * b.h - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Greedy NMS (same semantics as cv2.dnn.NMSBoxes).
fn nms(mut boxes: Vec<Box>) -> Vec<Box> {
    boxes.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut kept: Vec<Box> = Vec::new();
    for b in boxes {
        if kept.iter().all(|k| iou(k, &b) < IOU_THRESH) {
            kept.push(b);
        }
    }
    kept
}

/// CHW float32 value from RGB8 pixels, normalized to [0, 1].
fn to_value(rgb: &image::RgbImage) -> rten::Value {
    let (w, h) = rgb.dimensions();
    let pixels = rgb.as_raw();
    let mut chw = vec![0f32; (3 * h * w) as usize];
    for (i, px) in pixels.chunks(3).enumerate() {
        let x = i % w as usize;
        let y = i / w as usize;
        let base = (h * w) as usize;
        chw[0 * base + y * w as usize + x] = px[0] as f32 / 255.0;
        chw[1 * base + y * w as usize + x] = px[1] as f32 / 255.0;
        chw[2 * base + y * w as usize + x] = px[2] as f32 / 255.0;
    }
    rten::Value::from_shape(&[1usize, 3, h as usize, w as usize], chw).expect("tensor build")
}

/// Detect char boxes: returns (smalls, bigs) where smalls are the
/// instruction-strip glyphs (keyed by left x, sort = click order) and
/// bigs are the field chars.
fn detect(models: &GtModels, img: &image::RgbImage) -> Result<(Vec<(i32, image::RgbImage)>, Vec<Box>)> {
    let (ow, oh) = img.dimensions();
    let resized = image::imageops::resize(img, YOLO_IN, YOLO_IN, FilterType::Triangle);
    let input = to_value(&resized);
    let in_id = models.yolo.input_ids()[0];
    let out_id = models.yolo.output_ids()[0];
    let outputs = models
        .yolo
        .run(vec![(in_id, input.into())], &[out_id], None)
        .context("yolo inference")?;
    let out = outputs[0].as_view();
    let rten::ValueView::FloatTensor(tv) = &out else {
        return Err(anyhow!("yolo output is not a float tensor"));
    };
    let n = tv.size(2);
    let data = tv.data().context("yolo output data")?;

    let xf = ow as f32 / YOLO_IN as f32;
    let yf = oh as f32 / YOLO_IN as f32;

    let mut boxes: Vec<Box> = Vec::new();
    for i in 0..n {
        let x = data[i];
        let y = data[n + i];
        let w = data[2 * n + i];
        let h = data[3 * n + i];
        let c0 = data[4 * n + i];
        let c1 = data[5 * n + i];
        let score = c0.max(c1);
        if score < CONF {
            continue;
        }
        boxes.push(Box {
            x: (x - w / 2.0) * xf,
            y: (y - h / 2.0) * yf,
            w: w * xf,
            h: h * yf,
            score,
        });
    }
    let boxes = nms(boxes);

    let mut smalls: Vec<(i32, image::RgbImage)> = Vec::new();
    let mut bigs: Vec<Box> = Vec::new();
    for b in boxes {
        let crop = image::imageops::crop_imm(
            img,
            b.x.max(0.0) as u32,
            b.y.max(0.0) as u32,
            b.w.min(ow as f32 - b.x.max(0.0)) as u32,
            b.h.min(oh as f32 - b.y.max(0.0)) as u32,
        )
        .to_image();
        if crop.width() < 35 && crop.height() < 35 {
            smalls.push((b.x as i32, crop));
        } else {
            bigs.push(b);
        }
    }
    smalls.sort_by_key(|(x, _)| *x);
    Ok((smalls, bigs))
}

fn siamese_prep(crop: &image::RgbImage) -> rten::Value {
    let resized = image::imageops::resize(crop, SIAMESE_IN, SIAMESE_IN, FilterType::Triangle);
    to_value(&resized)
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Solve a challenge image (the raw composite: field + strip at the
/// bottom). Returns field-char box top-lefts `[[x, y], ...]` in the
/// image's pixel coordinates, IN CLICK ORDER.
pub async fn solve_image(img_bytes: &[u8]) -> Result<Vec<[i32; 2]>> {
    let models = models().await?;
    let img = image::load_from_memory(img_bytes).context("decoding challenge image")?;
    let rgb = img.to_rgb8();
    let (small_glyphs, big_boxes) = detect(models, &rgb)?;

    if small_glyphs.is_empty() || big_boxes.is_empty() {
        return Err(anyhow!(
            "detection found {} strip glyphs / {} field chars (need >= 1 each)",
            small_glyphs.len(),
            big_boxes.len()
        ));
    }

    let mut result: Vec<[i32; 2]> = Vec::new();
    for (_x, glyph) in &small_glyphs {
        let d1 = siamese_prep(glyph);
        for b in &big_boxes {
            if result.contains(&[b.x as i32, b.y as i32]) {
                continue;
            }
            let crop = image::imageops::crop_imm(
                &rgb,
                b.x.max(0.0) as u32,
                b.y.max(0.0) as u32,
                b.w.min(rgb.width() as f32 - b.x.max(0.0)) as u32,
                b.h.min(rgb.height() as f32 - b.y.max(0.0)) as u32,
            )
            .to_image();
            let d2 = siamese_prep(&crop);
            let in1 = models.siamese.node_id("input").context("siamese input node")?;
            let in2 = models.siamese.node_id("input.53").context("siamese input2 node")?;
            let out_id = models.siamese.output_ids()[0];
            let out = models
                .siamese
                .run(vec![(in1, d1.as_view().into()), (in2, d2.into())], &[out_id], None)
                .context("siamese inference")?;
            let v = out[0].as_view();
            let sim = match &v {
                rten::ValueView::FloatTensor(tv) => tv.data().and_then(|d| d.first().copied()).unwrap_or(0.0),
                _ => 0.0,
            };
            if sigmoid(sim) >= SIAMESE_THRESHOLD {
                result.push([b.x as i32, b.y as i32]);
                break;
            }
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn debug_solve_known_image() {
        let bytes = std::fs::read("/tmp/opencode/ga.png").expect("read test image");
        let boxes = solve_image(&bytes).await.expect("solve");
        println!("RUST BOXES: {:?}", boxes);
        println!("EXPECTED:   [[55, 36], [205, 12], [161, 86], [147, 255]]");
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[tokio::test]
    async fn debug_matrix() {
        let bytes = std::fs::read("/tmp/opencode/ga.png").expect("read test image");
        let m = models().await.expect("models");
        let img = image::load_from_memory(&bytes).unwrap().to_rgb8();
        let (smalls, bigs) = detect(m, &img).expect("detect");
        println!("RUST smalls x: {:?}", smalls.iter().map(|(x, c)| (x, c.width(), c.height())).collect::<Vec<_>>());
        println!("RUST bigs: {:?}", bigs.iter().map(|b| (b.x, b.y, b.w, b.h)).collect::<Vec<_>>());
        for (x, g) in &smalls {
            let d1 = siamese_prep(g);
            let mut row = Vec::new();
            for b in &bigs {
                let crop = image::imageops::crop_imm(&img, b.x.max(0.0) as u32, b.y.max(0.0) as u32,
                    b.w.min(img.width() as f32 - b.x.max(0.0)) as u32,
                    b.h.min(img.height() as f32 - b.y.max(0.0)) as u32).to_image();
                let d2 = siamese_prep(&crop);
                let in1 = m.siamese.node_id("input").unwrap();
                let in2 = m.siamese.node_id("input.53").unwrap();
                let out_id = m.siamese.output_ids()[0];
                let out = m.siamese.run(vec![(in1, d1.as_view().into()), (in2, d2.into())], &[out_id], None).unwrap();
                let v = out[0].as_view();
                let sim = match &v { rten::ValueView::FloatTensor(tv) => tv.data().and_then(|d| d.first().copied()).unwrap_or(0.0), _ => 0.0 };
                row.push(format!("{:.3}", sigmoid(sim)));
            }
            println!("glyph@{}: {:?}", x, row);
        }
    }
}
