#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDate, TimeDelta, Utc};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::{Action, ActionPerformed, ActionType, NotificationExt};
use warpgate_core::{Store, Upcoming, Uuid, tomorrow_morning};

const POLL: Duration = Duration::from_secs(30);
const MENU_TASKS: usize = 15;
const TRAY: &str = "warpgate";
const REMINDER_ACTIONS: &str = "reminder";

#[derive(Clone, Copy)]
enum Act {
    Done,
    SnoozeHour,
    Tomorrow,
}

impl Act {
    const ALL: [Act; 3] = [Act::Done, Act::SnoozeHour, Act::Tomorrow];

    fn id(self) -> &'static str {
        match self {
            Act::Done => "done",
            Act::SnoozeHour => "snooze",
            Act::Tomorrow => "tomorrow",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Act::Done => "Done",
            Act::SnoozeHour => "Snooze 1h",
            Act::Tomorrow => "Tomorrow 9:00",
        }
    }

    fn parse(id: &str) -> Option<Act> {
        Act::ALL.into_iter().find(|act| act.id() == id)
    }
}

struct State {
    data_dir: PathBuf,
    last_check: Mutex<DateTime<Utc>>,
    shown_menu: Mutex<Option<MenuModel>>,
}

#[derive(PartialEq)]
struct MenuModel {
    header: String,
    tasks: Vec<(Uuid, String)>,
    autostart: bool,
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(State {
                data_dir: data_dir(app)?,
                last_check: Mutex::new(Utc::now()),
                shown_menu: Mutex::default(),
            });

            let notifications = app.notification();
            notifications.register_action_types(vec![
                ActionType::builder(REMINDER_ACTIONS)
                    .actions(
                        Act::ALL.map(|act| Action::builder(act.id(), act.title()).build()).to_vec(),
                    )
                    .build(),
            ])?;
            let handle = app.handle().clone();
            notifications.on_action(move |performed| on_notification_action(&handle, performed))?;

            TrayIconBuilder::with_id(TRAY)
                .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .icon_as_template(true)
                .show_menu_on_left_click(true)
                .on_menu_event(on_menu_event)
                .build(app)?;

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    refresh(&handle).await;
                    tokio::time::sleep(POLL).await;
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build warpgate-tray");

    app.run(|_, event| {
        if let RunEvent::ExitRequested { api, code: None, .. } = event {
            api.prevent_exit();
        }
    });
}

/// `TASKDATA`, else Taskwarrior's default `~/.task`.
fn data_dir(app: &tauri::App) -> tauri::Result<PathBuf> {
    match std::env::var_os("TASKDATA") {
        Some(dir) => Ok(PathBuf::from(dir)),
        None => Ok(app.path().home_dir()?.join(".task")),
    }
}

async fn refresh(app: &AppHandle) {
    let state = app.state::<State>();
    let now = Utc::now();
    let upcoming = match load_upcoming(&state.data_dir).await {
        Ok(upcoming) => upcoming,
        Err(e) => {
            show_menu(app, now, &[], Some(format!("Can't read tasks: {e}")));
            return;
        }
    };
    let since = std::mem::replace(&mut *state.last_check.lock().unwrap(), now);
    for task in upcoming.iter().filter(|u| u.reminder.at > since && u.reminder.at <= now) {
        notify(app, task);
    }
    show_menu(app, now, &upcoming, None);
}

async fn load_upcoming(dir: &Path) -> Result<Vec<Upcoming>, warpgate_core::Error> {
    Store::open(dir).await?.upcoming(&Local).await
}

fn notify(app: &AppHandle, task: &Upcoming) {
    let _ = app
        .notification()
        .builder()
        .title(&task.description)
        .body(task.project.as_deref().unwrap_or("Reminder"))
        .action_type_id(REMINDER_ACTIONS)
        .extra("uuid", task.uuid.to_string())
        .show();
}

