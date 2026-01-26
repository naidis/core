//! Spaced Repetition System for Naidis
//!
//! Supports two algorithms:
//! - SM-2 (SuperMemo 2) - Traditional ease factor based algorithm
//! - Half-life decay (Readwise style) - Probability based algorithm
//!
//! Features:
//! - Mastery Cards (Q&A, Cloze deletion)
//! - Review Sessions
//! - Frequency Tuning
//! - Streak & Stats tracking

pub mod algorithm;
pub mod frequency;
pub mod mastery;
pub mod session;
pub mod stats;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

pub use algorithm::{AlgorithmType, HalfLifeData, ReviewFeedback, SM2Data};
pub use frequency::FrequencyTuning;
pub use mastery::{CreateMasteryCardRequest, MasteryCard};
#[allow(unused_imports)]
pub use session::{ReviewItem, ReviewItemType, ReviewSession};
pub use stats::{ReviewStats, StreakData};

#[derive(Error, Debug)]
pub enum SpacedRepetitionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Card not found: {0}")]
    CardNotFound(String),
    #[error("Highlight not found: {0}")]
    HighlightNotFound(String),
    #[error("Invalid algorithm: {0}")]
    InvalidAlgorithm(String),
    #[error("No items to review")]
    NoItemsToReview,
}

/// Main configuration for the spaced repetition system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacedRepetitionConfig {
    /// Which algorithm to use
    pub algorithm_type: AlgorithmType,
    /// Number of highlights to review per day
    pub highlights_per_day: usize,
    /// Number of mastery cards to review per day
    pub mastery_cards_per_day: usize,
    /// Enable themed reviews
    pub themed_reviews_enabled: bool,
    /// Enable streak tracking
    pub streak_enabled: bool,
}

impl Default for SpacedRepetitionConfig {
    fn default() -> Self {
        Self {
            algorithm_type: AlgorithmType::HalfLife,
            highlights_per_day: 10,
            mastery_cards_per_day: 10,
            themed_reviews_enabled: true,
            streak_enabled: true,
        }
    }
}

/// Spaced repetition data for a highlight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightSRData {
    pub highlight_id: String,
    /// SM-2 specific data (if using SM-2)
    pub sm2: Option<SM2Data>,
    /// Half-life specific data (if using half-life)
    pub half_life: Option<HalfLifeData>,
    /// Review status
    pub status: HighlightReviewStatus,
    /// Number of times reviewed
    pub review_count: u32,
    /// Last review date
    pub last_reviewed_at: Option<DateTime<Utc>>,
    /// Next scheduled review date
    pub next_review_at: Option<DateTime<Utc>>,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

impl HighlightSRData {
    pub fn new(highlight_id: String, algorithm_type: AlgorithmType) -> Self {
        let now = Utc::now();
        Self {
            highlight_id,
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
            status: HighlightReviewStatus::Active,
            review_count: 0,
            last_reviewed_at: None,
            next_review_at: Some(now), // Available for immediate review
            created_at: now,
            updated_at: now,
        }
    }
}

/// Review status for a highlight
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HighlightReviewStatus {
    /// Active in review rotation
    Active,
    /// Discarded (soft delete from rotation)
    Discarded,
    /// Converted to mastery card
    Mastered,
}

/// Main store for spaced repetition data
pub struct SpacedRepetitionStore {
    data_dir: PathBuf,
    config: SpacedRepetitionConfig,
    highlight_data: HashMap<String, HighlightSRData>,
    mastery_cards: HashMap<String, MasteryCard>,
    frequency_tuning: FrequencyTuning,
    stats: ReviewStats,
}

impl SpacedRepetitionStore {
    pub fn new(data_dir: PathBuf) -> Result<Self, SpacedRepetitionError> {
        let sr_dir = data_dir.join("spaced_repetition");
        fs::create_dir_all(&sr_dir)?;

        let mut store = Self {
            data_dir: sr_dir,
            config: SpacedRepetitionConfig::default(),
            highlight_data: HashMap::new(),
            mastery_cards: HashMap::new(),
            frequency_tuning: FrequencyTuning::default(),
            stats: ReviewStats::default(),
        };
        store.load_all()?;
        Ok(store)
    }

