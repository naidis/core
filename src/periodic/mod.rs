use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodicNoteConfig {
    pub folder: String,
    pub format: String,
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyNoteRequest {
    pub date: Option<String>,
    pub config: PeriodicNoteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyNoteRequest {
    pub date: Option<String>,
    pub config: PeriodicNoteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyNoteRequest {
    pub date: Option<String>,
    pub config: PeriodicNoteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YearlyNoteRequest {
    pub date: Option<String>,
    pub config: PeriodicNoteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarterlyNoteRequest {
    pub date: Option<String>,
    pub config: PeriodicNoteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodicNoteInfo {
    pub path: String,
    pub filename: String,
    pub title: String,
    pub date: String,
    pub period_start: String,
    pub period_end: String,
    pub content: String,
}

fn get_date(date_str: Option<&str>) -> NaiveDate {
    date_str
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Local::now().date_naive())
}

fn format_date(date: NaiveDate, format: &str) -> String {
    date.format(format).to_string()
}

fn apply_template(
    template: Option<&str>,
    vars: &std::collections::HashMap<&str, String>,
) -> String {
    let content = template.unwrap_or("# {{title}}\n\n").to_string();
    let mut result = content;

    for (key, value) in vars {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }

    result
}

pub fn generate_daily_note(request: &DailyNoteRequest) -> Result<PeriodicNoteInfo> {
    let date = get_date(request.date.as_deref());
    let filename = format_date(date, &request.config.format);
    let title = format_date(date, "%B %d, %Y");

    let mut vars = std::collections::HashMap::new();
    vars.insert("title", title.clone());
    vars.insert("date", format_date(date, "%Y-%m-%d"));
    vars.insert("day", format_date(date, "%d"));
    vars.insert("month", format_date(date, "%B"));
    vars.insert("year", format_date(date, "%Y"));
    vars.insert("weekday", format_date(date, "%A"));
    vars.insert("week", date.iso_week().week().to_string());

    let content = apply_template(request.config.template.as_deref(), &vars);
    let path = format!("{}/{}.md", request.config.folder, filename);

    Ok(PeriodicNoteInfo {
        path,
        filename: format!("{}.md", filename),
        title,
        date: format_date(date, "%Y-%m-%d"),
        period_start: format_date(date, "%Y-%m-%d"),
        period_end: format_date(date, "%Y-%m-%d"),
        content,
    })
}

pub fn generate_weekly_note(request: &WeeklyNoteRequest) -> Result<PeriodicNoteInfo> {
    let date = get_date(request.date.as_deref());

    let days_from_monday = date.weekday().num_days_from_monday() as i64;
    let week_start = date - Duration::days(days_from_monday);
    let week_end = week_start + Duration::days(6);

    let week_num = date.iso_week().week();
    let year = date.iso_week().year();

    let filename =
        format_date(date, &request.config.format).replace("WW", &format!("{:02}", week_num));
    let title = format!("Week {} - {}", week_num, year);

    let mut vars = std::collections::HashMap::new();
    vars.insert("title", title.clone());
    vars.insert("week", week_num.to_string());
    vars.insert("year", year.to_string());
    vars.insert("week_start", format_date(week_start, "%Y-%m-%d"));
    vars.insert("week_end", format_date(week_end, "%Y-%m-%d"));
    vars.insert("month", format_date(date, "%B"));

    let content = apply_template(request.config.template.as_deref(), &vars);
    let path = format!("{}/{}.md", request.config.folder, filename);

    Ok(PeriodicNoteInfo {
        path,
        filename: format!("{}.md", filename),
        title,
        date: format_date(date, "%Y-%m-%d"),
        period_start: format_date(week_start, "%Y-%m-%d"),
        period_end: format_date(week_end, "%Y-%m-%d"),
        content,
    })
}

pub fn generate_monthly_note(request: &MonthlyNoteRequest) -> Result<PeriodicNoteInfo> {
    let date = get_date(request.date.as_deref());

    let month_start = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap();
    let next_month = if date.month() == 12 {
        NaiveDate::from_ymd_opt(date.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1).unwrap()
    };
    let month_end = next_month - Duration::days(1);

    let filename = format_date(date, &request.config.format);
    let title = format_date(date, "%B %Y");

    let mut vars = std::collections::HashMap::new();
    vars.insert("title", title.clone());
    vars.insert("month", format_date(date, "%B"));
    vars.insert("month_num", format_date(date, "%m"));
    vars.insert("year", format_date(date, "%Y"));
    vars.insert("month_start", format_date(month_start, "%Y-%m-%d"));
    vars.insert("month_end", format_date(month_end, "%Y-%m-%d"));

    let content = apply_template(request.config.template.as_deref(), &vars);
    let path = format!("{}/{}.md", request.config.folder, filename);

    Ok(PeriodicNoteInfo {
        path,
        filename: format!("{}.md", filename),
        title,
        date: format_date(date, "%Y-%m-%d"),
        period_start: format_date(month_start, "%Y-%m-%d"),
        period_end: format_date(month_end, "%Y-%m-%d"),
        content,
    })
}

pub fn generate_quarterly_note(request: &QuarterlyNoteRequest) -> Result<PeriodicNoteInfo> {
    let date = get_date(request.date.as_deref());

    let quarter = (date.month() - 1) / 3 + 1;
    let quarter_start_month = (quarter - 1) * 3 + 1;
    let quarter_start = NaiveDate::from_ymd_opt(date.year(), quarter_start_month, 1).unwrap();
    let quarter_end_month = quarter * 3;
    let quarter_end = if quarter_end_month == 12 {
        NaiveDate::from_ymd_opt(date.year(), 12, 31).unwrap()
    } else {
        NaiveDate::from_ymd_opt(date.year(), quarter_end_month + 1, 1).unwrap() - Duration::days(1)
    };

    let filename = format!("{}-Q{}", date.year(), quarter);
    let title = format!("Q{} {}", quarter, date.year());

    let mut vars = std::collections::HashMap::new();
    vars.insert("title", title.clone());
    vars.insert("quarter", quarter.to_string());
    vars.insert("year", date.year().to_string());
    vars.insert("quarter_start", format_date(quarter_start, "%Y-%m-%d"));
    vars.insert("quarter_end", format_date(quarter_end, "%Y-%m-%d"));

    let content = apply_template(request.config.template.as_deref(), &vars);
    let path = format!("{}/{}.md", request.config.folder, filename);

    Ok(PeriodicNoteInfo {
        path,
        filename: format!("{}.md", filename),
        title,
        date: format_date(date, "%Y-%m-%d"),
        period_start: format_date(quarter_start, "%Y-%m-%d"),
        period_end: format_date(quarter_end, "%Y-%m-%d"),
        content,
    })
}

pub fn generate_yearly_note(request: &YearlyNoteRequest) -> Result<PeriodicNoteInfo> {
    let date = get_date(request.date.as_deref());

    let year_start = NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap();
    let year_end = NaiveDate::from_ymd_opt(date.year(), 12, 31).unwrap();

    let filename = format_date(date, &request.config.format);
    let title = format!("{}", date.year());

    let mut vars = std::collections::HashMap::new();
    vars.insert("title", title.clone());
    vars.insert("year", date.year().to_string());
    vars.insert("year_start", format_date(year_start, "%Y-%m-%d"));
    vars.insert("year_end", format_date(year_end, "%Y-%m-%d"));

    let content = apply_template(request.config.template.as_deref(), &vars);
    let path = format!("{}/{}.md", request.config.folder, filename);

    Ok(PeriodicNoteInfo {
        path,
        filename: format!("{}.md", filename),
        title,
        date: format_date(date, "%Y-%m-%d"),
        period_start: format_date(year_start, "%Y-%m-%d"),
        period_end: format_date(year_end, "%Y-%m-%d"),
        content,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigatePeriodicRequest {
    pub current_date: String,
    pub period_type: String,
    pub direction: String,
}

pub fn navigate_periodic(request: &NavigatePeriodicRequest) -> Result<String> {
    let date = NaiveDate::parse_from_str(&request.current_date, "%Y-%m-%d")?;

    let new_date = match (request.period_type.as_str(), request.direction.as_str()) {
        ("daily", "prev") => date - Duration::days(1),
        ("daily", "next") => date + Duration::days(1),
        ("weekly", "prev") => date - Duration::weeks(1),
        ("weekly", "next") => date + Duration::weeks(1),
        ("monthly", "prev") => {
            if date.month() == 1 {
                NaiveDate::from_ymd_opt(date.year() - 1, 12, date.day().min(31)).unwrap()
            } else {
                let prev_month = date.month() - 1;
                let max_day = days_in_month(date.year(), prev_month);
                NaiveDate::from_ymd_opt(date.year(), prev_month, date.day().min(max_day)).unwrap()
            }
        }
        ("monthly", "next") => {
            if date.month() == 12 {
                NaiveDate::from_ymd_opt(date.year() + 1, 1, date.day().min(31)).unwrap()
            } else {
                let next_month = date.month() + 1;
                let max_day = days_in_month(date.year(), next_month);
                NaiveDate::from_ymd_opt(date.year(), next_month, date.day().min(max_day)).unwrap()
            }
        }
        ("quarterly", "prev") => {
            let quarter = (date.month() - 1) / 3;
            if quarter == 0 {
                NaiveDate::from_ymd_opt(date.year() - 1, 10, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(date.year(), (quarter - 1) * 3 + 1, 1).unwrap()
            }
        }
        ("quarterly", "next") => {
            let quarter = (date.month() - 1) / 3;
            if quarter == 3 {
                NaiveDate::from_ymd_opt(date.year() + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(date.year(), (quarter + 1) * 3 + 1, 1).unwrap()
            }
        }
        ("yearly", "prev") => {
            NaiveDate::from_ymd_opt(date.year() - 1, date.month(), date.day()).unwrap()
        }
        ("yearly", "next") => {
            NaiveDate::from_ymd_opt(date.year() + 1, date.month(), date.day()).unwrap()
        }
        _ => date,
    };

    Ok(new_date.format("%Y-%m-%d").to_string())
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
