use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeRequest {
    pub url: String,
    pub include_transcript: bool,
    pub include_chapters: bool,
    #[serde(default)]
    pub generate_ai_chapters: bool,
    pub language: Option<String>,
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeResponse {
    pub title: String,
    pub channel: String,
    pub duration: u64,
    pub thumbnail: String,
    pub transcript: Option<Vec<TranscriptSegment>>,
    pub chapters: Option<Vec<Chapter>>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start: f64,
    pub duration: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub start: f64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeBatchRequest {
    pub urls: Vec<String>,
    pub include_transcript: bool,
    pub include_chapters: bool,
    #[serde(default)]
    pub generate_ai_chapters: bool,
    pub language: Option<String>,
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeBatchItem {
    pub url: String,
    pub result: Option<YouTubeResponse>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeBatchResponse {
    pub items: Vec<YouTubeBatchItem>,
    pub success_count: usize,
    pub error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebClipRequest {
    pub url: String,
    pub include_images: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebClipResponse {
    pub title: String,
    pub content: String,
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub excerpt: Option<String>,
    pub site_name: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssRequest {
    pub url: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssResponse {
    pub title: String,
    pub description: Option<String>,
    pub link: Option<String>,
    pub items: Vec<RssItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssItem {
    pub title: Option<String>,
    pub link: Option<String>,
    pub content: Option<String>,
    pub published: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfRequest {
    pub path: String,
    pub extract_tables: bool,
    pub ocr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfResponse {
    pub text: String,
    pub pages: usize,
    pub tables: Option<Vec<String>>,
    pub metadata: PdfMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfTablesRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatRequest {
    pub message: String,
    pub context: Option<Vec<String>>,
    pub system_prompt: Option<String>,
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatResponse {
    pub response: String,
    pub sources: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSummarizeRequest {
    pub text: String,
    pub max_length: Option<usize>,
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSummarizeResponse {
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteItem {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiIndexRequest {
    pub notes: Vec<NoteItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiIndexResponse {
    pub indexed_count: usize,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSearchResultItem {
    pub id: String,
    pub title: String,
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSearchResponse {
    pub results: Vec<AiSearchResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRagRequest {
    pub query: String,
    pub limit: Option<usize>,
    pub system_prompt: Option<String>,
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRagSource {
    pub id: String,
    pub score: f32,
    pub content: String,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRagResponse {
    pub response: String,
    pub sources: Vec<AiRagSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallabagConfig {
    pub url: String,
    pub client_id: String,
    pub client_secret: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallabagSyncRequest {
    pub config: WallabagConfig,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallabagEntry {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub content: Option<String>,
    pub created_at: String,
    pub reading_time: Option<u32>,
    pub is_archived: bool,
    pub is_starred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallabagSyncResponse {
    pub entries: Vec<WallabagEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoarderConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoarderSyncRequest {
    pub config: HoarderConfig,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoarderBookmark {
    pub id: String,
    pub title: Option<String>,
    pub url: String,
    pub content: Option<String>,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoarderSyncResponse {
    pub bookmarks: Vec<HoarderBookmark>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadwiseConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadwiseSyncRequest {
    pub config: ReadwiseConfig,
    pub updated_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadwiseHighlight {
    pub id: u64,
    pub text: String,
    pub note: Option<String>,
    pub location: Option<u32>,
    pub location_type: Option<String>,
    pub url: Option<String>,
    pub book_title: String,
    pub book_author: Option<String>,
    pub highlighted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadwiseSyncResponse {
    pub highlights: Vec<ReadwiseHighlight>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub install_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub loaded: bool,
    pub model_name: Option<String>,
    pub model_size: Option<String>,
    pub download_progress: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatusResponse {
    pub dependencies: Vec<DependencyStatus>,
    pub ai_model: ModelStatus,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadRequest {
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadResponse {
    pub success: bool,
    pub message: String,
    pub model_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDownloadRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDownloadResponse {
    pub success: bool,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInstallRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInstallResponse {
    pub success: bool,
    pub message: String,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManagerStatus {
    pub homebrew: bool,
    pub chocolatey: bool,
    pub apt: bool,
}

pub use crate::utils::calculator::*;
pub use crate::utils::datetime::*;
pub use crate::utils::emoji::*;
pub use crate::utils::favorites::*;
pub use crate::utils::history::*;
pub use crate::utils::layouts::*;
pub use crate::utils::links::*;
pub use crate::utils::snippets::*;
pub use crate::utils::vault::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KindleSyncRequest {
    pub clippings_path: String,
}

#[allow(unused_imports)]
pub use crate::integrations::kindle::{KindleBook, KindleHighlight, KindleSyncResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedNotesRequest {
    pub content: String,
    pub current_path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedNote {
    pub id: String,
    pub title: String,
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedNotesResponse {
    pub notes: Vec<RelatedNote>,
}

#[allow(unused_imports)]
pub use crate::audio::{TranscribeRequest, TranscriptionResult, TranscriptionSegment};
