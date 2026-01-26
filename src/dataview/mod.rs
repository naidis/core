use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMeta {
    pub path: String,
    pub name: String,
    pub frontmatter: HashMap<String, serde_json::Value>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseNoteRequest {
    pub content: String,
    pub path: String,
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub size: Option<u64>,
}

pub fn parse_note_metadata(request: &ParseNoteRequest) -> Result<NoteMeta> {
    let frontmatter = extract_frontmatter(&request.content)?;
    let tags = extract_tags(&request.content);
    let links = extract_links(&request.content);
    let name = std::path::Path::new(&request.path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    Ok(NoteMeta {
        path: request.path.clone(),
        name,
        frontmatter,
        tags,
        links,
        created: request.created,
        modified: request.modified,
        size: request.size,
    })
}

fn extract_frontmatter(content: &str) -> Result<HashMap<String, serde_json::Value>> {
    let fm_re = Regex::new(r"^---\s*\n([\s\S]*?)\n---")?;

    if let Some(caps) = fm_re.captures(content) {
        let yaml_str = &caps[1];
        let mut map = HashMap::new();

        for line in yaml_str.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim();

                let json_value = if value.starts_with('[') && value.ends_with(']') {
                    let items: Vec<String> = value[1..value.len() - 1]
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .collect();
                    serde_json::Value::Array(
                        items.into_iter().map(serde_json::Value::String).collect(),
                    )
                } else if value == "true" {
                    serde_json::Value::Bool(true)
                } else if value == "false" {
                    serde_json::Value::Bool(false)
                } else if let Ok(n) = value.parse::<i64>() {
                    serde_json::Value::Number(n.into())
                } else if let Ok(n) = value.parse::<f64>() {
                    serde_json::json!(n)
                } else {
                    serde_json::Value::String(
                        value.trim_matches('"').trim_matches('\'').to_string(),
                    )
                };

                map.insert(key, json_value);
            }
        }

        return Ok(map);
    }

    Ok(HashMap::new())
}

fn extract_tags(content: &str) -> Vec<String> {
    let tag_re = Regex::new(r"#([a-zA-Z][a-zA-Z0-9_/-]*)").unwrap();
    tag_re
        .captures_iter(content)
        .map(|c| c[1].to_string())
        .collect()
}

fn extract_links(content: &str) -> Vec<String> {
    let link_re = Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
    link_re
        .captures_iter(content)
        .map(|c| c[1].to_string())
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub notes: Vec<NoteMeta>,
    pub from: Option<String>,
    pub where_clause: Option<String>,
    pub sort_by: Option<String>,
    pub sort_desc: Option<bool>,
    pub limit: Option<usize>,
    pub fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub path: String,
    pub name: String,
    pub fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub results: Vec<QueryResult>,
    pub total: usize,
}

