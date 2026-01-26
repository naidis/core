use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::algorithm::halflife;
use super::{HighlightReviewStatus, SpacedRepetitionError, SpacedRepetitionStore};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewItemType {
    Highlight,
    MasteryCard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: String,
    pub item_type: ReviewItemType,
    pub highlight_id: String,
    pub text: String,
    pub source_title: Option<String>,
    pub source_author: Option<String>,
    pub note: Option<String>,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub recall_probability: Option<f64>,
    pub last_reviewed_at: Option<DateTime<Utc>>,
    pub review_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSession {
    pub id: String,
    pub items: Vec<ReviewItem>,
    pub current_index: usize,
    pub completed_count: usize,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub session_type: ReviewSessionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSessionType {
    Daily,
    Themed,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub session_type: ReviewSessionType,
    pub highlight_limit: Option<usize>,
    pub mastery_limit: Option<usize>,
    pub tags: Option<Vec<String>>,
    pub document_ids: Option<Vec<String>>,
}

impl ReviewSession {
    pub fn new(items: Vec<ReviewItem>, session_type: ReviewSessionType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            items,
            current_index: 0,
            completed_count: 0,
            started_at: Utc::now(),
            completed_at: None,
            session_type,
        }
    }

    pub fn current_item(&self) -> Option<&ReviewItem> {
        self.items.get(self.current_index)
    }

    pub fn next(&mut self) -> Option<&ReviewItem> {
        if self.current_index < self.items.len() {
            self.completed_count += 1;
            self.current_index += 1;
        }
        self.current_item()
    }

    pub fn previous(&mut self) -> Option<&ReviewItem> {
        if self.current_index > 0 {
            self.current_index -= 1;
        }
        self.current_item()
    }

    pub fn is_complete(&self) -> bool {
        self.current_index >= self.items.len()
    }

    pub fn complete(&mut self) {
        self.completed_at = Some(Utc::now());
    }

    pub fn progress(&self) -> (usize, usize) {
        (self.current_index, self.items.len())
    }
}

impl SpacedRepetitionStore {
    pub fn create_review_session(
        &self,
        req: CreateSessionRequest,
    ) -> Result<ReviewSession, SpacedRepetitionError> {
        let mut items = Vec::new();

        let highlight_limit = req
            .highlight_limit
            .unwrap_or(self.config.highlights_per_day);
        let mastery_limit = req
            .mastery_limit
            .unwrap_or(self.config.mastery_cards_per_day);
        let now = Utc::now();

        let mut highlights_due: Vec<_> = self
            .highlight_data
            .values()
            .filter(|h| {
                h.status == HighlightReviewStatus::Active
                    && h.next_review_at.map(|d| d <= now).unwrap_or(true)
            })
            .collect();

        highlights_due.sort_by(|a, b| {
            let prob_a = a
                .half_life
                .as_ref()
                .map(halflife::recall_probability)
                .unwrap_or(0.0);
            let prob_b = b
                .half_life
                .as_ref()
                .map(halflife::recall_probability)
                .unwrap_or(0.0);
            prob_a
                .partial_cmp(&prob_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for h in highlights_due.into_iter().take(highlight_limit) {
            items.push(ReviewItem {
                id: h.highlight_id.clone(),
                item_type: ReviewItemType::Highlight,
                highlight_id: h.highlight_id.clone(),
                text: String::new(),
                source_title: None,
                source_author: None,
                note: None,
                question: None,
                answer: None,
                recall_probability: h.half_life.as_ref().map(halflife::recall_probability),
                last_reviewed_at: h.last_reviewed_at,
                review_count: h.review_count,
            });
        }

        let mut mastery_due: Vec<_> = self
            .mastery_cards
            .values()
            .filter(|c| !c.is_suspended && c.next_review_at.map(|d| d <= now).unwrap_or(true))
            .collect();

        mastery_due.sort_by(|a, b| {
            let prob_a = a
                .half_life
                .as_ref()
                .map(halflife::recall_probability)
                .unwrap_or(0.0);
            let prob_b = b
                .half_life
                .as_ref()
                .map(halflife::recall_probability)
                .unwrap_or(0.0);
            prob_a
                .partial_cmp(&prob_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for c in mastery_due.into_iter().take(mastery_limit) {
            items.push(ReviewItem {
                id: c.id.clone(),
                item_type: ReviewItemType::MasteryCard,
                highlight_id: c.highlight_id.clone(),
                text: c.get_display_text(),
                source_title: None,
                source_author: None,
                note: None,
                question: c.question.clone(),
                answer: Some(c.get_answer_text()),
                recall_probability: c.half_life.as_ref().map(halflife::recall_probability),
                last_reviewed_at: c.last_reviewed_at,
                review_count: c.review_count,
            });
        }

        if items.is_empty() {
            return Err(SpacedRepetitionError::NoItemsToReview);
        }

        Ok(ReviewSession::new(items, req.session_type))
    }

    pub fn get_due_counts(&self) -> (usize, usize) {
        let now = Utc::now();

        let highlights_due = self
            .highlight_data
            .values()
            .filter(|h| {
                h.status == HighlightReviewStatus::Active
                    && h.next_review_at.map(|d| d <= now).unwrap_or(true)
            })
            .count();

        let mastery_due = self
            .mastery_cards
            .values()
            .filter(|c| !c.is_suspended && c.next_review_at.map(|d| d <= now).unwrap_or(true))
            .count();

        (highlights_due, mastery_due)
    }
}

pub fn create_review_session(
    data_dir: std::path::PathBuf,
    req: CreateSessionRequest,
) -> Result<ReviewSession, SpacedRepetitionError> {
    let store = SpacedRepetitionStore::new(data_dir)?;
    store.create_review_session(req)
}

pub fn get_due_counts(
    data_dir: std::path::PathBuf,
) -> Result<(usize, usize), SpacedRepetitionError> {
    let store = SpacedRepetitionStore::new(data_dir)?;
    Ok(store.get_due_counts())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let items = vec![ReviewItem {
            id: "1".to_string(),
            item_type: ReviewItemType::Highlight,
            highlight_id: "h1".to_string(),
            text: "Test text".to_string(),
            source_title: Some("Book".to_string()),
            source_author: None,
            note: None,
            question: None,
            answer: None,
            recall_probability: Some(0.5),
            last_reviewed_at: None,
            review_count: 0,
        }];
        let session = ReviewSession::new(items, ReviewSessionType::Daily);

        assert_eq!(session.current_index, 0);
        assert_eq!(session.completed_count, 0);
        assert!(!session.is_complete());
    }

    #[test]
    fn test_session_navigation() {
        let items = vec![
            ReviewItem {
                id: "1".to_string(),
                item_type: ReviewItemType::Highlight,
                highlight_id: "h1".to_string(),
                text: "First".to_string(),
                source_title: None,
                source_author: None,
                note: None,
                question: None,
                answer: None,
                recall_probability: None,
                last_reviewed_at: None,
                review_count: 0,
            },
            ReviewItem {
                id: "2".to_string(),
                item_type: ReviewItemType::Highlight,
                highlight_id: "h2".to_string(),
                text: "Second".to_string(),
                source_title: None,
                source_author: None,
                note: None,
                question: None,
                answer: None,
                recall_probability: None,
                last_reviewed_at: None,
                review_count: 0,
            },
        ];
        let mut session = ReviewSession::new(items, ReviewSessionType::Daily);

        assert_eq!(session.current_item().unwrap().text, "First");
        session.next();
        assert_eq!(session.current_item().unwrap().text, "Second");
        session.previous();
        assert_eq!(session.current_item().unwrap().text, "First");
    }

    #[test]
    fn test_session_completion() {
        let items = vec![ReviewItem {
            id: "1".to_string(),
            item_type: ReviewItemType::Highlight,
            highlight_id: "h1".to_string(),
            text: "Only one".to_string(),
            source_title: None,
            source_author: None,
            note: None,
            question: None,
            answer: None,
            recall_probability: None,
            last_reviewed_at: None,
            review_count: 0,
        }];
        let mut session = ReviewSession::new(items, ReviewSessionType::Daily);

        assert!(!session.is_complete());
        session.next();
        assert!(session.is_complete());
    }

    #[test]
    fn test_session_progress() {
        let items = vec![
            ReviewItem {
                id: "1".to_string(),
                item_type: ReviewItemType::Highlight,
                highlight_id: "h1".to_string(),
                text: "1".to_string(),
                source_title: None,
                source_author: None,
                note: None,
                question: None,
                answer: None,
                recall_probability: None,
                last_reviewed_at: None,
                review_count: 0,
            },
            ReviewItem {
                id: "2".to_string(),
                item_type: ReviewItemType::Highlight,
                highlight_id: "h2".to_string(),
                text: "2".to_string(),
                source_title: None,
                source_author: None,
                note: None,
                question: None,
                answer: None,
                recall_probability: None,
                last_reviewed_at: None,
                review_count: 0,
            },
            ReviewItem {
                id: "3".to_string(),
                item_type: ReviewItemType::Highlight,
                highlight_id: "h3".to_string(),
                text: "3".to_string(),
                source_title: None,
                source_author: None,
                note: None,
                question: None,
                answer: None,
                recall_probability: None,
                last_reviewed_at: None,
                review_count: 0,
            },
        ];
        let mut session = ReviewSession::new(items, ReviewSessionType::Daily);

        assert_eq!(session.progress(), (0, 3));
        session.next();
        assert_eq!(session.progress(), (1, 3));
        session.next();
        assert_eq!(session.progress(), (2, 3));
    }
}
