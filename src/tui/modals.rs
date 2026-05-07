use crate::{
    app::{App, CommandModalFocus, TextInput},
    preset::resolve_preset_path,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};

use super::helpers::{centered_rect, vars_modal_layout, vars_scroll_text, visible_var_start};
use super::widgets::{InputCursorState, ModalFrame, SingleLineInput, TextAreaInput};

pub(crate) fn draw_input_modal(
    frame: &mut Frame,
    app: &App,
    title: &str,
    input: &TextInput,
    hint: &str,
) {
    let area = centered_rect(70, 9, frame.area());
    frame.render_widget(Clear, area);
    ModalFrame {
        title,
        theme: &app.theme,
    }
    .render(area, frame.buffer_mut());
    let inner = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(format!("Pane {}", app.focused + 1))
            .style(Style::default().fg(app.theme.muted)),
        chunks[0],
    );
    let mut input_state = InputCursorState::default();
    SingleLineInput {
        input,
        theme: &app.theme,
        focused: true,
        title: Some("Name"),
    }
    .render(chunks[1], frame.buffer_mut(), &mut input_state);
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(app.theme.muted)),
        chunks[2],
    );
    frame.set_cursor_position((input_state.x, input_state.y));
}

pub(crate) fn draw_command_modal(frame: &mut Frame, app: &App) {
    let frame_area = frame.area();
    let width = frame_area.width.saturating_sub(6).clamp(60, 90);
    let area = centered_rect(width, 20, frame_area);
    frame.render_widget(Clear, area);
    let title = format!("Set Command · Pane {}", app.focused + 1);
    ModalFrame {
        title: &title,
        theme: &app.theme,
    }
    .render(area, frame.buffer_mut());
    let inner = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(15), Constraint::Length(3)])
        .split(inner);

    let mut command_state = InputCursorState::default();
    TextAreaInput {
        input: &app.command_input,
        theme: &app.theme,
        focused: app.command_modal_focus == CommandModalFocus::Command,
        title: Some("Command"),
    }
    .render(chunks[0], frame.buffer_mut(), &mut command_state);

    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);
    let mut title_state = InputCursorState::default();
    SingleLineInput {
        input: &app.title_input,
        theme: &app.theme,
        focused: app.command_modal_focus == CommandModalFocus::Title,
        title: Some("Name"),
    }
    .render(bottom_cols[0], frame.buffer_mut(), &mut title_state);
    let mut interval_state = InputCursorState::default();
    SingleLineInput {
        input: &app.interval_input,
        theme: &app.theme,
        focused: app.command_modal_focus == CommandModalFocus::Interval,
        title: Some("Interval (ms)"),
    }
    .render(bottom_cols[1], frame.buffer_mut(), &mut interval_state);

    let (x, y) = match app.command_modal_focus {
        CommandModalFocus::Command => (command_state.x, command_state.y),
        CommandModalFocus::Title => (title_state.x, title_state.y),
        CommandModalFocus::Interval => (interval_state.x, interval_state.y),
    };
    frame.set_cursor_position((x, y));
}

pub(crate) fn draw_save_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(82, 11, frame.area());
    frame.render_widget(Clear, area);
    ModalFrame {
        title: "Save preset",
        theme: &app.theme,
    }
    .render(area, frame.buffer_mut());
    let inner = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new("Current dashboard preset").style(Style::default().fg(app.theme.muted)),
        chunks[0],
    );
    let mut save_state = InputCursorState::default();
    SingleLineInput {
        input: &app.save_input,
        theme: &app.theme,
        focused: true,
        title: Some("Preset name"),
    }
    .render(chunks[1], frame.buffer_mut(), &mut save_state);
    let preview = resolve_preset_path(&app.save_input.value)
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<invalid path>".into());
    frame.render_widget(
        Paragraph::new(preview)
            .block(
                Block::bordered()
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(Style::default().fg(app.theme.border))
                    .title(" Target path "),
            )
            .style(Style::default().fg(app.theme.muted).bg(app.theme.panel))
            .wrap(Wrap { trim: false }),
        chunks[2],
    );
    frame.set_cursor_position((save_state.x, save_state.y));
}

pub(crate) fn draw_delete_confirm_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(54, 7, frame.area());
    frame.render_widget(Clear, area);
    ModalFrame {
        title: "Clear command",
        theme: &app.theme,
    }
    .render(area, frame.buffer_mut());
    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(inner.inner(Margin::new(1, 1)));
    frame.render_widget(
        Paragraph::new("This will clear the pane command and output.")
            .style(Style::default().fg(app.theme.muted))
            .alignment(Alignment::Center),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new("Delete command from this pane?")
            .style(Style::default().fg(app.theme.foreground))
            .alignment(Alignment::Center),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new("Enter/Y confirm · Esc/N cancel")
            .style(Style::default().fg(app.theme.muted))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

pub(crate) fn draw_vars_modal(frame: &mut Frame, app: &App) {
    let (input_rects, area) =
        vars_modal_layout(frame.area(), app.vars_fields.len(), app.vars_focus);
    frame.render_widget(Clear, area);
    ModalFrame {
        title: "Startup variables",
        theme: &app.theme,
    }
    .render(area, frame.buffer_mut());
    let inner = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );

    let visible_count = input_rects.len();
    let start = visible_var_start(app.vars_fields.len(), app.vars_focus, visible_count);
    for (field_idx, input_rect) in input_rects.iter() {
        let field = &app.vars_fields[*field_idx];
        let label = if field.required {
            format!("{} *", field.name)
        } else {
            field.name.clone()
        };
        let mut input_state = InputCursorState::default();
        SingleLineInput {
            input: &field.input,
            theme: &app.theme,
            focused: *field_idx == app.vars_focus,
            title: Some(&label),
        }
        .render(*input_rect, frame.buffer_mut(), &mut input_state);
        if *field_idx == app.vars_focus {
            frame.set_cursor_position((input_state.x, input_state.y));
        }
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
    ModalFrame {
        title: "Help",
        theme: &app.theme,
    }
    .render(area, frame.buffer_mut());
    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
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
