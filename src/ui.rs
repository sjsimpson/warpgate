use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
        TableState, Wrap,
    },
};

use crate::app::{App, InputMode, Pane};

pub fn draw(f: &mut Frame, app: &App) {
    let [main_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(f.area());

    let [left_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
            .areas(main_area);

    let [tasks_area, projects_area, stats_area] = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .areas(left_area);

    draw_tasks(f, app, tasks_area);
    draw_projects(f, app, projects_area);
    draw_stats(f, app, stats_area);
    draw_detail(f, app, detail_area);
    draw_status(f, app, status_area);

    match app.input_mode {
        InputMode::Help => draw_help(f, app),
        InputMode::Confirm => draw_confirm(f, app),
        InputMode::TaskForm => draw_task_form(f, app),
        InputMode::Annotate => draw_annotate(f, app),
        InputMode::Denotate => draw_denotate(f, app),
        InputMode::Normal | InputMode::Filter => {}
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
        .map(|(_, task)| {
            let style = match task.status.as_deref() {
                _ if task.is_started() => Style::default().fg(Color::Green),
                Some("completed") => Style::default().fg(Color::DarkGray),
                Some("deleted") => Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
                Some("recurring") => Style::default().fg(Color::Magenta),
                Some("waiting") => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::DIM),
                _ => Style::default().fg(Color::White), // pending
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

    let mut title_spans: Vec<Span> = vec![
        Span::styled(" [1] ", pane_num_style),
        Span::styled(
            "Tasks ",
            if app.active_pane == Pane::Tasks {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        ),
    ];
    for report in crate::app::ALL_REPORTS {
        if *report == app.active_report {
            title_spans.push(Span::styled(
                format!(" {} ", report.label()),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            title_spans.push(Span::styled(
                format!(" {} ", report.label()),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
    title_spans.push(Span::raw(" "));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Line::from(title_spans))
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
    .block(block)
    .row_highlight_style(if app.active_pane == Pane::Tasks {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default()
    });

    let selected = if app.active_pane == Pane::Tasks {
        Some(app.selected_task)
    } else {
        None
    };
    let mut state = TableState::default().with_selected(selected);
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_projects(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.active_pane == Pane::Projects;

    let items: Vec<ListItem> = app
        .project_list_items()
        .iter()
        .enumerate()
        .map(|(_, name)| {
            let prefix = "  ";
            let display = if name == "(all)" {
                format!("{}(all)", prefix)
            } else {
                let depth = name.matches('.').count();
                let leaf = name.rsplit('.').next().unwrap_or(name);
                format!("{}{}{}", prefix, "  ".repeat(depth), leaf)
            };
            ListItem::new(display).style(Style::default().fg(Color::White))
        })
        .collect();

    let pane_num_style = if focused {
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
                if focused {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]))
        .border_style(Style::default().fg(pane_border_color(app, Pane::Projects)));

    let highlight = if focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
    };

    let list = List::new(items).block(block).highlight_style(highlight,
    );

    let mut state = ListState::default().with_selected(Some(app.selected_project));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_stats(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let label = Style::default().fg(Color::Yellow);
    let val = Style::default().fg(Color::White);
    match app.selected_project_summary() {
        Some(summary) => {
            // Single project stats
            let pct = if summary.total > 0 {
                (summary.completed as f64 / summary.total as f64 * 100.0) as u16
            } else {
                0
            };

            let age_str = if summary.avg_age_days < 1.0 {
                "today".to_string()
            } else if summary.avg_age_days < 7.0 {
                format!("{:.0}d", summary.avg_age_days)
            } else if summary.avg_age_days < 30.0 {
                format!("{:.0}w", summary.avg_age_days / 7.0)
            } else {
                format!("{:.0}mo", summary.avg_age_days / 30.0)
            };

            let prefix = format!(" {:>3}% ", pct);
            let bar_len = (area.width as usize).saturating_sub(prefix.len() + 2);
            let filled = if bar_len > 0 {
                (bar_len * pct as usize) / 100
            } else {
                0
            };
            let empty = bar_len.saturating_sub(filled);
            let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

            let dim = Style::default().fg(Color::DarkGray);

            let mut lines = vec![
                Line::from(vec![
                    Span::styled(prefix, val),
                    Span::styled(bar, Style::default().fg(Color::Green)),
                ]),
                Line::from(vec![
                    Span::styled(" Remaining    ", label),
                    Span::styled(format!("{}", summary.remaining), val),
                    Span::styled("   ", val),
                    Span::styled("Completed  ", label),
                    Span::styled(format!("{}", summary.completed), val),
                    Span::styled("   ", val),
                    Span::styled("Avg age  ", label),
                    Span::styled(age_str, val),
                ]),
            ];

            // Sub-project breakdown
            let children: Vec<&crate::app::ProjectSummary> = app
                .project_summaries
                .iter()
                .filter(|s| {
                    s.name != summary.name
                        && s.name.starts_with(&summary.name)
                        && s.name[summary.name.len()..].starts_with('.')
                        && s.name[summary.name.len() + 1..].find('.').is_none()
                        && s.remaining > 0
                })
                .collect();

            if !children.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " Sub-projects",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                for child in &children {
                    let child_leaf = child.name.rsplit('.').next().unwrap_or(&child.name);
                    let child_pct = if child.total > 0 {
                        (child.completed as f64 / child.total as f64 * 100.0) as u16
                    } else {
                        0
                    };
                    lines.push(Line::from(vec![
                        Span::styled(format!("   {:<14}", child_leaf), val),
                        Span::styled(format!("{:>3}%", child_pct), dim),
                        Span::styled(format!("  {} left", child.remaining), dim),
                    ]));
                }
            }

            // Upcoming due dates
            let filtered = app.filtered_tasks();
            let mut upcoming: Vec<(&str, String)> = filtered
                .iter()
                .filter(|t| t.due.is_some())
                .map(|t| (t.description.as_str(), t.due_short()))
                .collect();
            upcoming.truncate(5);

            if !upcoming.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " Upcoming",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                for (desc, due) in &upcoming {
                    let short_desc: String = desc.chars().take(20).collect();
                    lines.push(Line::from(vec![
                        Span::styled(format!("   {} ", due), dim),
                        Span::styled(short_desc, val),
                    ]));
                }
            }

            let title = format!(" {} ", summary.name);
            let paragraph = Paragraph::new(lines)
                .block(block.title(title))
                .wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
        None => {
            // All projects overview — skip 100% complete, indent sub-projects
            let active: Vec<&crate::app::ProjectSummary> = app
                .project_summaries
                .iter()
                .filter(|s| s.remaining > 0)
                .collect();

            // Build display names with indentation for sub-projects
            let display_names: Vec<String> = active
                .iter()
                .map(|s| {
                    if let Some(dot_pos) = s.name.rfind('.') {
                        format!("  {}", &s.name[dot_pos + 1..])
                    } else {
                        s.name.clone()
                    }
                })
                .collect();

            let max_name_len = display_names.iter().map(|n| n.len()).max().unwrap_or(8);
            let name_col = max_name_len + 2; // padding

            let max_remaining_len = active
                .iter()
                .map(|s| format!("{}", s.remaining).len())
                .max()
                .unwrap_or(1);

            let mut lines: Vec<Line> = Vec::new();
            for (i, summary) in active.iter().enumerate() {
                let pct = if summary.total > 0 {
                    (summary.completed as f64 / summary.total as f64 * 100.0) as u16
                } else {
                    0
                };

                let prefix = format!(
                    " {:<name_w$} {:>rem_w$} left {:>3}% ",
                    display_names[i],
                    summary.remaining,
                    pct,
                    name_w = name_col,
                    rem_w = max_remaining_len,
                );

                let bar_len = (area.width as usize).saturating_sub(prefix.len() + 2);
                let filled = if bar_len > 0 {
                    (bar_len * pct as usize) / 100
                } else {
                    0
                };
                let empty = bar_len.saturating_sub(filled);
                let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

                lines.push(Line::from(vec![
                    Span::styled(prefix, val),
                    Span::styled(bar, Style::default().fg(Color::Green)),
                ]));
            }

            if lines.is_empty() {
                lines.push(Line::from(Span::styled(
                    " All projects complete!",
                    Style::default().fg(Color::Green),
                )));
            }

            let (remaining, completed) = app.all_projects_summary();
            let title = format!(" Summary ({} pending, {} done) ", remaining, completed);
            let paragraph = Paragraph::new(lines).block(block.title(title));
            f.render_widget(paragraph, area);
        }
    }
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
    if app.input_mode == InputMode::Filter {
        let input_line = Line::from(vec![
            Span::styled(
                " /",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&app.input_buffer, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::DarkGray)),
        ]);
        let paragraph = Paragraph::new(input_line);
        f.render_widget(paragraph, area);
        return;
    }

    let filter_label = if app.filter_text.is_empty() {
        String::new()
    } else {
        format!(" filter: {}", app.filter_text)
    };
    let help = "  ?:help  a:add  m:modify  d:done  x:del  /:filter  [/]:views ";

    let status_line = if app.status_msg.is_empty() {
        Line::from(vec![
            Span::styled(&filter_label, Style::default().fg(Color::Yellow)),
            Span::styled(help, Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(&filter_label, Style::default().fg(Color::Yellow)),
            Span::styled(" ", Style::default()),
            Span::styled(&app.status_msg, Style::default().fg(Color::Green)),
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
        .map(|o| o.label.len() + 6)
        .max()
        .unwrap_or(10) as u16;
    let width = (app.confirm_msg.len() as u16 + 4)
        .max(max_option_len + 4)
        .max(30);
    let height = app.confirm_options.len() as u16 + 4;
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
    } else {
        " New Task "
    };

    let fields = form.visible_fields();
    let term = f.area();
    let w = (term.width * 17 / 20).min(140);
    let h = (term.height * 4 / 5).min(40);
    let area = centered_rect_abs(w, h, term);
    f.render_widget(Clear, area);

    if form.dep_picker_active {
        // Split: form left, dep picker right
        let [form_area, picker_area] =
            Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
                .areas(area);

        // Form pane (dimmed)
        let form_lines = build_form_lines(form, &fields);
        let form_widget = Paragraph::new(form_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(format!("{}", title.trim()))
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(form_widget, form_area);

        // Dep picker pane
        let visible = form.filtered_dep_choices();
        let items: Vec<ListItem> = visible
            .iter()
            .enumerate()
            .map(|(vi, &idx)| {
                let choice = &form.dep_choices[idx];
                let check = if choice.selected { "[x]" } else { "[ ]" };
                let id_str = choice.id.map(|i| format!("{}", i)).unwrap_or_default();
                let proj = choice.project.as_deref().unwrap_or("");
                let text = if proj.is_empty() {
                    format!("  {} {:>3}  {}", check, id_str, choice.description)
                } else {
                    format!("  {} {:>3}  {}  ({})", check, id_str, choice.description, proj)
                };
                let style = if vi == form.dep_picker_cursor {
                    Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else if choice.selected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let filter_display = if form.dep_picker_filter.is_empty() {
            String::new()
        } else {
            format!("  filter: {}", form.dep_picker_filter)
        };
        let picker_title = format!(
            " Dependencies (j/k:nav  Enter:toggle  Tab/Esc:done){} ",
            filter_display
        );
        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(picker_title)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(list, picker_area);
    } else {
        // Single-pane form
        let dep_hint = if form.active_field == crate::app::FormField::Depends {
            " (Tab:pick deps)"
        } else {
            ""
        };
        let form_title = format!(
            "{} (Up/Down:fields  Enter:submit  Esc:cancel){}",
            title.trim(),
            dep_hint
        );
        let form_lines = build_form_lines(form, &fields);
        let form_widget = Paragraph::new(form_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(form_title)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(form_widget, area);
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
        if *field == crate::app::FormField::Description {
            if is_active {
                let display = format!("    {}_", value);
                lines.push(Line::from(Span::styled(
                    display,
                    Style::default().fg(Color::White),
                )));
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
    let term = f.area();
    let w = (term.width * 3 / 4).min(100);
    let area = centered_rect_abs(w, 6, term);
    f.render_widget(Clear, area);

    let input = Paragraph::new(app.input_buffer.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Add Annotation (Enter:submit  Esc:cancel) ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    f.render_widget(input, area);

    // Place cursor accounting for wrapping
    let inner_width = (area.width - 2) as usize;
    let cursor_pos = app.input_buffer.len();
    let cursor_y = area.y + 1 + (cursor_pos / inner_width) as u16;
    let cursor_x = area.x + 1 + (cursor_pos % inner_width) as u16;
    f.set_cursor_position((cursor_x, cursor_y));
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

    let term = f.area();
    let w = (term.width * 3 / 4).min(100);
    let height = (annotations.len() as u16 * 2 + 2).min(term.height * 3 / 4);
    let area = centered_rect_abs(w, height, term);
    f.render_widget(Clear, area);

    let dim = Style::default().fg(Color::DarkGray);
    let val = Style::default().fg(Color::White);

    let items: Vec<ListItem> = annotations
        .iter()
        .enumerate()
        .map(|(i, ann)| {
            let date = ann.entry.as_deref().map(format_date).unwrap_or_default();
            let desc = ann.description.as_deref().unwrap_or("");
            let lines = vec![
                Line::from(Span::styled(format!("  {}", date), dim)),
                Line::from(Span::styled(format!("  {}", desc), val)),
            ];
            let style = if i == app.selected_annotation {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(lines).style(style)
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

fn draw_help(f: &mut Frame, app: &App) {
    let section_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);

    let mut groups: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "Navigation",
            vec![
                ("j / k / Up / Down", "Navigate items"),
                ("h / l / Left / Right", "Switch pane"),
                ("1 / 2", "Jump to Tasks / Projects"),
            ],
        ),
    ];

    if app.active_pane == Pane::Tasks {
        groups.push((
            "Tasks",
            vec![
                ("a", "Add new task"),
                ("m", "Modify selected task"),
                ("d", "Mark task done"),
                ("x", "Delete task"),
                ("s", "Start / stop task"),
                ("D", "Duplicate task"),
            ],
        ));
        groups.push((
            "Annotations",
            vec![("n", "Add annotation"), ("N", "Remove annotation")],
        ));
        groups.push((
            "Views",
            vec![
                ("[ / ]", "Cycle views (pending/all/done/overdue/active)"),
            ],
        ));
    } else {
        groups.push((
            "Tasks",
            vec![
                ("a", "Add new task"),
            ],
        ));
    }

    groups.push((
        "General",
        vec![
            ("/", "Filter tasks"),
            ("u", "Undo last action"),
            ("r", "Refresh from taskwarrior"),
            ("?", "Toggle this help"),
            ("q / Ctrl+C", "Quit"),
        ],
    ));

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
    let area = centered_rect_abs(70, height, f.area());
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
