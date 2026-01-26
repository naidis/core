//! Usage tracking for freemium tier limits
//!
//! Tracks daily usage (AI/RAG queries) and persistent counts (RSS feeds, SR cards).

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::limits::{FeatureGate, FreeLimits};

/// Daily usage that resets at midnight UTC
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyUsage {
    pub date: NaiveDate,
    pub ai_queries: u32,
    pub rag_queries: u32,
}

impl DailyUsage {
    fn new() -> Self {
        Self {
            date: Utc::now().date_naive(),
            ai_queries: 0,
            rag_queries: 0,
        }
    }

    fn reset_if_new_day(&mut self) {
        let today = Utc::now().date_naive();
        if self.date != today {
            self.date = today;
            self.ai_queries = 0;
            self.rag_queries = 0;
        }
    }
}

/// Persistent usage counts (not reset daily)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistentUsage {
    /// Active RSS feed URLs
    pub rss_feed_urls: Vec<String>,
    /// Active SR card count (calculated from SR store, cached here)
    pub sr_card_count: u32,
}

/// Combined usage data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageData {
    pub daily: DailyUsage,
    pub persistent: PersistentUsage,
}

/// Usage tracker with file persistence
pub struct UsageTracker {
    data_dir: PathBuf,
    data: UsageData,
}

impl UsageTracker {
    const USAGE_FILE: &'static str = "tier_usage.json";

    /// Create a new usage tracker
    pub fn new(data_dir: PathBuf) -> Result<Self> {
        let tier_dir = data_dir.join("tier");
        fs::create_dir_all(&tier_dir)?;

        let mut tracker = Self {
            data_dir: tier_dir,
            data: UsageData::default(),
        };
        tracker.load()?;
        tracker.data.daily.reset_if_new_day();
        tracker.save()?;

        Ok(tracker)
    }

    fn usage_file(&self) -> PathBuf {
        self.data_dir.join(Self::USAGE_FILE)
    }

    fn load(&mut self) -> Result<()> {
        let path = self.usage_file();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            self.data = serde_json::from_str(&content)?;
        }
        Ok(())
    }

    fn save(&self) -> Result<()> {
        let path = self.usage_file();
        let content = serde_json::to_string_pretty(&self.data)?;
        fs::write(&path, content)?;
        Ok(())
    }

    // ========== Daily Usage (AI/RAG) ==========

    /// Get next reset time (midnight UTC)
    fn next_reset_time(&self) -> DateTime<Utc> {
        let tomorrow = Utc::now().date_naive() + chrono::Duration::days(1);
        tomorrow.and_hms_opt(0, 0, 0).unwrap().and_utc()
    }

    /// Check if AI query is allowed
    pub fn check_ai_query(&mut self) -> FeatureGate {
        self.data.daily.reset_if_new_day();
        let limits = FreeLimits::get();

        if self.data.daily.ai_queries >= limits.ai_queries_per_day {
            FeatureGate::DailyLimitReached {
                feature: "ai_chat".to_string(),
                used: self.data.daily.ai_queries,
                limit: limits.ai_queries_per_day,
                resets_at: self.next_reset_time(),
            }
        } else {
            FeatureGate::Allowed
        }
    }

    /// Increment AI query count
    pub fn increment_ai_query(&mut self) -> Result<()> {
        self.data.daily.reset_if_new_day();
        self.data.daily.ai_queries += 1;
        self.save()
    }

    /// Check if RAG query is allowed
    pub fn check_rag_query(&mut self) -> FeatureGate {
        self.data.daily.reset_if_new_day();
        let limits = FreeLimits::get();

        if self.data.daily.rag_queries >= limits.rag_queries_per_day {
            FeatureGate::DailyLimitReached {
                feature: "rag".to_string(),
                used: self.data.daily.rag_queries,
                limit: limits.rag_queries_per_day,
                resets_at: self.next_reset_time(),
            }
        } else {
            FeatureGate::Allowed
        }
    }

    /// Increment RAG query count
    pub fn increment_rag_query(&mut self) -> Result<()> {
        self.data.daily.reset_if_new_day();
        self.data.daily.rag_queries += 1;
        self.save()
    }

    // ========== Persistent Usage (RSS) ==========

    /// Check if adding a new RSS feed is allowed
    pub fn check_rss_feed(&self, url: &str) -> FeatureGate {
        let limits = FreeLimits::get();

        // If URL already tracked, allow (not a new feed)
        if self
            .data
            .persistent
            .rss_feed_urls
            .contains(&url.to_string())
        {
            return FeatureGate::Allowed;
        }

        let current = self.data.persistent.rss_feed_urls.len() as u32;
        if current >= limits.rss_feeds {
            FeatureGate::MaxLimitReached {
                feature: "rss".to_string(),
                current,
                limit: limits.rss_feeds,
            }
        } else {
            FeatureGate::Allowed
        }
    }

    /// Add RSS feed URL to tracking
    pub fn add_rss_feed(&mut self, url: &str) -> Result<()> {
        if !self
            .data
            .persistent
            .rss_feed_urls
            .contains(&url.to_string())
        {
            self.data.persistent.rss_feed_urls.push(url.to_string());
            self.save()?;
        }
        Ok(())
    }

    /// Remove RSS feed URL from tracking
    pub fn remove_rss_feed(&mut self, url: &str) -> Result<()> {
        self.data.persistent.rss_feed_urls.retain(|u| u != url);
        self.save()
    }

    /// Get current RSS feed count
    pub fn rss_feed_count(&self) -> u32 {
        self.data.persistent.rss_feed_urls.len() as u32
    }

    // ========== Persistent Usage (SR Cards) ==========

    /// Check if adding a new SR card is allowed
    pub fn check_sr_card(&self) -> FeatureGate {
        let limits = FreeLimits::get();

        if self.data.persistent.sr_card_count >= limits.sr_active_cards {
            FeatureGate::MaxLimitReached {
                feature: "spaced_repetition".to_string(),
                current: self.data.persistent.sr_card_count,
                limit: limits.sr_active_cards,
            }
        } else {
            FeatureGate::Allowed
        }
    }

    /// Update SR card count (called when cards change)
    pub fn set_sr_card_count(&mut self, count: u32) -> Result<()> {
        self.data.persistent.sr_card_count = count;
        self.save()
    }

    /// Increment SR card count
    pub fn increment_sr_card(&mut self) -> Result<()> {
        self.data.persistent.sr_card_count += 1;
        self.save()
    }

    /// Decrement SR card count
    pub fn decrement_sr_card(&mut self) -> Result<()> {
        self.data.persistent.sr_card_count = self.data.persistent.sr_card_count.saturating_sub(1);
        self.save()
    }

    // ========== Stats ==========

    /// Get usage statistics for display
    pub fn get_stats(&self) -> UsageStats {
        let limits = FreeLimits::get();

        UsageStats {
            ai_queries: QueryStats {
                used: self.data.daily.ai_queries,
                limit: limits.ai_queries_per_day,
                remaining: limits
                    .ai_queries_per_day
                    .saturating_sub(self.data.daily.ai_queries),
            },
            rag_queries: QueryStats {
                used: self.data.daily.rag_queries,
                limit: limits.rag_queries_per_day,
                remaining: limits
                    .rag_queries_per_day
                    .saturating_sub(self.data.daily.rag_queries),
            },
            rss_feeds: CountStats {
                current: self.data.persistent.rss_feed_urls.len() as u32,
                limit: limits.rss_feeds,
            },
            sr_cards: CountStats {
                current: self.data.persistent.sr_card_count,
                limit: limits.sr_active_cards,
            },
            resets_at: self.next_reset_time(),
        }
    }
}

