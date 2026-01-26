use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::embeddings::{cosine_similarity, AsyncEmbeddingEngine};

#[derive(Clone)]
pub struct NoteDocument {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: String,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: String,
    pub score: f32,
}

pub struct SearchIndex {
    documents: Vec<NoteDocument>,
    embeddings: Vec<(String, Vec<f32>)>,
    embedding_engine: Option<AsyncEmbeddingEngine>,
}

impl SearchIndex {
    pub fn new(_index_path: &Path) -> Result<Self> {
        let embedding_engine = AsyncEmbeddingEngine::new().ok();

        Ok(Self {
            documents: Vec::new(),
            embeddings: Vec::new(),
            embedding_engine,
        })
    }

    pub fn new_in_memory() -> Result<Self> {
        let embedding_engine = AsyncEmbeddingEngine::new().ok();

        Ok(Self {
            documents: Vec::new(),
            embeddings: Vec::new(),
            embedding_engine,
        })
    }

    pub async fn index_document(&mut self, doc: NoteDocument) -> Result<()> {
        if let Some(ref engine) = self.embedding_engine {
            let text = format!("{} {}", doc.title, doc.content);
            if let Ok(embedding) = engine.embed_single(&text).await {
                self.embeddings.push((doc.id.clone(), embedding));
            }
        }

        self.documents.push(doc);
        Ok(())
    }

    pub async fn index_documents(&mut self, docs: Vec<NoteDocument>) -> Result<()> {
        if let Some(ref engine) = self.embedding_engine {
            let texts: Vec<String> = docs
                .iter()
                .map(|d| format!("{} {}", d.title, d.content))
                .collect();
            let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

            if let Ok(embeddings) = engine.embed(&text_refs).await {
                for (doc, embedding) in docs.iter().zip(embeddings) {
                    self.embeddings.push((doc.id.clone(), embedding));
                }
            }
        }

        self.documents.extend(docs);
        Ok(())
    }

    pub fn fulltext_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(usize, f32)> = self
            .documents
            .iter()
            .enumerate()
            .map(|(idx, doc)| {
                let title_lower = doc.title.to_lowercase();
                let content_lower = doc.content.to_lowercase();

                let mut score = 0.0f32;
                for word in &query_words {
                    if title_lower.contains(word) {
                        score += 2.0;
                    }
                    if content_lower.contains(word) {
                        score += 1.0;
                    }
                }
                (idx, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let results: Vec<SearchResult> = scored
            .into_iter()
            .take(limit)
            .map(|(idx, score)| {
                let doc = &self.documents[idx];
                SearchResult {
                    id: doc.id.clone(),
                    title: doc.title.clone(),
                    content: doc.content.clone(),
                    path: doc.path.clone(),
                    score,
                }
            })
            .collect();

        Ok(results)
    }

    pub async fn semantic_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let engine = self
            .embedding_engine
            .as_ref()
            .context("Embedding engine not available")?;

        let query_embedding = engine.embed_single(query).await?;

        let mut scored: Vec<(String, f32)> = self
            .embeddings
            .iter()
            .map(|(id, emb)| (id.clone(), cosine_similarity(&query_embedding, emb)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_ids: Vec<(String, f32)> = scored.into_iter().take(limit).collect();

        let results: Vec<SearchResult> = top_ids
            .into_iter()
            .filter_map(|(id, score)| {
                self.documents
                    .iter()
                    .find(|d| d.id == id)
                    .map(|doc| SearchResult {
                        id: doc.id.clone(),
                        title: doc.title.clone(),
                        content: doc.content.clone(),
                        path: doc.path.clone(),
                        score,
                    })
            })
            .collect();

        Ok(results)
    }

    pub async fn hybrid_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let fulltext_results = self.fulltext_search(query, limit * 2)?;

        let semantic_results = if self.embedding_engine.is_some() {
            self.semantic_search(query, limit * 2)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut combined: std::collections::HashMap<String, SearchResult> =
            std::collections::HashMap::new();

        for result in fulltext_results {
            combined.insert(result.id.clone(), result);
        }

        for result in semantic_results {
            combined
                .entry(result.id.clone())
                .and_modify(|e| e.score = (e.score + result.score) / 2.0)
                .or_insert(result);
        }

        let mut results: Vec<SearchResult> = combined.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    pub fn clear(&mut self) -> Result<()> {
        self.documents.clear();
        self.embeddings.clear();
        Ok(())
    }
}

pub struct AsyncSearchIndex {
    inner: Arc<RwLock<SearchIndex>>,
}

impl AsyncSearchIndex {
    pub fn new(index_path: &Path) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(SearchIndex::new(index_path)?)),
        })
    }

    pub fn new_in_memory() -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(SearchIndex::new_in_memory()?)),
        })
    }

    pub async fn index_document(&self, doc: NoteDocument) -> Result<()> {
        let mut index = self.inner.write().await;
        index.index_document(doc).await
    }

    pub async fn index_documents(&self, docs: Vec<NoteDocument>) -> Result<()> {
        let mut index = self.inner.write().await;
        index.index_documents(docs).await
    }

    pub async fn fulltext_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let index = self.inner.read().await;
        index.fulltext_search(query, limit)
    }

    pub async fn semantic_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let index = self.inner.read().await;
        index.semantic_search(query, limit).await
    }

    pub async fn hybrid_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let index = self.inner.read().await;
        index.hybrid_search(query, limit).await
    }

    pub async fn clear(&self) -> Result<()> {
        let mut index = self.inner.write().await;
        index.clear()
    }
}
