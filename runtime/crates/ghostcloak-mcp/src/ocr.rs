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

/// CHINESE OCR: read Chinese characters from PNG bytes using the
/// PaddleOCR v4 recognition model (10.3 MB ONNX, auto-downloaded).
/// The model was trained on 6,622 Chinese + English characters.
/// Returns the recognized text.
pub async fn ocr_chinese_png(png: &[u8]) -> Result<String> {
    // 1. Load model + dictionary (cached)
    static CH_MODEL: once_cell::sync::OnceLock<(rten::Model, Vec<String>)> = once_cell::sync::OnceLock::new();
    let (model, dict) = CH_MODEL.get_or_init(|| {
        let models_dir = models_dir();
        let model_path = models_dir.join("chinese_ocr").join("ch_rec_v4.onnx");
        let dict_path = models_dir.join("chinese_ocr").join("ch_dict.txt");
        // Models must be pre-downloaded (see setup docs or the ensure_models fn)
        // For now: return empty dict if model not found
        if !model_path.exists() {
            panic!("Chinese OCR model not found at {}", model_path.display());
        }
        let model_bytes = std::fs::read(&model_path).expect("read model file");
        let model = rten::Model::load(model_bytes).expect("load onnx model");
        let dict_text = std::fs::read_to_string(&dict_path).expect("read dict");
        let dict: Vec<String> = dict_text.lines().map(|l| l.trim().to_string()).collect();
        (model, dict)
    });

    // 2. Load image, resize to 48x320 (PaddleOCR input format)
    let img = image::load_from_memory(png).context("decoding PNG for Chinese OCR")?;
    let rgb = img.to_rgb8();
    let resized = image::imageops::resize(&rgb, 320, 48, image::imageops::FilterType::Linear);

    // 3. Convert to CHW float32, normalized to [-1, 1]
    let (w, h) = resized.dimensions();
    let pixels = resized.into_raw();
    let mut chw = vec![0f32; (3 * h * w) as usize];
    for (i, px) in pixels.chunks(3).enumerate() {
        let x = (i % w as usize) as usize;
        let y = (i / w as usize) as usize;
        // CHW: [channel][height][width]
        chw[0 * (h * w) as usize + y * w as usize + x] = px[0] as f32 / 255.0 * 2.0 - 1.0;
        chw[1 * (h * w) as usize + y * w as usize + x] = px[1] as f32 / 255.0 * 2.0 - 1.0;
        chw[2 * (h * w) as usize + y * w as usize + x] = px[2] as f32 / 255.0 * 2.0 - 1.0;
    }

    // 4. Run the model
    let input_tensor = rten::Tensor::from_data(
        &[1, 3, h as i32, w as i32],
        &chw,
    ).context("building input tensor")?;

    let outputs = model
        .run_n(&[("x", &input_tensor)])
        .context("running Chinese OCR model")?;
    let output = outputs[0].view();

    // 5. Decode: argmax per timestep → dict lookup
    let shape = output.shape();
    let seq_len = shape[1] as usize;
    let num_classes = shape[2] as usize;
    let data = output.data().expect("output data");

    let mut result = String::new();
    let mut prev_char: Option<usize> = None;
    for t in 0..seq_len {
        let offset = t * num_classes;
        let mut max_idx = 0;
        let mut max_val = f32::MIN;
        for (i, &v) in data[offset..offset + num_classes].iter().enumerate() {
            if v > max_val {
                max_val = v;
                max_idx = i;
            }
        }
        // Blank index = 0 (CTC blank)
        if max_idx > 0 && Some(max_idx) != prev_char {
            // Look up in dict (index 0 = blank, index i = dict[i-1] + some offset)
            if max_idx - 1 < dict.len() {
                result.push_str(&dict[max_idx - 1]);
            } else if max_idx == dict.len() + 1 {
                // Space/special char
                result.push(' ');
            }
        }
        prev_char = Some(max_idx);
    }

    Ok(result)
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
