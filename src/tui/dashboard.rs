use crate::app::{App, Mode, PaneState};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Clear, Paragraph, StatefulWidget, Widget},
};

use super::{
    helpers::pane_rects,
    widgets::{InlineCommandInput, InputCursorState, PaneWidget},
};

pub(crate) fn draw_panes(frame: &mut Frame, app: &App, area: Rect) -> Option<(u16, u16)> {
    let pane_regions = pane_rects(area, app.panes.len());
    let mut inline_cursor = None;
    for (idx, rect) in &pane_regions {
        let pane = &app.panes[*idx];
        let focused = *idx == app.focused;
        PaneWidget { app, pane, focused }.render(*rect, frame.buffer_mut());
        let content_area = Rect::new(
            rect.x + 2,
            rect.y + 1,
            rect.width.saturating_sub(4),
            rect.height.saturating_sub(2),
        );
        if let Some(cursor) = draw_inline_overlay(frame, app, pane, content_area, focused) {
            inline_cursor = Some(cursor);
        }
    }
    inline_cursor
}

fn draw_inline_overlay(
    frame: &mut Frame,
    app: &App,
    pane: &PaneState,
    area: Rect,
    focused: bool,
) -> Option<(u16, u16)> {
    let is_editing = focused && app.mode == Mode::InlineCommand && pane.id == app.focused;
    if !is_editing && !pane.cmd.is_empty() {
        return None;
    }
    let mid_y = area.y + area.height / 2;
    let line_area = Rect::new(area.x, mid_y, area.width, 1);

    if is_editing {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default().style(Style::default().bg(app.theme.panel)),
            area,
        );
        let mut state = InputCursorState::default();
        InlineCommandInput {
            input: &app.command_input,
            theme: &app.theme,
        }
        .render(line_area, frame.buffer_mut(), &mut state);
        Some((state.x, state.y))
    } else {
        frame.render_widget(
            Block::default().style(Style::default().bg(app.theme.panel)),
            line_area,
        );
        frame.render_widget(
            Paragraph::new("Enter to set command")
                .style(Style::default().fg(app.theme.muted))
                .alignment(ratatui::layout::Alignment::Center),
            line_area,
        );
        None
    }
}
