use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, Utc};
use warpgate_core::{DEFAULT_MORNING, Reminder, ReminderInput, reminder_at};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Annotation {
    pub entry: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub description: String,
    pub status: Option<String>,
    pub project: Option<String>,
    pub due: Option<String>,
    pub priority: Option<String>,
    pub tags: Option<Vec<String>>,
    pub urgency: Option<f64>,
    pub entry: Option<String>,
    pub modified: Option<String>,
    pub annotations: Option<Vec<Annotation>>,
    pub recur: Option<String>,
    pub start: Option<String>,
    pub depends: Option<String>,
    pub scheduled: Option<String>,
    pub wait: Option<String>,
    pub until: Option<String>,
    pub parent: Option<String>,
    pub remind: Option<String>,
}

fn parse_tw_date(raw: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%SZ")
        .ok()
        .map(|dt| dt.and_utc())
}

impl Task {
    pub fn reminder(&self) -> Option<Reminder> {
        let input = ReminderInput {
            status: self.status.as_deref().unwrap_or("pending"),
            remind: self.remind.as_deref().and_then(parse_tw_date),
            due: self.due.as_deref().and_then(parse_tw_date),
            wait: self.wait.as_deref().and_then(parse_tw_date),
        };
        reminder_at(&input, &Local, DEFAULT_MORNING)
    }

    pub fn remind_local(&self) -> String {
        self.remind
            .as_deref()
            .and_then(parse_tw_date)
            .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%dT%H:%M").to_string())
            .unwrap_or_default()
    }

    pub fn is_started(&self) -> bool {
        self.start.is_some()
    }

    pub fn is_recurring(&self) -> bool {
        self.recur.is_some()
    }

    /// This is an instance of a recurring task (has a parent template)
    pub fn is_recurring_instance(&self) -> bool {
        self.parent.is_some()
    }

    pub fn age(&self) -> String {
        match &self.entry {
            Some(d) if d.len() >= 8 => {
                let now = Utc::now();
                let year: i32 = d[0..4].parse().unwrap_or(0);
                let month: u32 = d[4..6].parse().unwrap_or(1);
                let day: u32 = d[6..8].parse().unwrap_or(1);
                if let Some(created) = NaiveDate::from_ymd_opt(year, month, day) {
                    let days = (now.date_naive() - created).num_days();
                    if days == 0 {
                        "today".into()
                    } else if days < 7 {
                        format!("{}d", days)
                    } else if days < 30 {
                        format!("{}w", days / 7)
                    } else if days < 365 {
                        format!("{}mo", days / 30)
                    } else {
                        format!("{}y", days / 365)
                    }
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    }

    pub fn recur_short(&self) -> String {
        self.recur.clone().unwrap_or_default()
    }

    pub fn due_short(&self) -> String {
        match &self.due {
            Some(d) => {
                // Parse "20260506T060000Z" -> "05/06"
                if d.len() >= 8 {
                    let month = &d[4..6];
                    let day = &d[6..8];
                    format!("{}/{}", month, day)
                } else {
                    d.clone()
                }
            }
            None => String::new(),
        }
    }
}
