use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct YouTubeRequest {
    pub url: String,
    pub include_transcript: bool,
    pub include_chapters: bool,
    #[serde(default)]
    pub generate_ai_chapters: bool,
    #[ts(optional)]
    pub language: Option<String>,
    #[ts(optional)]
    pub provider: Option<String>,
    #[ts(optional)]
    pub api_key: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct YouTubeResponse {
    pub title: String,
    pub channel: String,
    pub duration: u64,
    pub thumbnail: String,
    #[ts(optional)]
    pub transcript: Option<Vec<TranscriptSegment>>,
    #[ts(optional)]
    pub chapters: Option<Vec<Chapter>>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TranscriptSegment {
    pub start: f64,
    pub duration: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Chapter {
    pub start: f64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct YouTubeBatchRequest {
    pub urls: Vec<String>,
    pub include_transcript: bool,
    pub include_chapters: bool,
    #[serde(default)]
    pub generate_ai_chapters: bool,
    #[ts(optional)]
    pub language: Option<String>,
    #[ts(optional)]
    pub provider: Option<String>,
    #[ts(optional)]
    pub api_key: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct YouTubeBatchItem {
    pub url: String,
    #[ts(optional)]
    pub result: Option<YouTubeResponse>,
    #[ts(optional)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct YouTubeBatchResponse {
    pub items: Vec<YouTubeBatchItem>,
    pub success_count: usize,
    pub error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct WebClipRequest {
    pub url: String,
    pub include_images: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct WebClipResponse {
    pub title: String,
    pub content: String,
    #[ts(optional)]
    pub author: Option<String>,
    #[ts(optional)]
    pub published_date: Option<String>,
    #[ts(optional)]
    pub excerpt: Option<String>,
    #[ts(optional)]
    pub site_name: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RssRequest {
    pub url: String,
    #[ts(optional)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RssResponse {
    pub title: String,
    #[ts(optional)]
    pub description: Option<String>,
    #[ts(optional)]
    pub link: Option<String>,
    pub items: Vec<RssItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RssItem {
    #[ts(optional)]
    pub title: Option<String>,
    #[ts(optional)]
    pub link: Option<String>,
    #[ts(optional)]
    pub content: Option<String>,
    #[ts(optional)]
    pub published: Option<String>,
    #[ts(optional)]
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct PdfRequest {
    pub path: String,
    pub extract_tables: bool,
    pub ocr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct PdfResponse {
    pub text: String,
    pub pages: usize,
    #[ts(optional)]
    pub tables: Option<Vec<String>>,
    pub metadata: PdfMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct PdfMetadata {
    #[ts(optional)]
    pub title: Option<String>,
    #[ts(optional)]
    pub author: Option<String>,
    #[ts(optional)]
    pub subject: Option<String>,
    #[ts(optional)]
    pub creator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct PdfTablesRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiChatRequest {
    pub message: String,
    #[ts(optional)]
    pub context: Option<Vec<String>>,
    #[ts(optional)]
    pub system_prompt: Option<String>,
    #[ts(optional)]
    pub provider: Option<String>,
    #[ts(optional)]
    pub api_key: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiChatResponse {
    pub response: String,
    #[ts(optional)]
    pub sources: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiSummarizeRequest {
    pub text: String,
    #[ts(optional)]
    pub max_length: Option<usize>,
    #[ts(optional)]
    pub provider: Option<String>,
    #[ts(optional)]
    pub api_key: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiSummarizeResponse {
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct NoteItem {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiIndexRequest {
    pub notes: Vec<NoteItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiIndexResponse {
    pub indexed_count: usize,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiSearchRequest {
    pub query: String,
    #[ts(optional)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiSearchResultItem {
    pub id: String,
    pub title: String,
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiSearchResponse {
    pub results: Vec<AiSearchResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiRagRequest {
    pub query: String,
    #[ts(optional)]
    pub limit: Option<usize>,
    #[ts(optional)]
    pub system_prompt: Option<String>,
    #[ts(optional)]
    pub provider: Option<String>,
    #[ts(optional)]
    pub api_key: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiRagSource {
    pub id: String,
    pub score: f32,
    pub content: String,
    #[ts(optional)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AiRagResponse {
    pub response: String,
    pub sources: Vec<AiRagSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct WallabagConfig {
    pub url: String,
    pub client_id: String,
    pub client_secret: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct WallabagSyncRequest {
    pub config: WallabagConfig,
    #[ts(optional)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct WallabagEntry {
    pub id: u64,
    pub title: String,
    pub url: String,
    #[ts(optional)]
    pub content: Option<String>,
    pub created_at: String,
    #[ts(optional)]
    pub reading_time: Option<u32>,
    pub is_archived: bool,
    pub is_starred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct WallabagSyncResponse {
    pub entries: Vec<WallabagEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct HoarderConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct HoarderSyncRequest {
    pub config: HoarderConfig,
    #[ts(optional)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct HoarderBookmark {
    pub id: String,
    #[ts(optional)]
    pub title: Option<String>,
    pub url: String,
    #[ts(optional)]
    pub content: Option<String>,
    #[ts(optional)]
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct HoarderSyncResponse {
    pub bookmarks: Vec<HoarderBookmark>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ReadwiseConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ReadwiseSyncRequest {
    pub config: ReadwiseConfig,
    #[ts(optional)]
    pub updated_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ReadwiseHighlight {
    pub id: u64,
    pub text: String,
    #[ts(optional)]
    pub note: Option<String>,
    #[ts(optional)]
    pub location: Option<u32>,
    #[ts(optional)]
    pub location_type: Option<String>,
    #[ts(optional)]
    pub url: Option<String>,
    pub book_title: String,
    #[ts(optional)]
    pub book_author: Option<String>,
    #[ts(optional)]
    pub highlighted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ReadwiseSyncResponse {
    pub highlights: Vec<ReadwiseHighlight>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DependencyStatus {
    pub name: String,
    pub installed: bool,
    #[ts(optional)]
    pub version: Option<String>,
    pub install_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ModelStatus {
    pub loaded: bool,
    #[ts(optional)]
    pub model_name: Option<String>,
    #[ts(optional)]
    pub model_size: Option<String>,
    #[ts(optional)]
    pub download_progress: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct SystemStatusResponse {
    pub dependencies: Vec<DependencyStatus>,
    pub ai_model: ModelStatus,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ModelDownloadRequest {
    #[ts(optional)]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ModelDownloadResponse {
    pub success: bool,
    pub message: String,
    #[ts(optional)]
    pub model_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DependencyDownloadRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DependencyDownloadResponse {
    pub success: bool,
    pub message: String,
    #[ts(optional)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DependencyInstallRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DependencyInstallResponse {
    pub success: bool,
    pub message: String,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
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

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct KindleSyncRequest {
    pub clippings_path: String,
}

#[allow(unused_imports)]
pub use crate::integrations::kindle::{KindleBook, KindleHighlight, KindleSyncResponse};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RelatedNotesRequest {
    pub content: String,
    #[ts(optional)]
    pub current_path: Option<String>,
    #[ts(optional)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RelatedNote {
    pub id: String,
    pub title: String,
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RelatedNotesResponse {
    pub notes: Vec<RelatedNote>,
}

#[allow(unused_imports)]
pub use crate::audio::{TranscribeRequest, TranscriptionResult, TranscriptionSegment};

#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ExtensionConfig {
    pub vault_path: String,
    pub default_save_folder: String,
    pub web_clip_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ConfigUpdateRequest {
    #[ts(optional)]
    pub vault_path: Option<String>,
    #[ts(optional)]
    pub default_save_folder: Option<String>,
    #[ts(optional)]
    pub web_clip_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct FoldersResponse {
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TagsResponse {
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct FoldersRequest {
    pub vault_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TagsRequest {
    pub vault_path: String,
    #[ts(optional)]
    pub limit: Option<usize>,
}
