use crate::app::App;
use ratatui::{Frame, layout::Rect, widgets::Widget};

use super::widgets::StatusBarWidget;

pub(crate) fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    StatusBarWidget { app }.render(area, frame.buffer_mut());
}
