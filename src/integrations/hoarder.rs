use anyhow::{Context, Result};
use serde::Deserialize;

use crate::rpc::{HoarderBookmark, HoarderConfig, HoarderSyncResponse};

pub struct HoarderClient {
    config: HoarderConfig,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct HoarderApiResponse {
    bookmarks: Vec<HoarderApiBookmark>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct HoarderApiBookmark {
    id: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    title: Option<String>,
    #[serde(rename = "type")]
    bookmark_type: Option<String>,
    content: Option<HoarderContent>,
    tags: Option<Vec<HoarderTag>>,
    summary: Option<String>,
}

#[derive(Deserialize)]
struct HoarderContent {
    #[serde(rename = "type")]
    content_type: String,
    url: Option<String>,
    title: Option<String>,
    description: Option<String>,
    #[serde(rename = "htmlContent")]
    html_content: Option<String>,
}

#[derive(Deserialize)]
struct HoarderTag {
    id: String,
    name: String,
}

impl HoarderClient {
    pub fn new(config: HoarderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub async fn sync(&self, limit: Option<usize>) -> Result<HoarderSyncResponse> {
        let api_url = format!("{}/api/v1/bookmarks", self.config.url.trim_end_matches('/'));

        let response = self
            .client
            .get(&api_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .query(&[("limit", limit.unwrap_or(50).to_string())])
            .send()
            .await
            .context("Failed to connect to Hoarder")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Hoarder API error: {} - {}", status, text);
        }

        let api_response: HoarderApiResponse = response
            .json()
            .await
            .context("Failed to parse Hoarder response")?;

        let bookmarks: Vec<HoarderBookmark> = api_response
            .bookmarks
            .into_iter()
            .map(|b| {
                let (url, content) = if let Some(c) = b.content {
                    (c.url.unwrap_or_default(), c.html_content.or(c.description))
                } else {
                    (String::new(), None)
                };

                let tags = b
                    .tags
                    .map(|t| t.into_iter().map(|tag| tag.name).collect())
                    .unwrap_or_default();

                HoarderBookmark {
                    id: b.id,
                    title: b.title,
                    url,
                    content,
                    summary: b.summary,
                    tags,
                    created_at: b.created_at,
                }
            })
            .collect();

        let total = bookmarks.len();

        Ok(HoarderSyncResponse { bookmarks, total })
    }
}

pub async fn sync(config: &HoarderConfig, limit: Option<usize>) -> Result<HoarderSyncResponse> {
    let client = HoarderClient::new(config.clone());
    client.sync(limit).await
}
