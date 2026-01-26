use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewStats {
    pub total_reviews: u64,
    pub total_highlights_reviewed: u64,
    pub total_mastery_cards_reviewed: u64,
    pub reviews_by_date: HashMap<String, DailyStats>,
    pub streak: StreakData,
    pub updated_at: DateTime<Utc>,
}

impl Default for ReviewStats {
    fn default() -> Self {
        Self {
            total_reviews: 0,
            total_highlights_reviewed: 0,
            total_mastery_cards_reviewed: 0,
            reviews_by_date: HashMap::new(),
            streak: StreakData::default(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub date: String,
    pub highlights_reviewed: u32,
    pub mastery_cards_reviewed: u32,
    pub session_count: u32,
    pub completed_sessions: u32,
}

impl DailyStats {
    pub fn new(date: String) -> Self {
        Self {
            date,
            highlights_reviewed: 0,
            mastery_cards_reviewed: 0,
            session_count: 0,
            completed_sessions: 0,
        }
    }

    pub fn total_items(&self) -> u32 {
        self.highlights_reviewed + self.mastery_cards_reviewed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreakData {
    pub current_streak: u32,
    pub longest_streak: u32,
    pub last_review_date: Option<String>,
    pub streak_start_date: Option<String>,
}

impl ReviewStats {
    pub fn record_review(&mut self, at: DateTime<Utc>) {
        self.total_reviews += 1;
        self.updated_at = at;

        let date_str = at.format("%Y-%m-%d").to_string();
        let daily = self
            .reviews_by_date
            .entry(date_str.clone())
            .or_insert_with(|| DailyStats::new(date_str.clone()));
        daily.highlights_reviewed += 1;

        self.update_streak(&date_str);
    }

    pub fn record_mastery_review(&mut self, at: DateTime<Utc>) {
        self.total_mastery_cards_reviewed += 1;
        self.updated_at = at;

        let date_str = at.format("%Y-%m-%d").to_string();
        let daily = self
            .reviews_by_date
            .entry(date_str.clone())
            .or_insert_with(|| DailyStats::new(date_str.clone()));
        daily.mastery_cards_reviewed += 1;

        self.update_streak(&date_str);
    }

    pub fn record_session_start(&mut self, at: DateTime<Utc>) {
        let date_str = at.format("%Y-%m-%d").to_string();
        let daily = self
            .reviews_by_date
            .entry(date_str.clone())
            .or_insert_with(|| DailyStats::new(date_str));
        daily.session_count += 1;
    }

    pub fn record_session_complete(&mut self, at: DateTime<Utc>) {
        let date_str = at.format("%Y-%m-%d").to_string();
        let daily = self
            .reviews_by_date
            .entry(date_str.clone())
            .or_insert_with(|| DailyStats::new(date_str));
        daily.completed_sessions += 1;
    }

    fn update_streak(&mut self, date_str: &str) {
        let today = date_str.to_string();

        match &self.streak.last_review_date {
            None => {
                self.streak.current_streak = 1;
                self.streak.streak_start_date = Some(today.clone());
            }
            Some(last_date) => {
                if last_date == &today {
                    return;
                }

                let last = NaiveDate::parse_from_str(last_date, "%Y-%m-%d").ok();
                let current = NaiveDate::parse_from_str(&today, "%Y-%m-%d").ok();

                if let (Some(last), Some(current)) = (last, current) {
                    let diff = current.signed_duration_since(last).num_days();

                    if diff == 1 {
                        self.streak.current_streak += 1;
                    } else if diff > 1 {
                        self.streak.current_streak = 1;
                        self.streak.streak_start_date = Some(today.clone());
                    }
                }
            }
        }

        self.streak.last_review_date = Some(today);

        if self.streak.current_streak > self.streak.longest_streak {
            self.streak.longest_streak = self.streak.current_streak;
        }
    }

    pub fn can_recover_streak(&self) -> bool {
        let Some(last_date) = &self.streak.last_review_date else {
            return false;
        };

        let last = match NaiveDate::parse_from_str(last_date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => return false,
        };

        let today = Utc::now().date_naive();
        let diff = today.signed_duration_since(last).num_days();

        (2..=7).contains(&diff)
    }

    pub fn recover_streak(&mut self, date_str: &str) -> bool {
        if !self.can_recover_streak() {
            return false;
        }

        self.streak.current_streak += 1;
        self.streak.last_review_date = Some(date_str.to_string());

        if self.streak.current_streak > self.streak.longest_streak {
            self.streak.longest_streak = self.streak.current_streak;
        }

        true
    }

    pub fn get_weekly_stats(&self) -> Vec<&DailyStats> {
        let today = Utc::now().date_naive();
        let mut stats = Vec::new();

        for i in 0..7 {
            let date = today - Duration::days(i);
            let date_str = date.format("%Y-%m-%d").to_string();
            if let Some(daily) = self.reviews_by_date.get(&date_str) {
                stats.push(daily);
            }
        }

        stats
    }

    pub fn get_monthly_stats(&self) -> Vec<&DailyStats> {
        let today = Utc::now().date_naive();
        let mut stats = Vec::new();

        for i in 0..30 {
            let date = today - Duration::days(i);
            let date_str = date.format("%Y-%m-%d").to_string();
            if let Some(daily) = self.reviews_by_date.get(&date_str) {
                stats.push(daily);
            }
        }

        stats
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_reviews: u64,
    pub total_highlights_reviewed: u64,
    pub total_mastery_cards_reviewed: u64,
    pub current_streak: u32,
    pub longest_streak: u32,
    pub can_recover_streak: bool,
    pub today_reviews: u32,
    pub week_reviews: u32,
}

impl From<&ReviewStats> for StatsResponse {
    fn from(stats: &ReviewStats) -> Self {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let today_reviews = stats
            .reviews_by_date
            .get(&today)
            .map(|d| d.total_items())
            .unwrap_or(0);

        let week_reviews: u32 = stats
            .get_weekly_stats()
            .iter()
            .map(|d| d.total_items())
            .sum();

        Self {
            total_reviews: stats.total_reviews,
            total_highlights_reviewed: stats.total_highlights_reviewed,
            total_mastery_cards_reviewed: stats.total_mastery_cards_reviewed,
            current_streak: stats.streak.current_streak,
            longest_streak: stats.streak.longest_streak,
            can_recover_streak: stats.can_recover_streak(),
            today_reviews,
            week_reviews,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_stats() {
        let stats = ReviewStats::default();
        assert_eq!(stats.total_reviews, 0);
        assert_eq!(stats.streak.current_streak, 0);
    }

    #[test]
    fn test_record_review() {
        let mut stats = ReviewStats::default();
        stats.record_review(Utc::now());
        assert_eq!(stats.total_reviews, 1);
        assert_eq!(stats.streak.current_streak, 1);
    }

    #[test]
    fn test_streak_same_day() {
        let mut stats = ReviewStats::default();
        let now = Utc::now();
        stats.record_review(now);
        stats.record_review(now);
        stats.record_review(now);
        assert_eq!(stats.streak.current_streak, 1);
    }

    #[test]
    fn test_daily_stats() {
        let mut stats = ReviewStats::default();
        let now = Utc::now();
        stats.record_review(now);
        stats.record_mastery_review(now);

        let date_str = now.format("%Y-%m-%d").to_string();
        let daily = stats.reviews_by_date.get(&date_str).unwrap();

        assert_eq!(daily.highlights_reviewed, 1);
        assert_eq!(daily.mastery_cards_reviewed, 1);
        assert_eq!(daily.total_items(), 2);
    }

    #[test]
    fn test_session_tracking() {
        let mut stats = ReviewStats::default();
        let now = Utc::now();
        stats.record_session_start(now);
        stats.record_session_complete(now);

        let date_str = now.format("%Y-%m-%d").to_string();
        let daily = stats.reviews_by_date.get(&date_str).unwrap();

        assert_eq!(daily.session_count, 1);
        assert_eq!(daily.completed_sessions, 1);
    }

    #[test]
    fn test_stats_response() {
        let mut stats = ReviewStats::default();
        stats.record_review(Utc::now());

        let response = StatsResponse::from(&stats);
        assert_eq!(response.total_reviews, 1);
        assert_eq!(response.current_streak, 1);
        assert_eq!(response.today_reviews, 1);
    }
}
