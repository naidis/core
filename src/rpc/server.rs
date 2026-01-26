use anyhow::Result;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tower_http::cors::{Any, CorsLayer};

use super::types::*;
use crate::ai;
use crate::audio;
use crate::dataview;
use crate::epub;
use crate::git;
use crate::highlights;
use crate::integrations::{gcal, hoarder, kindle, readwise, todoist, wallabag};
use crate::labels;
use crate::newsletter;
use crate::nlp;
use crate::pdf;
use crate::periodic;
use crate::reading;
use crate::rss;
use crate::spaced_repetition;
use crate::tables;
use crate::tasks;
use crate::tier::{
    self, check_ai_limit, check_pro_feature, check_rag_limit, check_rss_limit, check_sr_limit,
    extract_tier_from_headers, ProFeature, SharedUsageTracker,
};
use crate::tts;
use crate::utils;
use crate::web_clip;
use crate::youtube;

#[derive(Clone)]
pub struct AppState {
    pub usage_tracker: SharedUsageTracker,
}

fn get_data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("naidis")
}

pub async fn run_http_server(host: &str, port: u16) -> Result<()> {
    let data_dir = get_data_dir();
    std::fs::create_dir_all(&data_dir)?;

    let usage_tracker = tier::create_shared_tracker(data_dir.clone())?;
    let state = Arc::new(AppState { usage_tracker });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/status", get(system_status))
        .route("/api/tier/usage", get(tier_usage_stats))
        .route("/api/model/download", post(model_download))
        .route("/api/deps/download", post(deps_download))
        .route("/api/deps/install", post(deps_install))
        .route("/api/youtube/extract", post(youtube_extract))
        .route("/api/youtube/batch", post(youtube_extract_batch))
        .route("/api/webclip/extract", post(webclip_extract))
        .route("/api/rss/fetch", post(rss_fetch))
        .route("/api/pdf/extract", post(pdf_extract))
        .route("/api/pdf/tables", post(pdf_extract_tables))
        .route("/api/ai/chat", post(ai_chat))
        .route("/api/ai/summarize", post(ai_summarize))
        .route("/api/ai/index", post(ai_index))
        .route("/api/ai/search", post(ai_search))
        .route("/api/ai/rag", post(ai_rag))
        .route("/api/wallabag/sync", post(wallabag_sync))
        .route("/api/hoarder/sync", post(hoarder_sync))
        .route("/api/readwise/sync", post(readwise_sync))
        .route("/api/calc", post(calculate))
        .route("/api/calc/convert", post(convert_unit))
        .route("/api/snippets", get(list_snippets))
        .route("/api/snippets", post(create_snippet))
        .route("/api/snippets/expand", post(expand_snippet))
        .route("/api/snippets/{id}", axum::routing::put(update_snippet))
        .route(
            "/api/snippets/{id}",
            axum::routing::delete(delete_snippet_handler),
        )
        .route("/api/datetime/format", post(format_datetime))
        .route("/api/datetime/parse", post(parse_datetime))
        .route("/api/datetime/calc", post(calc_datetime))
        .route("/api/datetime/diff", post(diff_datetime))
        .route("/api/datetime/quick", post(quick_date))
        .route("/api/emoji/search", post(search_emoji))
        .route("/api/emoji/shortcode", post(get_emoji_by_shortcode))
        .route("/api/emoji/groups", get(list_emoji_groups))
        .route("/api/favorites", get(list_favorites_handler))
        .route("/api/favorites", post(add_favorite))
        .route("/api/favorites/toggle", post(toggle_favorite))
        .route(
            "/api/favorites/{id}",
            axum::routing::delete(remove_favorite),
        )
        .route("/api/vault/save", post(vault_save))
        .route("/api/vault/read", post(vault_read))
        .route("/api/vault/list", post(vault_list))
        .route("/api/vault/delete", post(vault_delete))
        .route("/api/vault/move", post(vault_move))
        .route("/api/vault/search", post(vault_search))
        .route("/api/history", get(list_history_handler))
        .route("/api/history", post(add_history))
        .route("/api/history/clear", post(clear_history))
        .route("/api/history/frequent", get(get_frequent_commands))
        .route("/api/layouts", get(list_layouts))
        .route("/api/layouts", post(save_layout))
        .route("/api/layouts/{id}", get(get_layout))
        .route("/api/layouts/{id}", axum::routing::put(update_layout))
        .route("/api/layouts/{id}", axum::routing::delete(delete_layout))
        .route("/api/links/suggest", post(suggest_links))
        .route("/api/links/backlinks", post(find_backlinks))
        .route("/api/tasks/parse", post(tasks_parse))
        .route("/api/tasks/query", post(tasks_query))
        .route("/api/nlp/parse-date", post(nlp_parse_date))
        .route("/api/nlp/suggest-dates", post(nlp_suggest_dates))
        .route("/api/dataview/parse", post(dataview_parse))
        .route("/api/dataview/query", post(dataview_query))
        .route("/api/dataview/table", post(dataview_table))
        .route("/api/tables/parse", post(tables_parse))
        .route("/api/tables/format", post(tables_format))
        .route("/api/tables/sort", post(tables_sort))
        .route("/api/tables/add-row", post(tables_add_row))
        .route("/api/tables/add-column", post(tables_add_column))
        .route("/api/periodic/daily", post(periodic_daily))
        .route("/api/periodic/weekly", post(periodic_weekly))
        .route("/api/periodic/monthly", post(periodic_monthly))
        .route("/api/periodic/quarterly", post(periodic_quarterly))
        .route("/api/periodic/yearly", post(periodic_yearly))
        .route("/api/periodic/navigate", post(periodic_navigate))
        .route("/api/todoist/tasks", post(todoist_fetch_tasks))
        .route("/api/todoist/projects", post(todoist_fetch_projects))
        .route("/api/todoist/create", post(todoist_create_task))
        .route("/api/todoist/complete", post(todoist_complete_task))
        .route("/api/todoist/sync", post(todoist_sync))
        .route("/api/gcal/events", post(gcal_fetch_events))
        .route("/api/gcal/today", post(gcal_fetch_today))
        .route("/api/gcal/create", post(gcal_create_event))
        .route("/api/gcal/sync", post(gcal_sync))
        .route("/api/git/status", post(git_status_handler))
        .route("/api/git/commit", post(git_commit_handler))
        .route("/api/git/push", post(git_push_handler))
        .route("/api/git/pull", post(git_pull_handler))
        .route("/api/git/sync", post(git_sync_handler))
        .route("/api/git/log", post(git_log_handler))
        .route("/api/git/diff", post(git_diff_handler))
        .route("/api/git/init", post(git_init_handler))
        .route("/api/highlights", post(highlights_create))
        .route("/api/highlights/query", post(highlights_query))
        .route("/api/highlights/update", post(highlights_update))
        .route("/api/highlights/delete", post(highlights_delete))
        .route("/api/highlights/export", post(highlights_export))
        .route("/api/reading/save", post(reading_save))
        .route("/api/reading/query", post(reading_query))
        .route("/api/reading/get", post(reading_get))
        .route("/api/reading/update", post(reading_update))
        .route("/api/reading/delete", post(reading_delete))
        .route("/api/reading/archive", post(reading_archive))
        .route("/api/reading/favorite", post(reading_toggle_favorite))
        .route("/api/reading/stats", get(reading_stats))
        .route("/api/reading/labels", get(reading_labels))
        .route("/api/epub/parse", post(epub_parse))
        .route("/api/epub/to-markdown", post(epub_to_markdown))
        .route("/api/epub/metadata", post(epub_metadata))
        .route("/api/epub/chapter", post(epub_chapter))
        .route("/api/newsletter/fetch", post(newsletter_fetch))
        .route("/api/newsletter/query", post(newsletter_query))
        .route("/api/newsletter/get", post(newsletter_get))
        .route("/api/newsletter/read", post(newsletter_mark_read))
        .route("/api/newsletter/star", post(newsletter_toggle_star))
        .route("/api/newsletter/delete", post(newsletter_delete))
        .route("/api/newsletter/to-markdown", post(newsletter_to_markdown))
        .route("/api/newsletter/senders", get(newsletter_senders))
        .route("/api/labels", get(labels_list))
        .route("/api/labels", post(labels_create))
        .route("/api/labels/update", post(labels_update))
        .route("/api/labels/delete", post(labels_delete))
        .route("/api/labels/tree", get(labels_tree))
        .route("/api/labels/stats", get(labels_stats))
        .route("/api/labels/merge", post(labels_merge))
        .route("/api/labels/search", post(labels_search))
        .route("/api/tts/speak", post(tts_speak))
        .route("/api/tts/stop", post(tts_stop))
        .route("/api/tts/voices", get(tts_voices))
        .route("/api/tts/status", get(tts_status))
        .route("/api/tts/read-article", post(tts_read_article))
        .route("/api/ollama/status", get(ollama_status))
        .route("/api/ollama/models", get(ollama_models))
        .route("/api/ollama/generate", post(ollama_generate))
        .route("/api/ollama/chat", post(ollama_chat))
        .route("/api/sr/config", get(sr_get_config))
        .route("/api/sr/config", post(sr_update_config))
        .route("/api/sr/highlight/register", post(sr_register_highlight))
        .route("/api/sr/highlight/review", post(sr_review_highlight))
        .route("/api/sr/mastery", post(sr_create_mastery_card))
        .route("/api/sr/mastery/review", post(sr_review_mastery_card))
        .route("/api/sr/mastery/delete", post(sr_delete_mastery_card))
        .route("/api/sr/session/create", post(sr_create_session))
        .route("/api/sr/session/due", get(sr_get_due_counts))
        .route(
            "/api/sr/frequency/document",
            post(sr_set_document_frequency),
        )
        .route("/api/sr/frequency/source", post(sr_set_source_frequency))
        .route("/api/sr/stats", get(sr_get_stats))
        .route("/api/kindle/sync", post(kindle_sync))
        .route("/api/ai/related", post(ai_related_notes))
        .route("/api/audio/transcribe", post(audio_transcribe))
        .route("/api/audio/model/download", post(audio_model_download))
        .route("/rpc", post(json_rpc_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("HTTP server listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn run_stdio_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut stdout = stdout;

    tracing::info!("JSON-RPC server running on stdio");

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let response = handle_jsonrpc_request(line).await;

        stdout.write_all(response.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

async fn health_check() -> &'static str {
    "ok"
}

async fn tier_usage_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tracker = state.usage_tracker.read().await;
    let stats = tracker.get_stats();
    (StatusCode::OK, Json(serde_json::to_value(stats).unwrap()))
}

async fn system_status() -> impl IntoResponse {
    let mut dependencies = Vec::new();

    let ytdlp_installed = check_command("yt-dlp", &["--version"]).await;
    dependencies.push(DependencyStatus {
        name: "yt-dlp".to_string(),
        installed: ytdlp_installed.0,
        version: ytdlp_installed.1,
        install_hint:
            "macOS: brew install yt-dlp\nWindows: choco install yt-dlp\nLinux: pip install yt-dlp"
                .to_string(),
    });

    let tesseract_installed = check_command("tesseract", &["--version"]).await;
    dependencies.push(DependencyStatus {
        name: "tesseract".to_string(),
        installed: tesseract_installed.0,
        version: tesseract_installed.1,
        install_hint: "macOS: brew install tesseract tesseract-lang\nWindows: choco install tesseract\nLinux: apt install tesseract-ocr".to_string(),
    });

    let pdftoppm_installed = check_command("pdftoppm", &["-v"]).await;
    dependencies.push(DependencyStatus {
        name: "pdftoppm".to_string(),
        installed: pdftoppm_installed.0,
        version: pdftoppm_installed.1,
        install_hint: "macOS: brew install poppler\nWindows: choco install poppler\nLinux: apt install poppler-utils".to_string(),
    });

    let magick_installed = check_command("magick", &["--version"]).await;
    dependencies.push(DependencyStatus {
        name: "imagemagick".to_string(),
        installed: magick_installed.0,
        version: magick_installed.1,
        install_hint: "macOS: brew install imagemagick\nWindows: choco install imagemagick\nLinux: apt install imagemagick".to_string(),
    });

    let model_loaded = false;
    let ai_model = ModelStatus {
        loaded: model_loaded,
        model_name: if model_loaded {
            Some("TinyLlama-1.1B".to_string())
        } else {
            None
        },
        model_size: if model_loaded {
            Some("~700MB".to_string())
        } else {
            None
        },
        download_progress: None,
    };

    let ready = ytdlp_installed.0;

    let response = SystemStatusResponse {
        dependencies,
        ai_model,
        ready,
    };

    (StatusCode::OK, Json(response))
}

async fn check_command(cmd: &str, args: &[&str]) -> (bool, Option<String>) {
    match tokio::process::Command::new(cmd).args(args).output().await {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string());
            (true, version)
        }
        _ => (false, None),
    }
}

async fn model_download(Json(request): Json<ModelDownloadRequest>) -> impl IntoResponse {
    let model_id = request
        .model_id
        .unwrap_or_else(|| "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF".to_string());

    match ai::download_model(&model_id).await {
        Ok(model_name) => (
            StatusCode::OK,
            Json(ModelDownloadResponse {
                success: true,
                message: "Model downloaded successfully".to_string(),
                model_name: Some(model_name),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ModelDownloadResponse {
                success: false,
                message: format!("Download failed: {}", e),
                model_name: None,
            }),
        ),
    }
}

async fn deps_download(Json(request): Json<DependencyDownloadRequest>) -> impl IntoResponse {
    match request.name.as_str() {
        "yt-dlp" => match youtube::ensure_yt_dlp().await {
            Ok(path) => (
                StatusCode::OK,
                Json(DependencyDownloadResponse {
                    success: true,
                    message: "yt-dlp downloaded successfully".to_string(),
                    path: Some(path.to_string_lossy().to_string()),
                }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DependencyDownloadResponse {
                    success: false,
                    message: format!("Download failed: {}", e),
                    path: None,
                }),
            ),
        },
        _ => (
            StatusCode::BAD_REQUEST,
            Json(DependencyDownloadResponse {
                success: false,
                message: format!(
                    "Unknown dependency: {}. Only 'yt-dlp' can be auto-downloaded.",
                    request.name
                ),
                path: None,
            }),
        ),
    }
}

async fn deps_install(Json(request): Json<DependencyInstallRequest>) -> impl IntoResponse {
    let install_config = get_install_config(&request.name);

    if install_config.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(DependencyInstallResponse {
                success: false,
                message: format!("Unknown dependency: {}", request.name),
                method: "none".to_string(),
            }),
        );
    }

    let config = install_config.unwrap();

    #[cfg(target_os = "macos")]
    {
        if has_homebrew().await {
            return run_install_command("brew", &["install"], &config.brew_packages, "homebrew")
                .await;
        }
    }

    #[cfg(target_os = "windows")]
    {
        if has_chocolatey().await {
            return run_install_command(
                "choco",
                &["install", "-y"],
                &config.choco_packages,
                "chocolatey",
            )
            .await;
        }
        if has_winget().await {
            return run_install_command(
                "winget",
                &[
                    "install",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                ],
                &config.winget_packages,
                "winget",
            )
            .await;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if has_apt().await {
            return run_install_command(
                "sudo",
                &["apt", "install", "-y"],
                &config.apt_packages,
                "apt",
            )
            .await;
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(DependencyInstallResponse {
            success: false,
            message: "No supported package manager found. Please install manually.".to_string(),
            method: "none".to_string(),
        }),
    )
}

struct InstallConfig {
    brew_packages: Vec<&'static str>,
    choco_packages: Vec<&'static str>,
    winget_packages: Vec<&'static str>,
    apt_packages: Vec<&'static str>,
}

fn get_install_config(name: &str) -> Option<InstallConfig> {
    match name {
        "yt-dlp" => Some(InstallConfig {
            brew_packages: vec!["yt-dlp"],
            choco_packages: vec!["yt-dlp"],
            winget_packages: vec!["yt-dlp.yt-dlp"],
            apt_packages: vec!["yt-dlp"],
        }),
        "tesseract" => Some(InstallConfig {
            brew_packages: vec!["tesseract", "tesseract-lang"],
            choco_packages: vec!["tesseract"],
            winget_packages: vec!["UB-Mannheim.TesseractOCR"],
            apt_packages: vec![
                "tesseract-ocr",
                "tesseract-ocr-kor",
                "tesseract-ocr-jpn",
                "tesseract-ocr-chi-sim",
            ],
        }),
        "pdftoppm" | "poppler" => Some(InstallConfig {
            brew_packages: vec!["poppler"],
            choco_packages: vec!["poppler"],
            winget_packages: vec![],
            apt_packages: vec!["poppler-utils"],
        }),
        "imagemagick" => Some(InstallConfig {
            brew_packages: vec!["imagemagick"],
            choco_packages: vec!["imagemagick"],
            winget_packages: vec!["ImageMagick.ImageMagick"],
            apt_packages: vec!["imagemagick"],
        }),
        _ => None,
    }
}

async fn has_homebrew() -> bool {
    tokio::process::Command::new("brew")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
async fn has_chocolatey() -> bool {
    tokio::process::Command::new("choco")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
async fn has_winget() -> bool {
    tokio::process::Command::new("winget")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
async fn has_apt() -> bool {
    tokio::process::Command::new("apt")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn run_install_command(
    cmd: &str,
    base_args: &[&str],
    packages: &[&str],
    method: &str,
) -> (StatusCode, Json<DependencyInstallResponse>) {
    if packages.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(DependencyInstallResponse {
                success: false,
                message: format!("No packages available for {}", method),
                method: method.to_string(),
            }),
        );
    }

    let mut args: Vec<&str> = base_args.to_vec();
    args.extend(packages);

    eprintln!("[naidis] Running: {} {}", cmd, args.join(" "));

    match tokio::process::Command::new(cmd).args(&args).output().await {
        Ok(output) if output.status.success() => (
            StatusCode::OK,
            Json(DependencyInstallResponse {
                success: true,
                message: format!("Installed via {}", method),
                method: method.to_string(),
            }),
        ),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DependencyInstallResponse {
                    success: false,
                    message: format!("Install failed: {}", stderr),
                    method: method.to_string(),
                }),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DependencyInstallResponse {
                success: false,
                message: format!("Failed to run {}: {}", cmd, e),
                method: method.to_string(),
            }),
        ),
    }
}

async fn youtube_extract(
    headers: HeaderMap,
    Json(request): Json<YouTubeRequest>,
) -> axum::response::Response {
    if request.generate_ai_chapters {
        let tier = extract_tier_from_headers(&headers);
        if let Err(err) = check_pro_feature(&tier, ProFeature::YoutubeAiChapters) {
            return err.into_response();
        }
    }

    match youtube::extract(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn youtube_extract_batch(
    headers: HeaderMap,
    Json(request): Json<YouTubeBatchRequest>,
) -> axum::response::Response {
    let tier = extract_tier_from_headers(&headers);
    let limits = tier::limits::FreeLimits::get();

    if !tier.is_pro() && request.urls.len() > limits.youtube_batch_size as usize {
        return tier::middleware::TierErrorResponse::pro_only(
            tier::limits::ProFeature::YoutubeBatch.as_str(),
        )
        .into_response();
    }

    if request.generate_ai_chapters {
        if let Err(err) = check_pro_feature(&tier, ProFeature::YoutubeAiChapters) {
            return err.into_response();
        }
    }

    let response = youtube::extract_batch(&request).await;
    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap()),
    )
        .into_response()
}

async fn webclip_extract(Json(request): Json<WebClipRequest>) -> impl IntoResponse {
    match web_clip::extract(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn rss_fetch(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(request): Json<RssRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);

    if let Err(err) = check_rss_limit(&tier, &state.usage_tracker, &request.url).await {
        return err.into_response();
    }

    match rss::fetch(&request).await {
        Ok(response) => {
            let mut tracker = state.usage_tracker.write().await;
            let _ = tracker.add_rss_feed(&request.url);
            (
                StatusCode::OK,
                Json(serde_json::to_value(response).unwrap()),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn pdf_extract(headers: HeaderMap, Json(request): Json<PdfRequest>) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);

    if request.ocr {
        if let Err(err) = check_pro_feature(&tier, ProFeature::PdfOcr) {
            return err.into_response();
        }
    }

    match pdf::extract(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn pdf_extract_tables(
    headers: HeaderMap,
    Json(request): Json<PdfTablesRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);

    if let Err(err) = check_pro_feature(&tier, ProFeature::PdfTables) {
        return err.into_response();
    }

    match pdf::extract_tables_only(&request.path).await {
        Ok(tables) => (StatusCode::OK, Json(serde_json::json!({"tables": tables}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn ai_chat(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(request): Json<AiChatRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);

    if let Err(err) = check_ai_limit(&tier, &state.usage_tracker).await {
        return err.into_response();
    }

    match ai::chat(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn ai_summarize(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(request): Json<AiSummarizeRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);

    if let Err(err) = check_ai_limit(&tier, &state.usage_tracker).await {
        return err.into_response();
    }

    match ai::summarize(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn ai_index(Json(request): Json<AiIndexRequest>) -> impl IntoResponse {
    match ai::index_notes(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn ai_search(Json(request): Json<AiSearchRequest>) -> impl IntoResponse {
    match ai::search_notes(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn ai_rag(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(request): Json<AiRagRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);

    if let Err(err) = check_rag_limit(&tier, &state.usage_tracker).await {
        return err.into_response();
    }

    match ai::rag_query(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn wallabag_sync(
    headers: HeaderMap,
    Json(request): Json<WallabagSyncRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncWallabag) {
        return err.into_response();
    }
    match wallabag::sync(&request.config, request.limit).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn hoarder_sync(
    headers: HeaderMap,
    Json(request): Json<HoarderSyncRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncHoarder) {
        return err.into_response();
    }
    match hoarder::sync(&request.config, request.limit).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn readwise_sync(
    headers: HeaderMap,
    Json(request): Json<ReadwiseSyncRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncReadwise) {
        return err.into_response();
    }
    match readwise::sync(&request.config, request.updated_after.as_deref()).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn calculate(Json(request): Json<CalcRequest>) -> impl IntoResponse {
    match utils::calculator::calculate(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn convert_unit(Json(request): Json<UnitConvertRequest>) -> impl IntoResponse {
    match utils::calculator::convert_unit(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn create_snippet(Json(request): Json<SnippetCreateRequest>) -> impl IntoResponse {
    match utils::snippets::create_snippet(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn update_snippet(
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(mut request): Json<SnippetUpdateRequest>,
) -> impl IntoResponse {
    request.id = id;
    match utils::snippets::update_snippet(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn delete_snippet_handler(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match utils::snippets::delete_snippet(&SnippetDeleteRequest { id }) {
        Ok(removed) => (
            StatusCode::OK,
            Json(serde_json::json!({"deleted": removed})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn expand_snippet(Json(request): Json<SnippetExpandRequest>) -> impl IntoResponse {
    match utils::snippets::expand_snippet(&request) {
        Ok(content) => (
            StatusCode::OK,
            Json(serde_json::json!({"content": content})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn list_snippets(
    axum::extract::Query(request): axum::extract::Query<SnippetListRequest>,
) -> impl IntoResponse {
    match utils::snippets::list_snippets(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn format_datetime(Json(request): Json<DateTimeFormatRequest>) -> impl IntoResponse {
    match utils::datetime::format_datetime(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn parse_datetime(Json(request): Json<DateTimeParseRequest>) -> impl IntoResponse {
    match utils::datetime::parse_datetime(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn calc_datetime(Json(request): Json<DateTimeCalcRequest>) -> impl IntoResponse {
    match utils::datetime::calc_datetime(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn diff_datetime(Json(request): Json<DateTimeDiffRequest>) -> impl IntoResponse {
    match utils::datetime::diff_datetime(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn quick_date(Json(request): Json<QuickDateRequest>) -> impl IntoResponse {
    match utils::datetime::quick_date(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn search_emoji(Json(request): Json<EmojiSearchRequest>) -> impl IntoResponse {
    match utils::emoji::search_emoji(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn get_emoji_by_shortcode(Json(request): Json<EmojiByShortcodeRequest>) -> impl IntoResponse {
    match utils::emoji::get_emoji_by_shortcode(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn list_emoji_groups() -> impl IntoResponse {
    let groups = utils::emoji::list_emoji_groups();
    (StatusCode::OK, Json(serde_json::json!({"groups": groups})))
}

async fn add_favorite(Json(request): Json<FavoriteAddRequest>) -> impl IntoResponse {
    match utils::favorites::add_favorite(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn remove_favorite(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match utils::favorites::remove_favorite(&FavoriteRemoveRequest { id }) {
        Ok(removed) => (
            StatusCode::OK,
            Json(serde_json::json!({"deleted": removed})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn toggle_favorite(Json(request): Json<FavoriteAddRequest>) -> impl IntoResponse {
    match utils::favorites::toggle_favorite(&request) {
        Ok((added, item)) => (
            StatusCode::OK,
            Json(serde_json::json!({"added": added, "item": item})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn list_favorites_handler(
    axum::extract::Query(request): axum::extract::Query<FavoriteListRequest>,
) -> impl IntoResponse {
    match utils::favorites::list_favorites(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn vault_save(Json(request): Json<VaultSaveRequest>) -> impl IntoResponse {
    match utils::vault::save_file(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn vault_read(Json(request): Json<VaultReadRequest>) -> impl IntoResponse {
    match utils::vault::read_file(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn vault_list(Json(request): Json<VaultListRequest>) -> impl IntoResponse {
    match utils::vault::list_files(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn vault_delete(Json(request): Json<VaultDeleteRequest>) -> impl IntoResponse {
    match utils::vault::delete_file(&request) {
        Ok(deleted) => (
            StatusCode::OK,
            Json(serde_json::json!({"deleted": deleted})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn vault_move(Json(request): Json<VaultMoveRequest>) -> impl IntoResponse {
    match utils::vault::move_file(&request) {
        Ok(moved) => (StatusCode::OK, Json(serde_json::json!({"moved": moved}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn vault_search(Json(request): Json<VaultSearchRequest>) -> impl IntoResponse {
    match utils::vault::search_vault(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn add_history(Json(request): Json<HistoryAddRequest>) -> impl IntoResponse {
    match utils::history::add_history(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn list_history_handler(
    axum::extract::Query(request): axum::extract::Query<HistoryListRequest>,
) -> impl IntoResponse {
    match utils::history::list_history(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn clear_history(Json(request): Json<HistoryClearRequest>) -> impl IntoResponse {
    match utils::history::clear_history(&request) {
        Ok(removed) => (
            StatusCode::OK,
            Json(serde_json::json!({"removed": removed})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn get_frequent_commands() -> impl IntoResponse {
    match utils::history::get_frequent_commands(10) {
        Ok(commands) => (
            StatusCode::OK,
            Json(serde_json::json!({"commands": commands})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn save_layout(Json(request): Json<LayoutSaveRequest>) -> impl IntoResponse {
    match utils::layouts::save_layout(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn update_layout(
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(mut request): Json<LayoutUpdateRequest>,
) -> impl IntoResponse {
    request.id = id;
    match utils::layouts::update_layout(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn delete_layout(axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
    match utils::layouts::delete_layout(&LayoutDeleteRequest { id }) {
        Ok(deleted) => (
            StatusCode::OK,
            Json(serde_json::json!({"deleted": deleted})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn get_layout(axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
    match utils::layouts::get_layout(&id) {
        Ok(preset) => (StatusCode::OK, Json(serde_json::to_value(preset).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn list_layouts() -> impl IntoResponse {
    match utils::layouts::list_layouts() {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn suggest_links(Json(request): Json<LinkSuggestRequest>) -> impl IntoResponse {
    match utils::links::suggest_links(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn find_backlinks(Json(request): Json<BacklinkRequest>) -> impl IntoResponse {
    match utils::links::find_backlinks(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tasks_parse(Json(request): Json<tasks::TaskParseRequest>) -> impl IntoResponse {
    match tasks::parse_tasks(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tasks_query(Json(request): Json<tasks::TaskQueryRequest>) -> impl IntoResponse {
    match tasks::query_tasks(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn nlp_parse_date(Json(request): Json<nlp::NlpDateParseRequest>) -> impl IntoResponse {
    match nlp::parse_natural_date(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn nlp_suggest_dates(Json(request): Json<nlp::DateSuggestRequest>) -> impl IntoResponse {
    let suggestions = nlp::suggest_dates(&request);
    (
        StatusCode::OK,
        Json(serde_json::json!({"suggestions": suggestions})),
    )
}

async fn dataview_parse(Json(request): Json<dataview::ParseNoteRequest>) -> impl IntoResponse {
    match dataview::parse_note_metadata(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn dataview_query(Json(request): Json<dataview::QueryRequest>) -> impl IntoResponse {
    match dataview::query_notes(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn dataview_table(Json(request): Json<dataview::TableQueryRequest>) -> impl IntoResponse {
    match dataview::table_query(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tables_parse(Json(request): Json<tables::ParseTableRequest>) -> impl IntoResponse {
    match tables::parse_table(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tables_format(Json(request): Json<tables::FormatTableRequest>) -> impl IntoResponse {
    match tables::format_table(&request) {
        Ok(markdown) => (
            StatusCode::OK,
            Json(serde_json::json!({"markdown": markdown})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tables_sort(Json(request): Json<tables::SortTableRequest>) -> impl IntoResponse {
    match tables::sort_table(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tables_add_row(Json(request): Json<tables::AddRowRequest>) -> impl IntoResponse {
    match tables::add_row(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tables_add_column(Json(request): Json<tables::AddColumnRequest>) -> impl IntoResponse {
    match tables::add_column(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn periodic_daily(Json(request): Json<periodic::DailyNoteRequest>) -> impl IntoResponse {
    match periodic::generate_daily_note(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn periodic_weekly(Json(request): Json<periodic::WeeklyNoteRequest>) -> impl IntoResponse {
    match periodic::generate_weekly_note(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn periodic_monthly(Json(request): Json<periodic::MonthlyNoteRequest>) -> impl IntoResponse {
    match periodic::generate_monthly_note(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn periodic_quarterly(
    Json(request): Json<periodic::QuarterlyNoteRequest>,
) -> impl IntoResponse {
    match periodic::generate_quarterly_note(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn periodic_yearly(Json(request): Json<periodic::YearlyNoteRequest>) -> impl IntoResponse {
    match periodic::generate_yearly_note(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn periodic_navigate(
    Json(request): Json<periodic::NavigatePeriodicRequest>,
) -> impl IntoResponse {
    match periodic::navigate_periodic(&request) {
        Ok(date) => (StatusCode::OK, Json(serde_json::json!({"date": date}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn todoist_fetch_tasks(
    headers: HeaderMap,
    Json(request): Json<todoist::FetchTasksRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncTodoist) {
        return err.into_response();
    }
    match todoist::fetch_tasks(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn todoist_fetch_projects(
    headers: HeaderMap,
    Json(request): Json<todoist::FetchProjectsRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncTodoist) {
        return err.into_response();
    }
    match todoist::fetch_projects(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn todoist_create_task(
    headers: HeaderMap,
    Json(request): Json<todoist::CreateTaskRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncTodoist) {
        return err.into_response();
    }
    match todoist::create_task(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn todoist_complete_task(
    headers: HeaderMap,
    Json(request): Json<todoist::CompleteTaskRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncTodoist) {
        return err.into_response();
    }
    match todoist::complete_task(&request).await {
        Ok(completed) => (
            StatusCode::OK,
            Json(serde_json::json!({"completed": completed})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn todoist_sync(
    headers: HeaderMap,
    Json(request): Json<todoist::SyncToObsidianRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncTodoist) {
        return err.into_response();
    }
    match todoist::sync_to_obsidian(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn gcal_fetch_events(
    headers: HeaderMap,
    Json(request): Json<gcal::FetchEventsRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncGcal) {
        return err.into_response();
    }
    match gcal::fetch_events(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn gcal_fetch_today(
    headers: HeaderMap,
    Json(request): Json<gcal::FetchTodayEventsRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncGcal) {
        return err.into_response();
    }
    match gcal::fetch_today_events(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn gcal_create_event(
    headers: HeaderMap,
    Json(request): Json<gcal::CreateEventRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncGcal) {
        return err.into_response();
    }
    match gcal::create_event(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn gcal_sync(
    headers: HeaderMap,
    Json(request): Json<gcal::SyncToObsidianRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::SyncGcal) {
        return err.into_response();
    }
    match gcal::sync_to_obsidian(&request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn git_status_handler(Json(request): Json<git::GitStatusRequest>) -> impl IntoResponse {
    match git::git_status(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn git_commit_handler(Json(request): Json<git::GitCommitRequest>) -> impl IntoResponse {
    match git::git_commit(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn git_push_handler(Json(request): Json<git::GitPushRequest>) -> impl IntoResponse {
    match git::git_push(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn git_pull_handler(Json(request): Json<git::GitPullRequest>) -> impl IntoResponse {
    match git::git_pull(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn git_sync_handler(Json(request): Json<git::GitSyncRequest>) -> impl IntoResponse {
    match git::git_sync(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn git_log_handler(Json(request): Json<git::GitLogRequest>) -> impl IntoResponse {
    match git::git_log(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn git_diff_handler(Json(request): Json<git::GitDiffRequest>) -> impl IntoResponse {
    match git::git_diff(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn git_init_handler(Json(request): Json<git::GitConfig>) -> impl IntoResponse {
    match git::git_init(&request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn highlights_create(
    Json(request): Json<highlights::CreateHighlightRequest>,
) -> impl IntoResponse {
    match highlights::create_highlight(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn highlights_query(Json(request): Json<highlights::HighlightQuery>) -> impl IntoResponse {
    match highlights::query_highlights(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn highlights_update(
    Json(request): Json<highlights::UpdateHighlightRequest>,
) -> impl IntoResponse {
    match highlights::update_highlight(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn highlights_delete(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match highlights::delete_highlight(get_data_dir(), id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"deleted": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn highlights_export(Json(request): Json<highlights::HighlightExport>) -> impl IntoResponse {
    match highlights::export_highlights(get_data_dir(), request) {
        Ok(content) => (
            StatusCode::OK,
            Json(serde_json::json!({"content": content})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_save(Json(request): Json<reading::SaveArticleRequest>) -> impl IntoResponse {
    match reading::save_article(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_query(Json(request): Json<reading::ArticleQuery>) -> impl IntoResponse {
    match reading::query_articles(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_get(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match reading::get_article(get_data_dir(), id) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_update(Json(request): Json<reading::UpdateArticleRequest>) -> impl IntoResponse {
    match reading::update_article(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_delete(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match reading::delete_article(get_data_dir(), id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"deleted": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_archive(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match reading::archive_article(get_data_dir(), id) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_toggle_favorite(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match reading::toggle_favorite(get_data_dir(), id) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_stats() -> impl IntoResponse {
    match reading::get_reading_stats(get_data_dir()) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn reading_labels() -> impl IntoResponse {
    match reading::get_all_labels(get_data_dir()) {
        Ok(labels) => (StatusCode::OK, Json(serde_json::json!({"labels": labels}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn epub_parse(
    headers: HeaderMap,
    Json(request): Json<epub::ParseEpubRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::Epub) {
        return err.into_response();
    }
    match epub::parse_epub(request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn epub_to_markdown(
    headers: HeaderMap,
    Json(request): Json<epub::EpubToMarkdownRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::Epub) {
        return err.into_response();
    }
    match epub::epub_to_markdown(request) {
        Ok(markdown) => (
            StatusCode::OK,
            Json(serde_json::json!({"markdown": markdown})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn epub_metadata(
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::Epub) {
        return err.into_response();
    }
    let path = request.get("path").and_then(|v| v.as_str()).unwrap_or("");
    match epub::get_epub_metadata(path) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn epub_chapter(
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::Epub) {
        return err.into_response();
    }
    let path = request.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let index = request.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    match epub::get_epub_chapter(path, index) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn newsletter_fetch(
    headers: HeaderMap,
    Json(request): Json<newsletter::FetchNewslettersRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::Newsletter) {
        return err.into_response();
    }
    match newsletter::fetch_newsletters(get_data_dir(), request).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn newsletter_query(Json(request): Json<newsletter::NewsletterQuery>) -> impl IntoResponse {
    match newsletter::query_newsletters(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn newsletter_get(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match newsletter::get_newsletter(get_data_dir(), id) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn newsletter_mark_read(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match newsletter::mark_newsletter_read(get_data_dir(), id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn newsletter_toggle_star(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match newsletter::toggle_newsletter_star(get_data_dir(), id) {
        Ok(starred) => (
            StatusCode::OK,
            Json(serde_json::json!({"starred": starred})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn newsletter_delete(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match newsletter::delete_newsletter(get_data_dir(), id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"deleted": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn newsletter_to_markdown(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let include_metadata = request
        .get("include_metadata")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    match newsletter::newsletter_to_markdown(get_data_dir(), id, include_metadata) {
        Ok(markdown) => (
            StatusCode::OK,
            Json(serde_json::json!({"markdown": markdown})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn newsletter_senders() -> impl IntoResponse {
    match newsletter::get_newsletter_senders(get_data_dir()) {
        Ok(senders) => (
            StatusCode::OK,
            Json(serde_json::json!({"senders": senders})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn labels_list() -> impl IntoResponse {
    match labels::list_labels(get_data_dir()) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn labels_create(Json(request): Json<labels::CreateLabelRequest>) -> impl IntoResponse {
    match labels::create_label(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn labels_update(Json(request): Json<labels::UpdateLabelRequest>) -> impl IntoResponse {
    match labels::update_label(get_data_dir(), request) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn labels_delete(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let id = request.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match labels::delete_label(get_data_dir(), id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"deleted": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn labels_tree() -> impl IntoResponse {
    match labels::get_label_tree(get_data_dir()) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn labels_stats() -> impl IntoResponse {
    match labels::get_label_stats(get_data_dir()) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn labels_merge(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let source_id = request
        .get("source_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let target_id = request
        .get("target_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match labels::merge_labels(get_data_dir(), source_id, target_id) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn labels_search(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let query = request.get("query").and_then(|v| v.as_str()).unwrap_or("");
    match labels::search_labels(get_data_dir(), query) {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tts_speak(headers: HeaderMap, Json(request): Json<tts::TtsRequest>) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::Tts) {
        return err.into_response();
    }
    match tts::speak(request) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn tts_stop(headers: HeaderMap) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::Tts) {
        return err.into_response();
    }
    match tts::stop_speaking() {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn tts_voices() -> impl IntoResponse {
    match tts::list_voices() {
        Ok(voices) => (StatusCode::OK, Json(serde_json::json!({"voices": voices}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn tts_status() -> impl IntoResponse {
    let status = tts::check_tts_status();
    (StatusCode::OK, Json(serde_json::to_value(status).unwrap()))
}

async fn tts_read_article(
    headers: HeaderMap,
    Json(request): Json<tts::ReadArticleRequest>,
) -> impl IntoResponse {
    let tier = extract_tier_from_headers(&headers);
    if let Err(err) = check_pro_feature(&tier, ProFeature::Tts) {
        return err.into_response();
    }
    match tts::read_article(request) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn ollama_status() -> impl IntoResponse {
    let status = ai::ollama::check_status().await;
    (StatusCode::OK, Json(serde_json::to_value(status).unwrap()))
}

async fn ollama_models() -> impl IntoResponse {
    match ai::ollama::list_models().await {
        Ok(models) => (StatusCode::OK, Json(serde_json::json!({"models": models}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string(), "models": []})),
        ),
    }
}

#[derive(serde::Deserialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    system: Option<String>,
}

async fn ollama_generate(Json(request): Json<OllamaGenerateRequest>) -> impl IntoResponse {
    match ai::ollama::generate(&request.model, &request.prompt, request.system.as_deref()).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::json!({"response": response})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

#[derive(serde::Deserialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    system: Option<String>,
}

#[derive(serde::Deserialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
}

async fn ollama_chat(Json(request): Json<OllamaChatRequest>) -> impl IntoResponse {
    let messages: Vec<(String, String)> = request
        .messages
        .into_iter()
        .map(|m| (m.role, m.content))
        .collect();

    match ai::ollama::chat(&request.model, messages, request.system.as_deref()).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::json!({"response": response})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn json_rpc_handler(body: String) -> impl IntoResponse {
    let response = handle_jsonrpc_request(&body).await;
    (StatusCode::OK, response)
}

async fn handle_jsonrpc_request(request: &str) -> String {
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(request);

    match parsed {
        Ok(json) => {
            let method = json.get("method").and_then(|m| m.as_str()).unwrap_or("");
            let params = json
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let id = json.get("id").cloned().unwrap_or(serde_json::Value::Null);

            let result = dispatch_method(method, params).await;

            match result {
                Ok(value) => serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": value,
                    "id": id
                })
                .to_string(),
                Err(e) => serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32000,
                        "message": e.to_string()
                    },
                    "id": id
                })
                .to_string(),
            }
        }
        Err(e) => serde_json::json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32700,
                "message": format!("Parse error: {}", e)
            },
            "id": null
        })
        .to_string(),
    }
}

async fn dispatch_method(method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
    match method {
        "youtube.extract" => {
            let request: YouTubeRequest = serde_json::from_value(params)?;
            let response = youtube::extract(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "webclip.extract" => {
            let request: WebClipRequest = serde_json::from_value(params)?;
            let response = web_clip::extract(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "rss.fetch" => {
            let request: RssRequest = serde_json::from_value(params)?;
            let response = rss::fetch(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "pdf.extract" => {
            let request: PdfRequest = serde_json::from_value(params)?;
            let response = pdf::extract(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "ai.chat" => {
            let request: AiChatRequest = serde_json::from_value(params)?;
            let response = ai::chat(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "ai.summarize" => {
            let request: AiSummarizeRequest = serde_json::from_value(params)?;
            let response = ai::summarize(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "ai.index" => {
            let request: AiIndexRequest = serde_json::from_value(params)?;
            let response = ai::index_notes(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "ai.search" => {
            let request: AiSearchRequest = serde_json::from_value(params)?;
            let response = ai::search_notes(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "ai.rag" => {
            let request: AiRagRequest = serde_json::from_value(params)?;
            let response = ai::rag_query(&request).await?;
            Ok(serde_json::to_value(response)?)
        }
        "wallabag.sync" => {
            let request: WallabagSyncRequest = serde_json::from_value(params)?;
            let response = wallabag::sync(&request.config, request.limit).await?;
            Ok(serde_json::to_value(response)?)
        }
        "hoarder.sync" => {
            let request: HoarderSyncRequest = serde_json::from_value(params)?;
            let response = hoarder::sync(&request.config, request.limit).await?;
            Ok(serde_json::to_value(response)?)
        }
        "readwise.sync" => {
            let request: ReadwiseSyncRequest = serde_json::from_value(params)?;
            let response =
                readwise::sync(&request.config, request.updated_after.as_deref()).await?;
            Ok(serde_json::to_value(response)?)
        }
        "health.check" => Ok(serde_json::Value::String("ok".to_string())),
        _ => {
            anyhow::bail!("Method not found: {}", method)
        }
    }
}

async fn sr_get_config() -> impl IntoResponse {
    match spaced_repetition::get_config(get_data_dir()) {
        Ok(config) => (StatusCode::OK, Json(serde_json::to_value(config).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn sr_update_config(
    Json(request): Json<spaced_repetition::SpacedRepetitionConfig>,
) -> impl IntoResponse {
    match spaced_repetition::update_config(get_data_dir(), request) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

#[derive(serde::Deserialize)]
struct RegisterHighlightRequest {
    highlight_id: String,
}

async fn sr_register_highlight(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(request): Json<RegisterHighlightRequest>,
) -> axum::response::Response {
    let tier = extract_tier_from_headers(&headers);

    if let Err(err) = check_sr_limit(&tier, &state.usage_tracker).await {
        return err.into_response();
    }

    match spaced_repetition::register_highlight(get_data_dir(), request.highlight_id) {
        Ok(data) => {
            let mut tracker = state.usage_tracker.write().await;
            let _ = tracker.increment_sr_card();
            (StatusCode::OK, Json(serde_json::to_value(data).unwrap())).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct ReviewHighlightRequest {
    highlight_id: String,
    action: spaced_repetition::HighlightReviewAction,
}

async fn sr_review_highlight(Json(request): Json<ReviewHighlightRequest>) -> impl IntoResponse {
    match spaced_repetition::review_highlight(get_data_dir(), request.highlight_id, request.action)
    {
        Ok(data) => (StatusCode::OK, Json(serde_json::to_value(data).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn sr_create_mastery_card(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(request): Json<spaced_repetition::CreateMasteryCardRequest>,
) -> axum::response::Response {
    let tier = extract_tier_from_headers(&headers);

    if let Err(err) = check_sr_limit(&tier, &state.usage_tracker).await {
        return err.into_response();
    }

    match spaced_repetition::create_mastery_card(get_data_dir(), request) {
        Ok(card) => {
            let mut tracker = state.usage_tracker.write().await;
            let _ = tracker.increment_sr_card();
            (StatusCode::OK, Json(serde_json::to_value(card).unwrap())).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct ReviewMasteryCardRequest {
    card_id: String,
    feedback: spaced_repetition::ReviewFeedback,
}

async fn sr_review_mastery_card(
    Json(request): Json<ReviewMasteryCardRequest>,
) -> impl IntoResponse {
    match spaced_repetition::review_mastery_card(get_data_dir(), request.card_id, request.feedback)
    {
        Ok(card) => (StatusCode::OK, Json(serde_json::to_value(card).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

#[derive(serde::Deserialize)]
struct DeleteMasteryCardRequest {
    card_id: String,
}

async fn sr_delete_mastery_card(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DeleteMasteryCardRequest>,
) -> impl IntoResponse {
    match spaced_repetition::delete_mastery_card(get_data_dir(), request.card_id) {
        Ok(()) => {
            let mut tracker = state.usage_tracker.write().await;
            let _ = tracker.decrement_sr_card();
            (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn sr_create_session(
    Json(request): Json<spaced_repetition::session::CreateSessionRequest>,
) -> impl IntoResponse {
    match spaced_repetition::session::create_review_session(get_data_dir(), request) {
        Ok(session) => (StatusCode::OK, Json(serde_json::to_value(session).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn sr_get_due_counts() -> impl IntoResponse {
    match spaced_repetition::session::get_due_counts(get_data_dir()) {
        Ok((highlights, mastery)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "highlights_due": highlights,
                "mastery_cards_due": mastery
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn sr_set_document_frequency(
    Json(request): Json<spaced_repetition::frequency::SetDocumentFrequencyRequest>,
) -> impl IntoResponse {
    match spaced_repetition::set_document_frequency(
        get_data_dir(),
        request.document_id,
        request.multiplier,
    ) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn sr_set_source_frequency(
    Json(request): Json<spaced_repetition::frequency::SetSourceTypeFrequencyRequest>,
) -> impl IntoResponse {
    match spaced_repetition::set_source_type_frequency(
        get_data_dir(),
        request.source_type,
        request.multiplier,
    ) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn sr_get_stats() -> impl IntoResponse {
    match spaced_repetition::get_stats(get_data_dir()) {
        Ok(stats) => {
            let response = spaced_repetition::stats::StatsResponse::from(&stats);
            (
                StatusCode::OK,
                Json(serde_json::to_value(response).unwrap()),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn kindle_sync(Json(request): Json<KindleSyncRequest>) -> impl IntoResponse {
    let path = std::path::Path::new(&request.clippings_path);
    match kindle::sync_from_file(path).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn ai_related_notes(Json(request): Json<RelatedNotesRequest>) -> impl IntoResponse {
    let limit = request.limit.unwrap_or(5);

    match ai::find_related_notes(&request.content, limit, request.current_path.as_deref()).await {
        Ok(results) => {
            let notes: Vec<RelatedNote> = results
                .into_iter()
                .map(|r| RelatedNote {
                    id: r.id,
                    title: r.title,
                    path: r.path,
                    score: r.score,
                    snippet: truncate_for_snippet(&r.content, 150),
                })
                .collect();
            (
                StatusCode::OK,
                Json(serde_json::to_value(RelatedNotesResponse { notes }).unwrap()),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

fn truncate_for_snippet(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

async fn audio_transcribe(Json(request): Json<TranscribeRequest>) -> impl IntoResponse {
    match audio::transcribe(&request).await {
        Ok(result) => (StatusCode::OK, Json(serde_json::to_value(result).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn audio_model_download(Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    let model_name = request.get("model_name").and_then(|v| v.as_str());
    match audio::download_model(model_name).await {
        Ok(path) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "path": path})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}
