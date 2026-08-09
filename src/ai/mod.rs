mod embeddings;
mod llm;
pub mod ollama;
pub mod providers;
mod rag;
mod search;

pub use providers::{create_provider, LlmConfig, LlmProvider, LlmProviderTrait};
pub use rag::RagPipeline;
pub use search::NoteDocument;
pub use search::SearchResult;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::rpc::{
    AiChatRequest, AiChatResponse, AiChatStreamRequest, AiIndexRequest, AiIndexResponse,
    AiRagRequest, AiRagResponse, AiRagSource, AiSearchRequest, AiSearchResponse,
    AiSummarizeRequest, AiSummarizeResponse,
};

static RAG_PIPELINE: std::sync::OnceLock<Arc<RwLock<Option<RagPipeline>>>> =
    std::sync::OnceLock::new();

fn get_pipeline_lock() -> &'static Arc<RwLock<Option<RagPipeline>>> {
    RAG_PIPELINE.get_or_init(|| Arc::new(RwLock::new(None)))
}

pub async fn init_pipeline(index_path: Option<PathBuf>, model_path: Option<String>) -> Result<()> {
    let pipeline = if let Some(path) = index_path {
        RagPipeline::new(&path)?
    } else {
        RagPipeline::new_in_memory()?
    };

    if let Some(model) = model_path {
        let mut p = pipeline;
        p.load_model(&model).await?;
        *get_pipeline_lock().write().await = Some(p);
    } else {
        *get_pipeline_lock().write().await = Some(pipeline);
    }

    Ok(())
}

pub async fn ensure_pipeline() -> Result<()> {
    let guard = get_pipeline_lock().read().await;
    if guard.is_none() {
        drop(guard);
        init_pipeline(None, None).await?;
    }
    Ok(())
}

pub async fn chat(request: &AiChatRequest) -> Result<AiChatResponse> {
    if let Some(ref provider_name) = request.provider {
        let provider_type = match provider_name.to_lowercase().as_str() {
            "openai" => LlmProvider::OpenAI,
            "anthropic" => LlmProvider::Anthropic,
            "zai" | "z.ai" => LlmProvider::Zai,
            "groq" => LlmProvider::Groq,
            _ => LlmProvider::Local,
        };

        if !matches!(provider_type, LlmProvider::Local) {
            let config = LlmConfig {
                provider: provider_type,
                api_key: request.api_key.clone(),
                model: request.model.clone(),
                base_url: None,
            };

            let llm = create_provider(&config)?;
            let response = llm.generate(&request.message, 1024).await?;

            return Ok(AiChatResponse {
                response,
                sources: None,
            });
        }
    }

    ensure_pipeline().await?;

    let guard = get_pipeline_lock().read().await;
    let pipeline = guard.as_ref().unwrap();

    if let Some(ref context) = request.context {
        if !context.is_empty() {
            for (i, note_content) in context.iter().enumerate() {
                let doc = NoteDocument {
                    id: format!("context_{}", i),
                    title: format!("Context {}", i + 1),
                    content: note_content.clone(),
                    path: String::new(),
                    embedding: None,
                };
                pipeline.index_note(doc).await?;
            }
        }
    }

    let response = pipeline.query(&request.message, 5).await?;

    let sources: Vec<String> = response
        .sources
        .iter()
        .map(|s| format!("{} ({})", s.title, s.path))
        .collect();

    Ok(AiChatResponse {
        response: response.answer,
        sources: if sources.is_empty() {
            None
        } else {
            Some(sources)
        },
    })
}

pub async fn chat_stream(
    request: &AiChatStreamRequest,
) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String>>(32);

    let provider_name = request.provider.clone();
    let api_key = request.api_key.clone();
    let model = request.model.clone();
    let message = request.message.clone();
    let system_prompt = request.system_prompt.clone();

    tokio::spawn(async move {
        let result = chat_stream_internal(&provider_name, &api_key, &model, &message, &system_prompt, tx.clone()).await;
        if let Err(e) = result {
            let _ = tx.send(Err(e)).await;
        }
    });

    Ok(rx)
}

