use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum HighlightError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Highlight not found: {0}")]
    NotFound(String),
    #[error("Article not found: {0}")]
    ArticleNotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Highlight {
    pub id: String,
    pub article_id: String,
    pub text: String,
    pub note: Option<String>,
    pub color: HighlightColor,
    pub position: HighlightPosition,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum HighlightColor {
    #[default]
    Yellow,
    Green,
    Blue,
    Pink,
    Purple,
    Orange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightPosition {
    pub start_offset: usize,
    pub end_offset: usize,
    pub paragraph_index: Option<usize>,
    pub page_number: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHighlightRequest {
    pub article_id: String,
    pub text: String,
    pub note: Option<String>,
    pub color: Option<HighlightColor>,
    pub position: HighlightPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateHighlightRequest {
    pub id: String,
    pub note: Option<String>,
    pub color: Option<HighlightColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightQuery {
    pub article_id: Option<String>,
    pub color: Option<HighlightColor>,
    pub has_note: Option<bool>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightExport {
    pub format: ExportFormat,
    pub article_id: Option<String>,
    pub include_notes: bool,
    pub group_by_color: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Json,
    Html,
}

pub struct HighlightStore {
    data_dir: PathBuf,
    highlights: HashMap<String, Highlight>,
}

impl HighlightStore {
    pub fn new(data_dir: PathBuf) -> Result<Self, HighlightError> {
        let highlights_dir = data_dir.join("highlights");
        fs::create_dir_all(&highlights_dir)?;

        let mut store = Self {
            data_dir: highlights_dir,
            highlights: HashMap::new(),
        };
        store.load_all()?;
        Ok(store)
    }

    fn load_all(&mut self) -> Result<(), HighlightError> {
        let index_path = self.data_dir.join("index.json");
        if index_path.exists() {
            let data = fs::read_to_string(&index_path)?;
            self.highlights = serde_json::from_str(&data)?;
        }
        Ok(())
    }

    fn save_all(&self) -> Result<(), HighlightError> {
        let index_path = self.data_dir.join("index.json");
        let data = serde_json::to_string_pretty(&self.highlights)?;
        fs::write(&index_path, data)?;
        Ok(())
    }

    pub fn create(&mut self, req: CreateHighlightRequest) -> Result<Highlight, HighlightError> {
        let now = Utc::now();
        let highlight = Highlight {
            id: Uuid::new_v4().to_string(),
            article_id: req.article_id,
            text: req.text,
            note: req.note,
            color: req.color.unwrap_or_default(),
            position: req.position,
            created_at: now,
            updated_at: now,
        };

        self.highlights
            .insert(highlight.id.clone(), highlight.clone());
        self.save_all()?;
        Ok(highlight)
    }

    pub fn update(&mut self, req: UpdateHighlightRequest) -> Result<Highlight, HighlightError> {
        let highlight = self
            .highlights
            .get_mut(&req.id)
            .ok_or_else(|| HighlightError::NotFound(req.id.clone()))?;

        if let Some(note) = req.note {
            highlight.note = Some(note);
        }
        if let Some(color) = req.color {
            highlight.color = color;
        }
        highlight.updated_at = Utc::now();

        let updated = highlight.clone();
        self.save_all()?;
        Ok(updated)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), HighlightError> {
        self.highlights
            .remove(id)
            .ok_or_else(|| HighlightError::NotFound(id.to_string()))?;
        self.save_all()?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Highlight> {
        self.highlights.get(id)
    }

    pub fn query(&self, q: HighlightQuery) -> Vec<&Highlight> {
        let mut results: Vec<&Highlight> = self
            .highlights
            .values()
            .filter(|h| {
                if let Some(ref article_id) = q.article_id {
                    if &h.article_id != article_id {
                        return false;
                    }
                }
                if let Some(has_note) = q.has_note {
                    if has_note != h.note.is_some() {
                        return false;
                    }
                }
                if let Some(ref search) = q.search {
                    let search_lower = search.to_lowercase();
                    let in_text = h.text.to_lowercase().contains(&search_lower);
                    let in_note = h
                        .note
                        .as_ref()
                        .map(|n| n.to_lowercase().contains(&search_lower))
                        .unwrap_or(false);
                    if !in_text && !in_note {
                        return false;
                    }
                }
                true
            })
            .collect();

        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let offset = q.offset.unwrap_or(0);
        let limit = q.limit.unwrap_or(100);

        results.into_iter().skip(offset).take(limit).collect()
    }

    pub fn get_by_article(&self, article_id: &str) -> Vec<&Highlight> {
        self.query(HighlightQuery {
            article_id: Some(article_id.to_string()),
            color: None,
            has_note: None,
            search: None,
            limit: None,
            offset: None,
        })
    }

    pub fn export(&self, req: HighlightExport) -> Result<String, HighlightError> {
        let highlights: Vec<&Highlight> = if let Some(ref article_id) = req.article_id {
            self.get_by_article(article_id)
        } else {
            self.highlights.values().collect()
        };

        match req.format {
            ExportFormat::Json => Ok(serde_json::to_string_pretty(&highlights)?),
            ExportFormat::Markdown => {
                let mut output = String::new();

                if req.group_by_color {
                    let mut by_color: HashMap<String, Vec<&Highlight>> = HashMap::new();
                    for h in &highlights {
                        let color_key = format!("{:?}", h.color);
                        by_color.entry(color_key).or_default().push(h);
                    }

                    for (color, hs) in by_color {
                        output.push_str(&format!("## {}\n\n", color));
                        for h in hs {
                            output.push_str(&format!("> {}\n", h.text));
                            if req.include_notes {
                                if let Some(ref note) = h.note {
                                    output.push_str(&format!("\n**Note:** {}\n", note));
                                }
                            }
                            output.push('\n');
                        }
                    }
                } else {
                    for h in &highlights {
                        output.push_str(&format!("> {}\n", h.text));
                        if req.include_notes {
                            if let Some(ref note) = h.note {
                                output.push_str(&format!("\n**Note:** {}\n", note));
                            }
                        }
                        output.push('\n');
                    }
                }

                Ok(output)
            }
            ExportFormat::Html => {
                let mut output = String::from("<div class=\"highlights\">\n");

                for h in &highlights {
                    let color_class = format!("{:?}", h.color).to_lowercase();
                    output.push_str(&format!(
                        "  <blockquote class=\"highlight {}\">{}</blockquote>\n",
                        color_class, h.text
                    ));
                    if req.include_notes {
                        if let Some(ref note) = h.note {
                            output.push_str(&format!("  <p class=\"note\">{}</p>\n", note));
                        }
                    }
                }

                output.push_str("</div>");
                Ok(output)
            }
        }
    }

    pub fn delete_by_article(&mut self, article_id: &str) -> Result<usize, HighlightError> {
        let ids_to_remove: Vec<String> = self
            .highlights
            .values()
            .filter(|h| h.article_id == article_id)
            .map(|h| h.id.clone())
            .collect();

        let count = ids_to_remove.len();
        for id in ids_to_remove {
            self.highlights.remove(&id);
        }

        if count > 0 {
            self.save_all()?;
        }

        Ok(count)
    }
}

pub fn create_highlight(
    data_dir: PathBuf,
    req: CreateHighlightRequest,
) -> Result<Highlight, HighlightError> {
    let mut store = HighlightStore::new(data_dir)?;
    store.create(req)
}

pub fn update_highlight(
    data_dir: PathBuf,
    req: UpdateHighlightRequest,
) -> Result<Highlight, HighlightError> {
    let mut store = HighlightStore::new(data_dir)?;
    store.update(req)
}

pub fn delete_highlight(data_dir: PathBuf, id: &str) -> Result<(), HighlightError> {
    let mut store = HighlightStore::new(data_dir)?;
    store.delete(id)
}

pub fn query_highlights(
    data_dir: PathBuf,
    query: HighlightQuery,
) -> Result<Vec<Highlight>, HighlightError> {
    let store = HighlightStore::new(data_dir)?;
    Ok(store.query(query).into_iter().cloned().collect())
}

pub fn export_highlights(
    data_dir: PathBuf,
    req: HighlightExport,
) -> Result<String, HighlightError> {
    let store = HighlightStore::new(data_dir)?;
    store.export(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_request() -> CreateHighlightRequest {
        CreateHighlightRequest {
            article_id: "article-123".to_string(),
            text: "This is a highlighted text".to_string(),
            note: Some("My note".to_string()),
            color: Some(HighlightColor::Yellow),
            position: HighlightPosition {
                start_offset: 0,
                end_offset: 26,
                paragraph_index: Some(1),
                page_number: None,
            },
        }
    }

    #[test]
    fn test_highlight_color_default() {
        assert!(matches!(HighlightColor::default(), HighlightColor::Yellow));
    }

    #[test]
    fn test_create_highlight() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        let req = create_test_request();
        let highlight = store.create(req).unwrap();

        assert_eq!(highlight.article_id, "article-123");
        assert_eq!(highlight.text, "This is a highlighted text");
        assert_eq!(highlight.note, Some("My note".to_string()));
        assert!(matches!(highlight.color, HighlightColor::Yellow));
    }

    #[test]
    fn test_update_highlight() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        let highlight = store.create(create_test_request()).unwrap();

        let updated = store
            .update(UpdateHighlightRequest {
                id: highlight.id.clone(),
                note: Some("Updated note".to_string()),
                color: Some(HighlightColor::Blue),
            })
            .unwrap();

        assert_eq!(updated.note, Some("Updated note".to_string()));
        assert!(matches!(updated.color, HighlightColor::Blue));
    }

    #[test]
    fn test_delete_highlight() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        let highlight = store.create(create_test_request()).unwrap();
        assert!(store.get(&highlight.id).is_some());

        store.delete(&highlight.id).unwrap();
        assert!(store.get(&highlight.id).is_none());
    }

    #[test]
    fn test_delete_nonexistent_highlight() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        let result = store.delete("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_query_by_article_id() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        store.create(create_test_request()).unwrap();
        store
            .create(CreateHighlightRequest {
                article_id: "article-456".to_string(),
                text: "Another highlight".to_string(),
                note: None,
                color: None,
                position: HighlightPosition {
                    start_offset: 0,
                    end_offset: 17,
                    paragraph_index: None,
                    page_number: None,
                },
            })
            .unwrap();

        let results = store.query(HighlightQuery {
            article_id: Some("article-123".to_string()),
            color: None,
            has_note: None,
            search: None,
            limit: None,
            offset: None,
        });

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].article_id, "article-123");
    }

    #[test]
    fn test_query_with_search() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        store.create(create_test_request()).unwrap();

        let results = store.query(HighlightQuery {
            article_id: None,
            color: None,
            has_note: None,
            search: Some("highlighted".to_string()),
            limit: None,
            offset: None,
        });

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_has_note() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        store.create(create_test_request()).unwrap();
        store
            .create(CreateHighlightRequest {
                article_id: "article-456".to_string(),
                text: "No note".to_string(),
                note: None,
                color: None,
                position: HighlightPosition {
                    start_offset: 0,
                    end_offset: 7,
                    paragraph_index: None,
                    page_number: None,
                },
            })
            .unwrap();

        let with_note = store.query(HighlightQuery {
            article_id: None,
            color: None,
            has_note: Some(true),
            search: None,
            limit: None,
            offset: None,
        });
        assert_eq!(with_note.len(), 1);

        let without_note = store.query(HighlightQuery {
            article_id: None,
            color: None,
            has_note: Some(false),
            search: None,
            limit: None,
            offset: None,
        });
        assert_eq!(without_note.len(), 1);
    }

    #[test]
    fn test_export_markdown() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        store.create(create_test_request()).unwrap();

        let output = store
            .export(HighlightExport {
                format: ExportFormat::Markdown,
                article_id: None,
                include_notes: true,
                group_by_color: false,
            })
            .unwrap();

        assert!(output.contains("> This is a highlighted text"));
        assert!(output.contains("**Note:** My note"));
    }

    #[test]
    fn test_export_json() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        store.create(create_test_request()).unwrap();

        let output = store
            .export(HighlightExport {
                format: ExportFormat::Json,
                article_id: None,
                include_notes: true,
                group_by_color: false,
            })
            .unwrap();

        assert!(output.contains("\"text\": \"This is a highlighted text\""));
    }

    #[test]
    fn test_export_html() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        store.create(create_test_request()).unwrap();

        let output = store
            .export(HighlightExport {
                format: ExportFormat::Html,
                article_id: None,
                include_notes: true,
                group_by_color: false,
            })
            .unwrap();

        assert!(output.contains("<blockquote"));
        assert!(output.contains("This is a highlighted text"));
    }

    #[test]
    fn test_delete_by_article() {
        let dir = tempdir().unwrap();
        let mut store = HighlightStore::new(dir.path().to_path_buf()).unwrap();

        store.create(create_test_request()).unwrap();
        store
            .create(CreateHighlightRequest {
                article_id: "article-123".to_string(),
                text: "Second highlight".to_string(),
                note: None,
                color: None,
                position: HighlightPosition {
                    start_offset: 50,
                    end_offset: 66,
                    paragraph_index: None,
                    page_number: None,
                },
            })
            .unwrap();

        let count = store.delete_by_article("article-123").unwrap();
        assert_eq!(count, 2);

        let remaining = store.get_by_article("article-123");
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        {
            let mut store = HighlightStore::new(path.clone()).unwrap();
            store.create(create_test_request()).unwrap();
        }

        {
            let store = HighlightStore::new(path).unwrap();
            let highlights = store.query(HighlightQuery {
                article_id: None,
                color: None,
                has_note: None,
                search: None,
                limit: None,
                offset: None,
            });
            assert_eq!(highlights.len(), 1);
        }
    }
}
