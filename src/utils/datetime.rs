use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateTimeFormatRequest {
    pub format: Option<String>,
    pub timestamp: Option<i64>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateTimeFormatResponse {
    pub formatted: String,
    pub timestamp: i64,
    pub iso: String,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub weekday: String,
    pub week_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateTimeParseRequest {
    pub input: String,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateTimeCalcRequest {
    pub base: Option<i64>,
    pub add_days: Option<i64>,
    pub add_hours: Option<i64>,
    pub add_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateTimeDiffRequest {
    pub from: i64,
    pub to: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateTimeDiffResponse {
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub human: String,
}

pub fn format_datetime(request: &DateTimeFormatRequest) -> Result<DateTimeFormatResponse> {
    let dt: DateTime<Local> = match request.timestamp {
        Some(ts) => Local
            .timestamp_opt(ts, 0)
            .single()
            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?,
        None => Local::now(),
    };

    let format_str = request.format.as_deref().unwrap_or("%Y-%m-%d %H:%M:%S");
    let formatted = dt.format(format_str).to_string();

    let weekday = match dt.weekday() {
        chrono::Weekday::Mon => "Monday",
        chrono::Weekday::Tue => "Tuesday",
        chrono::Weekday::Wed => "Wednesday",
        chrono::Weekday::Thu => "Thursday",
        chrono::Weekday::Fri => "Friday",
        chrono::Weekday::Sat => "Saturday",
        chrono::Weekday::Sun => "Sunday",
    };

    Ok(DateTimeFormatResponse {
        formatted,
        timestamp: dt.timestamp(),
        iso: dt.to_rfc3339(),
        year: dt.year(),
        month: dt.month(),
        day: dt.day(),
        hour: dt.hour(),
        minute: dt.minute(),
        second: dt.second(),
        weekday: weekday.to_string(),
        week_number: dt.iso_week().week(),
    })
}

pub fn parse_datetime(request: &DateTimeParseRequest) -> Result<DateTimeFormatResponse> {
    let input = request.input.trim();

    let dt: DateTime<Local> = if let Some(ref fmt) = request.format {
        let naive = chrono::NaiveDateTime::parse_from_str(input, fmt)?;
        Local
            .from_local_datetime(&naive)
            .single()
            .ok_or_else(|| anyhow::anyhow!("Invalid datetime"))?
    } else {
        let lower = input.to_lowercase();
        match lower.as_str() {
            "now" | "today" => Local::now(),
            "yesterday" => Local::now() - Duration::days(1),
            "tomorrow" => Local::now() + Duration::days(1),
            _ => {
                if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
                    dt.with_timezone(&Local)
                } else if let Ok(naive) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
                    Local
                        .from_local_datetime(&naive.and_hms_opt(0, 0, 0).unwrap())
                        .single()
                        .ok_or_else(|| anyhow::anyhow!("Invalid date"))?
                } else {
                    anyhow::bail!("Could not parse datetime: {}", input)
                }
            }
        }
    };

    format_datetime(&DateTimeFormatRequest {
        format: None,
        timestamp: Some(dt.timestamp()),
        timezone: None,
    })
}

pub fn calc_datetime(request: &DateTimeCalcRequest) -> Result<DateTimeFormatResponse> {
    let base: DateTime<Local> = match request.base {
        Some(ts) => Local
            .timestamp_opt(ts, 0)
            .single()
            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?,
        None => Local::now(),
    };

    let mut result = base;

    if let Some(days) = request.add_days {
        result += Duration::days(days);
    }
    if let Some(hours) = request.add_hours {
        result += Duration::hours(hours);
    }
    if let Some(minutes) = request.add_minutes {
        result += Duration::minutes(minutes);
    }

    format_datetime(&DateTimeFormatRequest {
        format: None,
        timestamp: Some(result.timestamp()),
        timezone: None,
    })
}

pub fn diff_datetime(request: &DateTimeDiffRequest) -> Result<DateTimeDiffResponse> {
    let from: DateTime<Utc> = Utc
        .timestamp_opt(request.from, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Invalid from timestamp"))?;
    let to: DateTime<Utc> = Utc
        .timestamp_opt(request.to, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Invalid to timestamp"))?;

    let duration = to.signed_duration_since(from);
    let total_seconds = duration.num_seconds();

    let days = total_seconds / 86400;
    let remaining = total_seconds % 86400;
    let hours = remaining / 3600;
    let remaining = remaining % 3600;
    let minutes = remaining / 60;
    let seconds = remaining % 60;

    let human = if days.abs() > 0 {
        format!("{} days, {} hours", days, hours)
    } else if hours.abs() > 0 {
        format!("{} hours, {} minutes", hours, minutes)
    } else if minutes.abs() > 0 {
        format!("{} minutes, {} seconds", minutes, seconds)
    } else {
        format!("{} seconds", seconds)
    };

    Ok(DateTimeDiffResponse {
        days,
        hours: duration.num_hours(),
        minutes: duration.num_minutes(),
        seconds: total_seconds,
        human,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickDateRequest {
    pub kind: String,
}

pub fn quick_date(request: &QuickDateRequest) -> Result<DateTimeFormatResponse> {
    let now = Local::now();

    let dt = match request.kind.to_lowercase().as_str() {
        "now" => now,
        "today" => now,
        "yesterday" => now - Duration::days(1),
        "tomorrow" => now + Duration::days(1),
        "last_week" => now - Duration::weeks(1),
        "next_week" => now + Duration::weeks(1),
        "last_month" => now - Duration::days(30),
        "next_month" => now + Duration::days(30),
        "start_of_week" => {
            let days_from_monday = now.weekday().num_days_from_monday() as i64;
            now - Duration::days(days_from_monday)
        }
        "end_of_week" => {
            let days_to_sunday = 6 - now.weekday().num_days_from_monday() as i64;
            now + Duration::days(days_to_sunday)
        }
        _ => anyhow::bail!("Unknown quick date: {}", request.kind),
    };

    format_datetime(&DateTimeFormatRequest {
        format: None,
        timestamp: Some(dt.timestamp()),
        timezone: None,
    })
}
