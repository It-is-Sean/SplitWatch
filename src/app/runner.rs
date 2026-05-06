use super::*;
use anyhow::Result;
use std::{
    process::{Command, Stdio},
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

fn spawn_command(
    pane_id: usize,
    command: String,
    tx: Sender<CommandResult>,
    slot: &mut Option<Arc<Mutex<std::process::Child>>>,
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
