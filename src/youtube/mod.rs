use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use tokio::fs;

use crate::ai::providers::{create_provider, ChatMessage, LlmConfig, LlmProvider};
use crate::rpc::{
    Chapter, TranscriptSegment, YouTubeBatchItem, YouTubeBatchRequest, YouTubeBatchResponse,
    YouTubeRequest, YouTubeResponse,
};

static YT_DLP_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct YtDlpInfo {
    title: String,
    channel: Option<String>,
    uploader: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    description: Option<String>,
    chapters: Option<Vec<YtDlpChapter>>,
}

#[derive(Debug, Deserialize)]
struct YtDlpChapter {
    start_time: f64,
    title: String,
}

pub async fn extract(request: &YouTubeRequest) -> Result<YouTubeResponse> {
    let yt_dlp = get_yt_dlp_path().await?;

    let info = fetch_video_info(&yt_dlp, &request.url).await?;

    let transcript = if request.include_transcript || request.generate_ai_chapters {
        Some(fetch_transcript(&yt_dlp, &request.url, request.language.as_deref()).await?)
    } else {
        None
    };

    let mut chapters = if request.include_chapters {
        info.chapters.map(|chs| {
            chs.into_iter()
                .map(|c| Chapter {
                    start: c.start_time,
                    title: c.title,
                })
                .collect()
        })
    } else {
        None
    };

    if request.generate_ai_chapters && chapters.is_none() {
        if let Some(ref transcript_segments) = transcript {
            chapters = generate_ai_chapters(
                transcript_segments,
                info.duration.unwrap_or(0.0),
                request.provider.as_deref(),
                request.api_key.as_deref(),
                request.model.as_deref(),
            )
            .await
            .ok();
        }
    }

    Ok(YouTubeResponse {
        title: info.title,
        channel: info.channel.or(info.uploader).unwrap_or_default(),
        duration: info.duration.map(|d| d as u64).unwrap_or(0),
        thumbnail: info.thumbnail.unwrap_or_default(),
        transcript: if request.include_transcript {
            transcript
        } else {
            None
        },
        chapters,
        description: info.description.unwrap_or_default(),
    })
}

pub async fn extract_batch(request: &YouTubeBatchRequest) -> YouTubeBatchResponse {
    use futures::future::join_all;

    let futures: Vec<_> = request
        .urls
        .iter()
        .map(|url| {
            let single_request = YouTubeRequest {
                url: url.clone(),
                include_transcript: request.include_transcript,
                include_chapters: request.include_chapters,
                generate_ai_chapters: request.generate_ai_chapters,
                language: request.language.clone(),
                provider: request.provider.clone(),
                api_key: request.api_key.clone(),
                model: request.model.clone(),
            };
            async move {
                let url_clone = url.clone();
                match extract(&single_request).await {
                    Ok(response) => YouTubeBatchItem {
                        url: url_clone,
                        result: Some(response),
                        error: None,
                    },
                    Err(e) => YouTubeBatchItem {
                        url: url_clone,
                        result: None,
                        error: Some(e.to_string()),
                    },
                }
            }
        })
        .collect();

    let items = join_all(futures).await;
    let success_count = items.iter().filter(|i| i.result.is_some()).count();
    let error_count = items.iter().filter(|i| i.error.is_some()).count();

    YouTubeBatchResponse {
        items,
        success_count,
        error_count,
    }
}

async fn get_yt_dlp_path() -> Result<PathBuf> {
    if let Some(path) = YT_DLP_PATH.get() {
        return Ok(path.clone());
    }

    if let Ok(output) = Command::new("yt-dlp").arg("--version").output() {
        if output.status.success() {
            let path = PathBuf::from("yt-dlp");
            let _ = YT_DLP_PATH.set(path.clone());
            return Ok(path);
        }
    }

    let data_dir = get_data_dir()?;
    let binary_name = if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    let local_path = data_dir.join(binary_name);

    if local_path.exists() {
        let _ = YT_DLP_PATH.set(local_path.clone());
        return Ok(local_path);
    }

    download_yt_dlp_binary(&local_path).await?;
    let _ = YT_DLP_PATH.set(local_path.clone());
    Ok(local_path)
}

fn get_data_dir() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("naidis");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub async fn ensure_yt_dlp() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    let binary_name = if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    let local_path = data_dir.join(binary_name);

    if local_path.exists() {
        return Ok(local_path);
    }

    download_yt_dlp_binary(&local_path).await?;
    Ok(local_path)
}

