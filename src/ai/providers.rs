use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum LlmProvider {
    #[default]
    Local,
    Ollama,
    OpenAI,
    Anthropic,
    #[serde(alias = "zai", alias = "z.ai")]
    Zai,
    Groq,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct LlmConfig {
    pub provider: LlmProvider,
    #[ts(optional)]
    pub api_key: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
    #[ts(optional)]
    pub base_url: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Local,
            api_key: None,
            model: None,
            base_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[async_trait]
pub trait LlmProviderTrait: Send + Sync {
    async fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String>;
    async fn chat(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> Result<String>;
    fn name(&self) -> &'static str;
}

pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        }
    }
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[async_trait]
impl LlmProviderTrait for OpenAIProvider {
    async fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String> {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }];
        self.chat(messages, max_tokens).await
    }

    async fn chat(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> Result<String> {
        let openai_messages: Vec<OpenAIMessage> = messages
            .into_iter()
            .map(|m| OpenAIMessage {
                role: m.role,
                content: m.content,
            })
            .collect();

        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
            max_tokens,
            temperature: 0.7,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to call OpenAI API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error: {} - {}", status, text);
        }

        let data: OpenAIResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        data.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))
    }

    fn name(&self) -> &'static str {
        "OpenAI"
    }
}

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
        }
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[async_trait]
impl LlmProviderTrait for AnthropicProvider {
    async fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String> {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }];
        self.chat(messages, max_tokens).await
    }

    async fn chat(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> Result<String> {
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .into_iter()
            .map(|m| AnthropicMessage {
                role: if m.role == "user" {
                    "user"
                } else {
                    "assistant"
                }
                .to_string(),
                content: m.content,
            })
            .collect();

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens,
            messages: anthropic_messages,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to call Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error: {} - {}", status, text);
        }

        let data: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        data.content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| anyhow::anyhow!("No response from Anthropic"))
    }

    fn name(&self) -> &'static str {
        "Anthropic"
    }
}

pub fn create_provider(config: &LlmConfig) -> Result<Box<dyn LlmProviderTrait>> {
    match config.provider {
        LlmProvider::OpenAI => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("OpenAI API key required"))?;
            Ok(Box::new(OpenAIProvider::new(
                api_key,
                config.model.clone(),
                config.base_url.clone(),
            )))
        }
        LlmProvider::Anthropic => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Anthropic API key required"))?;
            Ok(Box::new(AnthropicProvider::new(
                api_key,
                config.model.clone(),
            )))
        }
        LlmProvider::Zai => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("z.ai API key required"))?;
            Ok(Box::new(OpenAIProvider::new(
                api_key,
                config.model.clone().or_else(|| Some("glm-4.7".to_string())),
                Some("https://api.z.ai/api/paas/v4".to_string()),
            )))
        }
        LlmProvider::Groq => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Groq API key required"))?;
            Ok(Box::new(OpenAIProvider::new(
                api_key,
                config
                    .model
                    .clone()
                    .or_else(|| Some("llama-3.3-70b-versatile".to_string())),
                Some("https://api.groq.com/openai/v1".to_string()),
            )))
        }
        LlmProvider::Local | LlmProvider::Ollama => {
            anyhow::bail!("Use dedicated local/ollama engine instead of provider abstraction")
        }
    }
}

pub async fn openai_stream(
    api_key: &str,
    model: &str,
    message: &str,
    system: Option<&str>,
    tx: tokio::sync::mpsc::Sender<Result<String>>,
) -> Result<()> {
    openai_compatible_stream(
        api_key,
        model,
        message,
        system,
        "https://api.openai.com/v1",
        tx,
    )
    .await
}

pub async fn groq_stream(
    api_key: &str,
    model: &str,
    message: &str,
    system: Option<&str>,
    tx: tokio::sync::mpsc::Sender<Result<String>>,
) -> Result<()> {
    openai_compatible_stream(
        api_key,
        model,
        message,
        system,
        "https://api.groq.com/openai/v1",
        tx,
    )
    .await
}

async fn openai_compatible_stream(
    api_key: &str,
    model: &str,
    message: &str,
    system: Option<&str>,
    base_url: &str,
    tx: tokio::sync::mpsc::Sender<Result<String>>,
) -> Result<()> {
    let client = Client::new();

    let mut messages = Vec::new();
    if let Some(sys) = system {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }
    messages.push(serde_json::json!({"role": "user", "content": message}));

    let request_body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "max_tokens": 4096,
    });

    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .context("Failed to call streaming API")?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("API error: {} - {}", status, text);
    }

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("Failed to read stream chunk")?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = line.trim_start_matches("data: ");
                if data == "[DONE]" {
                    return Ok(());
                }

                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                        if tx.send(Ok(content.to_string())).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn anthropic_stream(
    api_key: &str,
    model: &str,
    message: &str,
    system: Option<&str>,
    tx: tokio::sync::mpsc::Sender<Result<String>>,
) -> Result<()> {
    let client = Client::new();

    let mut request_body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "messages": [{"role": "user", "content": message}],
        "stream": true,
    });

    if let Some(sys) = system {
        request_body["system"] = serde_json::json!(sys);
    }

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .context("Failed to call Anthropic streaming API")?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error: {} - {}", status, text);
    }

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("Failed to read stream chunk")?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = line.trim_start_matches("data: ");

                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                    if parsed["type"] == "content_block_delta" {
                        if let Some(content) = parsed["delta"]["text"].as_str() {
                            if tx.send(Ok(content.to_string())).await.is_err() {
                                return Ok(());
                            }
                        }
                    } else if parsed["type"] == "message_stop" {
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert!(matches!(config.provider, LlmProvider::Local));
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_create_provider_missing_key() {
        let config = LlmConfig {
            provider: LlmProvider::OpenAI,
            api_key: None,
            model: None,
            base_url: None,
        };
        assert!(create_provider(&config).is_err());
    }
}
