use crate::{
    preset::{PanePreset, Preset, save_to_path},
    theme::Theme,
};
use anyhow::Result;
use std::{
    collections::VecDeque,
    path::PathBuf,
    process::Child,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime},
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

pub enum KeyAction {
    None,
    Quit,
    SaveResumeAndQuit,
    SavePreset(String),
    ApplyVars,
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

    pub fn focused_pane_mut(&mut self) -> &mut PaneState {
        &mut self.panes[self.focused]
    }

    pub fn focused_pane(&self) -> &PaneState {
        &self.panes[self.focused]
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
}

fn current_layout_name(count: usize) -> &'static str {
    if count == 3 {
        "main-right-stack"
    } else {
        "grid"
    }
}
