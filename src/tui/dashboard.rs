use crate::app::{App, Mode, PaneState};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Paragraph, StatefulWidget, Widget},
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
        if pane.cmd.is_empty() {
            if let Some(cursor) = draw_empty_pane(frame, app, pane, content_area, focused) {
                inline_cursor = Some(cursor);
            }
        }
    }
    inline_cursor
}

fn draw_empty_pane(
    frame: &mut Frame,
    app: &App,
    pane: &PaneState,
    area: Rect,
    focused: bool,
) -> Option<(u16, u16)> {
    let is_editing = focused && app.mode == Mode::InlineCommand && pane.id == app.focused;
    let mid_y = area.y + area.height / 2;
    let line_area = Rect::new(area.x, mid_y, area.width, 1);

    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.panel)),
        line_area,
    );

    if is_editing {
        let mut state = InputCursorState::default();
        InlineCommandInput {
            input: &app.command_input,
            theme: &app.theme,
        }
        .render(line_area, frame.buffer_mut(), &mut state);
        Some((state.x, state.y))
    } else {
        frame.render_widget(
            Paragraph::new("Enter to set command")
                .style(Style::default().fg(app.theme.muted))
                .alignment(ratatui::layout::Alignment::Center),
            line_area,
        );
        None
    }
}