async fn chat_stream_internal(
    provider_name: &Option<String>,
    api_key: &Option<String>,
    model: &Option<String>,
    message: &str,
    system_prompt: &Option<String>,
    tx: tokio::sync::mpsc::Sender<Result<String>>,
) -> Result<()> {
    if let Some(ref provider_str) = provider_name {
        match provider_str.to_lowercase().as_str() {
            "ollama" => {
                return ollama::chat_stream(
                    model.as_deref().unwrap_or("llama3.2"),
                    message,
                    system_prompt.as_deref(),
                    tx,
                )
                .await;
            }
            "openai" => {
                return providers::openai_stream(
                    api_key.as_ref().ok_or_else(|| anyhow::anyhow!("OpenAI API key required"))?,
                    model.as_deref().unwrap_or("gpt-4o-mini"),
                    message,
                    system_prompt.as_deref(),
                    tx,
                )
                .await;
            }
            "anthropic" => {
                return providers::anthropic_stream(
                    api_key.as_ref().ok_or_else(|| anyhow::anyhow!("Anthropic API key required"))?,
                    model.as_deref().unwrap_or("claude-3-5-sonnet-20241022"),
                    message,
                    system_prompt.as_deref(),
                    tx,
                )
                .await;
            }
            "groq" => {
                return providers::groq_stream(
                    api_key.as_ref().ok_or_else(|| anyhow::anyhow!("Groq API key required"))?,
                    model.as_deref().unwrap_or("llama-3.3-70b-versatile"),
                    message,
                    system_prompt.as_deref(),
                    tx,
                )
                .await;
            }
            _ => {}
        }
    }

    ollama::chat_stream(
        model.as_deref().unwrap_or("llama3.2"),
        message,
        system_prompt.as_deref(),
        tx,
    )
    .await
}

pub async fn summarize(request: &AiSummarizeRequest) -> Result<AiSummarizeResponse> {
    if let Some(ref provider_name) = request.provider {
        let provider_type = match provider_name.to_lowercase().as_str() {
            "openai" => LlmProvider::OpenAI,
            "anthropic" => LlmProvider::Anthropic,
            "zai" | "z.ai" => LlmProvider::Zai,
            "groq" => LlmProvider::Groq,
            _ => LlmProvider::Local,
        };

        if !matches!(provider_type, LlmProvider::Local) {
            let config = LlmConfig {
                provider: provider_type,
                api_key: request.api_key.clone(),
                model: request.model.clone(),
                base_url: None,
            };

            let llm = create_provider(&config)?;
            let max_words = request.max_length.unwrap_or(500) / 5;
            let prompt = format!(
                "Summarize the following text in approximately {} words:\n\n{}",
                max_words, request.text
            );
            let summary = llm.generate(&prompt, (max_words * 2) as u32).await?;

            return Ok(AiSummarizeResponse { summary });
        }
    }

    ensure_pipeline().await?;

    let guard = get_pipeline_lock().read().await;
    let pipeline = guard.as_ref().unwrap();

    let max_words = request.max_length.unwrap_or(500) / 5;
    let summary = pipeline.summarize(&request.text, max_words).await?;

    Ok(AiSummarizeResponse { summary })
}

pub async fn index_notes(request: &AiIndexRequest) -> Result<AiIndexResponse> {
    ensure_pipeline().await?;

    let guard = get_pipeline_lock().read().await;
    let pipeline = guard.as_ref().unwrap();

    let docs: Vec<NoteDocument> = request
        .notes
        .iter()
        .map(|n| NoteDocument {
            id: n.id.clone(),
            title: n.title.clone(),
            content: n.content.clone(),
            path: n.path.clone(),
            embedding: None,
        })
        .collect();

    let count = docs.len();
    pipeline.index_notes(docs).await?;

    Ok(AiIndexResponse {
        indexed_count: count,
        success: true,
    })
}

