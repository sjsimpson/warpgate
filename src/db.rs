use std::collections::BTreeMap;
use std::path::PathBuf;

use color_eyre::Result;
use rusqlite::Connection;

use crate::task::{Annotation, Task};

fn db_path() -> PathBuf {
    let task_data = std::env::var("TASKDATA")
        .unwrap_or_else(|_| format!("{}/.task", std::env::var("HOME").unwrap_or_default()));
    PathBuf::from(task_data).join("taskchampion.sqlite3")
}

/// Convert epoch seconds string to taskwarrior ISO format "20260506T060000Z"
fn epoch_to_tw(epoch: &str) -> String {
    if let Ok(ts) = epoch.parse::<i64>() {
        let dt = chrono::DateTime::from_timestamp(ts, 0);
        match dt {
            Some(d) => d.format("%Y%m%dT%H%M%SZ").to_string(),
            None => epoch.to_string(),
        }
    } else {
        epoch.to_string()
    }
}

fn parse_task(uuid: &str, data: &str) -> Option<Task> {
    let map: BTreeMap<String, serde_json::Value> = serde_json::from_str(data).ok()?;

    let get_str = |key: &str| -> Option<String> {
        map.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    };

    let get_epoch = |key: &str| -> Option<String> {
        get_str(key).map(|s| epoch_to_tw(&s))
    };

    let description = get_str("description")?;

    // Tags: keys like "tag_next", "tag_urgent" -> vec!["next", "urgent"]
    let tags: Vec<String> = map
        .keys()
        .filter_map(|k| k.strip_prefix("tag_").map(|t| t.to_string()))
        .collect();

    // Annotations: keys like "annotation_1776897052" -> sorted by timestamp
    let mut annotations: Vec<(i64, Annotation)> = map
        .iter()
        .filter_map(|(k, v)| {
            k.strip_prefix("annotation_").and_then(|ts_str| {
                let ts: i64 = ts_str.parse().ok()?;
                let desc = v.as_str()?.to_string();
                Some((
                    ts,
                    Annotation {
                        entry: Some(epoch_to_tw(ts_str)),
                        description: Some(desc),
                    },
                ))
            })
        })
        .collect();
    annotations.sort_by_key(|(ts, _)| *ts);

    Some(Task {
        id: None, // filled in from working_set
        uuid: Some(uuid.to_string()),
        description,
        status: get_str("status"),
        project: get_str("project"),
        due: get_epoch("due"),
        priority: get_str("priority"),
        tags: if tags.is_empty() { None } else { Some(tags) },
        urgency: None, // not stored in DB, calculated by taskwarrior
        entry: get_epoch("entry"),
        modified: get_epoch("modified"),
        annotations: if annotations.is_empty() {
            None
        } else {
            Some(annotations.into_iter().map(|(_, a)| a).collect())
        },
        recur: get_str("recur"),
        start: get_epoch("start"),
        depends: get_str("dep"),
        scheduled: get_epoch("scheduled"),
        wait: get_epoch("wait"),
        until: get_epoch("until"),
        parent: get_str("parent"),
    })
}

pub fn read_all_tasks() -> Result<Vec<Task>> {
    let conn = Connection::open(db_path())?;

    // Build working set map: uuid -> id
    let mut id_map: BTreeMap<String, i64> = BTreeMap::new();
    {
        let mut stmt = conn.prepare("SELECT id, uuid FROM working_set")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let uuid: String = row.get(1)?;
            Ok((uuid, id))
        })?;
        for row in rows {
            if let Ok((uuid, id)) = row {
                id_map.insert(uuid, id);
            }
        }
    }

    // Read all tasks
    let mut stmt = conn.prepare("SELECT uuid, data FROM tasks")?;
    let rows = stmt.query_map([], |row| {
        let uuid: String = row.get(0)?;
        let data: String = row.get(1)?;
        Ok((uuid, data))
    })?;

    let mut tasks = Vec::new();
    for row in rows {
        if let Ok((uuid, data)) = row {
            if let Some(mut task) = parse_task(&uuid, &data) {
                task.id = id_map.get(&uuid).copied();
                tasks.push(task);
            }
        }
    }

    Ok(tasks)
}

pub fn read_tasks_by_status(status: &str) -> Result<Vec<Task>> {
    let all = read_all_tasks()?;
    Ok(all
        .into_iter()
        .filter(|t| t.status.as_deref() == Some(status))
        .collect())
}
