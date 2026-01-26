use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPreset {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub layout_data: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayoutStore {
    pub presets: HashMap<String, LayoutPreset>,
}

fn get_layouts_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("naidis")
        .join("layouts.json")
}

fn load_store() -> Result<LayoutStore> {
    let path = get_layouts_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(LayoutStore::default())
    }
}

fn save_store(store: &LayoutStore) -> Result<()> {
    let path = get_layouts_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(store)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutSaveRequest {
    pub name: String,
    pub description: Option<String>,
    pub layout_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutUpdateRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub layout_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutDeleteRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutListResponse {
    pub presets: Vec<LayoutPreset>,
    pub total: usize,
}

pub fn save_layout(request: &LayoutSaveRequest) -> Result<LayoutPreset> {
    let mut store = load_store()?;
    let now = chrono::Utc::now().timestamp();
    let id = uuid::Uuid::new_v4().to_string();

    let preset = LayoutPreset {
        id: id.clone(),
        name: request.name.clone(),
        description: request.description.clone(),
        layout_data: request.layout_data.clone(),
        created_at: now,
        updated_at: now,
    };

    store.presets.insert(id, preset.clone());
    save_store(&store)?;

    Ok(preset)
}

pub fn update_layout(request: &LayoutUpdateRequest) -> Result<LayoutPreset> {
    let mut store = load_store()?;

    let preset = store
        .presets
        .get_mut(&request.id)
        .ok_or_else(|| anyhow::anyhow!("Layout preset not found: {}", request.id))?;

    if let Some(ref name) = request.name {
        preset.name = name.clone();
    }
    if let Some(ref description) = request.description {
        preset.description = Some(description.clone());
    }
    if let Some(ref layout_data) = request.layout_data {
        preset.layout_data = layout_data.clone();
    }
    preset.updated_at = chrono::Utc::now().timestamp();

    let result = preset.clone();
    save_store(&store)?;

    Ok(result)
}

pub fn delete_layout(request: &LayoutDeleteRequest) -> Result<bool> {
    let mut store = load_store()?;
    let removed = store.presets.remove(&request.id).is_some();
    if removed {
        save_store(&store)?;
    }
    Ok(removed)
}

pub fn get_layout(id: &str) -> Result<Option<LayoutPreset>> {
    let store = load_store()?;
    Ok(store.presets.get(id).cloned())
}

pub fn list_layouts() -> Result<LayoutListResponse> {
    let store = load_store()?;
    let mut presets: Vec<LayoutPreset> = store.presets.values().cloned().collect();
    presets.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let total = presets.len();
    Ok(LayoutListResponse { presets, total })
}