    fn load_all(&mut self) -> Result<(), SpacedRepetitionError> {
        // Load config
        let config_path = self.data_dir.join("config.json");
        if config_path.exists() {
            let data = fs::read_to_string(&config_path)?;
            self.config = serde_json::from_str(&data)?;
        }

        // Load highlight SR data
        let highlights_path = self.data_dir.join("highlights_sr.json");
        if highlights_path.exists() {
            let data = fs::read_to_string(&highlights_path)?;
            self.highlight_data = serde_json::from_str(&data)?;
        }

        // Load mastery cards
        let mastery_path = self.data_dir.join("mastery_cards.json");
        if mastery_path.exists() {
            let data = fs::read_to_string(&mastery_path)?;
            self.mastery_cards = serde_json::from_str(&data)?;
        }

        // Load frequency tuning
        let frequency_path = self.data_dir.join("frequency.json");
        if frequency_path.exists() {
            let data = fs::read_to_string(&frequency_path)?;
            self.frequency_tuning = serde_json::from_str(&data)?;
        }

        // Load stats
        let stats_path = self.data_dir.join("stats.json");
        if stats_path.exists() {
            let data = fs::read_to_string(&stats_path)?;
            self.stats = serde_json::from_str(&data)?;
        }

        Ok(())
    }

    fn save_config(&self) -> Result<(), SpacedRepetitionError> {
        let path = self.data_dir.join("config.json");
        let data = serde_json::to_string_pretty(&self.config)?;
        fs::write(&path, data)?;
        Ok(())
    }

    fn save_highlight_data(&self) -> Result<(), SpacedRepetitionError> {
        let path = self.data_dir.join("highlights_sr.json");
        let data = serde_json::to_string_pretty(&self.highlight_data)?;
        fs::write(&path, data)?;
        Ok(())
    }

    fn save_mastery_cards(&self) -> Result<(), SpacedRepetitionError> {
        let path = self.data_dir.join("mastery_cards.json");
        let data = serde_json::to_string_pretty(&self.mastery_cards)?;
        fs::write(&path, data)?;
        Ok(())
    }

    fn save_frequency(&self) -> Result<(), SpacedRepetitionError> {
        let path = self.data_dir.join("frequency.json");
        let data = serde_json::to_string_pretty(&self.frequency_tuning)?;
        fs::write(&path, data)?;
        Ok(())
    }

    fn save_stats(&self) -> Result<(), SpacedRepetitionError> {
        let path = self.data_dir.join("stats.json");
        let data = serde_json::to_string_pretty(&self.stats)?;
        fs::write(&path, data)?;
        Ok(())
    }

    // ========== Config Methods ==========

    pub fn get_config(&self) -> &SpacedRepetitionConfig {
        &self.config
    }

    pub fn update_config(
        &mut self,
        config: SpacedRepetitionConfig,
    ) -> Result<(), SpacedRepetitionError> {
        self.config = config;
        self.save_config()
    }

    // ========== Highlight SR Methods ==========

    pub fn register_highlight(
        &mut self,
        highlight_id: String,
    ) -> Result<HighlightSRData, SpacedRepetitionError> {
        if self.highlight_data.contains_key(&highlight_id) {
            return Ok(self.highlight_data.get(&highlight_id).unwrap().clone());
        }

        let sr_data =
            HighlightSRData::new(highlight_id.clone(), self.config.algorithm_type.clone());
        self.highlight_data.insert(highlight_id, sr_data.clone());
        self.save_highlight_data()?;
        Ok(sr_data)
    }

    pub fn get_highlight_sr(&self, highlight_id: &str) -> Option<&HighlightSRData> {
        self.highlight_data.get(highlight_id)
    }

    pub fn review_highlight(
        &mut self,
        highlight_id: &str,
        action: HighlightReviewAction,
    ) -> Result<HighlightSRData, SpacedRepetitionError> {
        let sr_data = self
            .highlight_data
            .get_mut(highlight_id)
            .ok_or_else(|| SpacedRepetitionError::HighlightNotFound(highlight_id.to_string()))?;

        let now = Utc::now();
        sr_data.last_reviewed_at = Some(now);
        sr_data.review_count += 1;
        sr_data.updated_at = now;

        match action {
            HighlightReviewAction::Keep => {
                // Update next review based on algorithm
                match self.config.algorithm_type {
                    AlgorithmType::SM2 => {
                        if let Some(ref mut sm2) = sr_data.sm2 {
                            let feedback = ReviewFeedback::Good;
                            algorithm::sm2::update_sm2(sm2, feedback);
                            sr_data.next_review_at =
                                Some(now + chrono::Duration::days(sm2.interval as i64));
                        }
                    }
                    AlgorithmType::HalfLife => {
                        if let Some(ref mut hl) = sr_data.half_life {
                            let feedback = ReviewFeedback::Later;
                            algorithm::halflife::update_halflife(hl, feedback);
                            sr_data.next_review_at = algorithm::halflife::next_review_date(hl);
                        }
                    }
                }
            }
            HighlightReviewAction::Discard => {
                sr_data.status = HighlightReviewStatus::Discarded;
                sr_data.next_review_at = None;
            }
            HighlightReviewAction::Master => {
                sr_data.status = HighlightReviewStatus::Mastered;
                sr_data.next_review_at = None;
            }
        }

        let updated = sr_data.clone();
        self.save_highlight_data()?;

        // Update stats
        self.stats.record_review(now);
        self.save_stats()?;

        Ok(updated)
    }

