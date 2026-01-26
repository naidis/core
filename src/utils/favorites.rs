use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteItem {
    pub id: String,
    pub item_type: String,
    pub item_id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub data: Option<serde_json::Value>,
    pub created_at: i64,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FavoriteStore {
    pub items: HashMap<String, FavoriteItem>,
    pub order_counter: i32,
}

fn get_favorites_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("naidis")
        .join("favorites.json")
}

fn load_store() -> Result<FavoriteStore> {
    let path = get_favorites_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(FavoriteStore::default())
    }
}

fn save_store(store: &FavoriteStore) -> Result<()> {
    let path = get_favorites_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(store)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteAddRequest {
    pub item_type: String,
    pub item_id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteRemoveRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteReorderRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteListRequest {
    pub item_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteListResponse {
    pub items: Vec<FavoriteItem>,
    pub total: usize,
}

pub fn add_favorite(request: &FavoriteAddRequest) -> Result<FavoriteItem> {
    let mut store = load_store()?;
    let now = chrono::Utc::now().timestamp();
    let id = uuid::Uuid::new_v4().to_string();

    store.order_counter += 1;

    let item = FavoriteItem {
        id: id.clone(),
        item_type: request.item_type.clone(),
        item_id: request.item_id.clone(),
        name: request.name.clone(),
        description: request.description.clone(),
        icon: request.icon.clone(),
        data: request.data.clone(),
        created_at: now,
        order: store.order_counter,
    };

    store.items.insert(id, item.clone());
    save_store(&store)?;

    Ok(item)
}

pub fn remove_favorite(request: &FavoriteRemoveRequest) -> Result<bool> {
    let mut store = load_store()?;
    let removed = store.items.remove(&request.id).is_some();
    if removed {
        save_store(&store)?;
    }
    Ok(removed)
}

pub fn reorder_favorites(request: &FavoriteReorderRequest) -> Result<()> {
    let mut store = load_store()?;

    for (index, id) in request.ids.iter().enumerate() {
        if let Some(item) = store.items.get_mut(id) {
            item.order = index as i32;
        }
    }

    save_store(&store)?;
    Ok(())
}

pub fn list_favorites(request: &FavoriteListRequest) -> Result<FavoriteListResponse> {
    let store = load_store()?;

    let mut items: Vec<FavoriteItem> = store.items.values().cloned().collect();

    if let Some(ref item_type) = request.item_type {
        items.retain(|i| &i.item_type == item_type);
    }

    items.sort_by_key(|i| i.order);
    let total = items.len();

    Ok(FavoriteListResponse { items, total })
}

pub fn is_favorite(item_type: &str, item_id: &str) -> Result<bool> {
    let store = load_store()?;

    for item in store.items.values() {
        if item.item_type == item_type && item.item_id == item_id {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn toggle_favorite(request: &FavoriteAddRequest) -> Result<(bool, Option<FavoriteItem>)> {
    let store = load_store()?;

    for (id, item) in store.items.iter() {
        if item.item_type == request.item_type && item.item_id == request.item_id {
            remove_favorite(&FavoriteRemoveRequest { id: id.clone() })?;
            return Ok((false, None));
        }
    }

    let item = add_favorite(request)?;
    Ok((true, Some(item)))
}