/// Query usage stats (daily limits)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryStats {
    pub used: u32,
    pub limit: u32,
    pub remaining: u32,
}

/// Count stats (persistent limits)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountStats {
    pub current: u32,
    pub limit: u32,
}

/// Full usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub ai_queries: QueryStats,
    pub rag_queries: QueryStats,
    pub rss_feeds: CountStats,
    pub sr_cards: CountStats,
    pub resets_at: DateTime<Utc>,
}

/// Shared usage tracker type
pub type SharedUsageTracker = Arc<RwLock<UsageTracker>>;

/// Create a shared usage tracker
pub fn create_shared_tracker(data_dir: PathBuf) -> Result<SharedUsageTracker> {
    let tracker = UsageTracker::new(data_dir)?;
    Ok(Arc::new(RwLock::new(tracker)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_usage_tracker_creation() {
        let dir = tempdir().unwrap();
        let tracker = UsageTracker::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(tracker.data.daily.ai_queries, 0);
    }

    #[test]
    fn test_ai_query_limit() {
        let dir = tempdir().unwrap();
        let mut tracker = UsageTracker::new(dir.path().to_path_buf()).unwrap();

        // Use up all queries
        for _ in 0..5 {
            assert!(tracker.check_ai_query().is_allowed());
            tracker.increment_ai_query().unwrap();
        }

        // Should be limited now
        assert!(!tracker.check_ai_query().is_allowed());
    }

    #[test]
    fn test_rss_feed_limit() {
        let dir = tempdir().unwrap();
        let mut tracker = UsageTracker::new(dir.path().to_path_buf()).unwrap();

        // Add 3 feeds
        for i in 0..3 {
            let url = format!("https://example{}.com/feed", i);
            assert!(tracker.check_rss_feed(&url).is_allowed());
            tracker.add_rss_feed(&url).unwrap();
        }

        // 4th feed should be blocked
        assert!(!tracker
            .check_rss_feed("https://example4.com/feed")
            .is_allowed());

        // But existing feed should still be allowed
        assert!(tracker
            .check_rss_feed("https://example0.com/feed")
            .is_allowed());
    }

    #[test]
    fn test_usage_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        // Create tracker, add some usage
        {
            let mut tracker = UsageTracker::new(path.clone()).unwrap();
            tracker.increment_ai_query().unwrap();
            tracker.add_rss_feed("https://test.com/feed").unwrap();
        }

        // Create new tracker, should load previous data
        {
            let tracker = UsageTracker::new(path).unwrap();
            assert_eq!(tracker.data.daily.ai_queries, 1);
            assert_eq!(tracker.rss_feed_count(), 1);
        }
    }
}
