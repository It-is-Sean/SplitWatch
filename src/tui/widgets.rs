use crate::{
    app::{App, Mode, PaneState, TextInput, ToastLevel},
    theme::Theme,
};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget, Wrap},
};

use super::ansi::ansi_text;
use super::helpers::{
    input_cursor_x, text_area_cursor_position, truncate, visible_slice, visible_text_area,
};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct InputCursorState {
    pub x: u16,
    pub y: u16,
}

pub(crate) struct SingleLineInput<'a> {
    pub input: &'a TextInput,
    pub theme: &'a Theme,
    pub focused: bool,
    pub title: Option<&'a str>,
}

impl StatefulWidget for SingleLineInput<'_> {
    type State = InputCursorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let visible_width = area.width.saturating_sub(4) as usize;
        let content = visible_slice(&self.input.value, self.input.cursor_col(), visible_width);
        let border = if self.focused {
            self.theme.accent
        } else {
            self.theme.border
        };
        Paragraph::new(content)
            .block(
                Block::bordered()
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(Style::default().fg(border))
                    .title(
                        self.title
                            .map(|title| format!(" {} ", title))
                            .unwrap_or_default(),
                    ),
            )
            .style(
                Style::default()
                    .fg(self.theme.foreground)
                    .bg(self.theme.panel),
            )
            .render(area, buf);
        let cursor_x = input_cursor_x(self.input, visible_width);
        state.x = area.x + 1 + cursor_x as u16;
        state.y = area.y + 1;
    }
}

pub(crate) struct TextAreaInput<'a> {
    pub input: &'a TextInput,
    pub theme: &'a Theme,
    pub focused: bool,
    pub title: Option<&'a str>,
}

impl StatefulWidget for TextAreaInput<'_> {
    type State = InputCursorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let inner_width = area.width.saturating_sub(4) as usize;
        let inner_height = area.height.saturating_sub(2) as usize;
        let content = visible_text_area(self.input, inner_width, inner_height);
        let border = if self.focused {
            self.theme.accent
        } else {
            self.theme.border
        };
        Paragraph::new(content)
            .block(
                Block::bordered()
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(Style::default().fg(border))
                    .title(
                        self.title
                            .map(|title| format!(" {} ", title))
                            .unwrap_or_default(),
                    ),
            )
            .style(
                Style::default()
                    .fg(self.theme.foreground)
                    .bg(self.theme.panel),
            )
            .render(area, buf);
        let (x, y) = text_area_cursor_position(area, self.input);
        state.x = x;
        state.y = y;
    }
}

pub(crate) struct InlineCommandInput<'a> {
    pub input: &'a TextInput,
    pub theme: &'a Theme,
}

impl StatefulWidget for InlineCommandInput<'_> {
    type State = InputCursorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if self.input.value.is_empty() {
            Paragraph::new("> ")
                .style(Style::default().fg(self.theme.success).bg(self.theme.panel))
                .render(Rect::new(area.x, area.y, 2, 1), buf);

            Paragraph::new("Type the command")
                .style(Style::default().fg(self.theme.muted).bg(self.theme.panel))
                .alignment(Alignment::Center)
                .render(
                    Rect::new(area.x + 2, area.y, area.width.saturating_sub(2), 1),
                    buf,
                );
            state.x = area.x + 2;
            state.y = area.y;
            return;
        }

        let visible = visible_slice(
            &self.input.value,
            self.input.cursor_col(),
            area.width.saturating_sub(3) as usize,
        );
        Paragraph::new("> ")
            .style(Style::default().fg(self.theme.success).bg(self.theme.panel))
            .render(Rect::new(area.x, area.y, 2, 1), buf);
        Paragraph::new(visible)
            .style(
                Style::default()
                    .fg(self.theme.foreground)
                    .bg(self.theme.panel),
            )
            .render(
                Rect::new(area.x + 2, area.y, area.width.saturating_sub(2), 1),
                buf,
            );
        let cursor_x = input_cursor_x(self.input, area.width.saturating_sub(3) as usize);
        state.x = area.x + 2 + cursor_x as u16;
        state.y = area.y;
    }
}

