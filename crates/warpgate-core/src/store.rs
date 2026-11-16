use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use taskchampion::storage::AccessMode;
use taskchampion::{Operation, Replica, SqliteStorage};

pub use taskchampion::{Error, Uuid};

use crate::{Reminder, ReminderInput, reminder_at};

pub struct Store {
    replica: Replica<SqliteStorage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upcoming {
    pub uuid: Uuid,
    pub description: String,
    pub project: Option<String>,
    pub reminder: Reminder,
}

impl Store {
    /// Opens the TaskChampion replica in `dir` (Taskwarrior's `data.location`).
    pub async fn open(dir: &Path) -> Result<Self, Error> {
        let storage = SqliteStorage::new(dir, AccessMode::ReadWrite, false).await?;
        Ok(Self { replica: Replica::new(storage) })
    }

    /// Working-set tasks still pending that have a reminder, soonest first.
    pub async fn upcoming<Tz: TimeZone>(&mut self, tz: &Tz) -> Result<Vec<Upcoming>, Error> {
        let mut upcoming: Vec<Upcoming> = self
            .replica
            .pending_tasks()
            .await?
            .into_iter()
            .filter_map(|task| {
                let input = ReminderInput {
                    status: task.get_value("status").unwrap_or_default(),
                    remind: task.get_timestamp("remind"),
                    due: task.get_due(),
                    wait: task.get_wait(),
                };
                Some(Upcoming {
                    reminder: reminder_at(&input, tz)?,
                    uuid: task.get_uuid(),
                    description: task.get_description().to_owned(),
                    project: task.get_value("project").map(str::to_owned),
                })
            })
            .collect();
        upcoming.sort_by_key(|u| u.reminder.at);
        Ok(upcoming)
    }

    pub async fn done(&mut self, uuid: Uuid) -> Result<(), Error> {
        let mut task = self.replica.get_task(uuid).await?.ok_or_else(|| missing(uuid))?;
        let mut ops = vec![Operation::UndoPoint];
        task.done(&mut ops)?;
        self.replica.commit_operations(ops).await
    }

    pub async fn snooze(&mut self, uuid: Uuid, until: DateTime<Utc>) -> Result<(), Error> {
        let mut task = self.replica.get_task(uuid).await?.ok_or_else(|| missing(uuid))?;
        let mut ops = vec![Operation::UndoPoint];
        task.set_timestamp("remind", Some(until), &mut ops)?;
        self.replica.commit_operations(ops).await
    }
}

fn missing(uuid: Uuid) -> Error {
    Error::Usage(format!("no task with uuid {uuid}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReminderSource;
    use taskchampion::Status;
    use tempfile::TempDir;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    async fn seed(dir: &Path, tasks: &[(&str, Option<&str>, Option<&str>)]) -> Vec<Uuid> {
        let storage = SqliteStorage::new(dir, AccessMode::ReadWrite, true).await.unwrap();
        let mut replica = Replica::new(storage);
        let mut ops = Vec::new();
        let mut uuids = Vec::new();
        for (description, remind, due) in tasks {
            let uuid = Uuid::new_v4();
            let mut task = replica.create_task(uuid, &mut ops).await.unwrap();
            task.set_description(description.to_string(), &mut ops).unwrap();
            task.set_status(Status::Pending, &mut ops).unwrap();
            task.set_timestamp("remind", remind.map(utc), &mut ops).unwrap();
            task.set_due(due.map(utc), &mut ops).unwrap();
            uuids.push(uuid);
        }
        replica.commit_operations(ops).await.unwrap();
        uuids
    }

    #[tokio::test]
    async fn upcoming_lists_tasks_with_reminders_soonest_first() {
        let dir = TempDir::new().unwrap();
        seed(
            dir.path(),
            &[
                ("later", Some("2026-11-20T15:00:00Z"), None),
                ("no reminder", None, None),
                ("sooner", None, Some("2026-11-18T20:00:00Z")),
            ],
        )
        .await;
        let mut store = Store::open(dir.path()).await.unwrap();

        let upcoming = store.upcoming(&Utc).await.unwrap();

        let names: Vec<_> = upcoming.iter().map(|u| u.description.as_str()).collect();
        assert_eq!(names, ["sooner", "later"]);
        assert_eq!(upcoming[0].reminder.source, ReminderSource::Due);
    }

    #[tokio::test]
    async fn done_removes_the_task_from_upcoming() {
        let dir = TempDir::new().unwrap();
        let uuids = seed(dir.path(), &[("ping me", Some("2026-11-20T15:00:00Z"), None)]).await;
        let mut store = Store::open(dir.path()).await.unwrap();

        store.done(uuids[0]).await.unwrap();

        assert!(store.upcoming(&Utc).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn snooze_moves_the_reminder() {
        let dir = TempDir::new().unwrap();
        let uuids = seed(dir.path(), &[("ping me", None, Some("2026-11-20T15:00:00Z"))]).await;
        let mut store = Store::open(dir.path()).await.unwrap();

        store.snooze(uuids[0], utc("2026-11-20T16:00:00Z")).await.unwrap();

        let upcoming = store.upcoming(&Utc).await.unwrap();
        assert_eq!(upcoming[0].reminder.at, utc("2026-11-20T16:00:00Z"));
        assert_eq!(upcoming[0].reminder.source, ReminderSource::Remind);
    }

    #[tokio::test]
    async fn done_on_an_unknown_task_errors() {
        let dir = TempDir::new().unwrap();
        seed(dir.path(), &[]).await;
        let mut store = Store::open(dir.path()).await.unwrap();

        assert!(store.done(Uuid::new_v4()).await.is_err());
    }
}