async fn download_yt_dlp_binary(target_path: &PathBuf) -> Result<()> {
    let download_url = get_yt_dlp_download_url();

    eprintln!("[naidis] Downloading yt-dlp from {}...", download_url);

    let response = reqwest::get(&download_url)
        .await
        .context("Failed to download yt-dlp")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download yt-dlp: HTTP {}", response.status());
    }

    let bytes = response.bytes().await?;
    fs::write(target_path, &bytes).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(target_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(target_path, perms)?;
    }

    eprintln!(
        "[naidis] yt-dlp downloaded successfully to {:?}",
        target_path
    );
    Ok(())
}

fn get_yt_dlp_download_url() -> String {
    let base = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";

    if cfg!(target_os = "windows") {
        format!("{}/yt-dlp.exe", base)
    } else if cfg!(target_os = "macos") {
        format!("{}/yt-dlp_macos", base)
    } else {
        format!("{}/yt-dlp", base)
    }
}

async fn fetch_video_info(yt_dlp: &PathBuf, url: &str) -> Result<YtDlpInfo> {
    let output = Command::new(yt_dlp)
        .args(["--dump-json", "--no-download", "--no-warnings", url])
        .output()
        .context("Failed to execute yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {}", stderr);
    }

    let stdout = String::from_utf8(output.stdout)?;
    let info: YtDlpInfo = serde_json::from_str(&stdout).context("Failed to parse yt-dlp output")?;

    Ok(info)
}

async fn fetch_transcript(
    yt_dlp: &PathBuf,
    url: &str,
    lang: Option<&str>,
) -> Result<Vec<TranscriptSegment>> {
    let temp_dir = std::env::temp_dir();
    let output_template = temp_dir.join("naidis_transcript_%(id)s");

    let mut args = vec![
        "--write-subs".to_string(),
        "--write-auto-subs".to_string(),
        "--skip-download".to_string(),
        "--sub-format".to_string(),
        "json3".to_string(),
        "-o".to_string(),
        output_template.to_string_lossy().to_string(),
    ];

    if let Some(lang) = lang {
        args.push("--sub-langs".to_string());
        args.push(lang.to_string());
    } else {
        args.push("--sub-langs".to_string());
        args.push("en,ko,ja".to_string());
    }

    args.push(url.to_string());

    let output = Command::new(yt_dlp)
        .args(&args)
        .output()
        .context("Failed to execute yt-dlp for transcript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp transcript failed: {}", stderr);
    }

    let video_id = extract_video_id(url)?;
    let subtitle_pattern = format!("naidis_transcript_{}", video_id);

    let mut transcript = Vec::new();

    for entry in std::fs::read_dir(&temp_dir)? {
        let entry = entry?;
        let path = entry.path();
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if filename.starts_with(&subtitle_pattern) && filename.ends_with(".json3") {
            let content = std::fs::read_to_string(&path)?;
            transcript = parse_json3_transcript(&content)?;
            std::fs::remove_file(&path).ok();
            break;
        }
    }

    if transcript.is_empty() {
        for entry in std::fs::read_dir(&temp_dir)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if filename.starts_with(&subtitle_pattern)
                && (filename.ends_with(".vtt") || filename.ends_with(".srt"))
            {
                let content = std::fs::read_to_string(&path)?;
                transcript = parse_vtt_transcript(&content)?;
                std::fs::remove_file(&path).ok();
                break;
            }
        }
    }

    Ok(transcript)
}

fn extract_video_id(url: &str) -> Result<String> {
    let patterns =
        [r"(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/embed/)([a-zA-Z0-9_-]{11})"];

    for pattern in patterns {
        let re = Regex::new(pattern)?;
        if let Some(caps) = re.captures(url) {
            if let Some(id) = caps.get(1) {
                return Ok(id.as_str().to_string());
            }
        }
    }

    anyhow::bail!("Could not extract video ID from URL")
}

fn parse_json3_transcript(content: &str) -> Result<Vec<TranscriptSegment>> {
    #[derive(Deserialize)]
    struct Json3 {
        events: Option<Vec<Json3Event>>,
    }

    #[derive(Deserialize)]
    struct Json3Event {
        #[serde(rename = "tStartMs")]
        t_start_ms: Option<u64>,
        #[serde(rename = "dDurationMs")]
        d_duration_ms: Option<u64>,
        segs: Option<Vec<Json3Seg>>,
    }

    #[derive(Deserialize)]
    struct Json3Seg {
        utf8: Option<String>,
    }

    let json: Json3 = serde_json::from_str(content)?;
    let mut segments = Vec::new();

    if let Some(events) = json.events {
        for event in events {
            if let (Some(start), Some(duration), Some(segs)) =
                (event.t_start_ms, event.d_duration_ms, event.segs)
            {
                let text: String = segs
                    .iter()
                    .filter_map(|s| s.utf8.as_ref())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("");

                if !text.trim().is_empty() {
                    segments.push(TranscriptSegment {
                        start: start as f64 / 1000.0,
                        duration: duration as f64 / 1000.0,
                        text: text.trim().to_string(),
                    });
                }
            }
        }
    }

    Ok(segments)
}

