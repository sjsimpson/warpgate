use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap,
    },
};

use crate::app::{App, InputMode, Pane};

pub fn draw(f: &mut Frame, app: &App) {
    let [main_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(f.area());

    let [left_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
            .areas(main_area);

    let [tasks_area, projects_area] =
        Layout::vertical([Constraint::Percentage(65), Constraint::Percentage(35)]).areas(left_area);

    draw_tasks(f, app, tasks_area);
    draw_projects(f, app, projects_area);
    draw_detail(f, app, detail_area);
    draw_status(f, app, status_area);

    match app.input_mode {
        InputMode::Help => draw_help(f),
        InputMode::Confirm => draw_confirm(f, app),
        InputMode::TaskForm => draw_task_form(f, app),
        InputMode::Annotate => draw_annotate(f, app),
        InputMode::Denotate => draw_denotate(f, app),
        InputMode::Normal => {}
    }
}

fn pane_border_color(app: &App, pane: Pane) -> Color {
    if app.active_pane == pane {
        Color::Cyan
    } else {
        Color::DarkGray
    }
}

fn draw_tasks(f: &mut Frame, app: &App, area: Rect) {
    let filtered = app.filtered_tasks();

    let header = Row::new(vec![
        Cell::from("ID"),
        Cell::from("Age"),
        Cell::from("Description"),
        Cell::from("Project"),
        Cell::from("Recur"),
        Cell::from("Due"),
        Cell::from("Pri"),
        Cell::from("Urg"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let style = if i == app.selected_task {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if task.is_started() {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            let desc_cell = if task.is_recurring() {
                Cell::from(Line::from(vec![
                    Span::styled("\u{27F3} ", Style::default().fg(Color::Magenta)),
                    Span::raw(&task.description),
                ]))
            } else {
                Cell::from(task.description.as_str())
            };

            Row::new(vec![
                Cell::from(task.id.map(|id| id.to_string()).unwrap_or_default()),
                Cell::from(task.age()),
                desc_cell,
                Cell::from(task.project.clone().unwrap_or_default()),
                Cell::from(task.recur_short()),
                Cell::from(task.due_short()),
                Cell::from(task.priority.clone().unwrap_or_default()),
                Cell::from(
                    task.urgency
                        .map(|u| format!("{:.1}", u))
                        .unwrap_or_default(),
                ),
            ])
            .style(style)
        })
        .collect();

    let pane_num_style = if app.active_pane == Pane::Tasks {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Line::from(vec![
            Span::styled(" [1] ", pane_num_style),
            Span::styled(
                "Tasks ",
                if app.active_pane == Pane::Tasks {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]))
        .border_style(Style::default().fg(pane_border_color(app, Pane::Tasks)));

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Min(20),
            Constraint::Length(16),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(4),
            Constraint::Length(6),
        ],
    )
    .header(header)
    .block(block);

    f.render_widget(table, area);
}

fn draw_projects(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .project_list_items()
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == app.selected_project {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("  {}", name)).style(style)
        })
        .collect();

    let pane_num_style = if app.active_pane == Pane::Projects {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Line::from(vec![
            Span::styled(" [2] ", pane_num_style),
            Span::styled(
                "Projects ",
                if app.active_pane == Pane::Projects {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]))
        .border_style(Style::default().fg(pane_border_color(app, Pane::Projects)));

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Detail ")
        .border_style(Style::default().fg(pane_border_color(app, Pane::Detail)));

    let task = app.selected_task_detail();

    let content = match task {
        Some(t) => {
            let mut lines: Vec<Line> = Vec::new();
            let label = Style::default().fg(Color::Yellow);
            let label_bold = Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD);
            let val = Style::default().fg(Color::White);
            let dim = Style::default().fg(Color::DarkGray);

            lines.push(Line::from(vec![
                Span::styled("Description  ", label_bold),
                Span::styled(&t.description, val),
            ]));

            if let Some(ref status) = t.status {
                lines.push(Line::from(vec![
                    Span::styled("Status       ", label),
                    Span::styled(status, val),
                ]));
            }

            if let Some(ref project) = t.project {
                lines.push(Line::from(vec![
                    Span::styled("Project      ", label),
                    Span::styled(project, val),
                ]));
            }

            if let Some(ref pri) = t.priority {
                lines.push(Line::from(vec![
                    Span::styled("Priority     ", label),
                    Span::styled(pri, val),
                ]));
            }

            if let Some(ref due) = t.due {
                lines.push(Line::from(vec![
                    Span::styled("Due          ", label),
                    Span::styled(format_date(due), val),
                ]));
            }

            if let Some(urg) = t.urgency {
                lines.push(Line::from(vec![
                    Span::styled("Urgency      ", label),
                    Span::styled(format!("{:.2}", urg), val),
                ]));
            }

            if let Some(ref recur) = t.recur {
                lines.push(Line::from(vec![
                    Span::styled("Recur        ", label),
                    Span::styled(recur, val),
                ]));
            }

            if let Some(ref until) = t.until {
                lines.push(Line::from(vec![
                    Span::styled("Until        ", label),
                    Span::styled(format_date(until), val),
                ]));
            }

            if let Some(ref wait) = t.wait {
                lines.push(Line::from(vec![
                    Span::styled("Wait         ", label),
                    Span::styled(format_date(wait), val),
                ]));
            }

            if let Some(ref scheduled) = t.scheduled {
                lines.push(Line::from(vec![
                    Span::styled("Scheduled    ", label),
                    Span::styled(format_date(scheduled), val),
                ]));
            }

            if let Some(ref tags) = t.tags {
                if !tags.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Tags         ", label),
                        Span::styled(tags.join(", "), val),
                    ]));
                }
            }

            if let Some(ref depends) = t.depends {
                lines.push(Line::from(vec![
                    Span::styled("Depends      ", label),
                    Span::styled(depends, val),
                ]));
            }

            if let Some(ref uuid) = t.uuid {
                lines.push(Line::from(vec![
                    Span::styled("UUID         ", label),
                    Span::styled(uuid, dim),
                ]));
            }

            if let Some(ref entry) = t.entry {
                lines.push(Line::from(vec![
                    Span::styled("Created      ", label),
                    Span::styled(format_date(entry), val),
                ]));
            }

            if let Some(ref modified) = t.modified {
                lines.push(Line::from(vec![
                    Span::styled("Modified     ", label),
                    Span::styled(format_date(modified), val),
                ]));
            }

            if let Some(ref annotations) = t.annotations {
                if !annotations.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled("Annotations", label_bold)));
                    for ann in annotations {
                        let date = ann.entry.as_deref().map(format_date).unwrap_or_default();
                        let desc = ann.description.as_deref().unwrap_or("");
                        lines.push(Line::from(vec![
                            Span::styled(format!("  {} ", date), dim),
                            Span::styled(desc, val),
                        ]));
                    }
                }
            }

            lines
        }
        None => {
            vec![Line::from(Span::styled(
                "No task selected",
                Style::default().fg(Color::DarkGray),
            ))]
        }
    };

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let help = " ?:help  a:add  m:modify  n/N:note  d:done  x:del  s:start  D:dup  u:undo ";
    let status_line = if app.status_msg.is_empty() {
        Line::from(vec![Span::styled(
            help,
            Style::default().fg(Color::DarkGray),
        )])
    } else {
        Line::from(vec![
            Span::styled(&app.status_msg, Style::default().fg(Color::Green)),
            Span::styled("  ", Style::default()),
            Span::styled(help, Style::default().fg(Color::DarkGray)),
        ])
    };

    let paragraph = Paragraph::new(status_line);
    f.render_widget(paragraph, area);
}

