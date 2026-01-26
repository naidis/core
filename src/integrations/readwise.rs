use anyhow::{Context, Result};
use serde::Deserialize;

use crate::rpc::{ReadwiseConfig, ReadwiseHighlight, ReadwiseSyncResponse};

pub struct ReadwiseClient {
    config: ReadwiseConfig,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct ReadwiseApiResponse {
    count: usize,
    next: Option<String>,
    previous: Option<String>,
    results: Vec<ReadwiseApiHighlight>,
}

#[derive(Deserialize)]
struct ReadwiseApiHighlight {
    id: u64,
    text: String,
    note: Option<String>,
    location: Option<u32>,
    location_type: Option<String>,
    url: Option<String>,
    book_id: Option<u64>,
    highlighted_at: Option<String>,
}

#[derive(Deserialize)]
struct ReadwiseBook {
    id: u64,
    title: String,
    author: Option<String>,
}

impl ReadwiseClient {
    pub fn new(config: ReadwiseConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    async fn get_books(&self) -> Result<std::collections::HashMap<u64, (String, Option<String>)>> {
        let mut books = std::collections::HashMap::new();
        let mut page_url = Some("https://readwise.io/api/v2/books/".to_string());

        while let Some(url) = page_url {
            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Token {}", self.config.api_key))
                .send()
                .await
                .context("Failed to fetch Readwise books")?;

            if !response.status().is_success() {
                break;
            }

            #[derive(Deserialize)]
            struct BooksResponse {
                next: Option<String>,
                results: Vec<ReadwiseBook>,
            }

            let data: BooksResponse = response.json().await?;

            for book in data.results {
                books.insert(book.id, (book.title, book.author));
            }

            page_url = data.next;
        }

        Ok(books)
    }

    pub async fn sync(&self, updated_after: Option<&str>) -> Result<ReadwiseSyncResponse> {
        let books = self.get_books().await.unwrap_or_default();

        let mut all_highlights = Vec::new();
        let mut page_url = Some("https://readwise.io/api/v2/highlights/".to_string());

        while let Some(url) = page_url {
            let mut request = self
                .client
                .get(&url)
                .header("Authorization", format!("Token {}", self.config.api_key));

            if let Some(after) = updated_after {
                if url == "https://readwise.io/api/v2/highlights/" {
                    request = request.query(&[("updated__gt", after)]);
                }
            }

            let response = request
                .send()
                .await
                .context("Failed to fetch Readwise highlights")?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                anyhow::bail!("Readwise API error: {} - {}", status, text);
            }

            let data: ReadwiseApiResponse = response
                .json()
                .await
                .context("Failed to parse Readwise response")?;

            for h in data.results {
                let (book_title, book_author) = h
                    .book_id
                    .and_then(|id| books.get(&id).cloned())
                    .unwrap_or(("Unknown".to_string(), None));

                all_highlights.push(ReadwiseHighlight {
                    id: h.id,
                    text: h.text,
                    note: h.note,
                    location: h.location,
                    location_type: h.location_type,
                    url: h.url,
                    book_title,
                    book_author,
                    highlighted_at: h.highlighted_at,
                });
            }

            page_url = data.next;

            if all_highlights.len() >= 1000 {
                break;
            }
        }

        let total = all_highlights.len();

        Ok(ReadwiseSyncResponse {
            highlights: all_highlights,
            total,
        })
    }
}

pub async fn sync(
    config: &ReadwiseConfig,
    updated_after: Option<&str>,
) -> Result<ReadwiseSyncResponse> {
    let client = ReadwiseClient::new(config.clone());
    client.sync(updated_after).await
}
