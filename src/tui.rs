use crate::{
    app::{App, CommandModalFocus, KeyAction, Mode, ToastLevel},
    layout::grid_for_count,
    preset::{resolve_preset_path, resume_path},
};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::{io, sync::mpsc, time::Duration};

pub fn run_tui(mut app: App) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let (tx, rx) = mpsc::channel();

    let result = loop {
        while let Ok(msg) = rx.try_recv() {
            app.handle_command_result(msg);
        }
        app.tick(&tx);
        terminal.draw(|frame| draw(frame, &app))?;

        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key) => match app.handle_key(key) {
                    KeyAction::None => {}
                    KeyAction::Quit => {
                        app.should_quit = true;
                    }
                    KeyAction::SaveResumeAndQuit => {
                        let path = resume_path()?;
                        app.save_resume_view(path)?;
                        app.should_quit = true;
                    }
                    KeyAction::SavePreset(name) => {
                        if name.is_empty() {
                            app.mode = Mode::Normal;
                        } else {
                            let path = resolve_preset_path(&name)?;
                            app.save_named_preset(path, name.clone())?;
                            app.mode = Mode::Normal;
                            app.toast = Some(crate::app::Toast {
                                message: format!("saved preset `{name}`"),
                                level: ToastLevel::Success,
                                created: std::time::Instant::now(),
                            });
                        }
                    }
                    KeyAction::ApplyVars => {
                        app.apply_vars()?;
                    }
                },
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    let rects =
                        pane_rects(Rect::new(0, 0, size.width, size.height), app.panes.len());
                    app.handle_mouse(mouse, &rects);
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if app.should_quit {
            break Ok(());
        }
    };

    app.kill_running_commands();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    result
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.background)),
        area,
    );
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let pane_regions = pane_rects(chunks[0], app.panes.len());
    for (idx, rect) in &pane_regions {
        let pane = &app.panes[*idx];
        let focused = *idx == app.focused;
        let border_style = if focused {
            Style::default().fg(app.theme.border_focused)
        } else {
            Style::default().fg(app.theme.border)
        };
        let title_style = if focused {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.foreground)
        };
        let status = if app.global_paused || pane.paused {
            Span::styled(" ● ", Style::default().fg(app.theme.warning))
        } else if pane.running {
            Span::styled(" ● ", Style::default().fg(app.theme.accent))
        } else if let Some(code) = pane.last_exit_code {
            let color = if code == 0 {
                app.theme.success
            } else {
                app.theme.error
            };
            Span::styled(" ● ", Style::default().fg(color))
        } else if pane.last_error.is_some() {
            Span::styled(" ● ", Style::default().fg(app.theme.error))
        } else {
            Span::styled(" ● ", Style::default().fg(app.theme.muted))
        };
        let block = Block::bordered()
            .borders(Borders::ALL)
            .border_set(ratatui::symbols::border::ROUNDED)
            .border_style(border_style)
            .style(Style::default().bg(app.theme.panel))
            .title(Line::from(vec![
                Span::styled(format!(" {} ", pane.title), title_style),
                Span::styled(
                    truncate(&pane.cmd, rect.width.saturating_sub(40) as usize),
                    Style::default().fg(app.theme.muted),
                ),
                status,
            ]));
        let inner = block.inner(*rect);
        frame.render_widget(block, *rect);
        draw_interval_controls(frame, app, *rect, pane.interval_ms);
        let content_area = inner.inner(Margin::new(1, 0));
        if pane.cmd.is_empty() {
            draw_empty_pane(frame, app, pane, content_area, focused);
        } else {
            let para = Paragraph::new(pane.output_text())
                .style(Style::default().fg(app.theme.foreground))
                .wrap(Wrap { trim: false })
                .scroll((pane.scroll, 0));
            frame.render_widget(para, content_area);
        }
    }

    let hint_width = mode_hints_width(app).min(chunks[1].width.saturating_sub(8));
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(hint_width)])
        .split(chunks[1]);
    let left_status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", mode_label(app.mode)),
            Style::default()
                .fg(app.theme.background)
                .bg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            app.toast
                .as_ref()
                .map(|toast| format!(" {}", toast.message))
                .unwrap_or_default(),
            toast_style(app),
        ),
    ]))
    .style(Style::default().bg(app.theme.panel))
    .alignment(Alignment::Left);
    let right_status = Paragraph::new(Line::from(mode_hint_spans(app)))
        .style(Style::default().bg(app.theme.panel))
        .alignment(Alignment::Right);
    frame.render_widget(left_status, status_chunks[0]);
    frame.render_widget(right_status, status_chunks[1]);

    match app.mode {
        Mode::InlineCommand => {}
        Mode::CommandModal => {
            draw_command_modal(frame, app);
        }
        Mode::DeleteConfirm => draw_delete_confirm_modal(frame, app),
        Mode::TitleModal => {
            draw_input_modal(
                frame,
                app,
                "Rename pane",
                &app.title_input.value,
                "Enter to save · Esc to cancel",
            );
        }
        Mode::SaveModal => {
            draw_save_modal(frame, app);
        }
        Mode::VarsModal => draw_vars_modal(frame, app),
        Mode::Help => draw_help(frame, app),
        Mode::Normal => {}
    }

    if app.mode == Mode::InlineCommand {
        let rects = pane_rects(chunks[0], app.panes.len());
        if let Some((_, rect)) = rects.iter().find(|(idx, _)| *idx == app.focused) {
            let content_area = rect.inner(Margin::new(2, 1));
            let width = content_area.width.saturating_sub(3) as usize;
            let cursor_x = input_cursor_x(&app.command_input, width);
            let x = content_area.x + 2 + cursor_x as u16;
            let y = content_area.y + content_area.height / 2;
            frame.set_cursor_position((x, y));
        }
    } else if app.mode == Mode::VarsModal {
        let (input_rects, _) =
            vars_modal_layout(frame.area(), app.vars_fields.len(), app.vars_focus);
        if let Some((_, input_rect)) = input_rects.iter().find(|(idx, _)| *idx == app.vars_focus) {
            let field = &app.vars_fields[app.vars_focus];
            let visible_width = input_rect.width.saturating_sub(4) as usize;
            let cursor_x = input_cursor_x(&field.input, visible_width);
            frame.set_cursor_position((input_rect.x + 1 + cursor_x as u16, input_rect.y + 1));
        }
    }
}

