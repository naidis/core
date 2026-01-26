use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiSearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiItem {
    pub emoji: String,
    pub name: String,
    pub shortcode: String,
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiSearchResponse {
    pub emojis: Vec<EmojiItem>,
    pub total: usize,
}

pub fn search_emoji(request: &EmojiSearchRequest) -> Result<EmojiSearchResponse> {
    let query = request.query.to_lowercase();
    let limit = request.limit.unwrap_or(20);

    let mut results: Vec<EmojiItem> = Vec::new();

    for emoji in emojis::iter() {
        let name = emoji.name().to_lowercase();
        let shortcode = emoji.shortcode().unwrap_or("").to_lowercase();

        if name.contains(&query) || shortcode.contains(&query) {
            results.push(EmojiItem {
                emoji: emoji.as_str().to_string(),
                name: emoji.name().to_string(),
                shortcode: emoji.shortcode().unwrap_or("").to_string(),
                group: format!("{:?}", emoji.group()),
            });

            if results.len() >= limit {
                break;
            }
        }
    }

    let total = results.len();
    Ok(EmojiSearchResponse {
        emojis: results,
        total,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiByShortcodeRequest {
    pub shortcode: String,
}

pub fn get_emoji_by_shortcode(request: &EmojiByShortcodeRequest) -> Result<Option<EmojiItem>> {
    let shortcode = request.shortcode.trim_matches(':').to_lowercase();

    for emoji in emojis::iter() {
        if let Some(sc) = emoji.shortcode() {
            if sc.to_lowercase() == shortcode {
                return Ok(Some(EmojiItem {
                    emoji: emoji.as_str().to_string(),
                    name: emoji.name().to_string(),
                    shortcode: sc.to_string(),
                    group: format!("{:?}", emoji.group()),
                }));
            }
        }
    }

    Ok(None)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiGroupRequest {
    pub group: String,
    pub limit: Option<usize>,
}

pub fn get_emoji_by_group(request: &EmojiGroupRequest) -> Result<EmojiSearchResponse> {
    let group_lower = request.group.to_lowercase();
    let limit = request.limit.unwrap_or(50);

    let mut results: Vec<EmojiItem> = Vec::new();

    for emoji in emojis::iter() {
        let emoji_group = format!("{:?}", emoji.group()).to_lowercase();

        if emoji_group.contains(&group_lower) {
            results.push(EmojiItem {
                emoji: emoji.as_str().to_string(),
                name: emoji.name().to_string(),
                shortcode: emoji.shortcode().unwrap_or("").to_string(),
                group: format!("{:?}", emoji.group()),
            });

            if results.len() >= limit {
                break;
            }
        }
    }

    let total = results.len();
    Ok(EmojiSearchResponse {
        emojis: results,
        total,
    })
}

pub fn list_emoji_groups() -> Vec<String> {
    vec![
        "SmileysAndEmotion".to_string(),
        "PeopleAndBody".to_string(),
        "AnimalsAndNature".to_string(),
        "FoodAndDrink".to_string(),
        "TravelAndPlaces".to_string(),
        "Activities".to_string(),
        "Objects".to_string(),
        "Symbols".to_string(),
        "Flags".to_string(),
    ]
}
