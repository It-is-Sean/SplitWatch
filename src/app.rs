use crate::{
    preset::{PanePreset, Preset, save_to_path},
    theme::Theme,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{
    collections::VecDeque,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc::Sender},
    thread,
    time::{Duration, Instant, SystemTime},
};

pub const MAX_OUTPUT_LINES: usize = 400;

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created: Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    InlineCommand,
    CommandModal,
    DeleteConfirm,
    TitleModal,
    SaveModal,
    VarsModal,
    Help,
}

#[derive(Debug, Clone)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
}

impl TextInput {
    pub fn new(value: String) -> Self {
        let cursor = value.len();
        Self { value, cursor }
    }

    pub fn insert(&mut self, ch: char) {
        self.value.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.value[..self.cursor]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        self.value.replace_range(prev..self.cursor, "");
        self.cursor = prev;
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor = self.value[..self.cursor]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0);
    }

    pub fn move_right(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        let next = self.value[self.cursor..]
            .char_indices()
            .nth(1)
            .map(|(idx, _)| self.cursor + idx)
            .unwrap_or(self.value.len());
        self.cursor = next;
    }

    pub fn cursor_col(&self) -> usize {
        self.value[..self.cursor].chars().count()
    }

    pub fn insert_newline(&mut self) {
        self.insert('\n');
    }

    pub fn cursor_line_col(&self) -> (usize, usize) {
        let mut line = 0;
        let mut col = 0;
        for ch in self.value[..self.cursor].chars() {
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    pub fn move_up(&mut self) {
        let (line, col) = self.cursor_line_col();
        if line == 0 {
            return;
        }
        self.cursor = self.byte_index_for_line_col(line - 1, col);
    }

    pub fn move_down(&mut self) {
        let (line, col) = self.cursor_line_col();
        let total_lines = self.line_count();
        if line + 1 >= total_lines {
            return;
        }
        self.cursor = self.byte_index_for_line_col(line + 1, col);
    }

    pub fn line_count(&self) -> usize {
        self.value.split('\n').count().max(1)
    }

    pub fn line_at(&self, line_index: usize) -> &str {
        self.value.split('\n').nth(line_index).unwrap_or("")
    }

    fn byte_index_for_line_col(&self, target_line: usize, target_col: usize) -> usize {
        let mut current_line = 0;
        let mut line_start = 0;
        for segment in self.value.split_inclusive('\n') {
            let line = segment.strip_suffix('\n').unwrap_or(segment);
            if current_line == target_line {
                return nth_char_byte_offset(line, target_col)
                    .map(|offset| line_start + offset)
                    .unwrap_or(line_start + line.len());
            }
            line_start += segment.len();
            current_line += 1;
        }
        if current_line == target_line {
            let line = self.value.rsplit('\n').next().unwrap_or("");
            return nth_char_byte_offset(line, target_col)
                .map(|offset| self.value.len() - line.len() + offset)
                .unwrap_or(self.value.len());
        }
        self.value.len()
    }
}

fn nth_char_byte_offset(value: &str, target_col: usize) -> Option<usize> {
    if target_col == 0 {
        return Some(0);
    }
    value
        .char_indices()
        .nth(target_col)
        .map(|(byte_idx, _)| byte_idx)
}

#[derive(Debug, Clone)]
pub struct VarField {
    pub name: String,
    pub input: TextInput,
    pub required: bool,
}

#[derive(Debug)]
pub struct PaneState {
    pub id: usize,
    pub title: String,
    pub cmd: String,
    pub interval_ms: u64,
    pub paused: bool,
    pub running: bool,
    pub last_started: Option<SystemTime>,
    pub last_finished: Option<SystemTime>,
    pub last_exit_code: Option<i32>,
    pub last_error: Option<String>,
    pub output: VecDeque<String>,
    pub scroll: u16,
    pub next_run: Instant,
    pub child: Option<Arc<Mutex<Child>>>,
}

impl PaneState {
    pub fn from_preset(id: usize, pane: &PanePreset, default_interval: u64) -> Self {
        Self {
            id,
            title: pane.title.clone(),
            cmd: pane.cmd.clone(),
            interval_ms: pane.interval_ms.unwrap_or(default_interval),
            paused: pane.paused,
            running: false,
            last_started: None,
            last_finished: None,
            last_exit_code: None,
            last_error: None,
            output: VecDeque::new(),
            scroll: 0,
            next_run: Instant::now(),
            child: None,
        }
    }

    pub fn set_output(&mut self, output: String) {
        self.output.clear();
        for line in output.lines() {
            self.output.push_back(line.to_string());
            if self.output.len() > MAX_OUTPUT_LINES {
                self.output.pop_front();
            }
        }
        if self.output.is_empty() {
            self.output.push_back(String::new());
        }
        self.scroll = 0;
    }

    pub fn output_text(&self) -> String {
        self.output.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

#[derive(Debug)]
pub enum CommandResult {
    Finished {
        pane_id: usize,
        output: String,
        exit_code: i32,
    },
    Failed {
        pane_id: usize,
        error: String,
    },
}

pub struct App {
    pub panes: Vec<PaneState>,
    pub focused: usize,
    pub mode: Mode,
    pub command_input: TextInput,
    pub interval_input: TextInput,
    pub command_modal_focus: CommandModalFocus,
    pub title_input: TextInput,
    pub save_input: TextInput,
    pub vars_fields: Vec<VarField>,
    pub vars_focus: usize,
    pub theme: Theme,
    pub default_interval_ms: u64,
    pub loaded_preset: Option<PathBuf>,
    pub global_paused: bool,
    pub toast: Option<Toast>,
    pub should_quit: bool,
    pub accent_override: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandModalFocus {
    Command,
    Title,
    Interval,
}

impl App {
    pub fn from_preset(
        preset: Preset,
        theme: Theme,
        loaded_preset: Option<PathBuf>,
        unresolved_vars: Vec<VarField>,
    ) -> Self {
        let default_interval_ms = preset.default_interval_ms.unwrap_or(1000);
        let panes = preset
            .panes
            .iter()
            .enumerate()
            .map(|(idx, pane)| PaneState::from_preset(idx, pane, default_interval_ms))
            .collect::<Vec<_>>();
        Self {
            focused: preset
                .focused
                .unwrap_or(0)
                .min(panes.len().saturating_sub(1)),
            panes,
            mode: if unresolved_vars.is_empty() {
                Mode::Normal
            } else {
                Mode::VarsModal
            },
            command_input: TextInput::new(String::new()),
            interval_input: TextInput::new(String::new()),
            command_modal_focus: CommandModalFocus::Command,
            title_input: TextInput::new(String::new()),
            save_input: TextInput::new(String::new()),
            vars_fields: unresolved_vars,
            vars_focus: 0,
            theme,
            default_interval_ms,
            loaded_preset,
            global_paused: false,
            toast: None,
            should_quit: false,
            accent_override: preset.accent.clone(),
        }
    }

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

    pub fn focused_pane_mut(&mut self) -> &mut PaneState {
        &mut self.panes[self.focused]
    }

    pub fn focused_pane(&self) -> &PaneState {
        &self.panes[self.focused]
    }

    pub fn open_command_modal(&mut self) {
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
        pane.next_run = Instant::now() + Duration::from_millis(next);
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
            Mode::InlineCommand => unreachable!(),
            Mode::DeleteConfirm => unreachable!(),
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
            KeyCode::Enter if self.focused_pane().cmd.trim().is_empty() => {
                self.command_input = TextInput::new(String::new());
                self.mode = Mode::InlineCommand;
            }
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
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                let value = self.command_input.value.trim().to_string();
                let pane = self.focused_pane_mut();
                pane.cmd = value;
                pane.last_error = None;
                pane.next_run = Instant::now();
                self.command_input = TextInput::new(String::new());
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
            KeyCode::Esc | KeyCode::Char('n') => {
                self.mode = Mode::Normal;
            }
            KeyCode::Enter | KeyCode::Char('y') => {
                let pane = self.focused_pane_mut();
                pane.cmd.clear();
                pane.output.clear();
                pane.last_error = None;
                pane.last_exit_code = None;
                pane.running = false;
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
                ModalKind::Command => {
                    let value = self.command_input.value.trim().to_string();
                    let pane = self.focused_pane_mut();
                    pane.cmd = value;
                    pane.last_error = None;
                    pane.next_run = Instant::now();
                    self.mode = Mode::Normal;
                }
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
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
                            return KeyAction::None;
                        }
                        Err(_) => {
                            self.toast = Some(Toast {
                                message: "interval must be a number in milliseconds".into(),
                                level: ToastLevel::Error,
                                created: Instant::now(),
                            });
                            return KeyAction::None;
                        }
                    }
                };

                let pane = self.focused_pane_mut();
                pane.cmd = command;
                pane.title = title;
                pane.interval_ms = interval_ms;
                pane.last_error = None;
                pane.next_run = Instant::now();
                self.mode = Mode::Normal;
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
                    CommandModalFocus::Command => CommandModalFocus::Title,
                    CommandModalFocus::Title => CommandModalFocus::Interval,
                    CommandModalFocus::Interval => CommandModalFocus::Command,
                }
            }
            KeyCode::BackTab => {
                self.command_modal_focus = match self.command_modal_focus {
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

    fn active_command_input_mut(&mut self) -> &mut TextInput {
        match self.command_modal_focus {
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
        pane_rects: &[(usize, ratatui::layout::Rect)],
    ) {
        if self.mode != Mode::Normal {
            return;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
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

    pub fn tick(&mut self, tx: &Sender<CommandResult>) {
        if self.mode == Mode::VarsModal {
            if self
                .toast
                .as_ref()
                .is_some_and(|toast| toast.created.elapsed() > Duration::from_secs(4))
            {
                self.toast = None;
            }
            return;
        }
        let now = Instant::now();
        for pane in &mut self.panes {
            if self.global_paused || pane.paused || pane.running || pane.cmd.trim().is_empty() {
                continue;
            }
            if now >= pane.next_run {
                pane.running = true;
                pane.last_started = Some(SystemTime::now());
                pane.last_error = None;
                pane.next_run = now + Duration::from_millis(pane.interval_ms);
                spawn_command(pane.id, pane.cmd.clone(), tx.clone(), &mut pane.child);
            }
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| toast.created.elapsed() > Duration::from_secs(4))
        {
            self.toast = None;
        }
    }

    pub fn handle_command_result(&mut self, result: CommandResult) {
        match result {
            CommandResult::Finished {
                pane_id,
                output,
                exit_code,
            } => {
                if let Some(pane) = self.panes.get_mut(pane_id) {
                    pane.running = false;
                    pane.child = None;
                    pane.last_finished = Some(SystemTime::now());
                    pane.last_exit_code = Some(exit_code);
                    pane.set_output(output);
                }
            }
            CommandResult::Failed { pane_id, error } => {
                if let Some(pane) = self.panes.get_mut(pane_id) {
                    pane.running = false;
                    pane.child = None;
                    pane.last_finished = Some(SystemTime::now());
                    pane.last_error = Some(error.clone());
                    pane.last_exit_code = None;
                    pane.set_output(error);
                }
            }
        }
    }

    pub fn apply_vars(&mut self) -> Result<()> {
        for pane in &mut self.panes {
            for field in &self.vars_fields {
                let needle = format!("{{{{{}}}}}", field.name);
                pane.title = pane.title.replace(&needle, &field.input.value);
                pane.cmd = pane.cmd.replace(&needle, &field.input.value);
            }
        }
        self.mode = Mode::Normal;
        self.rerun_all();
        Ok(())
    }

    pub fn save_named_preset(&self, path: PathBuf, name: String) -> Result<()> {
        let preset = self.to_preset(Some(name));
        save_to_path(&path, &preset)
    }

    pub fn save_resume_view(&self, path: PathBuf) -> Result<()> {
        let preset = self.to_preset(Some("__resume__".into()));
        save_to_path(&path, &preset)
    }

    pub fn to_preset(&self, name: Option<String>) -> Preset {
        Preset {
            name,
            layout: Some(current_layout_name(self.panes.len()).into()),
            default_interval_ms: Some(self.default_interval_ms),
            theme: Some(self.theme.file.name.clone()),
            accent: self
                .accent_override
                .clone()
                .or_else(|| Some(self.theme.file.accent.clone())),
            focused: Some(self.focused),
            vars: Default::default(),
            panes: self
                .panes
                .iter()
                .map(|pane| PanePreset {
                    title: pane.title.clone(),
                    cmd: pane.cmd.clone(),
                    interval_ms: Some(pane.interval_ms),
                    paused: pane.paused,
                })
                .collect(),
        }
    }

    pub fn kill_running_commands(&mut self) {
        for pane in &mut self.panes {
            if let Some(child) = &pane.child {
                if let Ok(mut child) = child.lock() {
                    let _ = child.kill();
                }
            }
            pane.running = false;
            pane.child = None;
        }
    }
}

fn current_layout_name(count: usize) -> &'static str {
    if count == 3 {
        "main-right-stack"
    } else {
        "grid"
    }
}

pub enum KeyAction {
    None,
    Quit,
    SaveResumeAndQuit,
    SavePreset(String),
    ApplyVars,
}

enum ModalKind {
    Command,
    Title,
    Save,
}

fn spawn_command(
    pane_id: usize,
    command: String,
    tx: Sender<CommandResult>,
    slot: &mut Option<Arc<Mutex<Child>>>,
) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut child = match Command::new(shell)
        .arg("-lc")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = tx.send(CommandResult::Failed {
                pane_id,
                error: format!("failed to start command: {error}"),
            });
            return;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Arc::new(Mutex::new(child));
    *slot = Some(child.clone());
    thread::spawn(move || {
        let stdout_handle = stdout.map(|mut reader| {
            thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = std::io::Read::read_to_end(&mut reader, &mut buf);
                buf
            })
        });
        let stderr_handle = stderr.map(|mut reader| {
            thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = std::io::Read::read_to_end(&mut reader, &mut buf);
                buf
            })
        });

        let status = child.lock().map(|mut child| child.wait());
        let stdout_bytes = stdout_handle
            .map(|handle| handle.join().unwrap_or_default())
            .unwrap_or_default();
        let stderr_bytes = stderr_handle
            .map(|handle| handle.join().unwrap_or_default())
            .unwrap_or_default();

        let mut output = String::from_utf8_lossy(&stdout_bytes).into_owned();
        if !stderr_bytes.is_empty() {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str(&String::from_utf8_lossy(&stderr_bytes));
        }

        match status {
            Ok(Ok(status)) => {
                let _ = tx.send(CommandResult::Finished {
                    pane_id,
                    output,
                    exit_code: status.code().unwrap_or(-1),
                });
            }
            Ok(Err(error)) => {
                let _ = tx.send(CommandResult::Failed {
                    pane_id,
                    error: format!("wait failed: {error}"),
                });
            }
            Err(_) => {
                let _ = tx.send(CommandResult::Failed {
                    pane_id,
                    error: "failed to lock child".into(),
                });
            }
        }
    });
}

fn contains(rect: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
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