fn draw_empty_pane(
    frame: &mut Frame,
    app: &App,
    pane: &crate::app::PaneState,
    area: Rect,
    focused: bool,
) {
    let is_editing = focused && app.mode == Mode::InlineCommand && pane.id == app.focused;
    let mid_y = area.y + area.height / 2;
    let visible = visible_slice(
        &app.command_input.value,
        app.command_input.cursor_col(),
        area.width.saturating_sub(3) as usize,
    );

    if is_editing {
        frame.render_widget(
            Paragraph::new("> ").style(Style::default().fg(app.theme.success)),
            Rect::new(area.x, mid_y, 2, 1),
        );
        frame.render_widget(
            Paragraph::new(visible).style(
                Style::default()
                    .fg(app.theme.foreground)
                    .bg(app.theme.panel),
            ),
            Rect::new(area.x + 2, mid_y, area.width.saturating_sub(2), 1),
        );
    } else {
        frame.render_widget(
            Paragraph::new("Enter to set command")
                .style(Style::default().fg(app.theme.muted))
                .alignment(Alignment::Center),
            Rect::new(area.x, mid_y, area.width, 1),
        );
    }
}

fn draw_interval_controls(frame: &mut Frame, app: &App, rect: Rect, interval_ms: u64) {
    let text = format!("[-] {}ms [+]", interval_ms);
    let width = text.chars().count() as u16;
    let x = rect.x + rect.width.saturating_sub(width.saturating_add(2));
    let line = Line::from(vec![
        Span::styled("[-]", Style::default().fg(app.theme.warning)),
        Span::styled(
            format!(" {}ms ", interval_ms),
            Style::default().fg(app.theme.muted),
        ),
        Span::styled("[+]", Style::default().fg(app.theme.success)),
    ]);
    frame.render_widget(Paragraph::new(line), Rect::new(x, rect.y, width, 1));
}

fn draw_input_modal(frame: &mut Frame, app: &App, title: &str, value: &str, hint: &str) {
    let area = centered_rect(68, 9, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(app.theme.border_focused))
        .title(format!(" {title} "))
        .style(Style::default().bg(app.theme.panel));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(inner.inner(Margin::new(1, 1)));
    frame.render_widget(
        Paragraph::new(format!("Pane {}", app.focused + 1))
            .style(Style::default().fg(app.theme.muted)),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(value.to_string())
            .block(
                Block::bordered()
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(Style::default().fg(app.theme.accent)),
            )
            .style(Style::default().fg(app.theme.foreground)),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(app.theme.muted)),
        chunks[2],
    );
}

