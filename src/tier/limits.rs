//! Tier limits and types for freemium model
//!
//! Defines the limits for free tier and feature gates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Free tier limits (matching app/src/core/license.ts FREE_LIMITS)
#[derive(Debug, Clone)]
pub struct FreeLimits {
    /// AI chat queries per day
    pub ai_queries_per_day: u32,
    /// RAG queries per day
    pub rag_queries_per_day: u32,
    /// Maximum active spaced repetition cards
    pub sr_active_cards: u32,
    /// Maximum RSS feeds
    pub rss_feeds: u32,
    /// YouTube batch size
    pub youtube_batch_size: u32,
}

impl Default for FreeLimits {
    fn default() -> Self {
        Self {
            ai_queries_per_day: 5,
            rag_queries_per_day: 5,
            sr_active_cards: 50,
            rss_feeds: 3,
            youtube_batch_size: 1,
        }
    }
}

impl FreeLimits {
    pub fn get() -> &'static Self {
        static LIMITS: std::sync::OnceLock<FreeLimits> = std::sync::OnceLock::new();
        LIMITS.get_or_init(FreeLimits::default)
    }
}

/// User's subscription tier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[derive(Default)]
pub enum TierType {
    #[default]
    Free,
    Pro {
        subscription_id: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    },
}

impl TierType {
    /// Check if this is a Pro tier (active subscription)
    pub fn is_pro(&self) -> bool {
        match self {
            TierType::Free => false,
            TierType::Pro { expires_at, .. } => {
                // If no expiry, assume active
                // If has expiry, check if still valid
                expires_at.is_none_or(|exp| exp > Utc::now())
            }
        }
    }

    /// Parse from X-Naidis-Tier header value
    /// Format: "free" or "pro:sub_xxx:exp_2025-12-31"
    pub fn from_header(header: &str) -> Self {
        let parts: Vec<&str> = header.split(':').collect();

        match parts.first().map(|s| s.to_lowercase()).as_deref() {
            Some("pro") => {
                let subscription_id = parts.get(1).map(|s| s.to_string());
                let expires_at = parts.get(2).and_then(|s| {
                    s.strip_prefix("exp_")
                        .and_then(|date_str| DateTime::parse_from_rfc3339(date_str).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                });
                TierType::Pro {
                    subscription_id,
                    expires_at,
                }
            }
            _ => TierType::Free,
        }
    }

    /// Convert to header value
    pub fn to_header(&self) -> String {
        match self {
            TierType::Free => "free".to_string(),
            TierType::Pro {
                subscription_id,
                expires_at,
            } => {
                let mut parts = vec!["pro".to_string()];
                if let Some(sub_id) = subscription_id {
                    parts.push(sub_id.clone());
                }
                if let Some(exp) = expires_at {
                    parts.push(format!("exp_{}", exp.to_rfc3339()));
                }
                parts.join(":")
            }
        }
    }
}

/// Result of checking feature access
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FeatureGate {
    /// Feature is allowed
    Allowed,

    /// Daily limit reached (AI/RAG queries)
    DailyLimitReached {
        feature: String,
        used: u32,
        limit: u32,
        resets_at: DateTime<Utc>,
    },

    /// Maximum count limit reached (RSS feeds, SR cards)
    MaxLimitReached {
        feature: String,
        current: u32,
        limit: u32,
    },

    /// Feature requires Pro subscription
    ProOnly { feature: String },
}

impl FeatureGate {
    pub fn is_allowed(&self) -> bool {
        matches!(self, FeatureGate::Allowed)
    }
}

/// Pro-only features (matching app/src/core/license.ts PRO_FEATURES)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProFeature {
    PdfOcr,
    PdfTables,
    YoutubeBatch,
    YoutubeAiChapters,
    AiUnlimited,
    RagUnlimited,
    SrUnlimitedCards,
    SyncWallabag,
    SyncHoarder,
    SyncReadwise,
    SyncTodoist,
    SyncGcal,
    Newsletter,
    Tts,
    KindleImport,
    Epub,
    BulkOperations,
}

impl ProFeature {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProFeature::PdfOcr => "pdf-ocr",
            ProFeature::PdfTables => "pdf-tables",
            ProFeature::YoutubeBatch => "youtube-batch",
            ProFeature::YoutubeAiChapters => "youtube-ai-chapters",
            ProFeature::AiUnlimited => "ai-unlimited",
            ProFeature::RagUnlimited => "rag-unlimited",
            ProFeature::SrUnlimitedCards => "sr-unlimited-cards",
            ProFeature::SyncWallabag => "sync-wallabag",
            ProFeature::SyncHoarder => "sync-hoarder",
            ProFeature::SyncReadwise => "sync-readwise",
            ProFeature::SyncTodoist => "sync-todoist",
            ProFeature::SyncGcal => "sync-gcal",
            ProFeature::Newsletter => "newsletter",
            ProFeature::Tts => "tts",
            ProFeature::KindleImport => "kindle-import",
            ProFeature::Epub => "epub",
            ProFeature::BulkOperations => "bulk-operations",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_from_header_free() {
        let tier = TierType::from_header("free");
        assert!(!tier.is_pro());
        assert_eq!(tier, TierType::Free);
    }

    #[test]
    fn test_tier_from_header_pro() {
        let tier = TierType::from_header("pro:sub_123");
        assert!(tier.is_pro());
        if let TierType::Pro {
            subscription_id, ..
        } = tier
        {
            assert_eq!(subscription_id, Some("sub_123".to_string()));
        } else {
            panic!("Expected Pro tier");
        }
    }

    #[test]
    fn test_tier_from_header_pro_with_expiry() {
        let future = Utc::now() + chrono::Duration::days(30);
        let header = format!("pro:sub_123:exp_{}", future.to_rfc3339());
        let tier = TierType::from_header(&header);
        assert!(tier.is_pro());
    }

    #[test]
    fn test_tier_expired_pro() {
        let past = Utc::now() - chrono::Duration::days(1);
        let tier = TierType::Pro {
            subscription_id: Some("sub_123".to_string()),
            expires_at: Some(past),
        };
        assert!(!tier.is_pro()); // Expired = not pro
    }

    #[test]
    fn test_free_limits() {
        let limits = FreeLimits::get();
        assert_eq!(limits.ai_queries_per_day, 5);
        assert_eq!(limits.sr_active_cards, 50);
        assert_eq!(limits.rss_feeds, 3);
    }
}
