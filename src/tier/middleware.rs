//! Axum middleware for tier extraction and enforcement

use axum::{
    body::Body,
    extract::Request,
    http::{header::HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::limits::{FeatureGate, ProFeature, TierType};
use super::usage::UsageTracker;

const TIER_HEADER: &str = "x-naidis-tier";

#[derive(Clone)]
pub struct TierState {
    pub tier: TierType,
    pub usage: Arc<RwLock<UsageTracker>>,
}

pub fn extract_tier_from_headers(headers: &HeaderMap) -> TierType {
    headers
        .get(TIER_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(TierType::from_header)
        .unwrap_or_default()
}

pub async fn tier_middleware(mut request: Request<Body>, next: Next) -> Response {
    let tier = extract_tier_from_headers(request.headers());
    request.extensions_mut().insert(tier);
    next.run(request).await
}

#[derive(Debug, Serialize)]
pub struct TierErrorResponse {
    pub error: String,
    pub code: TierErrorCode,
    pub feature: String,
    pub upgrade_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
}

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TierErrorCode {
    ProOnly,
    DailyLimit,
    MaxLimit,
}

impl TierErrorResponse {
    const UPGRADE_URL: &'static str = "https://naidis.dev/pricing";

    pub fn pro_only(feature: &str) -> Self {
        Self {
            error: format!("'{}' requires a Pro subscription", feature),
            code: TierErrorCode::ProOnly,
            feature: feature.to_string(),
            upgrade_url: Self::UPGRADE_URL.to_string(),
            used: None,
            limit: None,
            current: None,
            resets_at: None,
        }
    }

    pub fn from_gate(gate: FeatureGate) -> Option<Self> {
        match gate {
            FeatureGate::Allowed => None,
            FeatureGate::DailyLimitReached {
                feature,
                used,
                limit,
                resets_at,
            } => Some(Self {
                error: format!("Daily {} limit reached ({}/{})", feature, used, limit),
                code: TierErrorCode::DailyLimit,
                feature,
                upgrade_url: Self::UPGRADE_URL.to_string(),
                used: Some(used),
                limit: Some(limit),
                current: None,
                resets_at: Some(resets_at.to_rfc3339()),
            }),
            FeatureGate::MaxLimitReached {
                feature,
                current,
                limit,
            } => Some(Self {
                error: format!("{} limit reached ({}/{})", feature, current, limit),
                code: TierErrorCode::MaxLimit,
                feature,
                upgrade_url: Self::UPGRADE_URL.to_string(),
                used: None,
                limit: Some(limit),
                current: Some(current),
                resets_at: None,
            }),
            FeatureGate::ProOnly { feature } => Some(Self::pro_only(&feature)),
        }
    }
}

impl IntoResponse for TierErrorResponse {
    fn into_response(self) -> Response {
        let status = match self.code {
            TierErrorCode::ProOnly => StatusCode::PAYMENT_REQUIRED,
            TierErrorCode::DailyLimit | TierErrorCode::MaxLimit => StatusCode::TOO_MANY_REQUESTS,
        };
        (status, Json(self)).into_response()
    }
}

#[allow(clippy::result_large_err)]
pub fn check_pro_feature(tier: &TierType, feature: ProFeature) -> Result<(), TierErrorResponse> {
    if tier.is_pro() {
        Ok(())
    } else {
        Err(TierErrorResponse::pro_only(feature.as_str()))
    }
}

pub async fn check_ai_limit(
    tier: &TierType,
    usage: &Arc<RwLock<UsageTracker>>,
) -> Result<(), TierErrorResponse> {
    if tier.is_pro() {
        return Ok(());
    }

    let mut tracker = usage.write().await;
    let gate = tracker.check_ai_query();

    if let Some(err) = TierErrorResponse::from_gate(gate) {
        return Err(err);
    }

    tracker.increment_ai_query().ok();
    Ok(())
}

pub async fn check_rag_limit(
    tier: &TierType,
    usage: &Arc<RwLock<UsageTracker>>,
) -> Result<(), TierErrorResponse> {
    if tier.is_pro() {
        return Ok(());
    }

    let mut tracker = usage.write().await;
    let gate = tracker.check_rag_query();

    if let Some(err) = TierErrorResponse::from_gate(gate) {
        return Err(err);
    }

    tracker.increment_rag_query().ok();
    Ok(())
}

pub async fn check_rss_limit(
    tier: &TierType,
    usage: &Arc<RwLock<UsageTracker>>,
    url: &str,
) -> Result<(), TierErrorResponse> {
    if tier.is_pro() {
        return Ok(());
    }

    let tracker = usage.read().await;
    let gate = tracker.check_rss_feed(url);

    TierErrorResponse::from_gate(gate).map_or(Ok(()), Err)
}

pub async fn check_sr_limit(
    tier: &TierType,
    usage: &Arc<RwLock<UsageTracker>>,
) -> Result<(), TierErrorResponse> {
    if tier.is_pro() {
        return Ok(());
    }

    let tracker = usage.read().await;
    let gate = tracker.check_sr_card();

    TierErrorResponse::from_gate(gate).map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tier_free() {
        let mut headers = HeaderMap::new();
        headers.insert(TIER_HEADER, "free".parse().unwrap());

        let tier = extract_tier_from_headers(&headers);
        assert!(!tier.is_pro());
    }

    #[test]
    fn test_extract_tier_pro() {
        let mut headers = HeaderMap::new();
        headers.insert(TIER_HEADER, "pro:sub_123".parse().unwrap());

        let tier = extract_tier_from_headers(&headers);
        assert!(tier.is_pro());
    }

    #[test]
    fn test_extract_tier_missing() {
        let headers = HeaderMap::new();
        let tier = extract_tier_from_headers(&headers);
        assert!(!tier.is_pro());
    }

    #[test]
    fn test_tier_error_response_json() {
        let err = TierErrorResponse::pro_only("pdf-ocr");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("PRO_ONLY"));
        assert!(json.contains("pdf-ocr"));
    }
}
