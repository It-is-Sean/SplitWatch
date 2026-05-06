use crate::{
    app::{App, CommandModalFocus, TextInput},
    preset::resolve_preset_path,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Clear, Paragraph, Wrap},
};

use super::helpers::{
    centered_rect, input_cursor_x, text_area_cursor_position, vars_modal_layout, vars_scroll_text,
    visible_slice, visible_text_area, visible_var_start,
};

pub(crate) fn draw_input_modal(frame: &mut Frame, app: &App, title: &str, value: &str, hint: &str) {
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

pub(crate) fn draw_command_modal(frame: &mut Frame, app: &App) {
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

pub(crate) fn draw_save_modal(frame: &mut Frame, app: &App) {
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

pub(crate) fn draw_delete_confirm_modal(frame: &mut Frame, app: &App) {
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

pub(crate) fn draw_vars_modal(frame: &mut Frame, app: &App) {
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

    if let Some(scroll_text) = vars_scroll_text(app.vars_fields.len(), start, visible_count) {
        frame.render_widget(
            Paragraph::new(scroll_text)
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

pub(crate) fn draw_help(frame: &mut Frame, app: &App) {
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
        Line::from("Presets: swatch name, swatch -f file.toml, swatch resume"),
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

fn draw_single_line_input(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    input: &TextInput,
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

fn draw_text_area(frame: &mut Frame, app: &App, area: Rect, input: &TextInput, focused: bool) {
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
