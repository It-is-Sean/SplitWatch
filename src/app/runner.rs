use super::*;
use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    sync::{Arc, Mutex, mpsc::Sender},
    thread,
    time::{Duration, Instant, SystemTime},
};

impl App {
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
            if pane.running && pane.is_long_running() {
                pane.long_running_latched = true;
                pane.last_long_running_ms = pane.current_run_elapsed_ms();
            }
            if pane.running || pane.cmd.trim().is_empty() {
                continue;
            }
            let should_run_once = pane.pending_run_once;
            if (self.global_paused || pane.paused) && !should_run_once {
                continue;
            }
            if should_run_once || now >= pane.next_run {
                pane.running = true;
                pane.pending_run_once = false;
                pane.run_started_at = Some(now);
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
                    let finished_elapsed_ms = pane.current_run_elapsed_ms();
                    if pane.long_running_latched {
                        pane.last_long_running_ms = finished_elapsed_ms;
                    }
                    pane.running = false;
                    pane.run_started_at = None;
                    pane.child = None;
                    pane.last_finished = Some(SystemTime::now());
                    pane.last_exit_code = Some(exit_code);
                    pane.set_output(output);
                }
            }
            CommandResult::Failed { pane_id, error } => {
                if let Some(pane) = self.panes.get_mut(pane_id) {
                    let finished_elapsed_ms = pane.current_run_elapsed_ms();
                    if pane.long_running_latched {
                        pane.last_long_running_ms = finished_elapsed_ms;
                    }
                    pane.running = false;
                    pane.run_started_at = None;
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

    pub fn kill_running_commands(&mut self) {
        for pane in &mut self.panes {
            if let Some(child) = &pane.child {
                if let Ok(mut child) = child.lock() {
                    let _ = child.kill();
                }
            }
            pane.running = false;
            pane.run_started_at = None;
            pane.child = None;
        }
    }
}

fn spawn_command(
    pane_id: usize,
    command: String,
    tx: Sender<CommandResult>,
    slot: &mut Option<Arc<Mutex<Box<dyn portable_pty::ChildKiller + Send + Sync>>>>,
) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let pty_system = native_pty_system();
    let pair = match pty_system.openpty(PtySize {
        rows: 48,
        cols: 160,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(pair) => pair,
        Err(error) => {
            let _ = tx.send(CommandResult::Failed {
                pane_id,
                error: format!("failed to allocate pty: {error}"),
            });
            return;
        }
    };
    let mut cmd = CommandBuilder::new(shell);
    cmd.arg("-lc");
    cmd.arg(command);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("CLICOLOR_FORCE", "1");
    cmd.env("FORCE_COLOR", "1");

    let mut reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(error) => {
            let _ = tx.send(CommandResult::Failed {
                pane_id,
                error: format!("failed to read from pty: {error}"),
            });
            return;
        }
    };
    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(child) => child,
        Err(error) => {
            let _ = tx.send(CommandResult::Failed {
                pane_id,
                error: format!("failed to start command: {error}"),
            });
            return;
        }
    };
    drop(pair.slave);

    *slot = Some(Arc::new(Mutex::new(child.clone_killer())));
    thread::spawn(move || {
        let read_handle = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut reader, &mut buf);
            buf
        });

        let status = child.wait();
        let output = String::from_utf8_lossy(&read_handle.join().unwrap_or_default()).into_owned();

        match status {
            Ok(status) => {
                let _ = tx.send(CommandResult::Finished {
                    pane_id,
                    output,
                    exit_code: status.exit_code().min(i32::MAX as u32) as i32,
                });
            }
            Err(error) => {
                let _ = tx.send(CommandResult::Failed {
                    pane_id,
                    error: format!("wait failed: {error}"),
                });
            }
        }
    });
}
