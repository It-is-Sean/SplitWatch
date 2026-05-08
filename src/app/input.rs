use super::*;
use crate::tui::{
    actions::ModalActions,
    helpers::{command_modal_rect, delete_modal_rect, help_modal_rect},
    modals::{command_modal_actions, delete_modal_actions, help_modal_actions},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::Instant;

enum ModalKind {
    Command,
    Title,
    Save,
}

impl App {
    pub fn focus_next(&mut self, delta_row: isize, delta_col: isize) {
        if self.panes.is_empty() {
            return;
        }
        if self.panes.len() == 3 {
            self.focused = match (self.focused, delta_row, delta_col) {
                (0, _, dc) if dc > 0 => 1,
                (0, dr, _) if dr > 0 => 2,
                (1, _, dc) if dc < 0 => 0,
                (1, dr, _) if dr > 0 => 2,
                (2, _, dc) if dc < 0 => 0,
                (2, dr, _) if dr < 0 => 1,
                _ => self.focused,
            };
            return;
        }
        let grid = crate::layout::grid_for_count(self.panes.len());
        let row = self.focused / grid.cols;
        let col = self.focused % grid.cols;
        let next_row =
            (row as isize + delta_row).clamp(0, grid.rows.saturating_sub(1) as isize) as usize;
        let next_col =
            (col as isize + delta_col).clamp(0, grid.cols.saturating_sub(1) as isize) as usize;
        let next = (next_row * grid.cols + next_col).min(self.panes.len().saturating_sub(1));
        self.focused = next;
    }

    pub fn open_command_modal(&mut self) {
        if !self.focused_pane().paused {
            self.focused_pane_mut().paused = true;
        }
        let command = self.focused_pane().cmd.clone();
        let title = if self.focused_pane().title.trim().is_empty() {
            format!("Pane {}", self.focused + 1)
        } else {
            self.focused_pane().title.clone()
        };
        let interval_ms = self.focused_pane().interval_ms;
        self.command_input = TextInput::new(command);
        self.title_input = TextInput::new(title);
        self.interval_input = TextInput::new(interval_ms.to_string());
        self.command_modal_focus = CommandModalFocus::Command;
        self.mode = Mode::CommandModal;
    }

    pub fn open_title_modal(&mut self) {
        let current = self.focused_pane().title.clone();
        self.title_input = TextInput::new(current);
        self.mode = Mode::TitleModal;
    }

    pub fn open_save_modal(&mut self) {
        let suggestion = self
            .loaded_preset
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|stem| stem.to_str())
            .unwrap_or("dashboard")
            .to_string();
        self.save_input = TextInput::new(suggestion);
        self.mode = Mode::SaveModal;
    }

    pub fn open_inline_command(&mut self) {
        let existing = !self.focused_pane().cmd.trim().is_empty();
        let command = self.focused_pane().cmd.clone();
        if existing {
            self.focused_pane_mut().paused = true;
        }
        self.command_input = TextInput::new(command);
        self.inline_resume_execution = existing;
        self.mode = Mode::InlineCommand;
    }

    pub fn toggle_pause_focused(&mut self) {
        let pane = self.focused_pane_mut();
        pane.paused = !pane.paused;
    }

    pub fn toggle_pause_all(&mut self) {
        self.global_paused = !self.global_paused;
    }

    pub fn rerun_focused(&mut self) {
        let pane = self.focused_pane_mut();
        pane.next_run = Instant::now();
        pane.scroll = 0;
    }

    pub fn rerun_all(&mut self) {
        let now = Instant::now();
        for pane in &mut self.panes {
            pane.next_run = now;
            pane.scroll = 0;
        }
    }

    pub fn adjust_interval_focused(&mut self, delta_ms: i64) {
        let pane = self.focused_pane_mut();
        let next = (pane.interval_ms as i64 + delta_ms).max(250) as u64;
        pane.interval_ms = next;
        let required_ms = pane
            .current_run_elapsed_ms()
            .or(pane.last_long_running_ms)
            .unwrap_or(0);
        if next > required_ms {
            pane.long_running_latched = false;
        }
        pane.next_run = Instant::now() + std::time::Duration::from_millis(next);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        if matches!(self.mode, Mode::Help) {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.mode = Mode::Normal;
            }
            return KeyAction::None;
        }

        if self.mode == Mode::VarsModal {
            return self.handle_vars_key(key);
        }
        if self.mode == Mode::InlineCommand {
            return self.handle_inline_command_key(key);
        }
        if self.mode == Mode::DeleteConfirm {
            return self.handle_delete_confirm_key(key);
        }

        match self.mode {
            Mode::InlineCommand | Mode::DeleteConfirm => unreachable!(),
            Mode::CommandModal => return self.handle_text_modal(key, ModalKind::Command),
            Mode::TitleModal => return self.handle_text_modal(key, ModalKind::Title),
            Mode::SaveModal => return self.handle_text_modal(key, ModalKind::Save),
            Mode::Normal | Mode::Help | Mode::VarsModal => {}
        }

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return KeyAction::Quit;
        }

        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.focus_next(0, -1),
            KeyCode::Right | KeyCode::Char('l') => self.focus_next(0, 1),
            KeyCode::Up | KeyCode::Char('k') => self.focus_next(-1, 0),
            KeyCode::Down | KeyCode::Char('j') => self.focus_next(1, 0),
            KeyCode::Char('i') => self.open_command_modal(),
            KeyCode::Enter => self.open_inline_command(),
            KeyCode::Char('t') => self.open_title_modal(),
            KeyCode::Char('r') => self.rerun_focused(),
            KeyCode::Char('R') => self.rerun_all(),
            KeyCode::Char(' ') => self.toggle_pause_focused(),
            KeyCode::Char('p') => self.toggle_pause_all(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.adjust_interval_focused(50),
            KeyCode::Char('-') | KeyCode::Char('_') => self.adjust_interval_focused(-50),
            KeyCode::Backspace if !self.focused_pane().cmd.trim().is_empty() => {
                self.mode = Mode::DeleteConfirm;
            }
            KeyCode::Char('s') => self.open_save_modal(),
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('z') => return KeyAction::SaveResumeAndQuit,
            KeyCode::Char('q') => return KeyAction::Quit,
            _ => {}
        }
        KeyAction::None
    }

    fn handle_inline_command_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => {
                self.command_input = TextInput::new(String::new());
                if self.inline_resume_execution {
                    let pane = self.focused_pane_mut();
                    pane.paused = false;
                    pane.next_run = Instant::now();
                    pane.pending_run_once = true;
                }
                self.inline_resume_execution = false;
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                let value = self.command_input.value.trim().to_string();
                let resume_execution = self.inline_resume_execution;
                let pane = self.focused_pane_mut();
                pane.cmd = value;
                pane.last_error = None;
                if resume_execution && !pane.cmd.trim().is_empty() {
                    pane.paused = false;
                    pane.next_run = Instant::now();
                    pane.pending_run_once = true;
                } else if pane.cmd.trim().is_empty() {
                    pane.paused = false;
                    pane.pending_run_once = false;
                    pane.next_run = Instant::now();
                } else {
                    pane.next_run = Instant::now();
                }
                self.command_input = TextInput::new(String::new());
                self.inline_resume_execution = false;
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => self.command_input.backspace(),
            KeyCode::Left => self.command_input.move_left(),
            KeyCode::Right => self.command_input.move_right(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.command_input.insert(ch);
            }
            _ => {}
        }
        KeyAction::None
    }

    fn handle_delete_confirm_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') => self.mode = Mode::Normal,
            KeyCode::Enter | KeyCode::Char('y') => {
                let pane = self.focused_pane_mut();
                pane.cmd.clear();
                pane.output.clear();
                pane.last_error = None;
                pane.last_exit_code = None;
                pane.running = false;
                pane.run_started_at = None;
                pane.scroll = 0;
                pane.next_run = Instant::now();
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        KeyAction::None
    }

    fn handle_text_modal(&mut self, key: KeyEvent, kind: ModalKind) -> KeyAction {
        if matches!(kind, ModalKind::Command) {
            return self.handle_command_modal_key(key);
        }
        let input = match kind {
            ModalKind::Command => unreachable!(),
            ModalKind::Title => &mut self.title_input,
            ModalKind::Save => &mut self.save_input,
        };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => match kind {
                ModalKind::Command => unreachable!(),
                ModalKind::Title => {
                    self.focused_pane_mut().title = self.title_input.value.clone();
                    self.mode = Mode::Normal;
                }
                ModalKind::Save => {
                    return KeyAction::SavePreset(self.save_input.value.trim().to_string());
                }
            },
            KeyCode::Backspace => input.backspace(),
            KeyCode::Left => input.move_left(),
            KeyCode::Right => input.move_right(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => input.insert(ch),
            _ => {}
        }
        KeyAction::None
    }

    fn handle_command_modal_key(&mut self, key: KeyEvent) -> KeyAction {
        if self.command_modal_focus == CommandModalFocus::None {
            match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Char('i') => self.command_modal_focus = CommandModalFocus::Command,
                KeyCode::Enter => self.apply_command_modal_save(),
                _ => {}
            }
            return KeyAction::None;
        }

        match key.code {
            KeyCode::Esc => self.command_modal_focus = CommandModalFocus::None,
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.apply_command_modal_save();
            }
            KeyCode::Enter => {
                if self.command_modal_focus == CommandModalFocus::Command {
                    self.command_input.insert_newline();
                }
            }
            KeyCode::Up => {
                if self.command_modal_focus == CommandModalFocus::Command {
                    self.command_input.move_up();
                } else {
                    self.command_modal_focus = CommandModalFocus::Command;
                }
            }
            KeyCode::Down => {
                if self.command_modal_focus == CommandModalFocus::Command {
                    self.command_input.move_down();
                } else {
                    self.command_modal_focus = CommandModalFocus::Title;
                }
            }
            KeyCode::Tab => {
                self.command_modal_focus = match self.command_modal_focus {
                    CommandModalFocus::None => CommandModalFocus::Command,
                    CommandModalFocus::Command => CommandModalFocus::Title,
                    CommandModalFocus::Title => CommandModalFocus::Interval,
                    CommandModalFocus::Interval => CommandModalFocus::Command,
                }
            }
            KeyCode::BackTab => {
                self.command_modal_focus = match self.command_modal_focus {
                    CommandModalFocus::None => CommandModalFocus::Interval,
                    CommandModalFocus::Command => CommandModalFocus::Interval,
                    CommandModalFocus::Title => CommandModalFocus::Command,
                    CommandModalFocus::Interval => CommandModalFocus::Title,
                }
            }
            KeyCode::Left => self.active_command_input_mut().move_left(),
            KeyCode::Right => self.active_command_input_mut().move_right(),
            KeyCode::Backspace => self.active_command_input_mut().backspace(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.command_modal_focus == CommandModalFocus::Interval && !ch.is_ascii_digit() {
                    self.toast = Some(Toast {
                        message: "interval only accepts digits".into(),
                        level: ToastLevel::Warning,
                        created: Instant::now(),
                    });
                    return KeyAction::None;
                }
                self.active_command_input_mut().insert(ch);
            }
            _ => {}
        }
        KeyAction::None
    }

    fn apply_command_modal_save(&mut self) {
        let command = self.command_input.value.trim().to_string();
        let title = self.title_input.value.trim().to_string();
        let interval_text = self.interval_input.value.trim();
        let interval_ms = if interval_text.is_empty() {
            self.default_interval_ms
        } else {
            match interval_text.parse::<u64>() {
                Ok(value) if value >= 250 => value,
                Ok(_) => {
                    self.toast = Some(Toast {
                        message: "interval must be at least 250 ms".into(),
                        level: ToastLevel::Error,
                        created: Instant::now(),
                    });
                    return;
                }
                Err(_) => {
                    self.toast = Some(Toast {
                        message: "interval must be a number in milliseconds".into(),
                        level: ToastLevel::Error,
                        created: Instant::now(),
                    });
                    return;
                }
            }
        };

        let pane = self.focused_pane_mut();
        let interval_changed = pane.interval_ms != interval_ms;
        pane.cmd = command;
        pane.title = title;
        pane.interval_ms = interval_ms;
        if interval_changed {
            let required_ms = pane
                .current_run_elapsed_ms()
                .or(pane.last_long_running_ms)
                .unwrap_or(0);
            if interval_ms > required_ms {
                pane.long_running_latched = false;
            }
        }
        pane.paused = false;
        pane.last_error = None;
        pane.next_run = Instant::now();
        pane.pending_run_once = true;
        self.mode = Mode::Normal;
    }

    fn active_command_input_mut(&mut self) -> &mut TextInput {
        match self.command_modal_focus {
            CommandModalFocus::None => &mut self.command_input,
            CommandModalFocus::Command => &mut self.command_input,
            CommandModalFocus::Title => &mut self.title_input,
            CommandModalFocus::Interval => &mut self.interval_input,
        }
    }

    fn handle_vars_key(&mut self, key: KeyEvent) -> KeyAction {
        if self.vars_fields.is_empty() {
            self.mode = Mode::Normal;
            return KeyAction::None;
        }
        let current = &mut self.vars_fields[self.vars_focus].input;
        match key.code {
            KeyCode::Esc => return KeyAction::Quit,
            KeyCode::Tab => self.vars_focus = (self.vars_focus + 1) % self.vars_fields.len(),
            KeyCode::BackTab => {
                if self.vars_focus == 0 {
                    self.vars_focus = self.vars_fields.len() - 1;
                } else {
                    self.vars_focus -= 1;
                }
            }
            KeyCode::Enter => {
                if self.vars_focus + 1 < self.vars_fields.len() {
                    self.vars_focus += 1;
                } else {
                    let invalid =
                        self.vars_fields.iter().enumerate().find(|(_, field)| {
                            field.required && field.input.value.trim().is_empty()
                        });
                    if let Some((idx, field)) = invalid {
                        self.vars_focus = idx;
                        self.toast = Some(Toast {
                            message: format!("variable `{}` is required", field.name),
                            level: ToastLevel::Error,
                            created: Instant::now(),
                        });
                    } else {
                        return KeyAction::ApplyVars;
                    }
                }
            }
            KeyCode::Backspace => current.backspace(),
            KeyCode::Left => current.move_left(),
            KeyCode::Right => current.move_right(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                current.insert(ch)
            }
            _ => {}
        }
        KeyAction::None
    }

    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        frame_area: ratatui::layout::Rect,
        pane_rects: &[(usize, ratatui::layout::Rect)],
    ) {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.mode == Mode::Help {
                    if hit_modal_action(
                        help_modal_actions(self),
                        help_modal_rect(frame_area),
                        event.column,
                        event.row,
                    ) == Some("quit")
                    {
                        self.mode = Mode::Normal;
                    }
                    return;
                }
                if self.mode == Mode::CommandModal {
                    match hit_modal_action(
                        command_modal_actions(self),
                        command_modal_rect(frame_area),
                        event.column,
                        event.row,
                    ) {
                        Some("cancel") => {
                            self.mode = Mode::Normal;
                            return;
                        }
                        Some("confirm") => {
                            self.apply_command_modal_save();
                            return;
                        }
                        _ => {}
                    }
                    return;
                }
                if self.mode == Mode::DeleteConfirm {
                    match hit_modal_action(
                        delete_modal_actions(self),
                        delete_modal_rect(frame_area),
                        event.column,
                        event.row,
                    ) {
                        Some("cancel") => {
                            self.mode = Mode::Normal;
                            return;
                        }
                        Some("delete") => {
                            let pane = self.focused_pane_mut();
                            pane.cmd.clear();
                            pane.output.clear();
                            pane.last_error = None;
                            pane.last_exit_code = None;
                            pane.running = false;
                            pane.run_started_at = None;
                            pane.scroll = 0;
                            pane.next_run = Instant::now();
                            self.mode = Mode::Normal;
                            return;
                        }
                        _ => {}
                    }
                    return;
                }
                if self.mode != Mode::Normal {
                    return;
                }
                for (idx, rect) in pane_rects {
                    if contains(*rect, event.column, event.row) {
                        self.focused = *idx;
                        let interval_ms = self.panes[*idx].interval_ms;
                        if let Some(delta_ms) =
                            interval_click_delta(*rect, event.column, event.row, interval_ms)
                        {
                            self.adjust_interval_focused(delta_ms);
                        }
                        break;
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                self.focused_pane_mut().scroll = self.focused_pane().scroll.saturating_add(3);
            }
            MouseEventKind::ScrollUp => {
                self.focused_pane_mut().scroll = self.focused_pane().scroll.saturating_sub(3);
            }
            _ => {}
        }
    }
}

fn contains(rect: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

fn hit_modal_action<'a>(
    actions: ModalActions<'a>,
    modal_rect: ratatui::layout::Rect,
    x: u16,
    y: u16,
) -> Option<&'a str> {
    actions.hit_test(modal_rect, x, y)
}

fn interval_click_delta(
    rect: ratatui::layout::Rect,
    x: u16,
    y: u16,
    interval_ms: u64,
) -> Option<i64> {
    if y != rect.y || rect.width < 16 {
        return None;
    }
    let control = format!("[-] {}ms [+]", interval_ms);
    let control_len = control.chars().count() as u16;
    let start = rect
        .x
        .saturating_add(rect.width.saturating_sub(control_len.saturating_add(2)));
    let minus_range = (start, start.saturating_add(2));
    let plus_start = start.saturating_add(control_len.saturating_sub(3));
    let plus_range = (plus_start, plus_start.saturating_add(2));
    if x >= minus_range.0 && x <= minus_range.1 {
        Some(-50)
    } else if x >= plus_range.0 && x <= plus_range.1 {
        Some(50)
    } else {
        None
    }
}
