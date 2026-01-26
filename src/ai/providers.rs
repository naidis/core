use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: Option<String>,
    pub model: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
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
