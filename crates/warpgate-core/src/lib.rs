use chrono::{DateTime, NaiveTime, TimeZone, Utc};

const MORNING: NaiveTime = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
const END_OF_DAY: NaiveTime = NaiveTime::from_hms_opt(23, 59, 59).unwrap();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderSource {
    Remind,
    Due,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reminder {
    pub at: DateTime<Utc>,
    pub source: ReminderSource,
}

pub struct ReminderInput<'a> {
    pub status: &'a str,
    pub remind: Option<DateTime<Utc>>,
    pub due: Option<DateTime<Utc>>,
    pub wait: Option<DateTime<Utc>>,
}

/// When a task should ping: `remind` if set, else `due`. A due with no
/// time of day (local midnight, or `eod`) pings at 9:00 that day.
pub fn reminder_at<Tz: TimeZone>(task: &ReminderInput, tz: &Tz) -> Option<Reminder> {
    if !matches!(task.status, "pending" | "waiting") {
        return None;
    }
    let reminder = match (task.remind, task.due) {
        (Some(at), _) => Reminder { at, source: ReminderSource::Remind },
        (None, Some(due)) => Reminder { at: due_reminder(due, tz), source: ReminderSource::Due },
        (None, None) => return None,
    };
    match task.wait {
        Some(wait) if wait > reminder.at => None,
        _ => Some(reminder),
    }
}

fn due_reminder<Tz: TimeZone>(due: DateTime<Utc>, tz: &Tz) -> DateTime<Utc> {
    let local = due.with_timezone(tz).naive_local();
    if local.time() != NaiveTime::MIN && local.time() != END_OF_DAY {
        return due;
    }
    tz.from_local_datetime(&local.date().and_time(MORNING))
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(due)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn mountain() -> FixedOffset {
        FixedOffset::west_opt(7 * 3600).unwrap()
    }

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn input(status: &str) -> ReminderInput<'_> {
        ReminderInput { status, remind: None, due: None, wait: None }
    }

    fn at(task: &ReminderInput) -> Option<Reminder> {
        reminder_at(task, &mountain())
    }

    #[test]
    fn remind_wins_over_due() {
        let task = ReminderInput {
            remind: Some(utc("2026-11-12T15:30:00Z")),
            due: Some(utc("2026-11-12T20:00:00Z")),
            ..input("pending")
        };
        assert_eq!(
            at(&task),
            Some(Reminder { at: utc("2026-11-12T15:30:00Z"), source: ReminderSource::Remind })
        );
    }

    #[test]
    fn due_with_time_pings_at_due() {
        let task = ReminderInput { due: Some(utc("2026-11-12T20:00:00Z")), ..input("pending") };
        assert_eq!(
            at(&task),
            Some(Reminder { at: utc("2026-11-12T20:00:00Z"), source: ReminderSource::Due })
        );
    }

    #[test]
    fn date_only_due_pings_in_the_morning() {
        let task = ReminderInput { due: Some(utc("2026-11-12T07:00:00Z")), ..input("pending") };
        assert_eq!(at(&task).unwrap().at, utc("2026-11-12T16:00:00Z"));
    }

    #[test]
    fn eod_due_pings_in_the_morning() {
        let task = ReminderInput { due: Some(utc("2026-11-13T06:59:59Z")), ..input("pending") };
        assert_eq!(at(&task).unwrap().at, utc("2026-11-12T16:00:00Z"));
    }

    #[test]
    fn no_remind_or_due_never_pings() {
        assert_eq!(at(&input("pending")), None);
    }

    #[test]
    fn closed_and_template_tasks_never_ping() {
        for status in ["completed", "deleted", "recurring"] {
            let task = ReminderInput { remind: Some(utc("2026-11-12T15:00:00Z")), ..input(status) };
            assert_eq!(at(&task), None, "{status}");
        }
    }

    #[test]
    fn waiting_past_the_reminder_suppresses_it() {
        let task = ReminderInput {
            remind: Some(utc("2026-11-12T15:00:00Z")),
            wait: Some(utc("2026-11-13T15:00:00Z")),
            ..input("waiting")
        };
        assert_eq!(at(&task), None);
    }

    #[test]
    fn wait_ending_exactly_at_the_reminder_keeps_it() {
        let task = ReminderInput {
            remind: Some(utc("2026-11-12T15:00:00Z")),
            wait: Some(utc("2026-11-12T15:00:00Z")),
            ..input("waiting")
        };
        assert_eq!(at(&task).unwrap().at, utc("2026-11-12T15:00:00Z"));
    }

    #[test]
    fn date_only_due_on_dst_days_pings_at_local_morning() {
        let tz = chrono_tz::America::New_York;
        for (due, expected) in [
            ("2026-03-08T05:00:00Z", "2026-03-08T13:00:00Z"),
            ("2026-11-01T04:00:00Z", "2026-11-01T14:00:00Z"),
        ] {
            let task = ReminderInput { due: Some(utc(due)), ..input("pending") };
            assert_eq!(reminder_at(&task, &tz).unwrap().at, utc(expected), "{due}");
        }
    }

    #[test]
    fn wait_ending_before_the_reminder_keeps_it() {
        let task = ReminderInput {
            remind: Some(utc("2026-11-12T15:00:00Z")),
            wait: Some(utc("2026-11-12T07:00:00Z")),
            ..input("waiting")
        };
        assert_eq!(at(&task).unwrap().at, utc("2026-11-12T15:00:00Z"));
    }
}
