mod app;
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
                }
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
        KeyCode::Char('d') => app.request_done(),
        KeyCode::Char('x') => app.request_delete(),
        KeyCode::Char('s') => app.toggle_start_selected(),
        KeyCode::Char('a') => app.open_add_form(),
        KeyCode::Char('m') => app.open_modify_form(),
        KeyCode::Char('n') => app.open_annotate(),
        KeyCode::Char('N') => app.open_denotate(),
        KeyCode::Char('D') => app.duplicate_selected(),
        KeyCode::Char('u') => app.undo(),
        KeyCode::Char('r') => {
            app.refresh();
            app.status_msg = "Refreshed".to_string();
        }
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
    let is_docs = app.task_form.as_ref().map(|f| f.docs_focused).unwrap_or(false);

    if is_docs {
        // Docs pane focused — scroll or switch back
        match key {
            KeyCode::Tab | KeyCode::Esc => {
                if let Some(ref mut form) = app.task_form {
                    form.docs_focused = false;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut form) = app.task_form {
                    form.docs_scroll = form.docs_scroll.saturating_add(1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut form) = app.task_form {
                    form.docs_scroll = form.docs_scroll.saturating_sub(1);
                }
            }
            KeyCode::Char('q') => {
                app.task_form = None;
                app.input_mode = InputMode::Normal;
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
            if let Some(ref mut form) = app.task_form {
                if form.is_advanced {
                    form.docs_focused = true;
                } else {
                    form.next_field();
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
