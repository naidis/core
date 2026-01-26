use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ReadingError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Article not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ReadingState {
    #[default]
    Inbox,
    Later,
    Reading,
    Finished,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ArticleType {
    #[default]
    Article,
    Pdf,
    Epub,
    Newsletter,
    Tweet,
    Video,
    Podcast,
    Image,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub content: String,
    pub excerpt: Option<String>,
    pub site_name: Option<String>,
    pub word_count: usize,
    pub reading_time_minutes: usize,
    pub article_type: ArticleType,
    pub state: ReadingState,
    pub progress: f32,
    pub labels: Vec<String>,
    pub is_favorite: bool,
    pub thumbnail_url: Option<String>,
    pub saved_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveArticleRequest {
    pub url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub content: String,
    pub excerpt: Option<String>,
    pub site_name: Option<String>,
    pub article_type: Option<ArticleType>,
    pub labels: Option<Vec<String>>,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateArticleRequest {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<ReadingState>,
    pub progress: Option<f32>,
    pub labels: Option<Vec<String>>,
    pub is_favorite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleQuery {
    pub state: Option<ReadingState>,
    pub article_type: Option<ArticleType>,
    pub labels: Option<Vec<String>>,
    pub is_favorite: Option<bool>,
    pub search: Option<String>,
    pub sort_by: Option<SortBy>,
    pub sort_order: Option<SortOrder>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    SavedAt,
    UpdatedAt,
    Title,
    ReadingTime,
    Progress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingStats {
    pub total_articles: usize,
    pub inbox_count: usize,
    pub reading_count: usize,
    pub finished_count: usize,
    pub archived_count: usize,
    pub favorite_count: usize,
    pub total_words_read: usize,
    pub total_reading_time_minutes: usize,
    pub articles_by_type: HashMap<String, usize>,
    pub articles_by_label: HashMap<String, usize>,
}

pub struct ReadingStore {
    data_dir: PathBuf,
    articles: HashMap<String, Article>,
}

impl ReadingStore {
    pub fn new(data_dir: PathBuf) -> Result<Self, ReadingError> {
        let reading_dir = data_dir.join("reading");
        fs::create_dir_all(&reading_dir)?;

        let mut store = Self {
            data_dir: reading_dir,
            articles: HashMap::new(),
        };
        store.load_all()?;
        Ok(store)
    }

    fn load_all(&mut self) -> Result<(), ReadingError> {
        let index_path = self.data_dir.join("articles.json");
        if index_path.exists() {
            let data = fs::read_to_string(&index_path)?;
            self.articles = serde_json::from_str(&data)?;
        }
        Ok(())
    }

    fn save_all(&self) -> Result<(), ReadingError> {
        let index_path = self.data_dir.join("articles.json");
        let data = serde_json::to_string_pretty(&self.articles)?;
        fs::write(&index_path, data)?;
        Ok(())
    }

    fn calculate_reading_time(word_count: usize) -> usize {
        (word_count as f32 / 200.0).ceil() as usize
    }

    fn count_words(content: &str) -> usize {
        content.split_whitespace().count()
    }

    pub fn save(&mut self, req: SaveArticleRequest) -> Result<Article, ReadingError> {
        let now = Utc::now();
        let word_count = Self::count_words(&req.content);

        let article = Article {
            id: Uuid::new_v4().to_string(),
            url: req.url,
            title: req.title,
            author: req.author,
            content: req.content,
            excerpt: req.excerpt,
            site_name: req.site_name,
            word_count,
            reading_time_minutes: Self::calculate_reading_time(word_count),
            article_type: req.article_type.unwrap_or_default(),
            state: ReadingState::Inbox,
            progress: 0.0,
            labels: req.labels.unwrap_or_default(),
            is_favorite: false,
            thumbnail_url: req.thumbnail_url,
            saved_at: now,
            updated_at: now,
            read_at: None,
            archived_at: None,
        };

        self.articles.insert(article.id.clone(), article.clone());
        self.save_all()?;
        Ok(article)
    }

    pub fn update(&mut self, req: UpdateArticleRequest) -> Result<Article, ReadingError> {
        let article = self
            .articles
            .get_mut(&req.id)
            .ok_or_else(|| ReadingError::NotFound(req.id.clone()))?;

        if let Some(title) = req.title {
            article.title = title;
        }
        if let Some(state) = req.state {
            let now = Utc::now();
            match state {
                ReadingState::Finished => {
                    article.read_at = Some(now);
                    article.progress = 100.0;
                }
                ReadingState::Archived => {
                    article.archived_at = Some(now);
                }
                _ => {}
            }
            article.state = state;
        }
        if let Some(progress) = req.progress {
            article.progress = progress.clamp(0.0, 100.0);
            if article.progress >= 100.0 && article.state == ReadingState::Reading {
                article.state = ReadingState::Finished;
                article.read_at = Some(Utc::now());
            }
        }
        if let Some(labels) = req.labels {
            article.labels = labels;
        }
        if let Some(is_favorite) = req.is_favorite {
            article.is_favorite = is_favorite;
        }

        article.updated_at = Utc::now();
        let updated = article.clone();
        self.save_all()?;
        Ok(updated)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), ReadingError> {
        self.articles
            .remove(id)
            .ok_or_else(|| ReadingError::NotFound(id.to_string()))?;
        self.save_all()?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Article> {
        self.articles.get(id)
    }

    pub fn query(&self, q: ArticleQuery) -> Vec<&Article> {
        let mut results: Vec<&Article> = self
            .articles
            .values()
            .filter(|a| {
                if let Some(ref state) = q.state {
                    if &a.state != state {
                        return false;
                    }
                }
                if let Some(ref article_type) = q.article_type {
                    if &a.article_type != article_type {
                        return false;
                    }
                }
                if let Some(is_favorite) = q.is_favorite {
                    if a.is_favorite != is_favorite {
                        return false;
                    }
                }
                if let Some(ref labels) = q.labels {
                    if !labels.iter().any(|l| a.labels.contains(l)) {
                        return false;
                    }
                }
                if let Some(ref search) = q.search {
                    let search_lower = search.to_lowercase();
                    let in_title = a.title.to_lowercase().contains(&search_lower);
                    let in_content = a.content.to_lowercase().contains(&search_lower);
                    let in_author = a
                        .author
                        .as_ref()
                        .map(|au| au.to_lowercase().contains(&search_lower))
                        .unwrap_or(false);
                    if !in_title && !in_content && !in_author {
                        return false;
                    }
                }
                true
            })
            .collect();

        let sort_order = q.sort_order.unwrap_or(SortOrder::Desc);
        let sort_by = q.sort_by.unwrap_or(SortBy::SavedAt);

        results.sort_by(|a, b| {
            let cmp = match sort_by {
                SortBy::SavedAt => a.saved_at.cmp(&b.saved_at),
                SortBy::UpdatedAt => a.updated_at.cmp(&b.updated_at),
                SortBy::Title => a.title.cmp(&b.title),
                SortBy::ReadingTime => a.reading_time_minutes.cmp(&b.reading_time_minutes),
                SortBy::Progress => a
                    .progress
                    .partial_cmp(&b.progress)
                    .unwrap_or(std::cmp::Ordering::Equal),
            };
            match sort_order {
                SortOrder::Asc => cmp,
                SortOrder::Desc => cmp.reverse(),
            }
        });

        let offset = q.offset.unwrap_or(0);
        let limit = q.limit.unwrap_or(50);

        results.into_iter().skip(offset).take(limit).collect()
    }

    pub fn archive(&mut self, id: &str) -> Result<Article, ReadingError> {
        self.update(UpdateArticleRequest {
            id: id.to_string(),
            title: None,
            state: Some(ReadingState::Archived),
            progress: None,
            labels: None,
            is_favorite: None,
        })
    }

    pub fn move_to_inbox(&mut self, id: &str) -> Result<Article, ReadingError> {
        self.update(UpdateArticleRequest {
            id: id.to_string(),
            title: None,
            state: Some(ReadingState::Inbox),
            progress: None,
            labels: None,
            is_favorite: None,
        })
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<Article, ReadingError> {
        let article = self
            .articles
            .get(id)
            .ok_or_else(|| ReadingError::NotFound(id.to_string()))?;
        let new_favorite = !article.is_favorite;

        self.update(UpdateArticleRequest {
            id: id.to_string(),
            title: None,
            state: None,
            progress: None,
            labels: None,
            is_favorite: Some(new_favorite),
        })
    }

    pub fn add_label(&mut self, id: &str, label: &str) -> Result<Article, ReadingError> {
        let article = self
            .articles
            .get(id)
            .ok_or_else(|| ReadingError::NotFound(id.to_string()))?;

        let mut labels = article.labels.clone();
        if !labels.contains(&label.to_string()) {
            labels.push(label.to_string());
        }

        self.update(UpdateArticleRequest {
            id: id.to_string(),
            title: None,
            state: None,
            progress: None,
            labels: Some(labels),
            is_favorite: None,
        })
    }

    pub fn remove_label(&mut self, id: &str, label: &str) -> Result<Article, ReadingError> {
        let article = self
            .articles
            .get(id)
            .ok_or_else(|| ReadingError::NotFound(id.to_string()))?;

        let labels: Vec<String> = article
            .labels
            .iter()
            .filter(|l| l.as_str() != label)
            .cloned()
            .collect();

        self.update(UpdateArticleRequest {
            id: id.to_string(),
            title: None,
            state: None,
            progress: None,
            labels: Some(labels),
            is_favorite: None,
        })
    }

    pub fn get_stats(&self) -> ReadingStats {
        let mut stats = ReadingStats {
            total_articles: self.articles.len(),
            inbox_count: 0,
            reading_count: 0,
            finished_count: 0,
            archived_count: 0,
            favorite_count: 0,
            total_words_read: 0,
            total_reading_time_minutes: 0,
            articles_by_type: HashMap::new(),
            articles_by_label: HashMap::new(),
        };

        for article in self.articles.values() {
            match article.state {
                ReadingState::Inbox => stats.inbox_count += 1,
                ReadingState::Later => stats.inbox_count += 1,
                ReadingState::Reading => stats.reading_count += 1,
                ReadingState::Finished => {
                    stats.finished_count += 1;
                    stats.total_words_read += article.word_count;
                    stats.total_reading_time_minutes += article.reading_time_minutes;
                }
                ReadingState::Archived => stats.archived_count += 1,
            }

            if article.is_favorite {
                stats.favorite_count += 1;
            }

            let type_key = format!("{:?}", article.article_type).to_lowercase();
            *stats.articles_by_type.entry(type_key).or_insert(0) += 1;

            for label in &article.labels {
                *stats.articles_by_label.entry(label.clone()).or_insert(0) += 1;
            }
        }

        stats
    }

    pub fn get_all_labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self
            .articles
            .values()
            .flat_map(|a| a.labels.iter().cloned())
            .collect();
        labels.sort();
        labels.dedup();
        labels
    }
}

pub fn save_article(data_dir: PathBuf, req: SaveArticleRequest) -> Result<Article, ReadingError> {
    let mut store = ReadingStore::new(data_dir)?;
    store.save(req)
}

pub fn update_article(
    data_dir: PathBuf,
    req: UpdateArticleRequest,
) -> Result<Article, ReadingError> {
    let mut store = ReadingStore::new(data_dir)?;
    store.update(req)
}

pub fn delete_article(data_dir: PathBuf, id: &str) -> Result<(), ReadingError> {
    let mut store = ReadingStore::new(data_dir)?;
    store.delete(id)
}

pub fn query_articles(
    data_dir: PathBuf,
    query: ArticleQuery,
) -> Result<Vec<Article>, ReadingError> {
    let store = ReadingStore::new(data_dir)?;
    Ok(store.query(query).into_iter().cloned().collect())
}

pub fn get_article(data_dir: PathBuf, id: &str) -> Result<Option<Article>, ReadingError> {
    let store = ReadingStore::new(data_dir)?;
    Ok(store.get(id).cloned())
}

pub fn archive_article(data_dir: PathBuf, id: &str) -> Result<Article, ReadingError> {
    let mut store = ReadingStore::new(data_dir)?;
    store.archive(id)
}

pub fn toggle_favorite(data_dir: PathBuf, id: &str) -> Result<Article, ReadingError> {
    let mut store = ReadingStore::new(data_dir)?;
    store.toggle_favorite(id)
}

pub fn get_reading_stats(data_dir: PathBuf) -> Result<ReadingStats, ReadingError> {
    let store = ReadingStore::new(data_dir)?;
    Ok(store.get_stats())
}

pub fn get_all_labels(data_dir: PathBuf) -> Result<Vec<String>, ReadingError> {
    let store = ReadingStore::new(data_dir)?;
    Ok(store.get_all_labels())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_article() -> SaveArticleRequest {
        SaveArticleRequest {
            url: Some("https://example.com/article".to_string()),
            title: "Test Article".to_string(),
            author: Some("John Doe".to_string()),
            content: "This is the content of the test article with enough words to test reading time calculation.".to_string(),
            excerpt: Some("Test excerpt".to_string()),
            site_name: Some("Example".to_string()),
            article_type: Some(ArticleType::Article),
            labels: Some(vec!["tech".to_string(), "rust".to_string()]),
            thumbnail_url: None,
        }
    }

    #[test]
    fn test_reading_state_default() {
        assert!(matches!(ReadingState::default(), ReadingState::Inbox));
    }

    #[test]
    fn test_article_type_default() {
        assert!(matches!(ArticleType::default(), ArticleType::Article));
    }

    #[test]
    fn test_calculate_reading_time() {
        assert_eq!(ReadingStore::calculate_reading_time(200), 1);
        assert_eq!(ReadingStore::calculate_reading_time(400), 2);
        assert_eq!(ReadingStore::calculate_reading_time(199), 1);
        assert_eq!(ReadingStore::calculate_reading_time(0), 0);
    }

    #[test]
    fn test_count_words() {
        assert_eq!(ReadingStore::count_words("hello world"), 2);
        assert_eq!(ReadingStore::count_words("one   two   three"), 3);
        assert_eq!(ReadingStore::count_words(""), 0);
    }

    #[test]
    fn test_save_article() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();

        assert_eq!(article.title, "Test Article");
        assert_eq!(article.author, Some("John Doe".to_string()));
        assert!(matches!(article.state, ReadingState::Inbox));
        assert_eq!(article.progress, 0.0);
        assert!(!article.is_favorite);
        assert!(article.word_count > 0);
        assert!(article.reading_time_minutes > 0);
    }

    #[test]
    fn test_update_article_title() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();

        let updated = store
            .update(UpdateArticleRequest {
                id: article.id,
                title: Some("Updated Title".to_string()),
                state: None,
                progress: None,
                labels: None,
                is_favorite: None,
            })
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
    }

    #[test]
    fn test_update_article_state_to_finished() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();

        let updated = store
            .update(UpdateArticleRequest {
                id: article.id,
                title: None,
                state: Some(ReadingState::Finished),
                progress: None,
                labels: None,
                is_favorite: None,
            })
            .unwrap();

        assert!(matches!(updated.state, ReadingState::Finished));
        assert_eq!(updated.progress, 100.0);
        assert!(updated.read_at.is_some());
    }

    #[test]
    fn test_update_progress_auto_finish() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();

        let reading = store
            .update(UpdateArticleRequest {
                id: article.id.clone(),
                title: None,
                state: Some(ReadingState::Reading),
                progress: None,
                labels: None,
                is_favorite: None,
            })
            .unwrap();

        assert!(matches!(reading.state, ReadingState::Reading));

        let finished = store
            .update(UpdateArticleRequest {
                id: article.id,
                title: None,
                state: None,
                progress: Some(100.0),
                labels: None,
                is_favorite: None,
            })
            .unwrap();

        assert!(matches!(finished.state, ReadingState::Finished));
    }

    #[test]
    fn test_progress_clamping() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();

        let over = store
            .update(UpdateArticleRequest {
                id: article.id.clone(),
                title: None,
                state: None,
                progress: Some(150.0),
                labels: None,
                is_favorite: None,
            })
            .unwrap();
        assert_eq!(over.progress, 100.0);

        let under = store
            .update(UpdateArticleRequest {
                id: article.id,
                title: None,
                state: None,
                progress: Some(-50.0),
                labels: None,
                is_favorite: None,
            })
            .unwrap();
        assert_eq!(under.progress, 0.0);
    }

    #[test]
    fn test_delete_article() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();
        assert!(store.get(&article.id).is_some());

        store.delete(&article.id).unwrap();
        assert!(store.get(&article.id).is_none());
    }

    #[test]
    fn test_archive_article() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();

        let archived = store.archive(&article.id).unwrap();

        assert!(matches!(archived.state, ReadingState::Archived));
        assert!(archived.archived_at.is_some());
    }

    #[test]
    fn test_toggle_favorite() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();
        assert!(!article.is_favorite);

        let favorited = store.toggle_favorite(&article.id).unwrap();
        assert!(favorited.is_favorite);

        let unfavorited = store.toggle_favorite(&article.id).unwrap();
        assert!(!unfavorited.is_favorite);
    }

    #[test]
    fn test_add_label() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();

        let updated = store.add_label(&article.id, "new-label").unwrap();
        assert!(updated.labels.contains(&"new-label".to_string()));

        let same = store.add_label(&article.id, "new-label").unwrap();
        assert_eq!(same.labels.iter().filter(|l| *l == "new-label").count(), 1);
    }

    #[test]
    fn test_remove_label() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();
        assert!(article.labels.contains(&"tech".to_string()));

        let updated = store.remove_label(&article.id, "tech").unwrap();
        assert!(!updated.labels.contains(&"tech".to_string()));
    }

    #[test]
    fn test_query_by_state() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        store.save(create_test_article()).unwrap();

        let inbox = store.query(ArticleQuery {
            state: Some(ReadingState::Inbox),
            article_type: None,
            labels: None,
            is_favorite: None,
            search: None,
            sort_by: None,
            sort_order: None,
            limit: None,
            offset: None,
        });
        assert_eq!(inbox.len(), 1);

        let archived = store.query(ArticleQuery {
            state: Some(ReadingState::Archived),
            article_type: None,
            labels: None,
            is_favorite: None,
            search: None,
            sort_by: None,
            sort_order: None,
            limit: None,
            offset: None,
        });
        assert_eq!(archived.len(), 0);
    }

    #[test]
    fn test_query_by_labels() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        store.save(create_test_article()).unwrap();

        let results = store.query(ArticleQuery {
            state: None,
            article_type: None,
            labels: Some(vec!["tech".to_string()]),
            is_favorite: None,
            search: None,
            sort_by: None,
            sort_order: None,
            limit: None,
            offset: None,
        });
        assert_eq!(results.len(), 1);

        let no_results = store.query(ArticleQuery {
            state: None,
            article_type: None,
            labels: Some(vec!["nonexistent".to_string()]),
            is_favorite: None,
            search: None,
            sort_by: None,
            sort_order: None,
            limit: None,
            offset: None,
        });
        assert_eq!(no_results.len(), 0);
    }

    #[test]
    fn test_query_search() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        store.save(create_test_article()).unwrap();

        let by_title = store.query(ArticleQuery {
            state: None,
            article_type: None,
            labels: None,
            is_favorite: None,
            search: Some("Test Article".to_string()),
            sort_by: None,
            sort_order: None,
            limit: None,
            offset: None,
        });
        assert_eq!(by_title.len(), 1);

        let by_author = store.query(ArticleQuery {
            state: None,
            article_type: None,
            labels: None,
            is_favorite: None,
            search: Some("John".to_string()),
            sort_by: None,
            sort_order: None,
            limit: None,
            offset: None,
        });
        assert_eq!(by_author.len(), 1);
    }

    #[test]
    fn test_get_stats() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        let article = store.save(create_test_article()).unwrap();
        store
            .update(UpdateArticleRequest {
                id: article.id,
                title: None,
                state: Some(ReadingState::Finished),
                progress: None,
                labels: None,
                is_favorite: Some(true),
            })
            .unwrap();

        let stats = store.get_stats();

        assert_eq!(stats.total_articles, 1);
        assert_eq!(stats.finished_count, 1);
        assert_eq!(stats.favorite_count, 1);
        assert!(stats.total_words_read > 0);
        assert!(stats.articles_by_type.contains_key("article"));
        assert!(stats.articles_by_label.contains_key("tech"));
    }

    #[test]
    fn test_get_all_labels() {
        let dir = tempdir().unwrap();
        let mut store = ReadingStore::new(dir.path().to_path_buf()).unwrap();

        store.save(create_test_article()).unwrap();

        let labels = store.get_all_labels();
        assert!(labels.contains(&"tech".to_string()));
        assert!(labels.contains(&"rust".to_string()));
    }

    #[test]
    fn test_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        {
            let mut store = ReadingStore::new(path.clone()).unwrap();
            store.save(create_test_article()).unwrap();
        }

        {
            let store = ReadingStore::new(path).unwrap();
            let articles = store.query(ArticleQuery {
                state: None,
                article_type: None,
                labels: None,
                is_favorite: None,
                search: None,
                sort_by: None,
                sort_order: None,
                limit: None,
                offset: None,
            });
            assert_eq!(articles.len(), 1);
        }
    }
}
