use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AlgorithmType {
    SM2,
    HalfLife,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFeedback {
    Again,
    Hard,
    Good,
    Easy,
    Soon,
    Later,
    Someday,
    Never,
}

pub trait Algorithm {
    fn calculate_next_review(&self) -> Option<DateTime<Utc>>;
    fn get_recall_probability(&self) -> f64;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SM2Data {
    pub ease_factor: f32,
    pub interval: i32,
    pub repetitions: i32,
}

impl Default for SM2Data {
    fn default() -> Self {
        Self {
            ease_factor: 2.5,
            interval: 0,
            repetitions: 0,
        }
    }
}

impl Algorithm for SM2Data {
    fn calculate_next_review(&self) -> Option<DateTime<Utc>> {
        if self.interval == 0 {
            return Some(Utc::now());
        }
        Some(Utc::now() + Duration::days(self.interval as i64))
    }

    fn get_recall_probability(&self) -> f64 {
        1.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalfLifeData {
    pub half_life_days: f32,
    pub last_reviewed_at: Option<DateTime<Utc>>,
}

impl Default for HalfLifeData {
    fn default() -> Self {
        Self {
            half_life_days: 7.0,
            last_reviewed_at: None,
        }
    }
}

impl Algorithm for HalfLifeData {
    fn calculate_next_review(&self) -> Option<DateTime<Utc>> {
        halflife::next_review_date(self)
    }

    fn get_recall_probability(&self) -> f64 {
        halflife::recall_probability(self)
    }
}

pub mod sm2 {
    use super::*;

    const MIN_EASE_FACTOR: f32 = 1.3;

    pub fn update_sm2(data: &mut SM2Data, feedback: ReviewFeedback) {
        let quality = match feedback {
            ReviewFeedback::Again | ReviewFeedback::Soon => 0,
            ReviewFeedback::Hard => 3,
            ReviewFeedback::Good | ReviewFeedback::Later => 4,
            ReviewFeedback::Easy | ReviewFeedback::Someday => 5,
            ReviewFeedback::Never => return,
        };

        if quality < 3 {
            data.repetitions = 0;
            data.interval = 1;
        } else {
            data.repetitions += 1;
            data.interval = match data.repetitions {
                1 => 1,
                2 => 6,
                _ => (data.interval as f32 * data.ease_factor).round() as i32,
            };
        }

        let q = quality as f32;
        data.ease_factor =
            (data.ease_factor + 0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02)).max(MIN_EASE_FACTOR);
    }

    pub fn quality_from_feedback(feedback: &ReviewFeedback) -> i32 {
        match feedback {
            ReviewFeedback::Again | ReviewFeedback::Soon => 0,
            ReviewFeedback::Hard => 3,
            ReviewFeedback::Good | ReviewFeedback::Later => 4,
            ReviewFeedback::Easy | ReviewFeedback::Someday => 5,
            ReviewFeedback::Never => -1,
        }
    }
}

pub mod halflife {
    use super::*;

    const DEFAULT_HALF_LIFE: f32 = 7.0;
    const RECALL_THRESHOLD: f64 = 0.5;

    pub fn update_halflife(data: &mut HalfLifeData, feedback: ReviewFeedback) {
        let multiplier = match feedback {
            ReviewFeedback::Again | ReviewFeedback::Soon => 0.5,
            ReviewFeedback::Hard => 0.75,
            ReviewFeedback::Good | ReviewFeedback::Later => 1.0,
            ReviewFeedback::Easy | ReviewFeedback::Someday => 1.5,
            ReviewFeedback::Never => return,
        };

        data.half_life_days = (data.half_life_days * multiplier).max(1.0);
        data.last_reviewed_at = Some(Utc::now());
    }

    pub fn recall_probability(data: &HalfLifeData) -> f64 {
        let last_reviewed = match data.last_reviewed_at {
            Some(dt) => dt,
            None => return 0.0,
        };

        let elapsed = Utc::now().signed_duration_since(last_reviewed);
        let elapsed_days = elapsed.num_seconds() as f64 / 86400.0;

        2.0_f64.powf(-elapsed_days / data.half_life_days as f64)
    }

    pub fn next_review_date(data: &HalfLifeData) -> Option<DateTime<Utc>> {
        let last_reviewed = data.last_reviewed_at?;
        let days_until_threshold = data.half_life_days;
        Some(last_reviewed + Duration::days(days_until_threshold as i64))
    }

    pub fn is_due_for_review(data: &HalfLifeData) -> bool {
        recall_probability(data) <= RECALL_THRESHOLD
    }

    pub fn get_initial_half_life(feedback: ReviewFeedback) -> f32 {
        match feedback {
            ReviewFeedback::Soon => 7.0,
            ReviewFeedback::Later => 14.0,
            ReviewFeedback::Someday => 28.0,
            _ => DEFAULT_HALF_LIFE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sm2_default() {
        let data = SM2Data::default();
        assert!((data.ease_factor - 2.5).abs() < 0.001);
        assert_eq!(data.interval, 0);
        assert_eq!(data.repetitions, 0);
    }

    #[test]
    fn test_sm2_first_review_good() {
        let mut data = SM2Data::default();
        sm2::update_sm2(&mut data, ReviewFeedback::Good);
        assert_eq!(data.repetitions, 1);
        assert_eq!(data.interval, 1);
    }

    #[test]
    fn test_sm2_second_review_good() {
        let mut data = SM2Data::default();
        sm2::update_sm2(&mut data, ReviewFeedback::Good);
        sm2::update_sm2(&mut data, ReviewFeedback::Good);
        assert_eq!(data.repetitions, 2);
        assert_eq!(data.interval, 6);
    }

    #[test]
    fn test_sm2_reset_on_again() {
        let mut data = SM2Data {
            ease_factor: 2.5,
            interval: 10,
            repetitions: 3,
        };
        sm2::update_sm2(&mut data, ReviewFeedback::Again);
        assert_eq!(data.repetitions, 0);
        assert_eq!(data.interval, 1);
    }

    #[test]
    fn test_sm2_ease_factor_decrease() {
        let mut data = SM2Data::default();
        sm2::update_sm2(&mut data, ReviewFeedback::Hard);
        assert!(data.ease_factor < 2.5);
    }

    #[test]
    fn test_sm2_ease_factor_min() {
        let mut data = SM2Data {
            ease_factor: 1.3,
            interval: 1,
            repetitions: 1,
        };
        sm2::update_sm2(&mut data, ReviewFeedback::Hard);
        assert!(data.ease_factor >= 1.3);
    }

    #[test]
    fn test_halflife_default() {
        let data = HalfLifeData::default();
        assert!((data.half_life_days - 7.0).abs() < 0.001);
        assert!(data.last_reviewed_at.is_none());
    }

    #[test]
    fn test_halflife_update_soon() {
        let mut data = HalfLifeData {
            half_life_days: 14.0,
            last_reviewed_at: Some(Utc::now()),
        };
        halflife::update_halflife(&mut data, ReviewFeedback::Soon);
        assert!((data.half_life_days - 7.0).abs() < 0.001);
    }

    #[test]
    fn test_halflife_update_someday() {
        let mut data = HalfLifeData {
            half_life_days: 14.0,
            last_reviewed_at: Some(Utc::now()),
        };
        halflife::update_halflife(&mut data, ReviewFeedback::Someday);
        assert!((data.half_life_days - 21.0).abs() < 0.001);
    }

    #[test]
    fn test_halflife_min_value() {
        let mut data = HalfLifeData {
            half_life_days: 1.0,
            last_reviewed_at: Some(Utc::now()),
        };
        halflife::update_halflife(&mut data, ReviewFeedback::Soon);
        assert!(data.half_life_days >= 1.0);
    }

    #[test]
    fn test_recall_probability_just_reviewed() {
        let data = HalfLifeData {
            half_life_days: 7.0,
            last_reviewed_at: Some(Utc::now()),
        };
        let prob = halflife::recall_probability(&data);
        assert!(prob > 0.99);
    }

    #[test]
    fn test_recall_probability_no_review() {
        let data = HalfLifeData::default();
        let prob = halflife::recall_probability(&data);
        assert!(prob < 0.01);
    }

    #[test]
    fn test_initial_half_life() {
        assert!((halflife::get_initial_half_life(ReviewFeedback::Soon) - 7.0).abs() < 0.001);
        assert!((halflife::get_initial_half_life(ReviewFeedback::Later) - 14.0).abs() < 0.001);
        assert!((halflife::get_initial_half_life(ReviewFeedback::Someday) - 28.0).abs() < 0.001);
    }
}
