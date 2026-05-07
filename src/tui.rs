pub(crate) mod actions;
mod ansi;
mod dashboard;
pub(crate) mod helpers;
pub(crate) mod modals;
mod status;
mod widgets;

use crate::{
    app::{App, KeyAction, Mode, ToastLevel},
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
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Paragraph},
};
use std::{io, sync::mpsc, time::Duration};

use dashboard::draw_panes;
use helpers::{
    MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH, cursor_if_visible, pane_rects, terminal_too_small,
};
use modals::{
    draw_command_modal, draw_delete_confirm_modal, draw_help, draw_input_modal, draw_save_modal,
    draw_vars_modal,
};
use status::draw_status_bar;

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
                    let frame_area = Rect::new(0, 0, size.width, size.height);
                    let rects = pane_rects(frame_area, app.panes.len());
                    app.handle_mouse(mouse, frame_area, &rects);
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
    if terminal_too_small(area) {
        let message = format!(
            "Terminal too small. Resize to at least {}×{}.",
            MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT
        );
        frame.render_widget(
            Paragraph::new(Line::from(message))
                .style(
                    Style::default()
                        .fg(app.theme.muted)
                        .bg(app.theme.background),
                )
                .alignment(ratatui::layout::Alignment::Center),
            area,
        );
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let inline_cursor = draw_panes(frame, app, chunks[0]);
    draw_status_bar(frame, app, chunks[1]);

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
                &app.title_input,
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
        if let Some((x, y)) = inline_cursor.and_then(|(x, y)| cursor_if_visible(area, x, y)) {
            frame.set_cursor_position((x, y));
        }
    }
}
