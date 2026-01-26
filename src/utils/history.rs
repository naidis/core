use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub command: String,
    pub args: Option<serde_json::Value>,
    pub timestamp: i64,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryStore {
    pub entries: VecDeque<HistoryEntry>,
    pub max_size: usize,
}

impl HistoryStore {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            max_size: 100,
        }
    }
}

fn get_history_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("naidis")
        .join("history.json")
}

fn load_store() -> Result<HistoryStore> {
    let path = get_history_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(HistoryStore::new())
    }
}

fn save_store(store: &HistoryStore) -> Result<()> {
    let path = get_history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(store)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryAddRequest {
    pub command: String,
    pub args: Option<serde_json::Value>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryListRequest {
    pub limit: Option<usize>,
    pub command_filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryListResponse {
    pub entries: Vec<HistoryEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryClearRequest {
    pub before: Option<i64>,
}

pub fn add_history(request: &HistoryAddRequest) -> Result<HistoryEntry> {
    let mut store = load_store()?;
    let now = chrono::Utc::now().timestamp();
    let id = uuid::Uuid::new_v4().to_string();

    let entry = HistoryEntry {
        id,
        command: request.command.clone(),
        args: request.args.clone(),
        timestamp: now,
        source: request.source.clone(),
    };

    store.entries.push_front(entry.clone());

    while store.entries.len() > store.max_size {
        store.entries.pop_back();
    }

    save_store(&store)?;
    Ok(entry)
}

pub fn list_history(request: &HistoryListRequest) -> Result<HistoryListResponse> {
    let store = load_store()?;
    let limit = request.limit.unwrap_or(20);

    let mut entries: Vec<HistoryEntry> = store.entries.iter().cloned().collect();

    if let Some(ref filter) = request.command_filter {
        let filter_lower = filter.to_lowercase();
        entries.retain(|e| e.command.to_lowercase().contains(&filter_lower));
    }

    entries.truncate(limit);
    let total = entries.len();

    Ok(HistoryListResponse { entries, total })
}

pub fn clear_history(request: &HistoryClearRequest) -> Result<usize> {
    let mut store = load_store()?;
    let original_len = store.entries.len();

    if let Some(before) = request.before {
        store.entries.retain(|e| e.timestamp >= before);
    } else {
        store.entries.clear();
    }

    let removed = original_len - store.entries.len();
    save_store(&store)?;

    Ok(removed)
}

pub fn get_frequent_commands(limit: usize) -> Result<Vec<(String, usize)>> {
    let store = load_store()?;

    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for entry in &store.entries {
        *counts.entry(entry.command.clone()).or_insert(0) += 1;
    }

    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(limit);

    Ok(sorted)
}