// --- Overlays ---

fn draw_confirm(f: &mut Frame, app: &App) {
    let max_option_len = app
        .confirm_options
        .iter()
        .map(|o| o.label.len() + 6) // " k  Label "
        .max()
        .unwrap_or(10) as u16;
    let width = (app.confirm_msg.len() as u16 + 4)
        .max(max_option_len + 4)
        .max(30);
    let height = app.confirm_options.len() as u16 + 4; // msg + blank + options + borders
    let area = centered_rect_abs(width, height, f.area());

    f.render_widget(Clear, area);

    let sel_style = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);
    let key_style = Style::default().fg(Color::Yellow);

    let mut lines = vec![
        Line::from(Span::styled(
            &app.confirm_msg,
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    for (i, opt) in app.confirm_options.iter().enumerate() {
        let label_style = if i == app.confirm_selected {
            sel_style
        } else {
            dim
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", opt.key), key_style),
            Span::styled(format!(" {} ", opt.label), label_style),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Confirm ")
            .border_style(Style::default().fg(Color::Red)),
    );

    f.render_widget(paragraph, area);
}

fn draw_task_form(f: &mut Frame, app: &App) {
    let form = match &app.task_form {
        Some(f) => f,
        None => return,
    };

    let title = if form.editing_uuid.is_some() {
        " Modify Task "
    } else if form.is_advanced {
        " New Task (Advanced) "
    } else {
        " New Task "
    };

    let fields = form.visible_fields();

    {
        // Floating window — ~85% of terminal, centered
        let term = f.area();
        let w = (term.width * 17 / 20).min(140);
        let h = (term.height * 4 / 5).min(40);
        let area = centered_rect_abs(w, h, term);
        f.render_widget(Clear, area);

        let [form_area, docs_area] =
            Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
                .areas(area);

        let form_border = if form.docs_focused {
            Color::DarkGray
        } else {
            Color::Yellow
        };
        let docs_border = if form.docs_focused {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        // Form pane
        let form_title = if form.docs_focused {
            format!("{}", title.trim())
        } else {
            format!(
                "{} (Up/Down:fields  Tab:docs  Enter:submit  Esc:cancel)",
                title.trim()
            )
        };
        let form_lines = build_form_lines(form, &fields);
        let form_widget = Paragraph::new(form_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(form_title)
                    .border_style(Style::default().fg(form_border)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(form_widget, form_area);

        // Docs pane
        let docs_title = if form.docs_focused {
            format!(
                " {} (j/k:scroll  Tab:back  Esc:back) ",
                form.active_field.label()
            )
        } else {
            format!(" {} ", form.active_field.label())
        };
        let docs_text = form.active_field.docs();
        let docs_lines: Vec<Line> = docs_text
            .lines()
            .map(|l| {
                Line::from(Span::styled(
                    format!(" {}", l),
                    Style::default().fg(Color::White),
                ))
            })
            .collect();
        let docs_widget = Paragraph::new(docs_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(docs_title)
                    .border_style(Style::default().fg(docs_border)),
            )
            .wrap(Wrap { trim: false })
            .scroll((form.docs_scroll, 0));
        f.render_widget(docs_widget, docs_area);
    }
}

fn build_form_lines(
    form: &crate::app::TaskForm,
    fields: &[crate::app::FormField],
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    let section_style = Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD);

    for field in fields {
        // Section headers for visual grouping
        if let Some(header) = field.section_header() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                format!("  ── {} ──", header),
                section_style,
            )));
        }

        let is_active = *field == form.active_field;
        let label_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let indicator = if is_active { "> " } else { "  " };
        lines.push(Line::from(vec![
            Span::styled(indicator.to_string(), label_style),
            Span::styled(field.label().to_string(), label_style),
        ]));

        let value = form.get_field(*field);
        // Description gets extra height for multiline display
        if *field == crate::app::FormField::Description {
            if is_active {
                // Show value with cursor, wrapping across multiple visual lines
                let display = format!("    {}_", value);
                lines.push(Line::from(Span::styled(
                    display,
                    Style::default().fg(Color::White),
                )));
                // Reserve visual space
                lines.push(Line::from(""));
            } else if value.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    -",
                    Style::default().fg(Color::White),
                )));
                lines.push(Line::from(""));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("    {}", value),
                    Style::default().fg(Color::White),
                )));
                lines.push(Line::from(""));
            }
        } else {
            let val_display = if is_active {
                format!("    {}_", value)
            } else if value.is_empty() {
                "    -".to_string()
            } else {
                format!("    {}", value)
            };
            lines.push(Line::from(Span::styled(
                val_display,
                Style::default().fg(Color::White),
            )));
        }
    }
    lines
}

