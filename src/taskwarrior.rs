use std::collections::BTreeSet;
use std::process::Command;

use color_eyre::Result;

use crate::task::Task;

pub fn get_pending_tasks() -> Result<Vec<Task>> {
    let output = Command::new("task")
        .args(["status:pending", "export", "rc.json.array=on"])
        .output()?;

    let json = String::from_utf8(output.stdout)?;
    let tasks: Vec<Task> = serde_json::from_str(&json)?;
    Ok(tasks)
}

pub fn get_tasks_with_filter(filter: &[&str]) -> Result<Vec<Task>> {
    let mut args: Vec<&str> = filter.to_vec();
    args.push("export");
    args.push("rc.json.array=on");
    let output = Command::new("task").args(&args).output()?;
    let json = String::from_utf8(output.stdout)?;
    let tasks: Vec<Task> = serde_json::from_str(&json)?;
    Ok(tasks)
}

pub fn get_projects(tasks: &[Task]) -> Vec<String> {
    let mut projects: BTreeSet<String> = BTreeSet::new();
    for task in tasks {
        if let Some(ref p) = task.project {
            projects.insert(p.clone());
            // Ensure all ancestor projects exist
            let mut parts: Vec<&str> = p.split('.').collect();
            while parts.len() > 1 {
                parts.pop();
                projects.insert(parts.join("."));
            }
        }
    }
    projects.into_iter().collect()
}

pub fn complete_task(uuid: &str) -> Result<()> {
    Command::new("task")
        .args(["rc.confirmation=off", uuid, "done"])
        .output()?;
    Ok(())
}

pub fn delete_task(uuid: &str) -> Result<()> {
    Command::new("task")
        .args(["rc.confirmation=off", uuid, "delete"])
        .output()?;
    Ok(())
}

pub fn start_task(uuid: &str) -> Result<()> {
    Command::new("task").args([uuid, "start"]).output()?;
    Ok(())
}

pub fn stop_task(uuid: &str) -> Result<()> {
    Command::new("task").args([uuid, "stop"]).output()?;
    Ok(())
}

fn push_attr(args: &mut Vec<String>, key: &str, val: &str) {
    if !val.is_empty() {
        args.push(format!("{}:{}", key, val));
    }
}

pub fn add_task_full(
    desc: &str, project: &str, priority: &str, due: &str, tags: &[String],
    recur: &str, until: &str, wait: &str, scheduled: &str, depends: &str,
) -> Result<()> {
    let mut args = vec!["add".to_string(), desc.to_string()];
    push_attr(&mut args, "project", project);
    push_attr(&mut args, "priority", priority);
    push_attr(&mut args, "due", due);
    push_attr(&mut args, "recur", recur);
    push_attr(&mut args, "until", until);
    push_attr(&mut args, "wait", wait);
    push_attr(&mut args, "scheduled", scheduled);
    push_attr(&mut args, "depends", depends);
    for tag in tags {
        if !tag.is_empty() {
            args.push(format!("+{}", tag));
        }
    }
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    Command::new("task").args(&arg_refs).output()?;
    Ok(())
}

pub fn modify_task(uuid: &str, mods: &[String]) -> Result<()> {
    let mut args = vec!["rc.confirmation=off".to_string(), uuid.to_string(), "modify".to_string()];
    args.extend(mods.iter().cloned());
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    Command::new("task").args(&arg_refs).output()?;
    Ok(())
}

pub fn annotate_task(uuid: &str, text: &str) -> Result<()> {
    Command::new("task")
        .args(["rc.confirmation=off", uuid, "annotate", text])
        .output()?;
    Ok(())
}

pub fn denotate_task(uuid: &str, pattern: &str) -> Result<()> {
    Command::new("task")
        .args(["rc.confirmation=off", uuid, "denotate", pattern])
        .output()?;
    Ok(())
}

pub fn duplicate_task(uuid: &str) -> Result<()> {
    Command::new("task")
        .args(["rc.confirmation=off", uuid, "duplicate"])
        .output()?;
    Ok(())
}

pub fn undo() -> Result<()> {
    Command::new("task")
        .args(["rc.confirmation=off", "undo"])
        .output()?;
    Ok(())
}
