mod app;
mod db;
mod task;
mod taskwarrior;
mod ui;

use std::io;
use std::time::Duration;

use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::{App, InputMode};

fn main() -> Result<()> {
    color_eyre::install()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;
    let mut tick_count: u32 = 0;

    while app.running {
        terminal.draw(|f| ui::draw(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => handle_normal(&mut app, key.code, key.modifiers),
                    InputMode::Help => {
                        app.input_mode = InputMode::Normal;
                    }
                    InputMode::Confirm => handle_confirm(&mut app, key.code),
                    InputMode::TaskForm => handle_task_form(&mut app, key.code),
                    InputMode::Annotate => handle_text_input(&mut app, key.code, true),
                    InputMode::Denotate => handle_denotate(&mut app, key.code),
                    InputMode::Filter => handle_filter(&mut app, key.code),
                }
            }
        } else {
            // Auto-refresh from DB every ~2s when idle in normal mode
            tick_count += 1;
            if tick_count >= 20 && app.input_mode == InputMode::Normal {
                app.refresh();
                tick_count = 0;
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn handle_normal(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        KeyCode::Char('q') => app.running = false,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.running = false,
        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
        KeyCode::Char('h') | KeyCode::Left => app.prev_pane(),
        KeyCode::Char('l') | KeyCode::Right => app.next_pane(),
        KeyCode::Char('1') => app.active_pane = app::Pane::Tasks,
        KeyCode::Char('2') => app.active_pane = app::Pane::Projects,
        // Task-specific actions — only in Tasks pane
        KeyCode::Char('d') if app.active_pane == app::Pane::Tasks => app.request_done(),
        KeyCode::Char('x') if app.active_pane == app::Pane::Tasks => app.request_delete(),
        KeyCode::Char('s') if app.active_pane == app::Pane::Tasks => app.toggle_start_selected(),
        KeyCode::Char('m') if app.active_pane == app::Pane::Tasks => app.open_modify_form(),
        KeyCode::Char('n') if app.active_pane == app::Pane::Tasks => app.open_annotate(),
        KeyCode::Char('N') if app.active_pane == app::Pane::Tasks => app.open_denotate(),
        KeyCode::Char('D') if app.active_pane == app::Pane::Tasks => app.duplicate_selected(),
        // Global actions
        KeyCode::Char('a') => app.open_add_form(),
        KeyCode::Char('u') => app.undo(),
        KeyCode::Char('r') => {
            app.refresh();
            app.status_msg = "Refreshed".to_string();
        }
        KeyCode::Char('/') => app.open_filter(),
        KeyCode::Char(']') if app.active_pane == app::Pane::Tasks => app.next_report(),
        KeyCode::Char('[') if app.active_pane == app::Pane::Tasks => app.prev_report(),
        KeyCode::Char('?') => {
            app.input_mode = InputMode::Help;
        }
        _ => {}
    }
}

fn handle_confirm(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => app.confirm_dismiss(),
        KeyCode::Char('j') | KeyCode::Down => app.confirm_move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.confirm_move_up(),
        KeyCode::Enter => app.confirm_execute(),
        KeyCode::Char(c) => app.confirm_by_key(c),
        _ => {}
    }
}

fn handle_task_form(app: &mut App, key: KeyCode) {
    let picker_active = app
        .task_form
        .as_ref()
        .map(|f| f.dep_picker_active)
        .unwrap_or(false);

    if picker_active {
        match key {
            KeyCode::Tab | KeyCode::Esc => {
                if let Some(ref mut form) = app.task_form {
                    form.close_dep_picker();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut form) = app.task_form {
                    let len = form.filtered_dep_choices().len();
                    if len > 0 && form.dep_picker_cursor < len - 1 {
                        form.dep_picker_cursor += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut form) = app.task_form {
                    if form.dep_picker_cursor > 0 {
                        form.dep_picker_cursor -= 1;
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(ref mut form) = app.task_form {
                    form.toggle_dep();
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut form) = app.task_form {
                    form.dep_picker_filter.pop();
                    form.dep_picker_cursor = 0;
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut form) = app.task_form {
                    form.dep_picker_filter.push(c);
                    form.dep_picker_cursor = 0;
                }
            }
            _ => {}
        }
        return;
    }

    // Form fields focused
    match key {
        KeyCode::Enter => app.submit_task_form(),
        KeyCode::Esc => {
            app.task_form = None;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Tab => {
            // Open dep picker when on Depends field
            if let Some(ref mut form) = app.task_form {
                if form.active_field == crate::app::FormField::Depends {
                    form.open_dep_picker();
                }
            }
        }
        KeyCode::Down => {
            if let Some(ref mut form) = app.task_form {
                form.next_field();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut form) = app.task_form {
                form.prev_field();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut form) = app.task_form {
                form.active_buffer_mut().pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut form) = app.task_form {
                form.active_buffer_mut().push(c);
            }
        }
        _ => {}
    }
}

fn handle_text_input(app: &mut App, key: KeyCode, is_annotate: bool) {
    match key {
        KeyCode::Enter => {
            if is_annotate {
                app.submit_annotation();
            }
        }
        KeyCode::Esc => {
            app.input_buffer.clear();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}

fn handle_denotate(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('j') | KeyCode::Down => app.denotate_move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.denotate_move_up(),
        KeyCode::Enter => app.submit_denotate(),
        KeyCode::Esc | KeyCode::Char('q') => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

fn handle_filter(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter => app.submit_filter(),
        KeyCode::Esc => app.clear_filter(),
        KeyCode::Backspace => {
            app.input_buffer.pop();
            app.filter_changed();
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
            app.filter_changed();
        }
        _ => {}
    }
}