fn draw_command_modal(frame: &mut Frame, app: &App) {
    let frame_area = frame.area();
    let width = frame_area.width.saturating_sub(6).clamp(84, 124);
    let area = centered_rect(width, 16, frame_area);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(app.theme.border_focused))
        .title(" Set command ")
        .style(Style::default().bg(app.theme.panel));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(6),
            Constraint::Length(5),
        ])
        .split(inner.inner(Margin::new(2, 1)));

    frame.render_widget(
        Paragraph::new("Command")
            .style(Style::default().fg(app.theme.foreground))
            .alignment(Alignment::Left),
        chunks[0],
    );
    draw_text_area(
        frame,
        app,
        chunks[1],
        &app.command_input,
        app.command_modal_focus == CommandModalFocus::Command,
    );

    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[2]);
    let title_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(3)])
        .split(bottom_cols[0]);
    let interval_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(3)])
        .split(bottom_cols[1]);

    frame.render_widget(
        Paragraph::new("Name")
            .style(Style::default().fg(app.theme.foreground))
            .alignment(Alignment::Left),
        title_chunks[0],
    );
    draw_single_line_input(
        frame,
        app,
        title_chunks[1],
        &app.title_input,
        app.command_modal_focus == CommandModalFocus::Title,
    );

    frame.render_widget(
        Paragraph::new("Interval (ms)")
            .style(Style::default().fg(app.theme.foreground))
            .alignment(Alignment::Left),
        interval_chunks[0],
    );
    draw_single_line_input(
        frame,
        app,
        interval_chunks[1],
        &app.interval_input,
        app.command_modal_focus == CommandModalFocus::Interval,
    );

    let (cursor_area, cursor_input) = match app.command_modal_focus {
        CommandModalFocus::Command => (chunks[1], &app.command_input),
        CommandModalFocus::Title => (title_chunks[1], &app.title_input),
        CommandModalFocus::Interval => (interval_chunks[1], &app.interval_input),
    };
    let (x, y) = match app.command_modal_focus {
        CommandModalFocus::Command => text_area_cursor_position(cursor_area, cursor_input),
        CommandModalFocus::Title | CommandModalFocus::Interval => {
            let visible_width = cursor_area.width.saturating_sub(4) as usize;
            let cursor_x = input_cursor_x(cursor_input, visible_width);
            (cursor_area.x + 1 + cursor_x as u16, cursor_area.y + 1)
        }
    };
    frame.set_cursor_position((x, y));
}

fn draw_save_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(80, 12, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(app.theme.border_focused))
        .title(" Save preset ")
        .style(Style::default().bg(app.theme.panel));
    let inner = block.inner(area).inner(Margin::new(1, 1));
    frame.render_widget(block, area);
    let name_label = Rect::new(inner.x, inner.y, inner.width, 1);
    let input_rect = Rect::new(inner.x, inner.y + 1, inner.width, 3);
    let path_label = Rect::new(inner.x, inner.y + 5, inner.width, 1);
    let path_rect = Rect::new(inner.x, inner.y + 6, inner.width, 2);

    frame.render_widget(
        Paragraph::new("Preset name")
            .style(Style::default().fg(app.theme.foreground))
            .alignment(Alignment::Left),
        name_label,
    );
    let visible_width = input_rect.width.saturating_sub(4) as usize;
    let content = visible_slice(
        &app.save_input.value,
        app.save_input.cursor_col(),
        visible_width,
    );
    frame.render_widget(
        Paragraph::new(content)
            .block(
                Block::bordered()
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(Style::default().fg(app.theme.accent)),
            )
            .style(Style::default().fg(app.theme.foreground)),
        input_rect,
    );

    frame.render_widget(
        Paragraph::new("Target path")
            .style(Style::default().fg(app.theme.foreground))
            .alignment(Alignment::Left),
        path_label,
    );
    let preview = resolve_preset_path(&app.save_input.value)
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<invalid path>".into());
    frame.render_widget(
        Paragraph::new(preview)
            .style(Style::default().fg(app.theme.muted))
            .wrap(Wrap { trim: false }),
        path_rect,
    );
    let cursor_x = input_cursor_x(&app.save_input, visible_width);
    frame.set_cursor_position((input_rect.x + 1 + cursor_x as u16, input_rect.y + 1));
}

