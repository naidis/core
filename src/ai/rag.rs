use anyhow::Result;
use std::path::Path;

use super::llm::AsyncLlmEngine;
use super::search::{AsyncSearchIndex, NoteDocument, SearchResult};

pub struct RagPipeline {
    search_index: AsyncSearchIndex,
    llm_engine: Option<AsyncLlmEngine>,
    model_path: Option<String>,
}

impl RagPipeline {
    pub fn new(index_path: &Path) -> Result<Self> {
        let search_index = AsyncSearchIndex::new(index_path)?;

        Ok(Self {
            search_index,
            llm_engine: None,
            model_path: None,
        })
    }

    pub fn new_in_memory() -> Result<Self> {
        let search_index = AsyncSearchIndex::new_in_memory()?;

        Ok(Self {
            search_index,
            llm_engine: None,
            model_path: None,
        })
    }

    pub async fn load_model(&mut self, model_path: &str) -> Result<()> {
        if self.llm_engine.is_none() {
            self.llm_engine = Some(AsyncLlmEngine::new()?);
        }

        if let Some(ref engine) = self.llm_engine {
            engine.load_model(model_path).await?;
            self.model_path = Some(model_path.to_string());
        }

        Ok(())
    }

    pub async fn index_notes(&self, notes: Vec<NoteDocument>) -> Result<()> {
        self.search_index.index_documents(notes).await
    }

    pub async fn index_note(&self, note: NoteDocument) -> Result<()> {
        self.search_index.index_document(note).await
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.search_index.hybrid_search(query, limit).await
    }

    pub async fn query(&self, question: &str, top_k: usize) -> Result<RagResponse> {
        let search_results = self.search_index.hybrid_search(question, top_k).await?;

        let context = search_results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "[{}] {}\n{}",
                    i + 1,
                    r.title,
                    truncate_content(&r.content, 1000)
                )
            })
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
            context, question
        );

        let answer = if let Some(ref engine) = self.llm_engine {
            engine.generate(&prompt, 512).await?
        } else {
            let engine = AsyncLlmEngine::new()?;
            engine.generate(&prompt, 512).await?
        };

        let sources: Vec<RagSource> = search_results
            .iter()
            .map(|r| RagSource {
                id: r.id.clone(),
                title: r.title.clone(),
                path: r.path.clone(),
                score: r.score,
                snippet: truncate_content(&r.content, 200),
            })
            .collect();

        Ok(RagResponse {
            answer,
            sources,
            context_used: context,
        })
    }

    pub async fn summarize(&self, text: &str, max_words: usize) -> Result<String> {
        let prompt = format!(
            r#"Summarize the following text in approximately {} words. Be concise and capture the main points.

Text:
{}

Summary:"#,
            max_words, text
        );

        if let Some(ref engine) = self.llm_engine {
            engine.generate(&prompt, (max_words * 2) as u32).await
        } else {
            let engine = AsyncLlmEngine::new()?;
            engine.generate(&prompt, (max_words * 2) as u32).await
        }
    }

    pub async fn detect_chapters(&self, transcript: &str) -> Result<Vec<Chapter>> {
        let prompt = format!(
            r#"Analyze this video transcript and identify major topic changes to create chapter markers.
Output format: One chapter per line in the format "TIMESTAMP_SECONDS|CHAPTER_TITLE"
Example: "0|Introduction" or "120|Main Topic"

Transcript (first 8000 chars):
{}

Chapters:"#,
            truncate_content(transcript, 8000)
        );

        let response = if let Some(ref engine) = self.llm_engine {
            engine.generate(&prompt, 256).await?
        } else {
            let engine = AsyncLlmEngine::new()?;
            engine.generate(&prompt, 256).await?
        };

        let mut chapters = Vec::new();
        for line in response.lines() {
            if let Some((ts_str, title)) = line.split_once('|') {
                if let Ok(timestamp) = ts_str.trim().parse::<f64>() {
                    chapters.push(Chapter {
                        start: timestamp,
                        title: title.trim().to_string(),
                    });
                }
            }
        }

        if chapters.is_empty() {
            chapters.push(Chapter {
                start: 0.0,
                title: "Introduction".to_string(),
            });
        }

        Ok(chapters)
    }

    async fn fallback_generate(&self, _prompt: &str) -> Result<String> {
        Ok("[AI not available] No LLM model loaded. Call load_model() first or the model will auto-load on first use.".to_string())
    }

    pub async fn clear_index(&self) -> Result<()> {
        self.search_index.clear().await
    }
}

fn truncate_content(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        content.to_string()
    } else {
        format!("{}...", &content[..max_chars])
    }
}

#[derive(Debug, Clone)]
pub struct RagResponse {
    pub answer: String,
    pub sources: Vec<RagSource>,
    pub context_used: String,
}

#[derive(Debug, Clone)]
pub struct RagSource {
    pub id: String,
    pub title: String,
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct Chapter {
    pub start: f64,
    pub title: String,
}
