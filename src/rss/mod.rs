use anyhow::{Context, Result};
use feed_rs::parser;

use crate::rpc::{RssItem, RssRequest, RssResponse};

pub async fn fetch(request: &RssRequest) -> Result<RssResponse> {
    let client = reqwest::Client::builder()
        .user_agent("Naidis RSS Reader/1.0")
        .build()?;

    let response = client
        .get(&request.url)
        .send()
        .await
        .context("Failed to fetch RSS feed")?;

    let bytes = response.bytes().await?;
    let feed = parser::parse(&bytes[..]).context("Failed to parse RSS feed")?;

    let limit = request.limit.unwrap_or(50);

    let items: Vec<RssItem> = feed
        .entries
        .into_iter()
        .take(limit)
        .map(|entry| {
            let content = entry
                .content
                .and_then(|c| c.body)
                .or_else(|| entry.summary.map(|s| s.content));

            let link = entry.links.first().map(|l| l.href.clone());

            let published = entry.published.or(entry.updated).map(|dt| dt.to_rfc3339());

            let author = entry.authors.first().map(|a| a.name.clone());

            RssItem {
                title: entry.title.map(|t| t.content),
                link,
                content,
                published,
                author,
            }
        })
        .collect();

    Ok(RssResponse {
        title: feed
            .title
            .map(|t| t.content)
            .unwrap_or_else(|| "Untitled Feed".to_string()),
        description: feed.description.map(|d| d.content),
        link: feed.links.first().map(|l| l.href.clone()),
        items,
    })
}