fn draw_delete_confirm_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(54, 7, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(app.theme.error))
        .title(" Clear command ")
        .style(Style::default().bg(app.theme.panel));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .split(inner.inner(Margin::new(1, 1)));
    frame.render_widget(
        Paragraph::new("Delete command from this pane?")
            .style(Style::default().fg(app.theme.foreground))
            .alignment(Alignment::Center),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new("Enter/Y confirm · Esc/N cancel")
            .style(Style::default().fg(app.theme.muted))
            .alignment(Alignment::Center),
        chunks[1],
    );
}

fn draw_single_line_input(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    input: &crate::app::TextInput,
    focused: bool,
) {
    let visible_width = area.width.saturating_sub(4) as usize;
    let content = visible_slice(&input.value, input.cursor_col(), visible_width);
    frame.render_widget(
        Paragraph::new(content)
            .block(
                Block::bordered()
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(if focused {
                        Style::default().fg(app.theme.accent)
                    } else {
                        Style::default().fg(app.theme.border)
                    }),
            )
            .style(Style::default().fg(app.theme.foreground)),
        area,
    );
}

fn draw_text_area(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    input: &crate::app::TextInput,
    focused: bool,
) {
    let inner_width = area.width.saturating_sub(4) as usize;
    let inner_height = area.height.saturating_sub(2) as usize;
    let content = visible_text_area(input, inner_width, inner_height);
    frame.render_widget(
        Paragraph::new(content)
            .block(
                Block::bordered()
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(if focused {
                        Style::default().fg(app.theme.accent)
                    } else {
                        Style::default().fg(app.theme.border)
                    }),
            )
            .style(Style::default().fg(app.theme.foreground)),
        area,
    );
}

fn draw_vars_modal(frame: &mut Frame, app: &App) {
    let (input_rects, area) =
        vars_modal_layout(frame.area(), app.vars_fields.len(), app.vars_focus);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(app.theme.border_focused))
        .title(" Startup variables ")
        .style(Style::default().bg(app.theme.panel));
    let inner = block.inner(area).inner(Margin::new(1, 1));
    frame.render_widget(block, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.panel)),
        inner,
    );

    let visible_count = input_rects.len();
    let start = visible_var_start(app.vars_fields.len(), app.vars_focus, visible_count);
    for (visible_idx, (field_idx, input_rect)) in input_rects.iter().enumerate() {
        let field = &app.vars_fields[*field_idx];
        let label_y = inner.y + visible_idx as u16 * 4;
        let label = if field.required {
            format!("{} *", field.name)
        } else {
            field.name.clone()
        };
        frame.render_widget(
            Paragraph::new(label).style(
                Style::default()
                    .fg(app.theme.foreground)
                    .bg(app.theme.panel),
            ),
            Rect::new(inner.x, label_y, inner.width, 1),
        );
        frame.render_widget(
            Paragraph::new(visible_slice(
                &field.input.value,
                field.input.cursor_col(),
                input_rect.width.saturating_sub(4) as usize,
            ))
            .block(
                Block::bordered()
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(if *field_idx == app.vars_focus {
                        Style::default().fg(app.theme.accent)
                    } else {
                        Style::default().fg(app.theme.border)
                    }),
            )
            .style(
                Style::default()
                    .fg(app.theme.foreground)
                    .bg(app.theme.panel),
            ),
            *input_rect,
        );
    }

    if app.vars_fields.len() > visible_count {
        frame.render_widget(
            Paragraph::new(format!(
                "{}-{} / {}",
                start + 1,
                start + visible_count,
                app.vars_fields.len()
            ))
            .style(Style::default().fg(app.theme.muted).bg(app.theme.panel))
            .alignment(Alignment::Right),
            Rect::new(
                inner.x,
                area.y + area.height.saturating_sub(2),
                inner.width,
                1,
            ),
        );
    }
}

fn vars_modal_layout(
    frame_area: Rect,
    field_count: usize,
    focus: usize,
) -> (Vec<(usize, Rect)>, Rect) {
    let max_height = frame_area.height.saturating_sub(4).max(12);
    let max_visible = ((max_height.saturating_sub(3)) / 4).max(1) as usize;
    let visible = field_count.max(1).min(max_visible);
    let modal_height = 3 + visible as u16 * 4;
    let area = centered_rect(76, modal_height, frame_area);
    let inner = area.inner(Margin::new(3, 2));
    let start = visible_var_start(field_count, focus, visible);
    let mut rects = Vec::with_capacity(visible);
    for offset in 0..visible {
        let idx = start + offset;
        if idx >= field_count {
            break;
        }
        let label_y = inner.y + offset as u16 * 4;
        rects.push((idx, Rect::new(inner.x, label_y + 1, inner.width, 3)));
    }
    (rects, area)
}

