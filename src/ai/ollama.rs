use anyhow::Result;
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub size: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaStatus {
    pub available: bool,
    pub models: Vec<OllamaModel>,
}

pub async fn check_status() -> OllamaStatus {
    let ollama = Ollama::default();

    match ollama.list_local_models().await {
        Ok(models) => OllamaStatus {
            available: true,
            models: models
                .into_iter()
                .map(|m| OllamaModel {
                    name: m.name,
                    size: format_size(m.size),
                    modified_at: m.modified_at,
                })
                .collect(),
        },
        Err(_) => OllamaStatus {
            available: false,
            models: vec![],
        },
    }
}

pub async fn list_models() -> Result<Vec<OllamaModel>> {
    let ollama = Ollama::default();
    let models = ollama.list_local_models().await?;

    Ok(models
        .into_iter()
        .map(|m| OllamaModel {
            name: m.name,
            size: format_size(m.size),
            modified_at: m.modified_at,
        })
        .collect())
}

pub async fn generate(model: &str, prompt: &str, system: Option<&str>) -> Result<String> {
    let ollama = Ollama::default();

    let mut request = GenerationRequest::new(model.to_string(), prompt.to_string());

    if let Some(sys) = system {
        request = request.system(sys.to_string());
    }

    let response = ollama.generate(request).await?;
    Ok(response.response)
}

pub async fn chat(
    model: &str,
    messages: Vec<(String, String)>,
    system: Option<&str>,
) -> Result<String> {
    use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, MessageRole};

    let ollama = Ollama::default();

    let mut chat_messages: Vec<ChatMessage> = messages
        .into_iter()
        .map(|(role, content)| {
            let msg_role = match role.as_str() {
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                _ => MessageRole::User,
            };
            ChatMessage::new(msg_role, content)
        })
        .collect();

    if let Some(sys) = system {
        chat_messages.insert(0, ChatMessage::new(MessageRole::System, sys.to_string()));
    }

    let request = ChatMessageRequest::new(model.to_string(), chat_messages);
    let response = ollama.send_chat_messages(request).await?;

    Ok(response.message.content)
}

fn format_size(bytes: u64) -> String {
    const GB: u64 = 1024 * 1024 * 1024;
    const MB: u64 = 1024 * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    }
}
