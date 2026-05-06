use crate::app::{App, Mode, ToastLevel};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub(crate) fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let hint_width = mode_hints_width(app).min(area.width.saturating_sub(8));
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(hint_width)])
        .split(area);
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
        Mode::InlineCommand => &[("Left/Right", "move"), ("Enter", "save"), ("Esc", "cancel")],
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
