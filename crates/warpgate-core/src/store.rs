use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use taskchampion::storage::AccessMode;
use taskchampion::{Operation, Replica, SqliteStorage, Status, TaskData, utc_timestamp};

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
        let timestamp = |task: &TaskData, property: &str| {
            task.get(property).and_then(|secs| secs.parse().ok()).map(utc_timestamp)
        };
        let mut upcoming: Vec<Upcoming> = self
            .replica
            .pending_task_data()
            .await?
            .into_iter()
            .filter_map(|task| {
                let input = ReminderInput {
                    status: task.get("status").unwrap_or_default(),
                    remind: timestamp(&task, "remind"),
                    due: timestamp(&task, "due"),
                    wait: timestamp(&task, "wait"),
                };
                Some(Upcoming {
                    reminder: reminder_at(&input, tz)?,
                    uuid: task.get_uuid(),
                    description: task.get("description").unwrap_or_default().to_owned(),
                    project: task.get("project").map(str::to_owned),
                })
            })
            .collect();
        upcoming.sort_by_key(|u| u.reminder.at);
        Ok(upcoming)
    }

    /// Like `task done`: only for pending tasks, stops a started task, and marks the
    /// instance done in its recurring template's `mask`.
    pub async fn done(&mut self, uuid: Uuid) -> Result<(), Error> {
        let mut task = self.replica.get_task(uuid).await?.ok_or_else(|| missing(uuid))?;
        if task.get_status() != Status::Pending {
            return Ok(());
        }
        let mut ops = vec![Operation::UndoPoint];
        if task.is_active() {
            task.stop(&mut ops)?;
        }
        task.done(&mut ops)?;

        let parent = task.get_value("parent").and_then(|parent| Uuid::parse_str(parent).ok());
        let index = task.get_value("imask").and_then(|index| index.parse::<usize>().ok());
        if let (Some(parent), Some(index)) = (parent, index)
            && let Some(mut template) = self.replica.get_task(parent).await?
            && let Some(mask) = template.get_value("mask").filter(|mask| index < mask.len())
        {
            let mut mask = mask.to_owned();
            mask.replace_range(index..=index, "+");
            template.set_value("mask", Some(mask), &mut ops)?;
        }
        self.replica.commit_operations(ops).await
    }

    pub async fn snooze(&mut self, uuid: Uuid, until: DateTime<Utc>) -> Result<(), Error> {
        let mut task = self.replica.get_task(uuid).await?.ok_or_else(|| missing(uuid))?;
        if task.get_status() != Status::Pending {
            return Ok(());
        }
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

    async fn task_value(store: &mut Store, uuid: Uuid, property: &str) -> Option<String> {
        let task = store.replica.get_task(uuid).await.unwrap().unwrap();
        task.get_value(property).map(str::to_owned)
    }

    #[tokio::test]
    async fn done_leaves_closed_tasks_alone() {
        let dir = TempDir::new().unwrap();
        let uuids = seed(dir.path(), &[("gone", Some("2026-11-20T15:00:00Z"), None)]).await;
        let mut store = Store::open(dir.path()).await.unwrap();
        let mut ops = Vec::new();
        let mut task = store.replica.get_task(uuids[0]).await.unwrap().unwrap();
        task.set_status(Status::Deleted, &mut ops).unwrap();
        store.replica.commit_operations(ops).await.unwrap();

        store.done(uuids[0]).await.unwrap();
        store.snooze(uuids[0], utc("2026-11-21T15:00:00Z")).await.unwrap();

        assert_eq!(task_value(&mut store, uuids[0], "status").await.as_deref(), Some("deleted"));
        assert_eq!(task_value(&mut store, uuids[0], "remind").await, Some(utc("2026-11-20T15:00:00Z").timestamp().to_string()));
    }

    #[tokio::test]
    async fn done_stops_a_started_task() {
        let dir = TempDir::new().unwrap();
        let uuids = seed(dir.path(), &[("started", None, None)]).await;
        let mut store = Store::open(dir.path()).await.unwrap();
        let mut ops = Vec::new();
        let mut task = store.replica.get_task(uuids[0]).await.unwrap().unwrap();
        task.start(&mut ops).unwrap();
        store.replica.commit_operations(ops).await.unwrap();

        store.done(uuids[0]).await.unwrap();

        assert_eq!(task_value(&mut store, uuids[0], "start").await, None);
        assert_eq!(task_value(&mut store, uuids[0], "status").await.as_deref(), Some("completed"));
    }

    #[tokio::test]
    async fn done_marks_the_recurring_template_mask() {
        let dir = TempDir::new().unwrap();
        let uuids = seed(dir.path(), &[("template", None, None), ("instance", None, None)]).await;
        let (template, instance) = (uuids[0], uuids[1]);
        let mut store = Store::open(dir.path()).await.unwrap();
        let mut ops = Vec::new();
        let mut task = store.replica.get_task(template).await.unwrap().unwrap();
        task.set_status(Status::Recurring, &mut ops).unwrap();
        task.set_value("mask", Some("---".into()), &mut ops).unwrap();
        let mut task = store.replica.get_task(instance).await.unwrap().unwrap();
        task.set_value("parent", Some(template.to_string()), &mut ops).unwrap();
        task.set_value("imask", Some("1".into()), &mut ops).unwrap();
        store.replica.commit_operations(ops).await.unwrap();

        store.done(instance).await.unwrap();

        assert_eq!(task_value(&mut store, template, "mask").await.as_deref(), Some("-+-"));
    }
}