    // ========== Mastery Card Methods ==========

    pub fn create_mastery_card(
        &mut self,
        req: CreateMasteryCardRequest,
    ) -> Result<MasteryCard, SpacedRepetitionError> {
        let card = MasteryCard::new(req, self.config.algorithm_type.clone());
        self.mastery_cards.insert(card.id.clone(), card.clone());
        self.save_mastery_cards()?;
        Ok(card)
    }

    pub fn get_mastery_card(&self, card_id: &str) -> Option<&MasteryCard> {
        self.mastery_cards.get(card_id)
    }

    pub fn review_mastery_card(
        &mut self,
        card_id: &str,
        feedback: ReviewFeedback,
    ) -> Result<MasteryCard, SpacedRepetitionError> {
        let card = self
            .mastery_cards
            .get_mut(card_id)
            .ok_or_else(|| SpacedRepetitionError::CardNotFound(card_id.to_string()))?;

        let now = Utc::now();
        card.last_reviewed_at = Some(now);
        card.review_count += 1;
        card.updated_at = now;

        match self.config.algorithm_type {
            AlgorithmType::SM2 => {
                if let Some(ref mut sm2) = card.sm2 {
                    algorithm::sm2::update_sm2(sm2, feedback);
                    card.next_review_at = Some(now + chrono::Duration::days(sm2.interval as i64));
                }
            }
            AlgorithmType::HalfLife => {
                if let Some(ref mut hl) = card.half_life {
                    algorithm::halflife::update_halflife(hl, feedback);
                    card.next_review_at = algorithm::halflife::next_review_date(hl);
                }
            }
        }

        let updated = card.clone();
        self.save_mastery_cards()?;

        // Update stats
        self.stats.record_review(now);
        self.save_stats()?;

        Ok(updated)
    }

    pub fn delete_mastery_card(&mut self, card_id: &str) -> Result<(), SpacedRepetitionError> {
        self.mastery_cards
            .remove(card_id)
            .ok_or_else(|| SpacedRepetitionError::CardNotFound(card_id.to_string()))?;
        self.save_mastery_cards()
    }

    // ========== Frequency Tuning Methods ==========

    pub fn get_frequency_tuning(&self) -> &FrequencyTuning {
        &self.frequency_tuning
    }

    pub fn set_document_frequency(
        &mut self,
        document_id: String,
        multiplier: f32,
    ) -> Result<(), SpacedRepetitionError> {
        self.frequency_tuning.set_document(document_id, multiplier);
        self.save_frequency()
    }

    pub fn set_source_type_frequency(
        &mut self,
        source_type: String,
        multiplier: f32,
    ) -> Result<(), SpacedRepetitionError> {
        self.frequency_tuning
            .set_source_type(source_type, multiplier);
        self.save_frequency()
    }

    // ========== Stats Methods ==========

    pub fn get_stats(&self) -> &ReviewStats {
        &self.stats
    }

    pub fn get_streak(&self) -> &StreakData {
        &self.stats.streak
    }
}

/// Action to take on a highlight during review
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightReviewAction {
    Keep,
    Discard,
    Master,
}

// ========== Public API Functions ==========

pub fn get_config(data_dir: PathBuf) -> Result<SpacedRepetitionConfig, SpacedRepetitionError> {
    let store = SpacedRepetitionStore::new(data_dir)?;
    Ok(store.get_config().clone())
}

pub fn update_config(
    data_dir: PathBuf,
    config: SpacedRepetitionConfig,
) -> Result<(), SpacedRepetitionError> {
    let mut store = SpacedRepetitionStore::new(data_dir)?;
    store.update_config(config)
}