fn draw_annotate(f: &mut Frame, app: &App) {
    let area = centered_rect_abs(60, 3, f.area());
    f.render_widget(Clear, area);

    let input = Paragraph::new(app.input_buffer.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Add Annotation (Enter:submit  Esc:cancel) ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(input, area);
    f.set_cursor_position((area.x + app.input_buffer.len() as u16 + 1, area.y + 1));
}

fn draw_denotate(f: &mut Frame, app: &App) {
    let task = match app.selected_task_detail() {
        Some(t) => t,
        None => return,
    };
    let annotations = match &task.annotations {
        Some(a) if !a.is_empty() => a,
        _ => return,
    };

    let height = annotations.len() as u16 + 2;
    let area = centered_rect_abs(70, height.min(20), f.area());
    f.render_widget(Clear, area);

    let items: Vec<ListItem> = annotations
        .iter()
        .enumerate()
        .map(|(i, ann)| {
            let date = ann.entry.as_deref().map(format_date).unwrap_or_default();
            let desc = ann.description.as_deref().unwrap_or("");
            let text = format!("  {} {}", date, desc);
            let style = if i == app.selected_annotation {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Remove Annotation (j/k:nav  Enter:remove  Esc:cancel) ")
            .border_style(Style::default().fg(Color::Red)),
    );

    f.render_widget(list, area);
}

fn draw_help(f: &mut Frame) {
    let section_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);

    let groups: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "Navigation",
            vec![
                ("j / DownArrow", "Next item"),
                ("k / UpArrow", "Prev item"),
                ("l / RightArrow", "Next pane"),
                ("h / LeftArrow", "Prev pane"),
            ],
        ),
        (
            "Tasks",
            vec![
                ("a", "Add new task"),
                ("m", "Modify selected task"),
                ("d", "Mark task done"),
                ("x", "Delete task"),
                ("s", "Start / stop task"),
                ("D", "Duplicate task"),
            ],
        ),
        (
            "Annotations",
            vec![("n", "Add annotation"), ("N", "Remove annotation")],
        ),
        (
            "General",
            vec![
                ("u", "Undo last action"),
                ("r", "Refresh from taskwarrior"),
                ("?", "Toggle this help"),
                ("q / Ctrl+C", "Quit"),
            ],
        ),
    ];

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, (section, binds)) in groups.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            format!("  {}", section),
            section_style,
        )));
        for (key, desc) in binds {
            lines.push(Line::from(vec![
                Span::styled(format!("    {:<26}", key), key_style),
                Span::styled(*desc, desc_style),
            ]));
        }
    }
    lines.push(Line::from(""));

    let height = lines.len() as u16 + 2;
    let area = centered_rect_abs(60, height, f.area());
    f.render_widget(Clear, area);

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Keybindings (press any key to close) ")
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(paragraph, area);
}

// --- Helpers ---

fn centered_rect_abs(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn format_date(raw: &str) -> String {
    if raw.len() >= 15 {
        format!(
            "{}-{}-{} {}:{}",
            &raw[0..4],
            &raw[4..6],
            &raw[6..8],
            &raw[9..11],
            &raw[11..13],
        )
    } else {
        raw.to_string()
    }
}
