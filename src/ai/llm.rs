use anyhow::{Context, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_llama as llama;
use hf_hub::{api::sync::Api, Repo, RepoType};
use std::path::PathBuf;
use std::sync::Arc;
use tokenizers::Tokenizer;
use tokio::sync::Mutex;

const DEFAULT_MODEL_REPO: &str = "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF";
const DEFAULT_MODEL_FILE: &str = "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf";
const DEFAULT_TOKENIZER_REPO: &str = "TinyLlama/TinyLlama-1.1B-Chat-v1.0";

pub struct LlmEngine {
    model: Option<llama::ModelWeights>,
    tokenizer: Option<Tokenizer>,
    device: Device,
    model_path: Option<PathBuf>,
}

impl LlmEngine {
    pub fn new() -> Result<Self> {
        let device = Device::Cpu;
        Ok(Self {
            model: None,
            tokenizer: None,
            device,
            model_path: None,
        })
    }

    pub fn load_model(&mut self, model_id: &str) -> Result<()> {
        tracing::info!("Loading model: {}", model_id);

        let (repo_id, filename) = if model_id.contains('/') {
            let parts: Vec<&str> = model_id.split(':').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (model_id.to_string(), DEFAULT_MODEL_FILE.to_string())
            }
        } else {
            (
                DEFAULT_MODEL_REPO.to_string(),
                DEFAULT_MODEL_FILE.to_string(),
            )
        };

        let api = Api::new().context("Failed to create HF API")?;
        let repo = api.repo(Repo::new(repo_id.clone(), RepoType::Model));

        tracing::info!("Downloading model from HuggingFace: {}", repo_id);
        let model_path = repo
            .get(&filename)
            .context("Failed to download model file")?;

        tracing::info!("Loading GGUF model from: {:?}", model_path);
        let mut file = std::fs::File::open(&model_path)?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| anyhow::anyhow!("Failed to read GGUF: {}", e))?;

        let model = llama::ModelWeights::from_gguf(content, &mut file, &self.device)
            .map_err(|e| anyhow::anyhow!("Failed to load model weights: {}", e))?;

        let tokenizer_repo = if repo_id.contains("TinyLlama") {
            DEFAULT_TOKENIZER_REPO
        } else {
            &repo_id
        };

        tracing::info!("Loading tokenizer from: {}", tokenizer_repo);
        let tokenizer_repo = api.repo(Repo::new(tokenizer_repo.to_string(), RepoType::Model));
        let tokenizer_path = tokenizer_repo
            .get("tokenizer.json")
            .context("Failed to download tokenizer")?;

        let tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| anyhow::anyhow!("{}", e))?;

        self.model = Some(model);
        self.tokenizer = Some(tokenizer);
        self.model_path = Some(model_path);

        tracing::info!("Model loaded successfully");
        Ok(())
    }

    pub fn generate_sync(&mut self, prompt: &str, max_tokens: u32) -> Result<String> {
        let model = self
            .model
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Model not loaded. Call load_model() first."))?;

        let tokenizer = self
            .tokenizer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tokenizer not loaded"))?;

        let formatted_prompt = format!(
            "<|system|>\nYou are a helpful assistant.</s>\n<|user|>\n{}</s>\n<|assistant|>\n",
            prompt
        );

        let tokens = tokenizer
            .encode(formatted_prompt.as_str(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let prompt_tokens = tokens.get_ids().to_vec();
        let mut all_tokens = prompt_tokens.clone();

        let mut logits_processor = LogitsProcessor::new(42, Some(0.8), Some(0.95));

        let mut next_token = {
            let input = Tensor::new(&prompt_tokens[..], &self.device)?.unsqueeze(0)?;
            let logits = model.forward(&input, 0)?;
            let logits = logits.squeeze(0)?.squeeze(0)?;
            logits_processor
                .sample(&logits)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        };

        all_tokens.push(next_token);

        let eos_token = tokenizer
            .token_to_id("</s>")
            .unwrap_or(tokenizer.token_to_id("<|endoftext|>").unwrap_or(2));

        for i in 0..max_tokens {
            if next_token == eos_token {
                break;
            }

            let input = Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
            let logits = model.forward(&input, prompt_tokens.len() + i as usize)?;
            let logits = logits.squeeze(0)?.squeeze(0)?;

            next_token = logits_processor
                .sample(&logits)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            all_tokens.push(next_token);
        }

        let generated_tokens = &all_tokens[prompt_tokens.len()..];
        let response = tokenizer
            .decode(generated_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decode failed: {}", e))?;

        Ok(response.trim().to_string())
    }

    pub fn is_loaded(&self) -> bool {
        self.model.is_some() && self.tokenizer.is_some()
    }
}

pub struct AsyncLlmEngine {
    inner: Arc<Mutex<LlmEngine>>,
    loaded: Arc<Mutex<bool>>,
}

impl AsyncLlmEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(LlmEngine::new()?)),
            loaded: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn load_model(&self, model_name: &str) -> Result<()> {
        let mut engine = self.inner.lock().await;
        engine.load_model(model_name)?;
        *self.loaded.lock().await = true;
        Ok(())
    }

    pub async fn ensure_loaded(&self) -> Result<()> {
        let loaded = *self.loaded.lock().await;
        if !loaded {
            self.load_model(DEFAULT_MODEL_REPO).await?;
        }
        Ok(())
    }

    pub async fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String> {
        if self.ensure_loaded().await.is_err() {
            return Ok(
                "[AI not available] Failed to load model. Check logs for details.".to_string(),
            );
        }

        let mut engine = self.inner.lock().await;
        match engine.generate_sync(prompt, max_tokens) {
            Ok(response) => Ok(response),
            Err(e) => {
                tracing::error!("Generation failed: {}", e);
                Ok(format!("[AI error] {}", e))
            }
        }
    }

    pub async fn is_loaded(&self) -> bool {
        *self.loaded.lock().await
    }
}
