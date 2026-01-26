use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlpDateParseRequest {
    pub text: String,
    pub reference_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlpDateParseResponse {
    pub date: String,
    pub formatted: String,
    pub weekday: String,
    pub relative: String,
    pub timestamp: i64,
}

pub fn parse_natural_date(request: &NlpDateParseRequest) -> Result<NlpDateParseResponse> {
    let reference = if let Some(ref d) = request.reference_date {
        NaiveDate::parse_from_str(d, "%Y-%m-%d")?
    } else {
        Local::now().date_naive()
    };

    let text = request.text.to_lowercase().trim().to_string();

    let date = parse_date_text(&text, reference)?;

    let weekday = match date.weekday() {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    };

    let days_diff = (date - reference).num_days();
    let relative = match days_diff {
        0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        -1 => "yesterday".to_string(),
        d if d > 0 && d < 7 => format!("in {} days", d),
        d if d > 0 => format!("in {} weeks", d / 7),
        d if d < 0 && d > -7 => format!("{} days ago", -d),
        d => format!("{} weeks ago", -d / 7),
    };

    Ok(NlpDateParseResponse {
        date: date.format("%Y-%m-%d").to_string(),
        formatted: date.format("%B %d, %Y").to_string(),
        weekday: weekday.to_string(),
        relative,
        timestamp: date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
    })
}

fn parse_date_text(text: &str, reference: NaiveDate) -> Result<NaiveDate> {
    match text {
        "today" | "now" => return Ok(reference),
        "tomorrow" | "tmr" | "tmrw" => return Ok(reference + Duration::days(1)),
        "yesterday" | "yest" => return Ok(reference - Duration::days(1)),
        "next week" => return Ok(reference + Duration::weeks(1)),
        "last week" => return Ok(reference - Duration::weeks(1)),
        "next month" => return Ok(add_months(reference, 1)),
        "last month" => return Ok(add_months(reference, -1)),
        "next year" => return Ok(add_months(reference, 12)),
        "last year" => return Ok(add_months(reference, -12)),
        _ => {}
    }

    if let Some(weekday) = parse_weekday(text) {
        let current_weekday = reference.weekday().num_days_from_monday();
        let target_weekday = weekday.num_days_from_monday();

        let days = if text.contains("next") {
            let diff = (target_weekday as i64 - current_weekday as i64 + 7) % 7;
            (if diff == 0 { 7 } else { diff }) + 7
        } else if text.contains("last") {
            let diff = (current_weekday as i64 - target_weekday as i64 + 7) % 7;
            -(if diff == 0 { 7 } else { diff })
        } else {
            let diff = (target_weekday as i64 - current_weekday as i64 + 7) % 7;
            if diff == 0 {
                7
            } else {
                diff
            }
        };

        return Ok(reference + Duration::days(days));
    }

    let in_days_re = Regex::new(r"in (\d+) days?")?;
    if let Some(caps) = in_days_re.captures(text) {
        let days: i64 = caps[1].parse()?;
        return Ok(reference + Duration::days(days));
    }

    let in_weeks_re = Regex::new(r"in (\d+) weeks?")?;
    if let Some(caps) = in_weeks_re.captures(text) {
        let weeks: i64 = caps[1].parse()?;
        return Ok(reference + Duration::weeks(weeks));
    }

    let in_months_re = Regex::new(r"in (\d+) months?")?;
    if let Some(caps) = in_months_re.captures(text) {
        let months: i32 = caps[1].parse()?;
        return Ok(add_months(reference, months));
    }

    let days_ago_re = Regex::new(r"(\d+) days? ago")?;
    if let Some(caps) = days_ago_re.captures(text) {
        let days: i64 = caps[1].parse()?;
        return Ok(reference - Duration::days(days));
    }

    let date_re = Regex::new(r"(\d{4})-(\d{2})-(\d{2})")?;
    if let Some(caps) = date_re.captures(text) {
        return Ok(NaiveDate::parse_from_str(&caps[0], "%Y-%m-%d")?);
    }

    let month_day_re = Regex::new(
        r"(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]* (\d{1,2})(?:st|nd|rd|th)?",
    )?;
    if let Some(caps) = month_day_re.captures(text) {
        let month = match &caps[1] {
            "jan" => 1,
            "feb" => 2,
            "mar" => 3,
            "apr" => 4,
            "may" => 5,
            "jun" => 6,
            "jul" => 7,
            "aug" => 8,
            "sep" => 9,
            "oct" => 10,
            "nov" => 11,
            "dec" => 12,
            _ => return Err(anyhow::anyhow!("Invalid month")),
        };
        let day: u32 = caps[2].parse()?;
        let year = if NaiveDate::from_ymd_opt(reference.year(), month, day)
            .map(|d| d >= reference)
            .unwrap_or(false)
        {
            reference.year()
        } else {
            reference.year() + 1
        };
        return NaiveDate::from_ymd_opt(year, month, day)
            .ok_or_else(|| anyhow::anyhow!("Invalid date"));
    }

    let end_of_re = Regex::new(r"end of (week|month|year)")?;
    if let Some(caps) = end_of_re.captures(text) {
        return match &caps[1] {
            "week" => {
                let days_to_sunday = 6 - reference.weekday().num_days_from_monday() as i64;
                Ok(reference + Duration::days(days_to_sunday))
            }
            "month" => {
                let next_month = add_months(reference, 1);
                Ok(
                    NaiveDate::from_ymd_opt(next_month.year(), next_month.month(), 1).unwrap()
                        - Duration::days(1),
                )
            }
            "year" => Ok(NaiveDate::from_ymd_opt(reference.year(), 12, 31).unwrap()),
            _ => Err(anyhow::anyhow!("Invalid period")),
        };
    }

    Err(anyhow::anyhow!("Could not parse date: {}", text))
}

fn parse_weekday(text: &str) -> Option<Weekday> {
    let text = text.replace("next ", "").replace("last ", "");
    match text.trim() {
        s if s.starts_with("mon") => Some(Weekday::Mon),
        s if s.starts_with("tue") => Some(Weekday::Tue),
        s if s.starts_with("wed") => Some(Weekday::Wed),
        s if s.starts_with("thu") => Some(Weekday::Thu),
        s if s.starts_with("fri") => Some(Weekday::Fri),
        s if s.starts_with("sat") => Some(Weekday::Sat),
        s if s.starts_with("sun") => Some(Weekday::Sun),
        _ => None,
    }
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total_months = date.year() * 12 + date.month() as i32 - 1 + months;
    let year = total_months / 12;
    let month = (total_months % 12 + 1) as u32;
    let day = date.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateSuggestRequest {
    pub partial: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateSuggestion {
    pub text: String,
    pub date: String,
    pub description: String,
}

pub fn suggest_dates(request: &DateSuggestRequest) -> Vec<DateSuggestion> {
    let today = Local::now().date_naive();
    let partial = request.partial.to_lowercase();

    let all_suggestions = vec![
        ("today", today, "Today"),
        ("tomorrow", today + Duration::days(1), "Tomorrow"),
        ("next week", today + Duration::weeks(1), "Next week"),
        (
            "next monday",
            find_next_weekday(today, Weekday::Mon),
            "Next Monday",
        ),
        (
            "next friday",
            find_next_weekday(today, Weekday::Fri),
            "Next Friday",
        ),
        ("in 2 days", today + Duration::days(2), "In 2 days"),
        ("in 3 days", today + Duration::days(3), "In 3 days"),
        ("in 1 week", today + Duration::weeks(1), "In 1 week"),
        ("in 2 weeks", today + Duration::weeks(2), "In 2 weeks"),
        ("end of week", find_end_of_week(today), "End of this week"),
        (
            "end of month",
            find_end_of_month(today),
            "End of this month",
        ),
    ];

    all_suggestions
        .into_iter()
        .filter(|(text, _, _)| text.contains(&partial) || partial.is_empty())
        .map(|(text, date, desc)| DateSuggestion {
            text: text.to_string(),
            date: date.format("%Y-%m-%d").to_string(),
            description: desc.to_string(),
        })
        .collect()
}

fn find_next_weekday(from: NaiveDate, target: Weekday) -> NaiveDate {
    let current = from.weekday().num_days_from_monday();
    let target_num = target.num_days_from_monday();
    let diff = (target_num as i64 - current as i64 + 7) % 7;
    let days = if diff == 0 { 7 } else { diff };
    from + Duration::days(days)
}

fn find_end_of_week(date: NaiveDate) -> NaiveDate {
    let days_to_sunday = 6 - date.weekday().num_days_from_monday() as i64;
    date + Duration::days(days_to_sunday)
}

fn find_end_of_month(date: NaiveDate) -> NaiveDate {
    let next_month = add_months(date, 1);
    NaiveDate::from_ymd_opt(next_month.year(), next_month.month(), 1).unwrap() - Duration::days(1)
}
