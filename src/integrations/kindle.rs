use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Kindle highlight extracted from My Clippings.txt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KindleHighlight {
    pub id: String,
    pub book_title: String,
    pub book_author: Option<String>,
    pub text: String,
    pub note: Option<String>,
    pub location: Option<String>,
    pub page: Option<u32>,
    pub highlighted_at: Option<String>,
}

/// A book with its highlights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KindleBook {
    pub title: String,
    pub author: Option<String>,
    pub highlights: Vec<KindleHighlight>,
    pub highlight_count: usize,
}

/// Response from kindle sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KindleSyncResponse {
    pub books: Vec<KindleBook>,
    pub total_highlights: usize,
    pub total_books: usize,
}

/// Parse Kindle's "My Clippings.txt" file format
///
/// Format:
/// ```
/// Book Title (Author Name)
/// - Your Highlight on Location 123-125 | Added on Monday, January 1, 2024 12:00:00 AM
///
/// Highlight text here
/// ==========
/// ```
pub fn parse_clippings(content: &str) -> Result<KindleSyncResponse> {
    let entries: Vec<&str> = content.split("==========").collect();
    let mut highlights: Vec<KindleHighlight> = Vec::new();

    // Regex patterns
    let title_author_re = Regex::new(r"^(.+?)\s*\(([^)]+)\)\s*$")?;
    let location_re = Regex::new(r"(?i)location\s+(\d+(?:-\d+)?)")?;
    let page_re = Regex::new(r"(?i)page\s+(\d+)")?;
    let date_re = Regex::new(r"Added on (.+)$")?;
    let note_marker_re = Regex::new(r"(?i)Your Note")?;
    let highlight_marker_re = Regex::new(r"(?i)Your Highlight")?;
    let bookmark_marker_re = Regex::new(r"(?i)Your Bookmark")?;

    for entry in entries {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        let lines: Vec<&str> = entry.lines().collect();
        if lines.len() < 3 {
            continue;
        }

        // First line: Book Title (Author)
        let title_line = lines[0].trim();
        let (book_title, book_author) = if let Some(caps) = title_author_re.captures(title_line) {
            (
                caps.get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default(),
                Some(
                    caps.get(2)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                ),
            )
        } else {
            (title_line.to_string(), None)
        };

        // Second line: Metadata (location, page, date, type)
        let meta_line = lines[1].trim();

        // Skip bookmarks
        if bookmark_marker_re.is_match(meta_line) {
            continue;
        }

        let is_note = note_marker_re.is_match(meta_line);
        let is_highlight = highlight_marker_re.is_match(meta_line);

        if !is_note && !is_highlight {
            continue;
        }

        let location = location_re
            .captures(meta_line)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

        let page = page_re
            .captures(meta_line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok());

        let highlighted_at = date_re
            .captures(meta_line)
            .and_then(|c| c.get(1))
            .map(|m| parse_kindle_date(m.as_str()));

        // Remaining lines: content (skip empty lines after metadata)
        let content_lines: Vec<&str> = lines[2..]
            .iter()
            .skip_while(|l| l.trim().is_empty())
            .copied()
            .collect();
        let text = content_lines.join("\n").trim().to_string();

        if text.is_empty() {
            continue;
        }

        // Generate deterministic ID
        let id = generate_highlight_id(&book_title, &text, location.as_deref());

        if is_note {
            // Notes are attached to the previous highlight with same location
            if let Some(last) = highlights
                .iter_mut()
                .rev()
                .find(|h| h.book_title == book_title && h.location == location)
            {
                last.note = Some(text);
            }
        } else {
            highlights.push(KindleHighlight {
                id,
                book_title,
                book_author,
                text,
                note: None,
                location,
                page,
                highlighted_at,
            });
        }
    }

    // Group by book
    let mut books_map: HashMap<String, KindleBook> = HashMap::new();

    for highlight in highlights {
        let key = format!(
            "{}|{}",
            highlight.book_title,
            highlight.book_author.as_deref().unwrap_or("")
        );

        books_map
            .entry(key)
            .or_insert_with(|| KindleBook {
                title: highlight.book_title.clone(),
                author: highlight.book_author.clone(),
                highlights: Vec::new(),
                highlight_count: 0,
            })
            .highlights
            .push(highlight);
    }

    let mut books: Vec<KindleBook> = books_map
        .into_values()
        .map(|mut b| {
            b.highlight_count = b.highlights.len();
            b
        })
        .collect();

    // Sort by title
    books.sort_by(|a, b| a.title.cmp(&b.title));

    let total_highlights: usize = books.iter().map(|b| b.highlight_count).sum();
    let total_books = books.len();

    Ok(KindleSyncResponse {
        books,
        total_highlights,
        total_books,
    })
}

