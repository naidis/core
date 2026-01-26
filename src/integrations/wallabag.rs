use anyhow::{Context, Result};
use serde::Deserialize;

use crate::rpc::{WallabagConfig, WallabagEntry, WallabagSyncResponse};

pub struct WallabagClient {
    config: WallabagConfig,
    client: reqwest::Client,
    access_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    token_type: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct WallabagApiResponse {
    page: u32,
    limit: u32,
    pages: u32,
    total: u32,
    #[serde(rename = "_embedded")]
    embedded: WallabagEmbedded,
}

#[derive(Deserialize)]
struct WallabagEmbedded {
    items: Vec<WallabagApiEntry>,
}

#[derive(Deserialize)]
struct WallabagApiEntry {
    id: u64,
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
    created_at: Option<String>,
    reading_time: Option<u32>,
    is_archived: Option<u8>,
    is_starred: Option<u8>,
}

impl WallabagClient {
    pub fn new(config: WallabagConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            access_token: None,
        }
    }

    async fn authenticate(&mut self) -> Result<()> {
        let token_url = format!("{}/oauth/v2/token", self.config.url.trim_end_matches('/'));

        let params = [
            ("grant_type", "password"),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
            ("username", &self.config.username),
            ("password", &self.config.password),
        ];

        let response = self
            .client
            .post(&token_url)
            .form(&params)
            .send()
            .await
            .context("Failed to connect to Wallabag")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Wallabag authentication failed: {} - {}", status, text);
        }

        let token: TokenResponse = response
            .json()
            .await
            .context("Failed to parse Wallabag token response")?;

        self.access_token = Some(token.access_token);
        Ok(())
    }

    pub async fn sync(&mut self, limit: Option<usize>) -> Result<WallabagSyncResponse> {
        if self.access_token.is_none() {
            self.authenticate().await?;
        }

        let token = self.access_token.as_ref().unwrap();
        let entries_url = format!("{}/api/entries.json", self.config.url.trim_end_matches('/'));

        let per_page = limit.unwrap_or(30).min(100);

        let response = self
            .client
            .get(&entries_url)
            .bearer_auth(token)
            .query(&[
                ("perPage", per_page.to_string()),
                ("sort", "created".to_string()),
                ("order", "desc".to_string()),
            ])
            .send()
            .await
            .context("Failed to fetch Wallabag entries")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Wallabag API error: {} - {}", status, text);
        }

        let api_response: WallabagApiResponse = response
            .json()
            .await
            .context("Failed to parse Wallabag entries")?;

        let entries: Vec<WallabagEntry> = api_response
            .embedded
            .items
            .into_iter()
            .map(|e| WallabagEntry {
                id: e.id,
                title: e.title.unwrap_or_default(),
                url: e.url.unwrap_or_default(),
                content: e.content,
                created_at: e.created_at.unwrap_or_default(),
                reading_time: e.reading_time,
                is_archived: e.is_archived.unwrap_or(0) == 1,
                is_starred: e.is_starred.unwrap_or(0) == 1,
            })
            .collect();

        let total = entries.len();

        Ok(WallabagSyncResponse { entries, total })
    }
}

pub async fn sync(config: &WallabagConfig, limit: Option<usize>) -> Result<WallabagSyncResponse> {
    let mut client = WallabagClient::new(config.clone());
    client.sync(limit).await
}
