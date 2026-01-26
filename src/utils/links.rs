use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMetadata {
    pub path: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkSuggestRequest {
    pub text: String,
    pub notes: Vec<NoteMetadata>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkSuggestion {
    pub path: String,
    pub title: String,
    pub match_type: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkSuggestResponse {
    pub suggestions: Vec<LinkSuggestion>,
    pub total: usize,
}

pub fn suggest_links(request: &LinkSuggestRequest) -> Result<LinkSuggestResponse> {
    let text_lower = request.text.to_lowercase();
    let limit = request.limit.unwrap_or(10);

    let mut suggestions: Vec<LinkSuggestion> = Vec::new();

    for note in &request.notes {
        let title_lower = note.title.to_lowercase();

        if title_lower == text_lower {
            suggestions.push(LinkSuggestion {
                path: note.path.clone(),
                title: note.title.clone(),
                match_type: "exact".to_string(),
                score: 1.0,
            });
            continue;
        }

        if title_lower.starts_with(&text_lower) {
            let score = text_lower.len() as f32 / title_lower.len() as f32;
            suggestions.push(LinkSuggestion {
                path: note.path.clone(),
                title: note.title.clone(),
                match_type: "prefix".to_string(),
                score: score * 0.9,
            });
            continue;
        }

        if title_lower.contains(&text_lower) {
            let score = text_lower.len() as f32 / title_lower.len() as f32;
            suggestions.push(LinkSuggestion {
                path: note.path.clone(),
                title: note.title.clone(),
                match_type: "contains".to_string(),
                score: score * 0.7,
            });
            continue;
        }

        for alias in &note.aliases {
            let alias_lower = alias.to_lowercase();
            if alias_lower.contains(&text_lower) {
                let score = text_lower.len() as f32 / alias_lower.len() as f32;
                suggestions.push(LinkSuggestion {
                    path: note.path.clone(),
                    title: note.title.clone(),
                    match_type: "alias".to_string(),
                    score: score * 0.6,
                });
                break;
            }
        }

        let fuzzy_score = fuzzy_match(&text_lower, &title_lower);
        if fuzzy_score > 0.5 {
            suggestions.push(LinkSuggestion {
                path: note.path.clone(),
                title: note.title.clone(),
                match_type: "fuzzy".to_string(),
                score: fuzzy_score * 0.5,
            });
        }
    }

    suggestions.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions.truncate(limit);

    let total = suggestions.len();
    Ok(LinkSuggestResponse { suggestions, total })
}

fn fuzzy_match(query: &str, target: &str) -> f32 {
    if query.is_empty() || target.is_empty() {
        return 0.0;
    }

    let query_chars: Vec<char> = query.chars().collect();
    let target_chars: Vec<char> = target.chars().collect();

    let mut matched = 0;
    let mut target_idx = 0;

    for qc in &query_chars {
        while target_idx < target_chars.len() {
            if target_chars[target_idx] == *qc {
                matched += 1;
                target_idx += 1;
                break;
            }
            target_idx += 1;
        }
    }

    matched as f32 / query_chars.len() as f32
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklinkRequest {
    pub note_path: String,
    pub all_notes: Vec<NoteContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteContent {
    pub path: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklinkResult {
    pub path: String,
    pub title: String,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklinkResponse {
    pub backlinks: Vec<BacklinkResult>,
    pub total: usize,
}

pub fn find_backlinks(request: &BacklinkRequest) -> Result<BacklinkResponse> {
    let target_path = &request.note_path;
    let target_name = std::path::Path::new(target_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let link_patterns = vec![
        format!("[[{}]]", target_name),
        format!("[[{}|", target_name),
        format!("]({})", target_path),
    ];

    let mut backlinks: Vec<BacklinkResult> = Vec::new();

    for note in &request.all_notes {
        if note.path == *target_path {
            continue;
        }

        for pattern in &link_patterns {
            if let Some(pos) = note.content.find(pattern) {
                let start = pos.saturating_sub(50);
                let end = (pos + pattern.len() + 50).min(note.content.len());
                let context = note.content[start..end].to_string();

                backlinks.push(BacklinkResult {
                    path: note.path.clone(),
                    title: note.title.clone(),
                    context,
                });
                break;
            }
        }
    }

    let total = backlinks.len();
    Ok(BacklinkResponse { backlinks, total })
}
