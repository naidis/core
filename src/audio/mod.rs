use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub segments: Vec<TranscriptionSegment>,
    pub language: String,
    pub duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeRequest {
    pub audio_path: String,
    pub model_path: Option<String>,
    pub language: Option<String>,
    pub translate: Option<bool>,
}

static MODEL_PATH: std::sync::OnceLock<std::sync::RwLock<Option<String>>> =
    std::sync::OnceLock::new();

fn get_model_lock() -> &'static std::sync::RwLock<Option<String>> {
    MODEL_PATH.get_or_init(|| std::sync::RwLock::new(None))
}

pub fn set_model_path(path: &str) {
    let mut guard = get_model_lock().write().unwrap();
    *guard = Some(path.to_string());
}

fn get_default_model_path() -> Result<String> {
    let guard = get_model_lock().read().unwrap();
    if let Some(ref path) = *guard {
        return Ok(path.clone());
    }

    let data_dir = dirs::data_dir()
        .context("Could not find data directory")?
        .join("naidis")
        .join("models");

    std::fs::create_dir_all(&data_dir)?;

    let model_path = data_dir.join("ggml-base.en.bin");
    Ok(model_path.to_string_lossy().to_string())
}

pub async fn transcribe(request: &TranscribeRequest) -> Result<TranscriptionResult> {
    let audio_path = request.audio_path.clone();
    let model_path = request
        .model_path
        .clone()
        .unwrap_or_else(|| get_default_model_path().unwrap_or_default());
    let language = request.language.clone();
    let translate = request.translate.unwrap_or(false);

    tokio::task::spawn_blocking(move || {
        transcribe_sync(&audio_path, &model_path, language.as_deref(), translate)
    })
    .await
    .context("Transcription task failed")?
}

fn transcribe_sync(
    audio_path: &str,
    model_path: &str,
    language: Option<&str>,
    translate: bool,
) -> Result<TranscriptionResult> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    if !Path::new(model_path).exists() {
        anyhow::bail!(
            "Whisper model not found at: {}. Please download a model first.",
            model_path
        );
    }

    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .context("Failed to load Whisper model")?;

    let audio_data = load_audio_file(audio_path)?;
    let duration = audio_data.len() as f64 / 16000.0;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    if let Some(lang) = language {
        params.set_language(Some(lang));
    } else {
        params.set_language(Some("en"));
    }

    params.set_translate(translate);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    let mut state = ctx
        .create_state()
        .context("Failed to create Whisper state")?;

    state
        .full(params, &audio_data)
        .context("Transcription failed")?;

    let num_segments = state
        .full_n_segments()
        .context("Failed to get segment count")?;

    let mut segments = Vec::new();
    let mut full_text = String::new();

    for i in 0..num_segments {
        let start = state
            .full_get_segment_t0(i)
            .context("Failed to get segment start")? as f64
            / 100.0;
        let end = state
            .full_get_segment_t1(i)
            .context("Failed to get segment end")? as f64
            / 100.0;
        let text = state
            .full_get_segment_text(i)
            .context("Failed to get segment text")?;

        if !full_text.is_empty() {
            full_text.push(' ');
        }
        full_text.push_str(text.trim());

        segments.push(TranscriptionSegment {
            start,
            end,
            text: text.trim().to_string(),
        });
    }

    let detected_language = language.unwrap_or("en").to_string();

    Ok(TranscriptionResult {
        text: full_text,
        segments,
        language: detected_language,
        duration,
    })
}

fn load_audio_file(path: &str) -> Result<Vec<f32>> {
    let path = Path::new(path);
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "wav" => load_wav_file(path),
        "mp3" | "m4a" | "ogg" | "flac" | "webm" => {
            anyhow::bail!(
                "Audio format '{}' requires ffmpeg conversion. Please convert to WAV first or ensure ffmpeg is installed.",
                extension
            )
        }
        _ => anyhow::bail!("Unsupported audio format: {}", extension),
    }
}

fn load_wav_file(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).context("Failed to open WAV file")?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / max_val)
                .collect()
        }
    };

    let mono: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    if sample_rate != 16000 {
        Ok(resample(&mono, sample_rate, 16000))
    } else {
        Ok(mono)
    }
}

fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut resampled = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - idx_floor as f64;

        let sample = samples[idx_floor] * (1.0 - frac as f32) + samples[idx_ceil] * frac as f32;
        resampled.push(sample);
    }

    resampled
}

pub async fn download_model(model_name: Option<&str>) -> Result<String> {
    let model = model_name.unwrap_or("base.en");
    let model_filename = format!("ggml-{}.bin", model);

    let data_dir = dirs::data_dir()
        .context("Could not find data directory")?
        .join("naidis")
        .join("models");

    std::fs::create_dir_all(&data_dir)?;

    let model_path = data_dir.join(&model_filename);

    if model_path.exists() {
        return Ok(model_path.to_string_lossy().to_string());
    }

    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        model_filename
    );

    tracing::info!("Downloading Whisper model from: {}", url);

    let response = reqwest::get(&url)
        .await
        .context("Failed to download Whisper model")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download model: HTTP {}", response.status());
    }

    let bytes = response.bytes().await?;
    tokio::fs::write(&model_path, bytes)
        .await
        .context("Failed to save model file")?;

    tracing::info!("Whisper model saved to: {:?}", model_path);

    set_model_path(&model_path.to_string_lossy());

    Ok(model_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample() {
        let samples = vec![0.0, 0.5, 1.0, 0.5, 0.0];
        let resampled = resample(&samples, 8000, 16000);
        assert!(resampled.len() > samples.len());
    }
}
