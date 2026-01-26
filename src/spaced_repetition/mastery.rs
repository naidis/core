use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::algorithm::{AlgorithmType, HalfLifeData, SM2Data};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MasteryCardType {
    QA,
    Cloze,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasteryCard {
    pub id: String,
    pub highlight_id: String,
    pub card_type: MasteryCardType,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub cloze_text: Option<String>,
    pub cloze_deletions: Vec<ClozeDeletion>,
    pub sm2: Option<SM2Data>,
    pub half_life: Option<HalfLifeData>,
    pub review_count: u32,
    pub last_reviewed_at: Option<DateTime<Utc>>,
    pub next_review_at: Option<DateTime<Utc>>,
    pub is_suspended: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClozeDeletion {
    pub start: usize,
    pub end: usize,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMasteryCardRequest {
    pub highlight_id: String,
    pub card_type: MasteryCardType,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub cloze_text: Option<String>,
    pub cloze_deletions: Option<Vec<ClozeDeletion>>,
}

impl MasteryCard {
    pub fn new(req: CreateMasteryCardRequest, algorithm_type: AlgorithmType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            highlight_id: req.highlight_id,
            card_type: req.card_type,
            question: req.question,
            answer: req.answer,
            cloze_text: req.cloze_text,
            cloze_deletions: req.cloze_deletions.unwrap_or_default(),
            sm2: if algorithm_type == AlgorithmType::SM2 {
                Some(SM2Data::default())
            } else {
                None
            },
            half_life: if algorithm_type == AlgorithmType::HalfLife {
                Some(HalfLifeData::default())
            } else {
                None
            },
            review_count: 0,
            last_reviewed_at: None,
            next_review_at: Some(now),
            is_suspended: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn get_display_text(&self) -> String {
        match self.card_type {
            MasteryCardType::QA => self.question.clone().unwrap_or_default(),
            MasteryCardType::Cloze => {
                let mut text = self.cloze_text.clone().unwrap_or_default();
                for (i, deletion) in self.cloze_deletions.iter().enumerate().rev() {
                    let replacement = match &deletion.hint {
                        Some(hint) => format!("[...{}...]", hint),
                        None => format!("[...{}...]", i + 1),
                    };
                    if deletion.end <= text.len() {
                        text.replace_range(deletion.start..deletion.end, &replacement);
                    }
                }
                text
            }
        }
    }

    pub fn get_answer_text(&self) -> String {
        match self.card_type {
            MasteryCardType::QA => self.answer.clone().unwrap_or_default(),
            MasteryCardType::Cloze => self.cloze_text.clone().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMasteryCardRequest {
    pub id: String,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub cloze_text: Option<String>,
    pub cloze_deletions: Option<Vec<ClozeDeletion>>,
    pub is_suspended: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasteryCardQuery {
    pub highlight_id: Option<String>,
    pub card_type: Option<MasteryCardType>,
    pub is_suspended: Option<bool>,
    pub due_before: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_qa_request() -> CreateMasteryCardRequest {
        CreateMasteryCardRequest {
            highlight_id: "h1".to_string(),
            card_type: MasteryCardType::QA,
            question: Some("What is the capital of France?".to_string()),
            answer: Some("Paris".to_string()),
            cloze_text: None,
            cloze_deletions: None,
        }
    }

    fn create_cloze_request() -> CreateMasteryCardRequest {
        CreateMasteryCardRequest {
            highlight_id: "h2".to_string(),
            card_type: MasteryCardType::Cloze,
            question: None,
            answer: None,
            cloze_text: Some("The capital of France is Paris.".to_string()),
            cloze_deletions: Some(vec![ClozeDeletion {
                start: 25,
                end: 30,
                hint: None,
            }]),
        }
    }

    #[test]
    fn test_create_qa_card() {
        let card = MasteryCard::new(create_qa_request(), AlgorithmType::SM2);
        assert_eq!(card.card_type, MasteryCardType::QA);
        assert_eq!(
            card.question,
            Some("What is the capital of France?".to_string())
        );
        assert_eq!(card.answer, Some("Paris".to_string()));
        assert!(card.sm2.is_some());
        assert!(card.half_life.is_none());
    }

    #[test]
    fn test_create_cloze_card() {
        let card = MasteryCard::new(create_cloze_request(), AlgorithmType::HalfLife);
        assert_eq!(card.card_type, MasteryCardType::Cloze);
        assert!(card.cloze_text.is_some());
        assert_eq!(card.cloze_deletions.len(), 1);
        assert!(card.half_life.is_some());
        assert!(card.sm2.is_none());
    }

    #[test]
    fn test_qa_display_text() {
        let card = MasteryCard::new(create_qa_request(), AlgorithmType::SM2);
        assert_eq!(card.get_display_text(), "What is the capital of France?");
    }

    #[test]
    fn test_qa_answer_text() {
        let card = MasteryCard::new(create_qa_request(), AlgorithmType::SM2);
        assert_eq!(card.get_answer_text(), "Paris");
    }

    #[test]
    fn test_cloze_display_text() {
        let card = MasteryCard::new(create_cloze_request(), AlgorithmType::HalfLife);
        let display = card.get_display_text();
        assert!(display.contains("[...1...]"));
        assert!(!display.contains("Paris"));
    }

    #[test]
    fn test_cloze_answer_text() {
        let card = MasteryCard::new(create_cloze_request(), AlgorithmType::HalfLife);
        assert_eq!(card.get_answer_text(), "The capital of France is Paris.");
    }

    #[test]
    fn test_cloze_with_hint() {
        let req = CreateMasteryCardRequest {
            highlight_id: "h3".to_string(),
            card_type: MasteryCardType::Cloze,
            question: None,
            answer: None,
            cloze_text: Some("The capital of France is Paris.".to_string()),
            cloze_deletions: Some(vec![ClozeDeletion {
                start: 25,
                end: 30,
                hint: Some("city".to_string()),
            }]),
        };
        let card = MasteryCard::new(req, AlgorithmType::HalfLife);
        let display = card.get_display_text();
        assert!(display.contains("[...city...]"));
    }
}