fn visible_var_start(total: usize, focus: usize, visible: usize) -> usize {
    if total <= visible {
        0
    } else if focus >= visible {
        focus - visible + 1
    } else {
        0
    }
}

fn draw_help(frame: &mut Frame, app: &App) {
    let area = centered_rect(76, 16, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(app.theme.border_focused))
        .title(" Help ")
        .style(Style::default().bg(app.theme.panel));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = vec![
        Line::from("Movement: h j k l / arrow keys"),
        Line::from("Pane: i edit command, t rename, r rerun, space pause, + / - interval"),
        Line::from("Global: R rerun all, p pause all, s save preset, z save view and exit"),
        Line::from("Presets: split name, split -f file.toml, split resume"),
        Line::from("App: ? toggle help, q or Ctrl-C quit"),
        Line::from("Mouse: click focuses pane, wheel scrolls output"),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Left)
            .style(Style::default().fg(app.theme.foreground))
            .wrap(Wrap { trim: false }),
        inner.inner(Margin::new(1, 1)),
    );
}

fn pane_rects(area: Rect, count: usize) -> Vec<(usize, Rect)> {
    if count == 3 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        let right_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(cols[1]);
        return vec![(0, cols[0]), (1, right_rows[0]), (2, right_rows[1])];
    }

    let grid = grid_for_count(count.max(1));
    let row_constraints = vec![Constraint::Ratio(1, grid.rows as u32); grid.rows];
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);
    let mut rects = Vec::with_capacity(count);
    for row_idx in 0..grid.rows {
        let col_constraints = vec![Constraint::Ratio(1, grid.cols as u32); grid.cols];
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints)
            .split(rows[row_idx]);
        for col_idx in 0..grid.cols {
            let idx = row_idx * grid.cols + col_idx;
            if idx >= count {
                break;
            }
            rects.push((idx, cols[col_idx]));
        }
    }
    rects
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "NORMAL",
        Mode::InlineCommand => "COMMAND",
        Mode::CommandModal => "COMMAND",
        Mode::DeleteConfirm => "CONFIRM",
        Mode::TitleModal => "TITLE",
        Mode::SaveModal => "SAVE",
        Mode::VarsModal => "VARS",
        Mode::Help => "HELP",
    }
}

fn mode_hints(app: &App) -> &'static [(&'static str, &'static str)] {
    match app.mode {
        Mode::Normal => {
            if app.focused_pane().cmd.trim().is_empty() {
                &[
                    ("type", "inline"),
                    ("Enter", "inline"),
                    ("i", "multiline"),
                    ("t", "title"),
                    ("s", "save"),
                    ("?", "help"),
                    ("q", "quit"),
                ]
            } else {
                &[
                    ("h/j/k/l", "move"),
                    ("i", "command"),
                    ("t", "title"),
                    ("r", "rerun"),
                    ("space", "pause"),
                    ("s", "save"),
                    ("?", "help"),
                    ("q", "quit"),
                ]
            }
        }
        Mode::InlineCommand => &[
            ("type", "command"),
            ("Left/Right", "move"),
            ("Enter", "save"),
            ("Esc", "cancel"),
        ],
        Mode::CommandModal => &[
            ("Enter", "newline"),
            ("Tab", "switch"),
            ("Ctrl-S", "save"),
            ("Esc", "cancel"),
        ],
        Mode::DeleteConfirm => &[("Enter/Y", "confirm"), ("Esc/N", "cancel")],
        Mode::TitleModal => &[("type", "title"), ("Enter", "save"), ("Esc", "cancel")],
        Mode::SaveModal => &[("type", "preset"), ("Enter", "save"), ("Esc", "cancel")],
        Mode::VarsModal => &[("Tab", "next"), ("Enter", "next/start"), ("Esc", "cancel")],
        Mode::Help => &[("?/Esc", "close")],
    }
}

fn mode_hint_spans(app: &App) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (idx, (key, desc)) in mode_hints(app).iter().enumerate() {
        spans.push(Span::styled(
            (*key).to_string(),
            Style::default().fg(app.theme.muted).bg(app.theme.panel),
        ));
        spans.push(Span::styled(
            format!(" {}", desc),
            Style::default().fg(app.theme.muted).bg(app.theme.panel),
        ));
        if idx + 1 < mode_hints(app).len() {
            spans.push(Span::styled(
                " • ".to_string(),
                Style::default().fg(app.theme.muted).bg(app.theme.panel),
            ));
        }
    }
    spans
}