fn parse_vtt_transcript(content: &str) -> Result<Vec<TranscriptSegment>> {
    let mut segments = Vec::new();
    let time_re =
        Regex::new(r"(\d{2}):(\d{2}):(\d{2})\.(\d{3})\s*-->\s*(\d{2}):(\d{2}):(\d{2})\.(\d{3})")?;
    let tag_re = Regex::new(r"<[^>]+>")?;

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if let Some(caps) = time_re.captures(line) {
            let start_h: f64 = caps.get(1).unwrap().as_str().parse()?;
            let start_m: f64 = caps.get(2).unwrap().as_str().parse()?;
            let start_s: f64 = caps.get(3).unwrap().as_str().parse()?;
            let start_ms: f64 = caps.get(4).unwrap().as_str().parse()?;

            let end_h: f64 = caps.get(5).unwrap().as_str().parse()?;
            let end_m: f64 = caps.get(6).unwrap().as_str().parse()?;
            let end_s: f64 = caps.get(7).unwrap().as_str().parse()?;
            let end_ms: f64 = caps.get(8).unwrap().as_str().parse()?;

            let start = start_h * 3600.0 + start_m * 60.0 + start_s + start_ms / 1000.0;
            let end = end_h * 3600.0 + end_m * 60.0 + end_s + end_ms / 1000.0;

            i += 1;
            let mut text = String::new();
            while i < lines.len() && !lines[i].trim().is_empty() && !time_re.is_match(lines[i]) {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(lines[i].trim());
                i += 1;
            }

            if !text.is_empty() {
                let clean_text = tag_re.replace_all(&text, "").to_string();
                segments.push(TranscriptSegment {
                    start,
                    duration: end - start,
                    text: clean_text,
                });
            }
        }

        i += 1;
    }

    Ok(segments)
}

async fn generate_ai_chapters(
    transcript: &[TranscriptSegment],
    duration: f64,
    provider: Option<&str>,
    api_key: Option<&str>,
    model: Option<&str>,
) -> Result<Vec<Chapter>> {
    let transcript_text = transcript
        .iter()
        .map(|s| format!("[{:.0}s] {}", s.start, s.text))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        r#"Analyze this video transcript and identify 5-10 logical chapters/sections.
For each chapter, provide:
1. The start timestamp (in seconds)
2. A concise title (under 50 characters)

Video duration: {:.0} seconds

Transcript:
{}

Respond ONLY with valid JSON array:
[{{"start": 0, "title": "Introduction"}}, {{"start": 120, "title": "Main Topic"}}]"#,
        duration, transcript_text
    );

    let llm_provider = match provider.unwrap_or("local") {
        "openai" => LlmProvider::OpenAI,
        "anthropic" => LlmProvider::Anthropic,
        "groq" => LlmProvider::Groq,
        "zai" | "z.ai" => LlmProvider::Zai,
        "ollama" => LlmProvider::Ollama,
        _ => LlmProvider::Local,
    };

    let response = match llm_provider {
        LlmProvider::Local => {
            let model_name = model.unwrap_or("llama3.2");
            crate::ai::ollama::generate(model_name, &prompt, None).await?
        }
        LlmProvider::Ollama => {
            let model_name = model.unwrap_or("llama3.2");
            crate::ai::ollama::generate(model_name, &prompt, None).await?
        }
        _ => {
            let config = LlmConfig {
                provider: llm_provider,
                api_key: api_key.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                base_url: None,
            };
            let llm = create_provider(&config)?;
            let messages = vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }];
            llm.chat(messages, 1024).await?
        }
    };

    parse_chapters_json(&response)
}

fn parse_chapters_json(response: &str) -> Result<Vec<Chapter>> {
    let json_start = response.find('[').context("No JSON array found")?;
    let json_end = response.rfind(']').context("No JSON array end found")?;
    let json_str = &response[json_start..=json_end];

    #[derive(Deserialize)]
    struct ChapterJson {
        start: f64,
        title: String,
    }

    let chapters: Vec<ChapterJson> =
        serde_json::from_str(json_str).context("Failed to parse chapters JSON")?;

    Ok(chapters
        .into_iter()
        .map(|c| Chapter {
            start: c.start,
            title: c.title,
        })
        .collect())
}
