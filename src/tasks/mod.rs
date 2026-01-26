use anyhow::Result;
use chrono::{Local, NaiveDate};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Todo,
    Done,
    Cancelled,
    InProgress,
    Scheduled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub text: String,
    pub status: TaskStatus,
    pub due_date: Option<String>,
    pub scheduled_date: Option<String>,
    pub start_date: Option<String>,
    pub done_date: Option<String>,
    pub priority: Option<String>,
    pub tags: Vec<String>,
    pub file_path: String,
    pub line_number: usize,
    pub raw_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskParseRequest {
    pub content: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskParseResponse {
    pub tasks: Vec<Task>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskQueryRequest {
    pub tasks: Vec<Task>,
    pub filter_status: Option<Vec<String>>,
    pub filter_due_before: Option<String>,
    pub filter_due_after: Option<String>,
    pub filter_tags: Option<Vec<String>>,
    pub filter_priority: Option<Vec<String>>,
    pub filter_path: Option<String>,
    pub sort_by: Option<String>,
    pub sort_desc: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskQueryResponse {
    pub tasks: Vec<Task>,
    pub total: usize,
}

pub fn parse_tasks(request: &TaskParseRequest) -> Result<TaskParseResponse> {
    let checkbox_re = Regex::new(r"^(\s*)-\s*\[([ xX\-/>])\]\s*(.*)$")?;
    let due_re = Regex::new(r"📅\s*(\d{4}-\d{2}-\d{2})")?;
    let scheduled_re = Regex::new(r"⏳\s*(\d{4}-\d{2}-\d{2})")?;
    let start_re = Regex::new(r"🛫\s*(\d{4}-\d{2}-\d{2})")?;
    let done_re = Regex::new(r"✅\s*(\d{4}-\d{2}-\d{2})")?;
    let priority_re = Regex::new(r"([🔺⏫🔼🔽⏬]|!!![!]*|[A-C])")?;
    let tag_re = Regex::new(r"#(\w+)")?;

    let mut tasks = Vec::new();

    for (line_num, line) in request.content.lines().enumerate() {
        if let Some(caps) = checkbox_re.captures(line) {
            let status_char = caps.get(2).map(|m| m.as_str()).unwrap_or(" ");
            let text = caps.get(3).map(|m| m.as_str()).unwrap_or("").to_string();

            let status = match status_char {
                "x" | "X" => TaskStatus::Done,
                "-" => TaskStatus::Cancelled,
                "/" => TaskStatus::InProgress,
                ">" => TaskStatus::Scheduled,
                _ => TaskStatus::Todo,
            };

            let due_date = due_re.captures(&text).map(|c| c[1].to_string());
            let scheduled_date = scheduled_re.captures(&text).map(|c| c[1].to_string());
            let start_date = start_re.captures(&text).map(|c| c[1].to_string());
            let done_date = done_re.captures(&text).map(|c| c[1].to_string());

            let priority = priority_re.captures(&text).map(|c| c[1].to_string());

            let tags: Vec<String> = tag_re
                .captures_iter(&text)
                .map(|c| c[1].to_string())
                .collect();

            let clean_text = text
                .replace(&format!("📅 {}", due_date.as_deref().unwrap_or("")), "")
                .replace(
                    &format!("⏳ {}", scheduled_date.as_deref().unwrap_or("")),
                    "",
                )
                .replace(&format!("🛫 {}", start_date.as_deref().unwrap_or("")), "")
                .replace(&format!("✅ {}", done_date.as_deref().unwrap_or("")), "")
                .trim()
                .to_string();

            tasks.push(Task {
                id: format!("{}:{}", request.file_path, line_num + 1),
                text: clean_text,
                status,
                due_date,
                scheduled_date,
                start_date,
                done_date,
                priority,
                tags,
                file_path: request.file_path.clone(),
                line_number: line_num + 1,
                raw_line: line.to_string(),
            });
        }
    }

    let total = tasks.len();
    Ok(TaskParseResponse { tasks, total })
}

pub fn query_tasks(request: &TaskQueryRequest) -> Result<TaskQueryResponse> {
    let mut tasks = request.tasks.clone();

    if let Some(ref statuses) = request.filter_status {
        tasks.retain(|t| {
            let status_str = match t.status {
                TaskStatus::Todo => "todo",
                TaskStatus::Done => "done",
                TaskStatus::Cancelled => "cancelled",
                TaskStatus::InProgress => "in_progress",
                TaskStatus::Scheduled => "scheduled",
            };
            statuses.iter().any(|s| s.to_lowercase() == status_str)
        });
    }

    if let Some(ref due_before) = request.filter_due_before {
        if let Ok(before_date) = NaiveDate::parse_from_str(due_before, "%Y-%m-%d") {
            tasks.retain(|t| {
                t.due_date.as_ref().is_some_and(|d| {
                    NaiveDate::parse_from_str(d, "%Y-%m-%d")
                        .map(|td| td <= before_date)
                        .unwrap_or(false)
                })
            });
        }
    }

    if let Some(ref due_after) = request.filter_due_after {
        if let Ok(after_date) = NaiveDate::parse_from_str(due_after, "%Y-%m-%d") {
            tasks.retain(|t| {
                t.due_date.as_ref().is_some_and(|d| {
                    NaiveDate::parse_from_str(d, "%Y-%m-%d")
                        .map(|td| td >= after_date)
                        .unwrap_or(false)
                })
            });
        }
    }

    if let Some(ref tags) = request.filter_tags {
        tasks.retain(|t| tags.iter().any(|tag| t.tags.contains(tag)));
    }

    if let Some(ref priorities) = request.filter_priority {
        tasks.retain(|t| t.priority.as_ref().is_some_and(|p| priorities.contains(p)));
    }

    if let Some(ref path) = request.filter_path {
        tasks.retain(|t| t.file_path.contains(path));
    }

    if let Some(ref sort_by) = request.sort_by {
        let desc = request.sort_desc.unwrap_or(false);
        match sort_by.as_str() {
            "due" => tasks.sort_by(|a, b| {
                let cmp = a.due_date.cmp(&b.due_date);
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
            "priority" => tasks.sort_by(|a, b| {
                let cmp = a.priority.cmp(&b.priority);
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
            "status" => tasks.sort_by(|a, b| {
                let a_ord = match a.status {
                    TaskStatus::Todo => 0,
                    TaskStatus::InProgress => 1,
                    TaskStatus::Scheduled => 2,
                    TaskStatus::Done => 3,
                    TaskStatus::Cancelled => 4,
                };
                let b_ord = match b.status {
                    TaskStatus::Todo => 0,
                    TaskStatus::InProgress => 1,
                    TaskStatus::Scheduled => 2,
                    TaskStatus::Done => 3,
                    TaskStatus::Cancelled => 4,
                };
                let cmp = a_ord.cmp(&b_ord);
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
            "file" => tasks.sort_by(|a, b| {
                let cmp = a.file_path.cmp(&b.file_path);
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
            _ => {}
        }
    }

    if let Some(limit) = request.limit {
        tasks.truncate(limit);
    }

    let total = tasks.len();
    Ok(TaskQueryResponse { tasks, total })
}

pub fn get_today_tasks(tasks: &[Task]) -> Vec<Task> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    tasks
        .iter()
        .filter(|t| {
            t.status == TaskStatus::Todo
                && (t.due_date.as_ref() == Some(&today)
                    || t.scheduled_date.as_ref() == Some(&today))
        })
        .cloned()
        .collect()
}

pub fn get_overdue_tasks(tasks: &[Task]) -> Vec<Task> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Todo && t.due_date.as_ref().is_some_and(|d| d < &today))
        .cloned()
        .collect()
}

pub fn get_upcoming_tasks(tasks: &[Task], days: i64) -> Vec<Task> {
    let today = Local::now().date_naive();
    let future = today + chrono::Duration::days(days);

    tasks
        .iter()
        .filter(|t| {
            t.status == TaskStatus::Todo
                && t.due_date.as_ref().is_some_and(|d| {
                    NaiveDate::parse_from_str(d, "%Y-%m-%d")
                        .map(|td| td > today && td <= future)
                        .unwrap_or(false)
                })
        })
        .cloned()
        .collect()
}