/// Parse a file path to My Clippings.txt
pub async fn sync_from_file(path: &Path) -> Result<KindleSyncResponse> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context("Failed to read My Clippings.txt")?;

    parse_clippings(&content)
}

/// Parse Kindle date format
/// "Monday, January 1, 2024 12:00:00 AM" -> ISO 8601
fn parse_kindle_date(date_str: &str) -> String {
    // Try common Kindle date formats
    let formats = [
        "%A, %B %d, %Y %I:%M:%S %p",
        "%A, %B %d, %Y %H:%M:%S",
        "%d %B %Y %H:%M:%S",
    ];

    for fmt in formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(date_str.trim(), fmt) {
            let utc: DateTime<Utc> = DateTime::from_naive_utc_and_offset(dt, Utc);
            return utc.to_rfc3339();
        }
    }

    // Return original if parsing fails
    date_str.to_string()
}

/// Generate a deterministic ID for a highlight
fn generate_highlight_id(book_title: &str, text: &str, location: Option<&str>) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    book_title.hash(&mut hasher);
    text.hash(&mut hasher);
    if let Some(loc) = location {
        loc.hash(&mut hasher);
    }

    format!("kindle_{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CLIPPINGS: &str = r#"The Pragmatic Programmer (David Thomas and Andrew Hunt)
- Your Highlight on Location 123-125 | Added on Monday, January 15, 2024 10:30:00 AM

Care About Your Craft. Why spend your life developing software unless you care about doing it well?
==========
The Pragmatic Programmer (David Thomas and Andrew Hunt)
- Your Note on Location 123-125 | Added on Monday, January 15, 2024 10:31:00 AM

This is so important!
==========
Atomic Habits (James Clear)
- Your Highlight on page 23 | Location 456-458 | Added on Tuesday, January 16, 2024 2:00:00 PM

Habits are the compound interest of self-improvement.
==========
Atomic Habits (James Clear)
- Your Bookmark on page 50 | Added on Tuesday, January 16, 2024 3:00:00 PM

==========
"#;

    #[test]
    fn test_parse_clippings() {
        let result = parse_clippings(SAMPLE_CLIPPINGS).unwrap();

        assert_eq!(result.total_books, 2);
        assert_eq!(result.total_highlights, 2);

        let pragmatic = result
            .books
            .iter()
            .find(|b| b.title.contains("Pragmatic"))
            .unwrap();
        assert_eq!(pragmatic.highlights.len(), 1);
        assert_eq!(
            pragmatic.highlights[0].note,
            Some("This is so important!".to_string())
        );

        let atomic = result
            .books
            .iter()
            .find(|b| b.title.contains("Atomic"))
            .unwrap();
        assert_eq!(atomic.highlights.len(), 1);
        assert_eq!(atomic.highlights[0].page, Some(23));
    }

    #[test]
    fn test_parse_title_author() {
        let content = r#"Clean Code (Robert C. Martin)
- Your Highlight on Location 100 | Added on Monday, January 1, 2024 12:00:00 AM

Test highlight
==========
"#;
        let result = parse_clippings(content).unwrap();
        assert_eq!(result.books[0].title, "Clean Code");
        assert_eq!(result.books[0].author, Some("Robert C. Martin".to_string()));
    }

    #[test]
    fn test_empty_clippings() {
        let result = parse_clippings("").unwrap();
        assert_eq!(result.total_books, 0);
        assert_eq!(result.total_highlights, 0);
    }
}