pub(crate) struct ModalFrame<'a> {
    pub title: &'a str,
    pub theme: &'a Theme,
}

impl Widget for ModalFrame<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::bordered()
            .border_set(ratatui::symbols::border::ROUNDED)
            .border_style(Style::default().fg(self.theme.border_focused))
            .title(format!(" {} ", self.title))
            .style(Style::default().bg(self.theme.panel))
            .render(area, buf);
    }
}

pub(crate) struct StatusBarWidget<'a> {
    pub app: &'a App,
}

impl Widget for StatusBarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let hint_width = mode_hints_width(self.app).min(area.width.saturating_sub(8));
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(hint_width)])
            .split(area);

        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", mode_label(self.app.mode)),
                Style::default()
                    .fg(self.app.theme.background)
                    .bg(self.app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                self.app
                    .toast
                    .as_ref()
                    .map(|toast| format!(" {}", toast.message))
                    .unwrap_or_default(),
                toast_style(self.app),
            ),
        ]))
        .style(Style::default().bg(self.app.theme.panel))
        .alignment(Alignment::Left)
        .render(chunks[0], buf);

        Paragraph::new(Line::from(mode_hint_spans(self.app)))
            .style(Style::default().bg(self.app.theme.panel))
            .alignment(Alignment::Right)
            .render(chunks[1], buf);
    }
}

pub(crate) struct PaneWidget<'a> {
    pub app: &'a App,
    pub pane: &'a PaneState,
    pub focused: bool,
}

impl Widget for PaneWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.app.theme.border_focused)
        } else {
            Style::default().fg(self.app.theme.border)
        };
        let title_style = if self.focused {
            Style::default()
                .fg(self.app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.app.theme.foreground)
        };

        let block = Block::bordered()
            .borders(Borders::ALL)
            .border_set(ratatui::symbols::border::ROUNDED)
            .border_style(border_style)
            .style(Style::default().bg(self.app.theme.panel))
            .title(Line::from(vec![
                Span::styled(format!(" {} ", self.pane.title), title_style),
                Span::styled(
                    truncate(&self.pane.cmd, area.width.saturating_sub(40) as usize),
                    Style::default().fg(self.app.theme.muted),
                ),
                pane_status(self.app, self.pane),
            ]));
        let inner = block.inner(area);
        block.render(area, buf);
        render_interval_controls(buf, self.app, area, self.pane);

        let content_area = inner.inner(Margin::new(1, 0));
        if !self.pane.cmd.is_empty() {
            let base = Style::default()
                .fg(self.app.theme.foreground)
                .bg(self.app.theme.panel);
            Paragraph::new(ansi_text(&self.pane.output_text(), base))
                .style(base)
                .wrap(Wrap { trim: false })
                .scroll((self.pane.scroll, 0))
                .render(content_area, buf);
        }
    }
}

fn render_interval_controls(buf: &mut Buffer, app: &App, rect: Rect, pane: &PaneState) {
    let text = format!("[-] {}ms [+]", pane.interval_ms);
    let width = text.chars().count() as u16;
    if rect.width <= width.saturating_add(2) {
        return;
    }
    let x = rect.x + rect.width.saturating_sub(width.saturating_add(2));
    let interval_color = if pane.long_running_latched {
        app.theme.error
    } else {
        app.theme.muted
    };
    let line = Line::from(vec![
        Span::styled("[-]", Style::default().fg(app.theme.warning)),
        Span::styled(
            format!(" {}ms ", pane.interval_ms),
            Style::default().fg(interval_color),
        ),
        Span::styled("[+]", Style::default().fg(app.theme.success)),
    ]);
    Paragraph::new(line).render(Rect::new(x, rect.y, width, 1), buf);
}

fn pane_status(app: &App, pane: &PaneState) -> Span<'static> {
    if app.global_paused || pane.paused {
        Span::styled(" ● ", Style::default().fg(app.theme.warning))
    } else if pane.is_long_running() {
        Span::styled(" ● ", Style::default().fg(app.theme.long_running))
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
    }
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
            ("Enter", "newline/save"),
            ("Tab", "switch"),
            ("Esc", "blur/close"),
            ("i", "focus"),
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