pub fn query_notes(request: &QueryRequest) -> Result<QueryResponse> {
    let mut notes = request.notes.clone();

    if let Some(ref from) = request.from {
        notes.retain(|n| n.path.contains(from) || n.tags.iter().any(|t| t == from));
    }

    if let Some(ref where_clause) = request.where_clause {
        notes.retain(|n| evaluate_where(n, where_clause));
    }

    if let Some(ref sort_by) = request.sort_by {
        let desc = request.sort_desc.unwrap_or(false);
        notes.sort_by(|a, b| {
            let a_val = get_sort_value(a, sort_by);
            let b_val = get_sort_value(b, sort_by);
            let cmp = a_val.cmp(&b_val);
            if desc {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }

    if let Some(limit) = request.limit {
        notes.truncate(limit);
    }

    let results: Vec<QueryResult> = notes
        .iter()
        .map(|n| {
            let mut fields = HashMap::new();

            if let Some(ref requested_fields) = request.fields {
                for field in requested_fields {
                    let value = match field.as_str() {
                        "name" | "file.name" => serde_json::Value::String(n.name.clone()),
                        "path" | "file.path" => serde_json::Value::String(n.path.clone()),
                        "tags" => serde_json::json!(n.tags),
                        "links" | "outlinks" => serde_json::json!(n.links),
                        "created" | "file.ctime" => n
                            .created
                            .map(|c| serde_json::json!(c))
                            .unwrap_or(serde_json::Value::Null),
                        "modified" | "file.mtime" => n
                            .modified
                            .map(|m| serde_json::json!(m))
                            .unwrap_or(serde_json::Value::Null),
                        "size" | "file.size" => n
                            .size
                            .map(|s| serde_json::json!(s))
                            .unwrap_or(serde_json::Value::Null),
                        _ => n
                            .frontmatter
                            .get(field)
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    };
                    fields.insert(field.clone(), value);
                }
            } else {
                fields.insert(
                    "name".to_string(),
                    serde_json::Value::String(n.name.clone()),
                );
                fields.insert(
                    "path".to_string(),
                    serde_json::Value::String(n.path.clone()),
                );
                for (k, v) in &n.frontmatter {
                    fields.insert(k.clone(), v.clone());
                }
            }

            QueryResult {
                path: n.path.clone(),
                name: n.name.clone(),
                fields,
            }
        })
        .collect();

    let total = results.len();
    Ok(QueryResponse { results, total })
}

fn evaluate_where(note: &NoteMeta, clause: &str) -> bool {
    let contains_re = Regex::new(r#"contains\((\w+),\s*"([^"]+)"\)"#).unwrap();
    if let Some(caps) = contains_re.captures(clause) {
        let field = &caps[1];
        let value = &caps[2];

        return match field {
            "tags" => note.tags.iter().any(|t| t.contains(value)),
            "links" => note.links.iter().any(|l| l.contains(value)),
            "path" => note.path.contains(value),
            "name" => note.name.contains(value),
            _ => note
                .frontmatter
                .get(field)
                .and_then(|v| v.as_str())
                .map(|s| s.contains(value))
                .unwrap_or(false),
        };
    }

    let eq_re = Regex::new(r#"(\w+)\s*=\s*"([^"]+)""#).unwrap();
    if let Some(caps) = eq_re.captures(clause) {
        let field = &caps[1];
        let value = &caps[2];

        return match field {
            "name" => note.name == value,
            "path" => note.path == value,
            _ => note
                .frontmatter
                .get(field)
                .and_then(|v| v.as_str())
                .map(|s| s == value)
                .unwrap_or(false),
        };
    }

    let exists_re = Regex::new(r"(\w+)\s*!=\s*null").unwrap();
    if let Some(caps) = exists_re.captures(clause) {
        let field = &caps[1];
        return note.frontmatter.contains_key(field);
    }

    true
}

fn get_sort_value(note: &NoteMeta, sort_by: &str) -> String {
    match sort_by {
        "name" | "file.name" => note.name.clone(),
        "path" | "file.path" => note.path.clone(),
        "created" | "file.ctime" => note.created.map(|c| c.to_string()).unwrap_or_default(),
        "modified" | "file.mtime" => note.modified.map(|m| m.to_string()).unwrap_or_default(),
        _ => note
            .frontmatter
            .get(sort_by)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableQueryRequest {
    pub notes: Vec<NoteMeta>,
    pub from: Option<String>,
    pub where_clause: Option<String>,
    pub columns: Vec<String>,
    pub sort_by: Option<String>,
    pub sort_desc: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRow {
    pub values: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableQueryResponse {
    pub headers: Vec<String>,
    pub rows: Vec<TableRow>,
    pub total: usize,
}

pub fn table_query(request: &TableQueryRequest) -> Result<TableQueryResponse> {
    let query_req = QueryRequest {
        notes: request.notes.clone(),
        from: request.from.clone(),
        where_clause: request.where_clause.clone(),
        sort_by: request.sort_by.clone(),
        sort_desc: request.sort_desc,
        limit: request.limit,
        fields: Some(request.columns.clone()),
    };

    let result = query_notes(&query_req)?;

    let rows: Vec<TableRow> = result
        .results
        .iter()
        .map(|r| {
            let values: Vec<serde_json::Value> = request
                .columns
                .iter()
                .map(|col| {
                    r.fields
                        .get(col)
                        .cloned()
                        .unwrap_or(serde_json::Value::Null)
                })
                .collect();
            TableRow { values }
        })
        .collect();

    Ok(TableQueryResponse {
        headers: request.columns.clone(),
        rows,
        total: result.total,
    })
}

pub fn list_query(request: &QueryRequest) -> Result<Vec<String>> {
    let result = query_notes(request)?;
    Ok(result
        .results
        .iter()
        .map(|r| {
            if let Some(ref fields) = request.fields {
                if let Some(field) = fields.first() {
                    return r
                        .fields
                        .get(field)
                        .and_then(|v| v.as_str())
                        .unwrap_or(&r.name)
                        .to_string();
                }
            }
            r.name.clone()
        })
        .collect())
}
