use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum LabelError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Label not found: {0}")]
    NotFound(String),
    #[error("Label already exists: {0}")]
    AlreadyExists(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    pub parent_id: Option<String>,
    pub item_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLabelRequest {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelTree {
    pub label: Label,
    pub children: Vec<LabelTree>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelStats {
    pub total_labels: usize,
    pub labels_with_items: usize,
    pub total_items_labeled: usize,
    pub top_labels: Vec<(String, usize)>,
}

const DEFAULT_COLORS: &[&str] = &[
    "#ef4444", "#f97316", "#eab308", "#22c55e", "#14b8a6", "#3b82f6", "#8b5cf6", "#ec4899",
    "#6b7280",
];

pub struct LabelStore {
    data_dir: PathBuf,
    labels: HashMap<String, Label>,
}

impl LabelStore {
    pub fn new(data_dir: PathBuf) -> Result<Self, LabelError> {
        let labels_dir = data_dir.join("labels");
        fs::create_dir_all(&labels_dir)?;

        let mut store = Self {
            data_dir: labels_dir,
            labels: HashMap::new(),
        };
        store.load_all()?;
        Ok(store)
    }

    fn load_all(&mut self) -> Result<(), LabelError> {
        let index_path = self.data_dir.join("labels.json");
        if index_path.exists() {
            let data = fs::read_to_string(&index_path)?;
            self.labels = serde_json::from_str(&data)?;
        }
        Ok(())
    }

    fn save_all(&self) -> Result<(), LabelError> {
        let index_path = self.data_dir.join("labels.json");
        let data = serde_json::to_string_pretty(&self.labels)?;
        fs::write(&index_path, data)?;
        Ok(())
    }

    fn generate_color(&self) -> String {
        let used_colors: Vec<&str> = self.labels.values().map(|l| l.color.as_str()).collect();

        for color in DEFAULT_COLORS {
            if !used_colors.contains(color) {
                return color.to_string();
            }
        }

        DEFAULT_COLORS[self.labels.len() % DEFAULT_COLORS.len()].to_string()
    }

    pub fn create(&mut self, req: CreateLabelRequest) -> Result<Label, LabelError> {
        let name_lower = req.name.to_lowercase();
        if self
            .labels
            .values()
            .any(|l| l.name.to_lowercase() == name_lower)
        {
            return Err(LabelError::AlreadyExists(req.name));
        }

        if let Some(ref parent_id) = req.parent_id {
            if !self.labels.contains_key(parent_id) {
                return Err(LabelError::NotFound(parent_id.clone()));
            }
        }

        let now = Utc::now();
        let label = Label {
            id: Uuid::new_v4().to_string(),
            name: req.name,
            color: req.color.unwrap_or_else(|| self.generate_color()),
            description: req.description,
            parent_id: req.parent_id,
            item_count: 0,
            created_at: now,
            updated_at: now,
        };

        self.labels.insert(label.id.clone(), label.clone());
        self.save_all()?;
        Ok(label)
    }

    pub fn update(&mut self, req: UpdateLabelRequest) -> Result<Label, LabelError> {
        if !self.labels.contains_key(&req.id) {
            return Err(LabelError::NotFound(req.id.clone()));
        }

        if let Some(ref name) = req.name {
            let name_lower = name.to_lowercase();
            let exists = self
                .labels
                .values()
                .any(|l| l.id != req.id && l.name.to_lowercase() == name_lower);
            if exists {
                return Err(LabelError::AlreadyExists(name.clone()));
            }
        }

        if let Some(ref parent_id) = req.parent_id {
            if !self.labels.contains_key(parent_id) {
                return Err(LabelError::NotFound(parent_id.clone()));
            }
            if parent_id == &req.id {
                return Err(LabelError::NotFound(
                    "Cannot set parent to self".to_string(),
                ));
            }
        }

        let label = self.labels.get_mut(&req.id).unwrap();

        if let Some(name) = req.name {
            label.name = name;
        }
        if let Some(color) = req.color {
            label.color = color;
        }
        if let Some(description) = req.description {
            label.description = Some(description);
        }
        label.parent_id = req.parent_id;
        label.updated_at = Utc::now();

        let updated = label.clone();
        self.save_all()?;
        Ok(updated)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), LabelError> {
        self.labels
            .remove(id)
            .ok_or_else(|| LabelError::NotFound(id.to_string()))?;

        let children: Vec<String> = self
            .labels
            .values()
            .filter(|l| l.parent_id.as_deref() == Some(id))
            .map(|l| l.id.clone())
            .collect();

        for child_id in children {
            if let Some(child) = self.labels.get_mut(&child_id) {
                child.parent_id = None;
            }
        }

        self.save_all()
    }

    pub fn get(&self, id: &str) -> Option<&Label> {
        self.labels.get(id)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&Label> {
        let name_lower = name.to_lowercase();
        self.labels
            .values()
            .find(|l| l.name.to_lowercase() == name_lower)
    }

    pub fn list(&self) -> Vec<&Label> {
        let mut labels: Vec<&Label> = self.labels.values().collect();
        labels.sort_by(|a, b| a.name.cmp(&b.name));
        labels
    }

    pub fn get_tree(&self) -> Vec<LabelTree> {
        let root_labels: Vec<&Label> = self
            .labels
            .values()
            .filter(|l| l.parent_id.is_none())
            .collect();

        root_labels
            .into_iter()
            .map(|l| self.build_tree(l))
            .collect()
    }

    fn build_tree(&self, label: &Label) -> LabelTree {
        let children: Vec<LabelTree> = self
            .labels
            .values()
            .filter(|l| l.parent_id.as_deref() == Some(&label.id))
            .map(|l| self.build_tree(l))
            .collect();

        LabelTree {
            label: label.clone(),
            children,
        }
    }

    pub fn increment_count(&mut self, id: &str) -> Result<(), LabelError> {
        let label = self
            .labels
            .get_mut(id)
            .ok_or_else(|| LabelError::NotFound(id.to_string()))?;
        label.item_count += 1;
        self.save_all()
    }

    pub fn decrement_count(&mut self, id: &str) -> Result<(), LabelError> {
        let label = self
            .labels
            .get_mut(id)
            .ok_or_else(|| LabelError::NotFound(id.to_string()))?;
        if label.item_count > 0 {
            label.item_count -= 1;
        }
        self.save_all()
    }

    pub fn get_stats(&self) -> LabelStats {
        let mut top_labels: Vec<(String, usize)> = self
            .labels
            .values()
            .map(|l| (l.name.clone(), l.item_count))
            .collect();
        top_labels.sort_by(|a, b| b.1.cmp(&a.1));
        top_labels.truncate(10);

        LabelStats {
            total_labels: self.labels.len(),
            labels_with_items: self.labels.values().filter(|l| l.item_count > 0).count(),
            total_items_labeled: self.labels.values().map(|l| l.item_count).sum(),
            top_labels,
        }
    }

    pub fn merge(&mut self, source_id: &str, target_id: &str) -> Result<Label, LabelError> {
        let source = self
            .labels
            .get(source_id)
            .ok_or_else(|| LabelError::NotFound(source_id.to_string()))?
            .clone();

        let target = self
            .labels
            .get_mut(target_id)
            .ok_or_else(|| LabelError::NotFound(target_id.to_string()))?;

        target.item_count += source.item_count;
        target.updated_at = Utc::now();
        let merged = target.clone();

        self.labels.remove(source_id);
        self.save_all()?;
        Ok(merged)
    }

    pub fn search(&self, query: &str) -> Vec<&Label> {
        let query_lower = query.to_lowercase();
        self.labels
            .values()
            .filter(|l| {
                l.name.to_lowercase().contains(&query_lower)
                    || l.description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
            })
            .collect()
    }
}

pub fn create_label(data_dir: PathBuf, req: CreateLabelRequest) -> Result<Label, LabelError> {
    let mut store = LabelStore::new(data_dir)?;
    store.create(req)
}

pub fn update_label(data_dir: PathBuf, req: UpdateLabelRequest) -> Result<Label, LabelError> {
    let mut store = LabelStore::new(data_dir)?;
    store.update(req)
}

pub fn delete_label(data_dir: PathBuf, id: &str) -> Result<(), LabelError> {
    let mut store = LabelStore::new(data_dir)?;
    store.delete(id)
}

pub fn get_label(data_dir: PathBuf, id: &str) -> Result<Option<Label>, LabelError> {
    let store = LabelStore::new(data_dir)?;
    Ok(store.get(id).cloned())
}

pub fn list_labels(data_dir: PathBuf) -> Result<Vec<Label>, LabelError> {
    let store = LabelStore::new(data_dir)?;
    Ok(store.list().into_iter().cloned().collect())
}

pub fn get_label_tree(data_dir: PathBuf) -> Result<Vec<LabelTree>, LabelError> {
    let store = LabelStore::new(data_dir)?;
    Ok(store.get_tree())
}

pub fn get_label_stats(data_dir: PathBuf) -> Result<LabelStats, LabelError> {
    let store = LabelStore::new(data_dir)?;
    Ok(store.get_stats())
}

pub fn merge_labels(
    data_dir: PathBuf,
    source_id: &str,
    target_id: &str,
) -> Result<Label, LabelError> {
    let mut store = LabelStore::new(data_dir)?;
    store.merge(source_id, target_id)
}

pub fn search_labels(data_dir: PathBuf, query: &str) -> Result<Vec<Label>, LabelError> {
    let store = LabelStore::new(data_dir)?;
    Ok(store.search(query).into_iter().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_label() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let label = store
            .create(CreateLabelRequest {
                name: "Tech".to_string(),
                color: Some("#3b82f6".to_string()),
                description: Some("Technology articles".to_string()),
                parent_id: None,
            })
            .unwrap();

        assert_eq!(label.name, "Tech");
        assert_eq!(label.color, "#3b82f6");
        assert_eq!(label.description, Some("Technology articles".to_string()));
        assert_eq!(label.item_count, 0);
    }

    #[test]
    fn test_create_label_auto_color() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let label = store
            .create(CreateLabelRequest {
                name: "Test".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        assert!(!label.color.is_empty());
        assert!(label.color.starts_with('#'));
    }

    #[test]
    fn test_create_label_duplicate_name() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        store
            .create(CreateLabelRequest {
                name: "Tech".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let result = store.create(CreateLabelRequest {
            name: "tech".to_string(),
            color: None,
            description: None,
            parent_id: None,
        });

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LabelError::AlreadyExists(_)));
    }

    #[test]
    fn test_create_label_with_parent() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let parent = store
            .create(CreateLabelRequest {
                name: "Programming".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let child = store
            .create(CreateLabelRequest {
                name: "Rust".to_string(),
                color: None,
                description: None,
                parent_id: Some(parent.id.clone()),
            })
            .unwrap();

        assert_eq!(child.parent_id, Some(parent.id));
    }

    #[test]
    fn test_create_label_invalid_parent() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let result = store.create(CreateLabelRequest {
            name: "Test".to_string(),
            color: None,
            description: None,
            parent_id: Some("nonexistent".to_string()),
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_update_label() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let label = store
            .create(CreateLabelRequest {
                name: "Tech".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let updated = store
            .update(UpdateLabelRequest {
                id: label.id,
                name: Some("Technology".to_string()),
                color: Some("#ef4444".to_string()),
                description: Some("Updated description".to_string()),
                parent_id: None,
            })
            .unwrap();

        assert_eq!(updated.name, "Technology");
        assert_eq!(updated.color, "#ef4444");
        assert_eq!(updated.description, Some("Updated description".to_string()));
    }

    #[test]
    fn test_update_label_not_found() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let result = store.update(UpdateLabelRequest {
            id: "nonexistent".to_string(),
            name: None,
            color: None,
            description: None,
            parent_id: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_update_label_duplicate_name() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        store
            .create(CreateLabelRequest {
                name: "First".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let second = store
            .create(CreateLabelRequest {
                name: "Second".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let result = store.update(UpdateLabelRequest {
            id: second.id,
            name: Some("First".to_string()),
            color: None,
            description: None,
            parent_id: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_delete_label() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let label = store
            .create(CreateLabelRequest {
                name: "Tech".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        store.delete(&label.id).unwrap();
        assert!(store.get(&label.id).is_none());
    }

    #[test]
    fn test_delete_label_updates_children() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let parent = store
            .create(CreateLabelRequest {
                name: "Parent".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let child = store
            .create(CreateLabelRequest {
                name: "Child".to_string(),
                color: None,
                description: None,
                parent_id: Some(parent.id.clone()),
            })
            .unwrap();

        store.delete(&parent.id).unwrap();

        let updated_child = store.get(&child.id).unwrap();
        assert!(updated_child.parent_id.is_none());
    }

    #[test]
    fn test_get_by_name() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        store
            .create(CreateLabelRequest {
                name: "Tech".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        assert!(store.get_by_name("Tech").is_some());
        assert!(store.get_by_name("tech").is_some());
        assert!(store.get_by_name("TECH").is_some());
        assert!(store.get_by_name("Unknown").is_none());
    }

    #[test]
    fn test_list_labels() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        store
            .create(CreateLabelRequest {
                name: "Zebra".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        store
            .create(CreateLabelRequest {
                name: "Apple".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let labels = store.list();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].name, "Apple");
        assert_eq!(labels[1].name, "Zebra");
    }

    #[test]
    fn test_get_tree() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let parent = store
            .create(CreateLabelRequest {
                name: "Programming".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        store
            .create(CreateLabelRequest {
                name: "Rust".to_string(),
                color: None,
                description: None,
                parent_id: Some(parent.id.clone()),
            })
            .unwrap();

        store
            .create(CreateLabelRequest {
                name: "Python".to_string(),
                color: None,
                description: None,
                parent_id: Some(parent.id),
            })
            .unwrap();

        let tree = store.get_tree();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].label.name, "Programming");
        assert_eq!(tree[0].children.len(), 2);
    }

    #[test]
    fn test_increment_decrement_count() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let label = store
            .create(CreateLabelRequest {
                name: "Tech".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        assert_eq!(store.get(&label.id).unwrap().item_count, 0);

        store.increment_count(&label.id).unwrap();
        assert_eq!(store.get(&label.id).unwrap().item_count, 1);

        store.increment_count(&label.id).unwrap();
        assert_eq!(store.get(&label.id).unwrap().item_count, 2);

        store.decrement_count(&label.id).unwrap();
        assert_eq!(store.get(&label.id).unwrap().item_count, 1);

        store.decrement_count(&label.id).unwrap();
        store.decrement_count(&label.id).unwrap();
        assert_eq!(store.get(&label.id).unwrap().item_count, 0);
    }

    #[test]
    fn test_get_stats() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let label1 = store
            .create(CreateLabelRequest {
                name: "Tech".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        store
            .create(CreateLabelRequest {
                name: "Empty".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        store.increment_count(&label1.id).unwrap();
        store.increment_count(&label1.id).unwrap();

        let stats = store.get_stats();

        assert_eq!(stats.total_labels, 2);
        assert_eq!(stats.labels_with_items, 1);
        assert_eq!(stats.total_items_labeled, 2);
        assert!(!stats.top_labels.is_empty());
    }

    #[test]
    fn test_merge_labels() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        let source = store
            .create(CreateLabelRequest {
                name: "Source".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let target = store
            .create(CreateLabelRequest {
                name: "Target".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        store.increment_count(&source.id).unwrap();
        store.increment_count(&source.id).unwrap();
        store.increment_count(&target.id).unwrap();

        let merged = store.merge(&source.id, &target.id).unwrap();

        assert_eq!(merged.item_count, 3);
        assert!(store.get(&source.id).is_none());
        assert!(store.get(&target.id).is_some());
    }

    #[test]
    fn test_search_labels() {
        let dir = tempdir().unwrap();
        let mut store = LabelStore::new(dir.path().to_path_buf()).unwrap();

        store
            .create(CreateLabelRequest {
                name: "Technology".to_string(),
                color: None,
                description: Some("Tech related".to_string()),
                parent_id: None,
            })
            .unwrap();

        store
            .create(CreateLabelRequest {
                name: "Science".to_string(),
                color: None,
                description: None,
                parent_id: None,
            })
            .unwrap();

        let by_name = store.search("tech");
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].name, "Technology");

        let by_desc = store.search("related");
        assert_eq!(by_desc.len(), 1);
    }

    #[test]
    fn test_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        {
            let mut store = LabelStore::new(path.clone()).unwrap();
            store
                .create(CreateLabelRequest {
                    name: "Test".to_string(),
                    color: None,
                    description: None,
                    parent_id: None,
                })
                .unwrap();
        }

        {
            let store = LabelStore::new(path).unwrap();
            let labels = store.list();
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0].name, "Test");
        }
    }
}
