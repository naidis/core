use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct EmbeddingEngine {
    model: TextEmbedding,
}

impl EmbeddingEngine {
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))
            .context("Failed to initialize embedding model")?;

        Ok(Self { model })
    }

    pub fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let embeddings = self
            .model
            .embed(texts.to_vec(), None)
            .context("Failed to generate embeddings")?;
        Ok(embeddings)
    }

    pub fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed(&[text])?;
        embeddings
            .into_iter()
            .next()
            .context("No embedding generated")
    }

    pub fn dimension(&self) -> usize {
        384
    }
}

pub struct AsyncEmbeddingEngine {
    inner: Arc<Mutex<EmbeddingEngine>>,
}

impl AsyncEmbeddingEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(EmbeddingEngine::new()?)),
        })
    }

    pub async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let engine = self.inner.lock().await;
        engine.embed(texts)
    }

    pub async fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let engine = self.inner.lock().await;
        engine.embed_single(text)
    }

    pub fn dimension(&self) -> usize {
        384
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}
