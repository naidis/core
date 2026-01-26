use anyhow::Result;
use chrono::{Duration, Local};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const GCAL_API_BASE: &str = "https://www.googleapis.com/calendar/v3";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCalConfig {
    pub access_token: String,
    pub calendar_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCalEvent {
    pub id: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: EventDateTime,
    pub end: EventDateTime,
    pub status: String,
    pub html_link: String,
    pub attendees: Vec<Attendee>,
    pub is_all_day: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDateTime {
    pub date: Option<String>,
    pub date_time: Option<String>,
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    pub email: String,
    pub display_name: Option<String>,
    pub response_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiEvent {
    id: String,
    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,
    start: ApiDateTime,
    end: ApiDateTime,
    status: Option<String>,
    #[serde(rename = "htmlLink")]
    html_link: Option<String>,
    attendees: Option<Vec<ApiAttendee>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiDateTime {
    date: Option<String>,
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
    #[serde(rename = "timeZone")]
    time_zone: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiAttendee {
    email: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "responseStatus")]
    response_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct EventsListResponse {
    items: Option<Vec<ApiEvent>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchEventsRequest {
    pub config: GCalConfig,
    pub time_min: Option<String>,
    pub time_max: Option<String>,
    pub max_results: Option<i32>,
    pub single_events: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchEventsResponse {
    pub events: Vec<GCalEvent>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchTodayEventsRequest {
    pub config: GCalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub config: GCalConfig,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_date: Option<String>,
    pub start_datetime: Option<String>,
    pub end_date: Option<String>,
    pub end_datetime: Option<String>,
    pub attendees: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncToObsidianRequest {
    pub config: GCalConfig,
    pub vault_path: String,
    pub target_folder: String,
    pub days_ahead: Option<i32>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncToObsidianResponse {
    pub events_synced: usize,
    pub file_path: String,
    pub content: String,
}

pub async fn fetch_events(request: &FetchEventsRequest) -> Result<FetchEventsResponse> {
    let client = Client::new();
    let calendar_id = request.config.calendar_id.as_deref().unwrap_or("primary");

    let mut url = format!(
        "{}/calendars/{}/events",
        GCAL_API_BASE,
        urlencoding::encode(calendar_id)
    );
    let mut query_params = vec![];

    if let Some(ref time_min) = request.time_min {
        query_params.push(format!("timeMin={}", urlencoding::encode(time_min)));
    }
    if let Some(ref time_max) = request.time_max {
        query_params.push(format!("timeMax={}", urlencoding::encode(time_max)));
    }
    if let Some(max_results) = request.max_results {
        query_params.push(format!("maxResults={}", max_results));
    }
    if request.single_events.unwrap_or(true) {
        query_params.push("singleEvents=true".to_string());
    }
    query_params.push("orderBy=startTime".to_string());

    if !query_params.is_empty() {
        url = format!("{}?{}", url, query_params.join("&"));
    }

    let response = client
        .get(&url)
        .header(
            "Authorization",
            format!("Bearer {}", request.config.access_token),
        )
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Google Calendar API error: {} - {}", status, body);
    }

    let api_response: EventsListResponse = response.json().await?;

    let events: Vec<GCalEvent> = api_response
        .items
        .unwrap_or_default()
        .into_iter()
        .map(|e| {
            let is_all_day = e.start.date.is_some();
            GCalEvent {
                id: e.id,
                summary: e.summary.unwrap_or_else(|| "(No title)".to_string()),
                description: e.description,
                location: e.location,
                start: EventDateTime {
                    date: e.start.date,
                    date_time: e.start.date_time,
                    time_zone: e.start.time_zone,
                },
                end: EventDateTime {
                    date: e.end.date,
                    date_time: e.end.date_time,
                    time_zone: e.end.time_zone,
                },
                status: e.status.unwrap_or_else(|| "confirmed".to_string()),
                html_link: e.html_link.unwrap_or_default(),
                attendees: e
                    .attendees
                    .unwrap_or_default()
                    .into_iter()
                    .map(|a| Attendee {
                        email: a.email.unwrap_or_default(),
                        display_name: a.display_name,
                        response_status: a.response_status,
                    })
                    .collect(),
                is_all_day,
            }
        })
        .collect();

    let total = events.len();
    Ok(FetchEventsResponse { events, total })
}

pub async fn fetch_today_events(request: &FetchTodayEventsRequest) -> Result<FetchEventsResponse> {
    let today = Local::now();
    let start_of_day = today.format("%Y-%m-%dT00:00:00Z").to_string();
    let end_of_day = today.format("%Y-%m-%dT23:59:59Z").to_string();

    let fetch_req = FetchEventsRequest {
        config: request.config.clone(),
        time_min: Some(start_of_day),
        time_max: Some(end_of_day),
        max_results: Some(50),
        single_events: Some(true),
    };

    fetch_events(&fetch_req).await
}

pub async fn create_event(request: &CreateEventRequest) -> Result<GCalEvent> {
    let client = Client::new();
    let calendar_id = request.config.calendar_id.as_deref().unwrap_or("primary");

    let mut body = serde_json::json!({
        "summary": request.summary
    });

    if let Some(ref desc) = request.description {
        body["description"] = serde_json::json!(desc);
    }
    if let Some(ref loc) = request.location {
        body["location"] = serde_json::json!(loc);
    }

    if let Some(ref start_date) = request.start_date {
        body["start"] = serde_json::json!({ "date": start_date });
    } else if let Some(ref start_datetime) = request.start_datetime {
        body["start"] = serde_json::json!({ "dateTime": start_datetime });
    }

    if let Some(ref end_date) = request.end_date {
        body["end"] = serde_json::json!({ "date": end_date });
    } else if let Some(ref end_datetime) = request.end_datetime {
        body["end"] = serde_json::json!({ "dateTime": end_datetime });
    }

    if let Some(ref attendees) = request.attendees {
        let attendee_list: Vec<serde_json::Value> = attendees
            .iter()
            .map(|email| serde_json::json!({ "email": email }))
            .collect();
        body["attendees"] = serde_json::json!(attendee_list);
    }

    let url = format!(
        "{}/calendars/{}/events",
        GCAL_API_BASE,
        urlencoding::encode(calendar_id)
    );

    let response = client
        .post(&url)
        .header(
            "Authorization",
            format!("Bearer {}", request.config.access_token),
        )
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to create event: {} - {}", status, body);
    }

    let api_event: ApiEvent = response.json().await?;
    let is_all_day = api_event.start.date.is_some();

    Ok(GCalEvent {
        id: api_event.id,
        summary: api_event
            .summary
            .unwrap_or_else(|| "(No title)".to_string()),
        description: api_event.description,
        location: api_event.location,
        start: EventDateTime {
            date: api_event.start.date,
            date_time: api_event.start.date_time,
            time_zone: api_event.start.time_zone,
        },
        end: EventDateTime {
            date: api_event.end.date,
            date_time: api_event.end.date_time,
            time_zone: api_event.end.time_zone,
        },
        status: api_event.status.unwrap_or_else(|| "confirmed".to_string()),
        html_link: api_event.html_link.unwrap_or_default(),
        attendees: api_event
            .attendees
            .unwrap_or_default()
            .into_iter()
            .map(|a| Attendee {
                email: a.email.unwrap_or_default(),
                display_name: a.display_name,
                response_status: a.response_status,
            })
            .collect(),
        is_all_day,
    })
}

pub async fn sync_to_obsidian(request: &SyncToObsidianRequest) -> Result<SyncToObsidianResponse> {
    let days_ahead = request.days_ahead.unwrap_or(7);
    let today = Local::now();
    let future = today + Duration::days(days_ahead as i64);

    let fetch_req = FetchEventsRequest {
        config: request.config.clone(),
        time_min: Some(today.format("%Y-%m-%dT00:00:00Z").to_string()),
        time_max: Some(future.format("%Y-%m-%dT23:59:59Z").to_string()),
        max_results: Some(100),
        single_events: Some(true),
    };

    let events_response = fetch_events(&fetch_req).await?;
    let format = request.format.as_deref().unwrap_or("list");

    let content = match format {
        "table" => format_events_as_table(&events_response.events),
        "timeline" => format_events_as_timeline(&events_response.events),
        _ => format_events_as_list(&events_response.events),
    };

    let file_path = format!(
        "{}/{}/calendar-sync.md",
        request.vault_path, request.target_folder
    );

    Ok(SyncToObsidianResponse {
        events_synced: events_response.total,
        file_path,
        content,
    })
}

fn format_events_as_list(events: &[GCalEvent]) -> String {
    let mut output = String::from("# Upcoming Events\n\n");
    let mut current_date = String::new();

    for event in events {
        let event_date = get_event_date(event);

        if event_date != current_date {
            current_date = event_date.clone();
            output.push_str(&format!("\n## {}\n\n", current_date));
        }

        let time = get_event_time(event);
        let location = event
            .location
            .as_ref()
            .map(|l| format!(" 📍 {}", l))
            .unwrap_or_default();

        output.push_str(&format!("- {} **{}**{}\n", time, event.summary, location));
    }

    output
}

fn format_events_as_table(events: &[GCalEvent]) -> String {
    let mut output = String::from("# Upcoming Events\n\n");
    output.push_str("| Date | Time | Event | Location |\n");
    output.push_str("|------|------|-------|----------|\n");

    for event in events {
        let date = get_event_date(event);
        let time = get_event_time(event);
        let location = event.location.as_deref().unwrap_or("-");

        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            date, time, event.summary, location
        ));
    }

    output
}

fn format_events_as_timeline(events: &[GCalEvent]) -> String {
    let mut output = String::from("# Calendar Timeline\n\n");
    let mut current_date = String::new();

    for event in events {
        let event_date = get_event_date(event);

        if event_date != current_date {
            current_date = event_date.clone();
            output.push_str(&format!("\n### {}\n\n", current_date));
        }

        let time = get_event_time(event);
        let status_icon = match event.status.as_str() {
            "confirmed" => "✅",
            "tentative" => "❓",
            "cancelled" => "❌",
            _ => "📅",
        };

        output.push_str("```timeline\n");
        output.push_str(&format!("{} {} {}\n", time, status_icon, event.summary));

        if let Some(ref desc) = event.description {
            let short_desc = desc.lines().next().unwrap_or("");
            if !short_desc.is_empty() {
                output.push_str(&format!("  {}\n", short_desc));
            }
        }

        if let Some(ref loc) = event.location {
            output.push_str(&format!("  📍 {}\n", loc));
        }

        output.push_str("```\n\n");
    }

    output
}

fn get_event_date(event: &GCalEvent) -> String {
    if let Some(ref date) = event.start.date {
        return date.clone();
    }
    if let Some(ref datetime) = event.start.date_time {
        return datetime.split('T').next().unwrap_or("").to_string();
    }
    "Unknown".to_string()
}

fn get_event_time(event: &GCalEvent) -> String {
    if event.is_all_day {
        return "All day".to_string();
    }
    if let Some(ref datetime) = event.start.date_time {
        if let Some(time_part) = datetime.split('T').nth(1) {
            let time = time_part
                .split_once('+')
                .or_else(|| time_part.split_once('-'))
                .or_else(|| time_part.split_once('Z'))
                .map(|(t, _)| t)
                .unwrap_or(time_part);
            return time[..5].to_string();
        }
    }
    "-".to_string()
}
