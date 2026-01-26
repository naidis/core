use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EpubError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("EPUB parsing error: {0}")]
    Parse(String),
    #[error("File not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpubMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub published_date: Option<String>,
    pub isbn: Option<String>,
    pub subjects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpubChapter {
    pub index: usize,
    pub title: String,
    pub content: String,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpubContent {
    pub metadata: EpubMetadata,
    pub chapters: Vec<EpubChapter>,
    pub total_word_count: usize,
    pub reading_time_minutes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseEpubRequest {
    pub path: String,
    pub include_content: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpubToMarkdownRequest {
    pub path: String,
    pub include_metadata: bool,
    pub chapter_heading_level: Option<u8>,
}

fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut last_was_space = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => {
                if c.is_whitespace() {
                    if !last_was_space {
                        result.push(' ');
                        last_was_space = true;
                    }
                } else {
                    result.push(c);
                    last_was_space = false;
                }
            }
            _ => {}
        }
    }

    result.trim().to_string()
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn calculate_reading_time(word_count: usize) -> usize {
    (word_count as f32 / 200.0).ceil() as usize
}

pub fn parse_epub(req: ParseEpubRequest) -> Result<EpubContent, EpubError> {
    let path = Path::new(&req.path);
    if !path.exists() {
        return Err(EpubError::NotFound(req.path.clone()));
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut doc =
        epub::doc::EpubDoc::from_reader(reader).map_err(|e| EpubError::Parse(e.to_string()))?;

    let metadata = EpubMetadata {
        title: doc.mdata("title").map(|m| m.value.clone()),
        author: doc.mdata("creator").map(|m| m.value.clone()),
        language: doc.mdata("language").map(|m| m.value.clone()),
        publisher: doc.mdata("publisher").map(|m| m.value.clone()),
        description: doc.mdata("description").map(|m| m.value.clone()),
        published_date: doc.mdata("date").map(|m| m.value.clone()),
        isbn: doc.mdata("identifier").map(|m| m.value.clone()),
        subjects: doc
            .mdata("subject")
            .map(|m| m.value.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default(),
    };

    let mut chapters = Vec::new();
    let mut total_word_count = 0;
    let mut index = 0;

    let spine_len = doc.spine.len();
    for _ in 0..spine_len {
        if let Some((content, _)) = doc.get_current_str() {
            let text = strip_html(&content);
            let word_count = count_words(&text);
            total_word_count += word_count;

            let title = format!("Chapter {}", index + 1);

            chapters.push(EpubChapter {
                index,
                title,
                content: if req.include_content {
                    text
                } else {
                    String::new()
                },
                word_count,
            });

            index += 1;
        }
        doc.go_next();
    }

    Ok(EpubContent {
        metadata,
        chapters,
        total_word_count,
        reading_time_minutes: calculate_reading_time(total_word_count),
    })
}

pub fn epub_to_markdown(req: EpubToMarkdownRequest) -> Result<String, EpubError> {
    let epub = parse_epub(ParseEpubRequest {
        path: req.path,
        include_content: true,
    })?;

    let mut output = String::new();
    let heading_level = req.chapter_heading_level.unwrap_or(2);

    if req.include_metadata {
        if let Some(ref title) = epub.metadata.title {
            output.push_str(&format!("# {}\n\n", title));
        }

        let mut meta_lines = Vec::new();
        if let Some(ref author) = epub.metadata.author {
            meta_lines.push(format!("- **Author**: {}", author));
        }
        if let Some(ref publisher) = epub.metadata.publisher {
            meta_lines.push(format!("- **Publisher**: {}", publisher));
        }
        if let Some(ref date) = epub.metadata.published_date {
            meta_lines.push(format!("- **Published**: {}", date));
        }
        if let Some(ref language) = epub.metadata.language {
            meta_lines.push(format!("- **Language**: {}", language));
        }
        meta_lines.push(format!("- **Word Count**: {}", epub.total_word_count));
        meta_lines.push(format!(
            "- **Reading Time**: {} min",
            epub.reading_time_minutes
        ));

        if !meta_lines.is_empty() {
            output.push_str(&meta_lines.join("\n"));
            output.push_str("\n\n");
        }

        if let Some(ref description) = epub.metadata.description {
            output.push_str(&format!("> {}\n\n", description));
        }

        if !epub.metadata.subjects.is_empty() {
            let tags: Vec<String> = epub
                .metadata
                .subjects
                .iter()
                .map(|s| format!("#{}", s.replace(' ', "-")))
                .collect();
            output.push_str(&format!("{}\n\n", tags.join(" ")));
        }

        output.push_str("---\n\n");
    }

    for chapter in &epub.chapters {
        let heading = "#".repeat(heading_level as usize);
        output.push_str(&format!("{} {}\n\n", heading, chapter.title));
        output.push_str(&chapter.content);
        output.push_str("\n\n");
    }

    Ok(output)
}

pub fn get_epub_metadata(path: &str) -> Result<EpubMetadata, EpubError> {
    let epub = parse_epub(ParseEpubRequest {
        path: path.to_string(),
        include_content: false,
    })?;
    Ok(epub.metadata)
}

pub fn get_epub_toc(path: &str) -> Result<Vec<(usize, String)>, EpubError> {
    let epub = parse_epub(ParseEpubRequest {
        path: path.to_string(),
        include_content: false,
    })?;

    Ok(epub
        .chapters
        .into_iter()
        .map(|c| (c.index, c.title))
        .collect())
}

pub fn get_epub_chapter(path: &str, chapter_index: usize) -> Result<EpubChapter, EpubError> {
    let epub = parse_epub(ParseEpubRequest {
        path: path.to_string(),
        include_content: true,
    })?;

    epub.chapters
        .into_iter()
        .find(|c| c.index == chapter_index)
        .ok_or_else(|| EpubError::Parse(format!("Chapter {} not found", chapter_index)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_simple() {
        let html = "<p>Hello World</p>";
        assert_eq!(strip_html(html), "Hello World");
    }

    #[test]
    fn test_strip_html_nested() {
        let html = "<div><p>Hello <strong>World</strong></p></div>";
        assert_eq!(strip_html(html), "Hello World");
    }

    #[test]
    fn test_strip_html_whitespace_normalization() {
        let html = "<p>Hello    World</p>";
        assert_eq!(strip_html(html), "Hello World");
    }

    #[test]
    fn test_strip_html_multiple_paragraphs() {
        let html = "<p>First</p>   <p>Second</p>";
        assert_eq!(strip_html(html), "First Second");
    }

    #[test]
    fn test_strip_html_empty() {
        assert_eq!(strip_html(""), "");
        assert_eq!(strip_html("<div></div>"), "");
    }

    #[test]
    fn test_strip_html_no_tags() {
        assert_eq!(strip_html("Plain text"), "Plain text");
    }

    #[test]
    fn test_strip_html_special_chars() {
        let html = "<p>&lt;script&gt;alert('xss')&lt;/script&gt;</p>";
        assert_eq!(
            strip_html(html),
            "&lt;script&gt;alert('xss')&lt;/script&gt;"
        );
    }

    #[test]
    fn test_count_words_basic() {
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("one two three four five"), 5);
    }

    #[test]
    fn test_count_words_empty() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("   "), 0);
    }

    #[test]
    fn test_count_words_multiple_spaces() {
        assert_eq!(count_words("hello    world"), 2);
        assert_eq!(count_words("  hello   world  "), 2);
    }

    #[test]
    fn test_count_words_newlines() {
        assert_eq!(count_words("hello\nworld"), 2);
        assert_eq!(count_words("hello\n\n\nworld"), 2);
    }

    #[test]
    fn test_calculate_reading_time_zero() {
        assert_eq!(calculate_reading_time(0), 0);
    }

    #[test]
    fn test_calculate_reading_time_under_one_minute() {
        assert_eq!(calculate_reading_time(100), 1);
        assert_eq!(calculate_reading_time(1), 1);
    }

    #[test]
    fn test_calculate_reading_time_exact_minutes() {
        assert_eq!(calculate_reading_time(200), 1);
        assert_eq!(calculate_reading_time(400), 2);
        assert_eq!(calculate_reading_time(1000), 5);
    }

    #[test]
    fn test_calculate_reading_time_rounds_up() {
        assert_eq!(calculate_reading_time(201), 2);
        assert_eq!(calculate_reading_time(401), 3);
    }

    #[test]
    fn test_parse_epub_file_not_found() {
        let result = parse_epub(ParseEpubRequest {
            path: "/nonexistent/path/book.epub".to_string(),
            include_content: false,
        });

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EpubError::NotFound(_)));
    }

    #[test]
    fn test_epub_metadata_serialization() {
        let metadata = EpubMetadata {
            title: Some("Test Book".to_string()),
            author: Some("Test Author".to_string()),
            language: Some("en".to_string()),
            publisher: Some("Test Publisher".to_string()),
            description: Some("A test book".to_string()),
            published_date: Some("2024-01-01".to_string()),
            isbn: Some("978-0-00-000000-0".to_string()),
            subjects: vec!["Fiction".to_string(), "Test".to_string()],
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: EpubMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.title, Some("Test Book".to_string()));
        assert_eq!(deserialized.author, Some("Test Author".to_string()));
    }

    #[test]
    fn test_epub_chapter_serialization() {
        let chapter = EpubChapter {
            index: 0,
            title: "Chapter 1".to_string(),
            content: "Chapter content".to_string(),
            word_count: 2,
        };

        let json = serde_json::to_string(&chapter).unwrap();
        let deserialized: EpubChapter = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.index, 0);
        assert_eq!(deserialized.title, "Chapter 1");
    }

    #[test]
    fn test_epub_content_serialization() {
        let content = EpubContent {
            metadata: EpubMetadata {
                title: Some("Book".to_string()),
                author: None,
                language: None,
                publisher: None,
                description: None,
                published_date: None,
                isbn: None,
                subjects: vec![],
            },
            chapters: vec![],
            total_word_count: 1000,
            reading_time_minutes: 5,
        };

        let json = serde_json::to_string(&content).unwrap();
        let deserialized: EpubContent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.total_word_count, 1000);
        assert_eq!(deserialized.reading_time_minutes, 5);
    }
}