fn mode_hints_width(app: &App) -> u16 {
    let mut width = 0usize;
    for (idx, (key, desc)) in mode_hints(app).iter().enumerate() {
        width += key.chars().count();
        width += 1;
        width += desc.chars().count();
        if idx + 1 < mode_hints(app).len() {
            width += 3;
        }
    }
    width as u16
}

fn toast_style(app: &App) -> Style {
    if let Some(toast) = &app.toast {
        let color = match toast.level {
            ToastLevel::Info => app.theme.accent,
            ToastLevel::Success => app.theme.success,
            ToastLevel::Warning => app.theme.warning,
            ToastLevel::Error => app.theme.error,
        };
        Style::default().fg(color).bg(app.theme.panel)
    } else {
        Style::default()
            .fg(app.theme.foreground)
            .bg(app.theme.panel)
    }
}

fn truncate(value: &str, width: usize) -> String {
    if value.is_empty() || width == 0 {
        return String::new();
    }
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return value.to_string();
    }
    chars[..width.saturating_sub(1)].iter().collect::<String>() + "…"
}

fn visible_slice(value: &str, cursor_col: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return value.to_string();
    }

    let mut start = cursor_col.saturating_sub(width.saturating_sub(1));
    if start + width > chars.len() {
        start = chars.len().saturating_sub(width);
    }
    let end = (start + width).min(chars.len());
    let mut rendered = chars[start..end].iter().collect::<String>();
    if start > 0 && !rendered.is_empty() {
        rendered.replace_range(0..1, "…");
    }
    if end < chars.len() && !rendered.is_empty() {
        let len = rendered.chars().count();
        let last_idx = rendered
            .char_indices()
            .nth(len.saturating_sub(1))
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        rendered.replace_range(last_idx..rendered.len(), "…");
    }
    rendered
}

fn input_cursor_x(input: &crate::app::TextInput, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let total = input.value.chars().count();
    let cursor = input.cursor_col();
    if total <= width {
        return cursor.min(width.saturating_sub(1));
    }

    let mut start = cursor.saturating_sub(width.saturating_sub(1));
    if start + width > total {
        start = total.saturating_sub(width);
    }
    let visible_cursor = cursor.saturating_sub(start);
    visible_cursor.min(width.saturating_sub(1))
}

fn visible_text_area(input: &crate::app::TextInput, width: usize, height: usize) -> String {
    if width == 0 || height == 0 {
        return String::new();
    }
    let (cursor_line, cursor_col) = input.cursor_line_col();
    let total_lines = input.line_count();
    let mut start_line = cursor_line.saturating_sub(height.saturating_sub(1));
    if start_line + height > total_lines {
        start_line = total_lines.saturating_sub(height);
    }

    let mut lines = Vec::with_capacity(height);
    for line_index in start_line..(start_line + height).min(total_lines) {
        let line = input.line_at(line_index);
        let col = if line_index == cursor_line {
            cursor_col
        } else {
            0
        };
        lines.push(visible_slice(line, col, width));
    }
    while lines.len() < height {
        lines.push(String::new());
    }
    lines.join("\n")
}

fn text_area_cursor_position(area: Rect, input: &crate::app::TextInput) -> (u16, u16) {
    let width = area.width.saturating_sub(4) as usize;
    let height = area.height.saturating_sub(2) as usize;
    let (cursor_line, cursor_col) = input.cursor_line_col();
    let total_lines = input.line_count();

    let mut start_line = cursor_line.saturating_sub(height.saturating_sub(1));
    if start_line + height > total_lines {
        start_line = total_lines.saturating_sub(height);
    }
    let visible_line = cursor_line
        .saturating_sub(start_line)
        .min(height.saturating_sub(1));
    let line_text = input.line_at(cursor_line);
    let visible_col = input_cursor_x_for_line(line_text, cursor_col, width);

    (
        area.x + 1 + visible_col as u16,
        area.y + 1 + visible_line as u16,
    )
}

fn input_cursor_x_for_line(line: &str, cursor_col: usize, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let total = line.chars().count();
    if total <= width {
        return cursor_col.min(width.saturating_sub(1));
    }

    let mut start = cursor_col.saturating_sub(width.saturating_sub(1));
    if start + width > total {
        start = total.saturating_sub(width);
    }
    cursor_col
        .saturating_sub(start)
        .min(width.saturating_sub(1))
}