fn show_menu(app: &AppHandle, now: DateTime<Utc>, upcoming: &[Upcoming], error: Option<String>) {
    let Some(tray) = app.tray_by_id(TRAY) else { return };
    let today = now.with_timezone(&Local).date_naive();
    let tasks: Vec<(Uuid, String)> = upcoming
        .iter()
        .filter(|u| u.reminder.at.with_timezone(&Local).date_naive() <= today)
        .take(MENU_TASKS)
        .map(|u| (u.uuid, task_label(u, today)))
        .collect();
    let header = error.unwrap_or_else(|| {
        if tasks.is_empty() { "Nothing due today" } else { "Due today" }.to_string()
    });
    let model = MenuModel { header, tasks, autostart: app.autolaunch().is_enabled().unwrap_or(false) };

    // Replacing the menu closes it if it's open, so only swap it when its contents change.
    let state = app.state::<State>();
    let mut shown_menu = state.shown_menu.lock().unwrap();
    if shown_menu.as_ref() != Some(&model) {
        let Ok(menu) = build_menu(app, &model) else { return };
        let _ = tray.set_menu(Some(menu));
        *shown_menu = Some(model);
    }

    let due_now = upcoming.iter().filter(|u| u.reminder.at <= now).count();
    #[cfg(target_os = "macos")]
    let _ = tray.set_title((due_now > 0).then(|| due_now.to_string()));
    let _ = tray.set_tooltip(Some(format!("warpgate: {due_now} due")));
}

fn build_menu(app: &AppHandle, model: &MenuModel) -> tauri::Result<Menu<tauri::Wry>> {
    let mut items: Vec<Box<dyn IsMenuItem<tauri::Wry>>> =
        vec![Box::new(MenuItem::new(app, &model.header, false, None::<&str>)?)];
    for (uuid, label) in &model.tasks {
        let actions = Act::ALL
            .into_iter()
            .map(|act| MenuItem::with_id(app, format!("{}:{uuid}", act.id()), act.title(), true, None::<&str>))
            .collect::<tauri::Result<Vec<_>>>()?;
        let actions: Vec<&dyn IsMenuItem<tauri::Wry>> =
            actions.iter().map(|item| item as &dyn IsMenuItem<tauri::Wry>).collect();
        items.push(Box::new(Submenu::with_items(app, label, true, &actions)?));
    }
    items.push(Box::new(PredefinedMenuItem::separator(app)?));
    items.push(Box::new(CheckMenuItem::with_id(
        app,
        "autostart",
        "Start at login",
        true,
        model.autostart,
        None::<&str>,
    )?));
    items.push(Box::new(MenuItem::with_id(app, "quit", "Quit warpgate", true, None::<&str>)?));

    let items: Vec<&dyn IsMenuItem<tauri::Wry>> = items.iter().map(|item| item.as_ref()).collect();
    Menu::with_items(app, &items)
}

fn task_label(task: &Upcoming, today: NaiveDate) -> String {
    let at = task.reminder.at.with_timezone(&Local);
    let when = if at.date_naive() == today {
        at.format("%H:%M").to_string()
    } else {
        at.format("%a %-m/%-d").to_string()
    };
    let description: String = task.description.chars().take(48).collect();
    let ellipsis = if task.description.chars().count() > 48 { "…" } else { "" };
    format!("{when}  {description}{ellipsis}")
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "quit" => app.exit(0),
        "autostart" => {
            let autolaunch = app.autolaunch();
            let _ = if autolaunch.is_enabled().unwrap_or(false) { autolaunch.disable() } else { autolaunch.enable() };
        }
        id => {
            let Some((act, uuid)) = id.split_once(':') else { return };
            if let (Some(act), Ok(uuid)) = (Act::parse(act), Uuid::parse_str(uuid)) {
                spawn_act(app, act, uuid);
            }
        }
    }
}

fn on_notification_action(app: &AppHandle, performed: &ActionPerformed) {
    let Some(act) = Act::parse(performed.action_id()) else { return };
    let uuid = performed
        .notification()
        .and_then(|n| n.extra().get("uuid"))
        .and_then(|uuid| uuid.as_str())
        .and_then(|uuid| Uuid::parse_str(uuid).ok());
    if let Some(uuid) = uuid {
        spawn_act(app, act, uuid);
    }
}

fn spawn_act(app: &AppHandle, act: Act, uuid: Uuid) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let dir = app.state::<State>().data_dir.clone();
        if let Err(e) = apply(&dir, act, uuid).await {
            let _ = app.notification().builder().title("warpgate").body(format!("Couldn't update the task: {e}")).show();
        }
        refresh(&app).await;
    });
}

async fn apply(dir: &Path, act: Act, uuid: Uuid) -> Result<(), warpgate_core::Error> {
    let mut store = Store::open(dir).await?;
    let now = Utc::now();
    match act {
        Act::Done => store.done(uuid).await,
        Act::SnoozeHour => store.snooze(uuid, now + TimeDelta::hours(1)).await,
        Act::Tomorrow => store.snooze(uuid, tomorrow_morning(now, &Local)).await,
    }
}
