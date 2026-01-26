use anyhow::{Context, Result};
use scraper::{Html, Selector};
use url::Url;

use crate::rpc::{WebClipRequest, WebClipResponse};

pub async fn extract(request: &WebClipRequest) -> Result<WebClipResponse> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .build()?;

    let response = client
        .get(&request.url)
        .send()
        .await
        .context("Failed to fetch URL")?;

    let html = response.text().await?;
    let document = Html::parse_document(&html);

    let title = extract_title(&document);
    let content = extract_content(&html, request.include_images)?;
    let author = extract_meta(&document, "author");
    let published_date = extract_meta(&document, "article:published_time")
        .or_else(|| extract_meta(&document, "datePublished"));
    let excerpt = extract_meta(&document, "description")
        .or_else(|| extract_meta(&document, "og:description"));
    let site_name = extract_meta(&document, "og:site_name");

    Ok(WebClipResponse {
        title,
        content,
        author,
        published_date,
        excerpt,
        site_name,
        url: request.url.clone(),
    })
}

fn extract_title(document: &Html) -> String {
    let og_title_selector = Selector::parse("meta[property='og:title']").unwrap();
    if let Some(element) = document.select(&og_title_selector).next() {
        if let Some(content) = element.value().attr("content") {
            return content.to_string();
        }
    }

    let title_selector = Selector::parse("title").unwrap();
    if let Some(element) = document.select(&title_selector).next() {
        return element.text().collect::<String>().trim().to_string();
    }

    let h1_selector = Selector::parse("h1").unwrap();
    if let Some(element) = document.select(&h1_selector).next() {
        return element.text().collect::<String>().trim().to_string();
    }

    "Untitled".to_string()
}

fn extract_meta(document: &Html, name: &str) -> Option<String> {
    let selectors = [
        format!("meta[name='{}']", name),
        format!("meta[property='{}']", name),
        format!("meta[itemprop='{}']", name),
    ];

    for selector_str in &selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                if let Some(content) = element.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    None
}

fn extract_content(html: &str, include_images: bool) -> Result<String> {
    use readability::extractor;

    let url = Url::parse("https://example.com")?;
    let extracted = extractor::extract(&mut html.as_bytes(), &url)?;

    let mut content = extracted.content;

    if !include_images {
        let img_regex = regex::Regex::new(r"<img[^>]*>")?;
        content = img_regex.replace_all(&content, "").to_string();
    }

    let content = html_to_markdown(&content)?;

    Ok(content)
}

fn html_to_markdown(html: &str) -> Result<String> {
    let document = Html::parse_fragment(html);
    let mut result = String::new();

    convert_node_to_markdown(&document.root_element(), &mut result)?;

    let lines: Vec<&str> = result.lines().collect();
    let mut cleaned = Vec::new();
    let mut prev_empty = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_empty {
                cleaned.push("");
                prev_empty = true;
            }
        } else {
            cleaned.push(trimmed);
            prev_empty = false;
        }
    }

    Ok(cleaned.join("\n"))
}

fn convert_node_to_markdown(element: &scraper::ElementRef, output: &mut String) -> Result<()> {
    for child in element.children() {
        if let Some(element_ref) = scraper::ElementRef::wrap(child) {
            let tag = element_ref.value().name();

            match tag {
                "h1" => {
                    output.push_str("\n# ");
                    let text: String = element_ref.text().collect();
                    output.push_str(text.trim());
                    output.push_str("\n\n");
                }
                "h2" => {
                    output.push_str("\n## ");
                    let text: String = element_ref.text().collect();
                    output.push_str(text.trim());
                    output.push_str("\n\n");
                }
                "h3" => {
                    output.push_str("\n### ");
                    let text: String = element_ref.text().collect();
                    output.push_str(text.trim());
                    output.push_str("\n\n");
                }
                "h4" | "h5" | "h6" => {
                    output.push_str("\n#### ");
                    let text: String = element_ref.text().collect();
                    output.push_str(text.trim());
                    output.push_str("\n\n");
                }
                "p" => {
                    convert_node_to_markdown(&element_ref, output)?;
                    output.push_str("\n\n");
                }
                "br" => {
                    output.push('\n');
                }
                "strong" | "b" => {
                    output.push_str("**");
                    let text: String = element_ref.text().collect();
                    output.push_str(text.trim());
                    output.push_str("**");
                }
                "em" | "i" => {
                    output.push('*');
                    let text: String = element_ref.text().collect();
                    output.push_str(text.trim());
                    output.push('*');
                }
                "code" => {
                    output.push('`');
                    let text: String = element_ref.text().collect();
                    output.push_str(&text);
                    output.push('`');
                }
                "pre" => {
                    output.push_str("\n```\n");
                    let text: String = element_ref.text().collect();
                    output.push_str(&text);
                    output.push_str("\n```\n\n");
                }
                "a" => {
                    if let Some(href) = element_ref.value().attr("href") {
                        output.push('[');
                        let text: String = element_ref.text().collect();
                        output.push_str(text.trim());
                        output.push_str("](");
                        output.push_str(href);
                        output.push(')');
                    } else {
                        convert_node_to_markdown(&element_ref, output)?;
                    }
                }
                "img" => {
                    if let Some(src) = element_ref.value().attr("src") {
                        let alt = element_ref.value().attr("alt").unwrap_or("");
                        output.push_str("![");
                        output.push_str(alt);
                        output.push_str("](");
                        output.push_str(src);
                        output.push_str(")\n");
                    }
                }
                "ul" => {
                    output.push('\n');
                    for li in element_ref.children() {
                        if let Some(li_ref) = scraper::ElementRef::wrap(li) {
                            if li_ref.value().name() == "li" {
                                output.push_str("- ");
                                let text: String = li_ref.text().collect();
                                output.push_str(text.trim());
                                output.push('\n');
                            }
                        }
                    }
                    output.push('\n');
                }
                "ol" => {
                    output.push('\n');
                    let mut num = 1;
                    for li in element_ref.children() {
                        if let Some(li_ref) = scraper::ElementRef::wrap(li) {
                            if li_ref.value().name() == "li" {
                                output.push_str(&format!("{}. ", num));
                                let text: String = li_ref.text().collect();
                                output.push_str(text.trim());
                                output.push('\n');
                                num += 1;
                            }
                        }
                    }
                    output.push('\n');
                }
                "blockquote" => {
                    output.push_str("\n> ");
                    let text: String = element_ref.text().collect();
                    output.push_str(&text.trim().replace('\n', "\n> "));
                    output.push_str("\n\n");
                }
                "hr" => {
                    output.push_str("\n---\n\n");
                }
                "div" | "article" | "section" | "main" | "span" => {
                    convert_node_to_markdown(&element_ref, output)?;
                }
                "script" | "style" | "nav" | "footer" | "header" | "aside" => {}
                _ => {
                    convert_node_to_markdown(&element_ref, output)?;
                }
            }
        } else if let Some(text) = child.value().as_text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                output.push_str(trimmed);
                output.push(' ');
            }
        }
    }

    Ok(())
}
