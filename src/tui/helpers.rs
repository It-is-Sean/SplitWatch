use crate::{app::TextInput, layout::grid_for_count};
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

pub(crate) const MIN_TERMINAL_WIDTH: u16 = 24;
pub(crate) const MIN_TERMINAL_HEIGHT: u16 = 6;

pub(crate) fn pane_rects(area: Rect, count: usize) -> Vec<(usize, Rect)> {
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

pub(crate) fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

pub(crate) fn terminal_too_small(area: Rect) -> bool {
    area.width < MIN_TERMINAL_WIDTH || area.height < MIN_TERMINAL_HEIGHT
}

pub(crate) fn cursor_if_visible(area: Rect, x: u16, y: u16) -> Option<(u16, u16)> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    let max_x = area.x + area.width.saturating_sub(1);
    let max_y = area.y + area.height.saturating_sub(1);
    if x >= area.x && x <= max_x && y >= area.y && y <= max_y {
        Some((x, y))
    } else {
        None
    }
}

pub(crate) fn command_modal_rect(area: Rect) -> Rect {
    let width = area.width.saturating_sub(6).clamp(60, 90);
    centered_rect(width, 20, area)
}

pub(crate) fn delete_modal_rect(area: Rect) -> Rect {
    centered_rect(54, 7, area)
}

pub(crate) fn help_modal_rect(area: Rect) -> Rect {
    centered_rect(76, 16, area)
}

pub(crate) fn truncate(value: &str, width: usize) -> String {
    if value.is_empty() || width == 0 {
        return String::new();
    }
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return value.to_string();
    }
    chars[..width.saturating_sub(1)].iter().collect::<String>() + "…"
}

pub(crate) fn visible_slice(value: &str, cursor_col: usize, width: usize) -> String {
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

pub(crate) fn input_cursor_x(input: &TextInput, width: usize) -> usize {
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

pub(crate) fn visible_text_area(input: &TextInput, width: usize, height: usize) -> String {
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

pub(crate) fn text_area_cursor_position(area: Rect, input: &TextInput) -> (u16, u16) {
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

pub(crate) fn vars_modal_layout(
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

pub(crate) fn visible_var_start(total: usize, focus: usize, visible: usize) -> usize {
    if total <= visible {
        0
    } else if focus >= visible {
        focus - visible + 1
    } else {
        0
    }
}

pub(crate) fn vars_scroll_text(total: usize, start: usize, visible: usize) -> Option<String> {
    if total > visible {
        Some(format!("{}-{} / {}", start + 1, start + visible, total))
    } else {
        None
    }
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
