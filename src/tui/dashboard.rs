use crate::app::{App, Mode, PaneState};
use ratatui::{
    Frame,
    layout::{Alignment, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::helpers::{pane_rects, truncate, visible_slice};

pub(crate) fn draw_panes(frame: &mut Frame, app: &App, area: Rect) {
    let pane_regions = pane_rects(area, app.panes.len());
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
        let status = pane_status(app, pane);
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
}

fn draw_empty_pane(frame: &mut Frame, app: &App, pane: &PaneState, area: Rect, focused: bool) {
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

fn pane_status(app: &App, pane: &PaneState) -> Span<'static> {
    if app.global_paused || pane.paused {
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
    }
}
