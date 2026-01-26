use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub trigger: String,
    pub content: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SnippetStore {
    pub snippets: HashMap<String, Snippet>,
}

fn get_snippets_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("naidis")
        .join("snippets.json")
}

fn load_store() -> Result<SnippetStore> {
    let path = get_snippets_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(SnippetStore::default())
    }
}

fn save_store(store: &SnippetStore) -> Result<()> {
    let path = get_snippets_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(store)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetCreateRequest {
    pub trigger: String,
    pub content: String,
    pub description: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetUpdateRequest {
    pub id: String,
    pub trigger: Option<String>,
    pub content: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetDeleteRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetExpandRequest {
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetListRequest {
    pub category: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetListResponse {
    pub snippets: Vec<Snippet>,
    pub total: usize,
}

pub fn create_snippet(request: &SnippetCreateRequest) -> Result<Snippet> {
    let mut store = load_store()?;
    let now = chrono::Utc::now().timestamp();
    let id = uuid::Uuid::new_v4().to_string();

    let snippet = Snippet {
        id: id.clone(),
        trigger: request.trigger.clone(),
        content: request.content.clone(),
        description: request.description.clone(),
        category: request.category.clone(),
        created_at: now,
        updated_at: now,
    };

    store.snippets.insert(id, snippet.clone());
    save_store(&store)?;

    Ok(snippet)
}

pub fn update_snippet(request: &SnippetUpdateRequest) -> Result<Snippet> {
    let mut store = load_store()?;

    let snippet = store
        .snippets
        .get_mut(&request.id)
        .ok_or_else(|| anyhow::anyhow!("Snippet not found: {}", request.id))?;

    if let Some(ref trigger) = request.trigger {
        snippet.trigger = trigger.clone();
    }
    if let Some(ref content) = request.content {
        snippet.content = content.clone();
    }
    if let Some(ref description) = request.description {
        snippet.description = Some(description.clone());
    }
    if let Some(ref category) = request.category {
        snippet.category = Some(category.clone());
    }
    snippet.updated_at = chrono::Utc::now().timestamp();

    let result = snippet.clone();
    save_store(&store)?;

    Ok(result)
}

pub fn delete_snippet(request: &SnippetDeleteRequest) -> Result<bool> {
    let mut store = load_store()?;
    let removed = store.snippets.remove(&request.id).is_some();
    if removed {
        save_store(&store)?;
    }
    Ok(removed)
}

pub fn expand_snippet(request: &SnippetExpandRequest) -> Result<Option<String>> {
    let store = load_store()?;

    for snippet in store.snippets.values() {
        if snippet.trigger == request.trigger {
            return Ok(Some(snippet.content.clone()));
        }
    }

    Ok(None)
}

pub fn list_snippets(request: &SnippetListRequest) -> Result<SnippetListResponse> {
    let store = load_store()?;

    let mut snippets: Vec<Snippet> = store.snippets.values().cloned().collect();

    if let Some(ref category) = request.category {
        snippets.retain(|s| s.category.as_ref() == Some(category));
    }

    if let Some(ref search) = request.search {
        let search_lower = search.to_lowercase();
        snippets.retain(|s| {
            s.trigger.to_lowercase().contains(&search_lower)
                || s.content.to_lowercase().contains(&search_lower)
                || s.description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&search_lower))
                    .unwrap_or(false)
        });
    }

    snippets.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let total = snippets.len();

    Ok(SnippetListResponse { snippets, total })
}
