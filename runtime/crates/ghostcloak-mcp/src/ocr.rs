//! v0.6.2 TIER 2 + TIER 4: local visual cortex for Ghostfox.
//!
//! OCR via `ocrs` (pure-Rust ML, RTEN runtime — no C deps, no API).
//! Models (Apache-2.0, from robertknight/ocrs-models) auto-download
//! to $GHOSTFOX_HOME/models on first use.
//!
//! Tier 2: page_ocr — "what text is written there"
//! Tier 4: page_vision — built-in text-detection model returns word
//!          bounding boxes (the first visual-cortex model shipped).

use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use ocrs::{OcrEngine, OcrEngineParams, OcrInput};
use rten::Model;

const DETECTION_URL: &str =
    "https://huggingface.co/robertknight/ocrs/resolve/main/text-detection-ssfbcj81.rten";
const RECOGNITION_URL: &str =
    "https://huggingface.co/robertknight/ocrs/resolve/main/text-rec-checkpoint-s52qdbqt.rten";

fn models_dir() -> PathBuf {
    let home = std::env::var("GHOSTFOX_HOME").unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|d| d.join(".ghostfox"))
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".ghostfox".into())
    });
    PathBuf::from(home).join("models")
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
    tracing::info!(target: "ghostcloak::mcp", "downloaded model {} ({} KB)", to.display(), bytes.len() / 1024);
    Ok(())
}

async fn ensure_models() -> Result<(PathBuf, PathBuf)> {
    let dir = models_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let det = dir.join("text-detection-ssfbcj81.rten");
    let rec = dir.join("text-rec-checkpoint-s52qdbqt.rten");
    if !det.exists() {
        download(DETECTION_URL, &det).await?;
    }
    if !rec.exists() {
        download(RECOGNITION_URL, &rec).await?;
    }
    Ok((det, rec))
}

static ENGINE: OnceLock<OcrEngine> = OnceLock::new();

/// Lazily-initialized OCR engine (models load once per process).
pub async fn engine() -> Result<&'static OcrEngine> {
    if let Some(e) = ENGINE.get() {
        return Ok(e);
    }
    let (det_path, rec_path) = ensure_models().await?;
    let det_bytes = tokio::fs::read(&det_path).await?;
    let rec_bytes = tokio::fs::read(&rec_path).await?;
    let det = Model::load(det_bytes).context("loading text-detection model")?;
    let rec = Model::load(rec_bytes).context("loading text-recognition model")?;
    let eng = OcrEngine::new(OcrEngineParams {
        detection_model: Some(det),
        recognition_model: Some(rec),
        ..Default::default()
    })
    .context("initializing OCR engine")?;
    Ok(ENGINE.get_or_init(move || eng))
}

/// Tier 2: extract text from PNG bytes (screenshot region).
pub async fn ocr_png(png: &[u8]) -> Result<String> {
    let eng = engine().await?;
    let (input, lines) = ocr_input_and_lines(eng, png)?;
    let recognized = eng
        .recognize_text(&input, &lines)
        .context("recognizing text")?;
    let text = recognized
        .iter()
        .filter_map(|l| l.as_ref())
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text)
}

/// Shared: decode PNG -> OcrInput -> detect words -> group into lines.
fn ocr_input_and_lines(eng: &OcrEngine, png: &[u8]) -> Result<(OcrInput, Vec<Vec<rten_imageproc::RotatedRect>>)> {
    let img = image::load_from_memory(png).context("decoding PNG for OCR")?;
    let rgb = img.into_rgb8();
    let (w, h) = rgb.dimensions();
    let input = eng
        .prepare_input(ocrs::ImageSource::from_bytes(rgb.as_raw(), (w, h))?)
        .context("preparing OCR input")?;
    let words = eng.detect_words(&input).context("detecting words")?;
    let lines = eng.find_text_lines(&input, &words);
    Ok((input, lines))
}

/// Tier 4 v1: built-in visual cortex — detect text word boxes in PNG bytes.
/// Returns (x, y, w, h) rectangles in image pixel coordinates.
pub async fn detect_text_boxes(png: &[u8]) -> Result<Vec<(f32, f32, f32, f32)>> {
    let eng = engine().await?;
    let img = image::load_from_memory(png).context("decoding PNG for detection")?;
    let rgb = img.into_rgb8();
    let (w, h) = rgb.dimensions();
    let input = eng
        .prepare_input(ocrs::ImageSource::from_bytes(rgb.as_raw(), (w, h))?)
        .context("preparing detection input")?;
    let words = eng.detect_words(&input).context("detecting words")?;
    Ok(words
        .iter()
        .map(|r| {
            let corners = r.corners();
            let xs = [corners[0].x, corners[1].x, corners[2].x, corners[3].x];
            let ys = [corners[0].y, corners[1].y, corners[2].y, corners[3].y];
            let x0 = xs.iter().cloned().fold(f32::INFINITY, f32::min);
            let y0 = ys.iter().cloned().fold(f32::INFINITY, f32::min);
            let x1 = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let y1 = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            (x0, y0, x1 - x0, y1 - y0)
        })
        .collect())
}