pub fn register_highlight(
    data_dir: PathBuf,
    highlight_id: String,
) -> Result<HighlightSRData, SpacedRepetitionError> {
    let mut store = SpacedRepetitionStore::new(data_dir)?;
    store.register_highlight(highlight_id)
}

pub fn review_highlight(
    data_dir: PathBuf,
    highlight_id: String,
    action: HighlightReviewAction,
) -> Result<HighlightSRData, SpacedRepetitionError> {
    let mut store = SpacedRepetitionStore::new(data_dir)?;
    store.review_highlight(&highlight_id, action)
}

pub fn create_mastery_card(
    data_dir: PathBuf,
    req: CreateMasteryCardRequest,
) -> Result<MasteryCard, SpacedRepetitionError> {
    let mut store = SpacedRepetitionStore::new(data_dir)?;
    store.create_mastery_card(req)
}

pub fn review_mastery_card(
    data_dir: PathBuf,
    card_id: String,
    feedback: ReviewFeedback,
) -> Result<MasteryCard, SpacedRepetitionError> {
    let mut store = SpacedRepetitionStore::new(data_dir)?;
    store.review_mastery_card(&card_id, feedback)
}

pub fn delete_mastery_card(
    data_dir: PathBuf,
    card_id: String,
) -> Result<(), SpacedRepetitionError> {
    let mut store = SpacedRepetitionStore::new(data_dir)?;
    store.delete_mastery_card(&card_id)
}

pub fn get_stats(data_dir: PathBuf) -> Result<ReviewStats, SpacedRepetitionError> {
    let store = SpacedRepetitionStore::new(data_dir)?;
    Ok(store.get_stats().clone())
}

pub fn set_document_frequency(
    data_dir: PathBuf,
    document_id: String,
    multiplier: f32,
) -> Result<(), SpacedRepetitionError> {
    let mut store = SpacedRepetitionStore::new(data_dir)?;
    store.set_document_frequency(document_id, multiplier)
}

pub fn set_source_type_frequency(
    data_dir: PathBuf,
    source_type: String,
    multiplier: f32,
) -> Result<(), SpacedRepetitionError> {
    let mut store = SpacedRepetitionStore::new(data_dir)?;
    store.set_source_type_frequency(source_type, multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_store_creation() {
        let dir = tempdir().unwrap();
        let store = SpacedRepetitionStore::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(store.config.algorithm_type, AlgorithmType::HalfLife);
    }

    #[test]
    fn test_config_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        {
            let mut store = SpacedRepetitionStore::new(path.clone()).unwrap();
            let mut config = store.get_config().clone();
            config.algorithm_type = AlgorithmType::SM2;
            config.highlights_per_day = 20;
            store.update_config(config).unwrap();
        }

        {
            let store = SpacedRepetitionStore::new(path).unwrap();
            assert_eq!(store.config.algorithm_type, AlgorithmType::SM2);
            assert_eq!(store.config.highlights_per_day, 20);
        }
    }

    #[test]
    fn test_register_highlight() {
        let dir = tempdir().unwrap();
        let mut store = SpacedRepetitionStore::new(dir.path().to_path_buf()).unwrap();

        let sr_data = store.register_highlight("h1".to_string()).unwrap();
        assert_eq!(sr_data.highlight_id, "h1");
        assert_eq!(sr_data.status, HighlightReviewStatus::Active);
        assert!(sr_data.half_life.is_some()); // Default is half-life
    }

    #[test]
    fn test_review_highlight_keep() {
        let dir = tempdir().unwrap();
        let mut store = SpacedRepetitionStore::new(dir.path().to_path_buf()).unwrap();

        store.register_highlight("h1".to_string()).unwrap();
        let updated = store
            .review_highlight("h1", HighlightReviewAction::Keep)
            .unwrap();

        assert_eq!(updated.review_count, 1);
        assert!(updated.last_reviewed_at.is_some());
        assert!(updated.next_review_at.is_some());
    }

    #[test]
    fn test_review_highlight_discard() {
        let dir = tempdir().unwrap();
        let mut store = SpacedRepetitionStore::new(dir.path().to_path_buf()).unwrap();

        store.register_highlight("h1".to_string()).unwrap();
        let updated = store
            .review_highlight("h1", HighlightReviewAction::Discard)
            .unwrap();

        assert_eq!(updated.status, HighlightReviewStatus::Discarded);
        assert!(updated.next_review_at.is_none());
    }
}