pub async fn search_notes(request: &AiSearchRequest) -> Result<AiSearchResponse> {
    ensure_pipeline().await?;

    let guard = get_pipeline_lock().read().await;
    let pipeline = guard.as_ref().unwrap();

    let limit = request.limit.unwrap_or(10);
    let results = pipeline.search(&request.query, limit).await?;

    let items: Vec<crate::rpc::AiSearchResultItem> = results
        .into_iter()
        .map(|r| crate::rpc::AiSearchResultItem {
            id: r.id,
            title: r.title,
            path: r.path,
            score: r.score,
            snippet: truncate(&r.content, 200),
        })
        .collect();

    Ok(AiSearchResponse { results: items })
}

pub async fn generate_chapters(transcript: &str) -> Result<Vec<(f64, String)>> {
    ensure_pipeline().await?;

    let guard = get_pipeline_lock().read().await;
    let pipeline = guard.as_ref().unwrap();

    let chapters = pipeline.detect_chapters(transcript).await?;
    Ok(chapters.into_iter().map(|c| (c.start, c.title)).collect())
}

pub async fn rag_query(request: &AiRagRequest) -> Result<AiRagResponse> {
    ensure_pipeline().await?;

    let guard = get_pipeline_lock().read().await;
    let pipeline = guard.as_ref().unwrap();

    let limit = request.limit.unwrap_or(5);
    let search_results = pipeline.search(&request.query, limit).await?;

    let context = search_results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{}] {}\n{}", i + 1, r.title, truncate(&r.content, 1000)))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let prompt = format!(
        r#"You are a helpful assistant that answers questions based on the user's notes.

Context from relevant notes:
{}

Question: {}

Instructions:
- Answer based ONLY on the provided context
- If the context doesn't contain relevant information, say so
- Reference the source notes by their numbers [1], [2], etc.
- Be concise and direct

Answer:"#,
        context, request.query
    );

    let answer = if let Some(ref provider_name) = request.provider {
        let provider_type = match provider_name.to_lowercase().as_str() {
            "openai" => LlmProvider::OpenAI,
            "anthropic" => LlmProvider::Anthropic,
            "zai" | "z.ai" => LlmProvider::Zai,
            "groq" => LlmProvider::Groq,
            _ => LlmProvider::Local,
        };

        if !matches!(provider_type, LlmProvider::Local) {
            let config = LlmConfig {
                provider: provider_type,
                api_key: request.api_key.clone(),
                model: request.model.clone(),
                base_url: None,
            };
            let llm = create_provider(&config)?;
            llm.generate(&prompt, 1024).await?
        } else {
            pipeline.query(&request.query, limit).await?.answer
        }
    } else {
        pipeline.query(&request.query, limit).await?.answer
    };

    let sources: Vec<AiRagSource> = search_results
        .into_iter()
        .map(|s| {
            let mut metadata = std::collections::HashMap::new();
            metadata.insert("title".to_string(), s.title.clone());
            metadata.insert("path".to_string(), s.path.clone());
            AiRagSource {
                id: s.path,
                score: s.score,
                content: truncate(&s.content, 200),
                metadata: Some(metadata),
            }
        })
        .collect();

    Ok(AiRagResponse {
        response: answer,
        sources,
    })
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

pub async fn download_model(model_id: &str) -> Result<String> {
    use hf_hub::{api::sync::Api, Repo, RepoType};

    let (repo_id, filename) = if model_id.contains(':') {
        let parts: Vec<&str> = model_id.split(':').collect();
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (
            model_id.to_string(),
            "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf".to_string(),
        )
    };

    tracing::info!("Downloading model: {} / {}", repo_id, filename);

    let api = Api::new()?;
    let repo = api.repo(Repo::new(repo_id.clone(), RepoType::Model));

    let _model_path = repo.get(&filename)?;

    tracing::info!("Model downloaded successfully");
    Ok(format!("{}:{}", repo_id, filename))
}

pub async fn find_related_notes(
    content: &str,
    limit: usize,
    exclude_path: Option<&str>,
) -> Result<Vec<SearchResult>> {
    ensure_pipeline().await?;

    let guard = get_pipeline_lock().read().await;
    let pipeline = guard.as_ref().unwrap();

    let mut results = pipeline.search(content, limit + 1).await?;

    if let Some(exclude) = exclude_path {
        results.retain(|r| r.path != exclude);
    }

    results.truncate(limit);
    Ok(results)
}
