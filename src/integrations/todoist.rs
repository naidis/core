use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const TODOIST_API_BASE: &str = "https://api.todoist.com/rest/v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoistConfig {
    pub api_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoistTask {
    pub id: String,
    pub content: String,
    pub description: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub section_id: Option<String>,
    pub parent_id: Option<String>,
    pub priority: i32,
    pub due: Option<TodoistDue>,
    pub labels: Vec<String>,
    pub is_completed: bool,
    pub created_at: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoistDue {
    pub date: String,
    pub string: Option<String>,
    pub datetime: Option<String>,
    pub timezone: Option<String>,
    pub is_recurring: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoistProject {
    pub id: String,
    pub name: String,
    pub color: String,
    pub parent_id: Option<String>,
    pub order: i32,
    pub is_favorite: bool,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiTask {
    pub id: String,
    pub content: String,
    pub description: String,
    pub project_id: String,
    pub section_id: Option<String>,
    pub parent_id: Option<String>,
    pub priority: i32,
    pub due: Option<ApiDue>,
    pub labels: Vec<String>,
    pub is_completed: bool,
    pub created_at: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiDue {
    pub date: String,
    pub string: Option<String>,
    pub datetime: Option<String>,
    pub timezone: Option<String>,
    pub is_recurring: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiProject {
    pub id: String,
    pub name: String,
    pub color: String,
    pub parent_id: Option<String>,
    pub order: i32,
    pub is_favorite: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTasksRequest {
    pub config: TodoistConfig,
    pub project_id: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTasksResponse {
    pub tasks: Vec<TodoistTask>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchProjectsRequest {
    pub config: TodoistConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchProjectsResponse {
    pub projects: Vec<TodoistProject>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub config: TodoistConfig,
    pub content: String,
    pub description: Option<String>,
    pub project_id: Option<String>,
    pub due_string: Option<String>,
    pub due_date: Option<String>,
    pub priority: Option<i32>,
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteTaskRequest {
    pub config: TodoistConfig,
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncToObsidianRequest {
    pub config: TodoistConfig,
    pub vault_path: String,
    pub target_folder: String,
    pub project_id: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncToObsidianResponse {
    pub tasks_synced: usize,
    pub file_path: String,
    pub content: String,
}

pub async fn fetch_tasks(request: &FetchTasksRequest) -> Result<FetchTasksResponse> {
    let client = Client::new();

    let mut url = format!("{}/tasks", TODOIST_API_BASE);
    let mut query_params = vec![];

    if let Some(ref project_id) = request.project_id {
        query_params.push(format!("project_id={}", project_id));
    }
    if let Some(ref filter) = request.filter {
        query_params.push(format!("filter={}", urlencoding::encode(filter)));
    }

    if !query_params.is_empty() {
        url = format!("{}?{}", url, query_params.join("&"));
    }

    let response = client
        .get(&url)
        .header(
            "Authorization",
            format!("Bearer {}", request.config.api_token),
        )
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Todoist API error: {} - {}", status, body);
    }

    let api_tasks: Vec<ApiTask> = response.json().await?;

    let projects = fetch_projects_internal(&request.config).await.ok();
    let project_map: std::collections::HashMap<String, String> = projects
        .map(|p| {
            p.into_iter()
                .map(|proj| (proj.id.clone(), proj.name.clone()))
                .collect()
        })
        .unwrap_or_default();

    let tasks: Vec<TodoistTask> = api_tasks
        .into_iter()
        .map(|t| TodoistTask {
            id: t.id,
            content: t.content,
            description: if t.description.is_empty() {
                None
            } else {
                Some(t.description)
            },
            project_id: Some(t.project_id.clone()),
            project_name: project_map.get(&t.project_id).cloned(),
            section_id: t.section_id,
            parent_id: t.parent_id,
            priority: t.priority,
            due: t.due.map(|d| TodoistDue {
                date: d.date,
                string: d.string,
                datetime: d.datetime,
                timezone: d.timezone,
                is_recurring: d.is_recurring,
            }),
            labels: t.labels,
            is_completed: t.is_completed,
            created_at: t.created_at,
            url: t.url,
        })
        .collect();

    let total = tasks.len();
    Ok(FetchTasksResponse { tasks, total })
}

async fn fetch_projects_internal(config: &TodoistConfig) -> Result<Vec<TodoistProject>> {
    let client = Client::new();

    let response = client
        .get(format!("{}/projects", TODOIST_API_BASE))
        .header("Authorization", format!("Bearer {}", config.api_token))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch projects");
    }

    let api_projects: Vec<ApiProject> = response.json().await?;

    Ok(api_projects
        .into_iter()
        .map(|p| TodoistProject {
            id: p.id,
            name: p.name,
            color: p.color,
            parent_id: p.parent_id,
            order: p.order,
            is_favorite: p.is_favorite,
            url: p.url,
        })
        .collect())
}

pub async fn fetch_projects(request: &FetchProjectsRequest) -> Result<FetchProjectsResponse> {
    let projects = fetch_projects_internal(&request.config).await?;
    let total = projects.len();
    Ok(FetchProjectsResponse { projects, total })
}

pub async fn create_task(request: &CreateTaskRequest) -> Result<TodoistTask> {
    let client = Client::new();

    let mut body = serde_json::json!({
        "content": request.content
    });

    if let Some(ref desc) = request.description {
        body["description"] = serde_json::json!(desc);
    }
    if let Some(ref project_id) = request.project_id {
        body["project_id"] = serde_json::json!(project_id);
    }
    if let Some(ref due_string) = request.due_string {
        body["due_string"] = serde_json::json!(due_string);
    }
    if let Some(ref due_date) = request.due_date {
        body["due_date"] = serde_json::json!(due_date);
    }
    if let Some(priority) = request.priority {
        body["priority"] = serde_json::json!(priority);
    }
    if let Some(ref labels) = request.labels {
        body["labels"] = serde_json::json!(labels);
    }

    let response = client
        .post(format!("{}/tasks", TODOIST_API_BASE))
        .header(
            "Authorization",
            format!("Bearer {}", request.config.api_token),
        )
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to create task: {} - {}", status, body);
    }

    let api_task: ApiTask = response.json().await?;

    Ok(TodoistTask {
        id: api_task.id,
        content: api_task.content,
        description: if api_task.description.is_empty() {
            None
        } else {
            Some(api_task.description)
        },
        project_id: Some(api_task.project_id),
        project_name: None,
        section_id: api_task.section_id,
        parent_id: api_task.parent_id,
        priority: api_task.priority,
        due: api_task.due.map(|d| TodoistDue {
            date: d.date,
            string: d.string,
            datetime: d.datetime,
            timezone: d.timezone,
            is_recurring: d.is_recurring,
        }),
        labels: api_task.labels,
        is_completed: api_task.is_completed,
        created_at: api_task.created_at,
        url: api_task.url,
    })
}

pub async fn complete_task(request: &CompleteTaskRequest) -> Result<bool> {
    let client = Client::new();

    let response = client
        .post(format!(
            "{}/tasks/{}/close",
            TODOIST_API_BASE, request.task_id
        ))
        .header(
            "Authorization",
            format!("Bearer {}", request.config.api_token),
        )
        .send()
        .await?;

    Ok(response.status().is_success())
}

pub async fn sync_to_obsidian(request: &SyncToObsidianRequest) -> Result<SyncToObsidianResponse> {
    let fetch_req = FetchTasksRequest {
        config: request.config.clone(),
        project_id: request.project_id.clone(),
        filter: None,
    };

    let tasks_response = fetch_tasks(&fetch_req).await?;
    let format = request.format.as_deref().unwrap_or("tasks");

    let content = match format {
        "table" => format_tasks_as_table(&tasks_response.tasks),
        "list" => format_tasks_as_list(&tasks_response.tasks),
        _ => format_tasks_as_tasks(&tasks_response.tasks),
    };

    let file_path = format!(
        "{}/{}/todoist-sync.md",
        request.vault_path, request.target_folder
    );

    Ok(SyncToObsidianResponse {
        tasks_synced: tasks_response.total,
        file_path,
        content,
    })
}

fn format_tasks_as_tasks(tasks: &[TodoistTask]) -> String {
    let mut output = String::from("# Todoist Tasks\n\n");

    for task in tasks {
        let checkbox = if task.is_completed { "[x]" } else { "[ ]" };
        let priority = match task.priority {
            4 => "🔺 ",
            3 => "⏫ ",
            2 => "🔼 ",
            _ => "",
        };

        let due = task
            .due
            .as_ref()
            .map(|d| format!(" 📅 {}", d.date))
            .unwrap_or_default();

        let labels = if task.labels.is_empty() {
            String::new()
        } else {
            format!(
                " {}",
                task.labels
                    .iter()
                    .map(|l| format!("#{}", l))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        let project = task
            .project_name
            .as_ref()
            .map(|p| format!(" ({})", p))
            .unwrap_or_default();

        output.push_str(&format!(
            "- {} {}{}{}{}{}\n",
            checkbox, priority, task.content, due, labels, project
        ));
    }

    output
}

fn format_tasks_as_list(tasks: &[TodoistTask]) -> String {
    let mut output = String::from("# Todoist Tasks\n\n");

    for task in tasks {
        let bullet = if task.is_completed { "✅" } else { "◻️" };
        output.push_str(&format!("- {} {}\n", bullet, task.content));
    }

    output
}

fn format_tasks_as_table(tasks: &[TodoistTask]) -> String {
    let mut output = String::from("# Todoist Tasks\n\n");
    output.push_str("| Status | Task | Due | Project | Priority |\n");
    output.push_str("|:------:|------|-----|---------|:--------:|\n");

    for task in tasks {
        let status = if task.is_completed { "✅" } else { "◻️" };
        let due = task.due.as_ref().map(|d| d.date.as_str()).unwrap_or("-");
        let project = task.project_name.as_deref().unwrap_or("-");
        let priority = match task.priority {
            4 => "🔺",
            3 => "⏫",
            2 => "🔼",
            _ => "-",
        };

        output.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            status, task.content, due, project, priority
        ));
    }

    output
}
