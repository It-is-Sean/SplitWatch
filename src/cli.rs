use crate::{
    app::{App, Toast, ToastLevel, VarField},
    preset::{
        PanePreset, Preset, ensure_editable_preset, list_presets, load_from_path,
        resolve_preset_path, resume_path,
    },
    theme::{Theme, parse_hex},
    tui::run_tui,
    vars,
};
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::{collections::BTreeMap, path::PathBuf, process::Command};

#[derive(Debug, Parser)]
#[command(
    name = "swatch",
    version,
    about = "Split Watch: a lightweight multi-pane command dashboard"
)]
pub struct Cli {
    #[arg(short = 'n')]
    panes: Option<usize>,
    #[arg(short = 'f', long = "file")]
    file: Option<PathBuf>,
    #[arg(long)]
    theme: Option<String>,
    #[arg(long)]
    accent: Option<String>,
    #[arg(long = "theme-file")]
    theme_file: Option<PathBuf>,
    #[arg(long = "var", value_parser = parse_var)]
    vars: Vec<(String, String)>,
    #[command(subcommand)]
    command: Option<Commands>,
    preset: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    List,
    Edit { preset: String },
    Resume,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::List) => {
            for name in list_presets()? {
                println!("{name}");
            }
            Ok(())
        }
        Some(Commands::Edit { preset }) => edit_preset(&preset),
        Some(Commands::Resume) => {
            let path = resume_path()?;
            if !path.exists() {
                bail!("no saved view to resume");
            }
            let preset = load_from_path(&path)?;
            launch_dashboard(cli, preset, Some(path))
        }
        None => {
            let (preset, loaded_path) = load_requested_preset(&cli)?;
            launch_dashboard(cli, preset, loaded_path)
        }
    }
}

fn load_requested_preset(cli: &Cli) -> Result<(Preset, Option<PathBuf>)> {
    if let Some(path) = &cli.file {
        return Ok((load_from_path(path)?, Some(path.clone())));
    }
    if let Some(name) = &cli.preset {
        let path = resolve_preset_path(name)?;
        return Ok((load_from_path(&path)?, Some(path)));
    }
    let count = cli.panes.unwrap_or(3);
    Ok((
        Preset::empty(count, cli.theme.clone(), cli.accent.clone()),
        None,
    ))
}

fn launch_dashboard(cli: Cli, mut preset: Preset, loaded_path: Option<PathBuf>) -> Result<()> {
    if let Some(theme_name) = &cli.theme {
        preset.theme = Some(theme_name.clone());
    }
    if let Some(accent) = &cli.accent {
        preset.accent = Some(accent.clone());
    }

    let overrides = cli.vars.into_iter().collect::<BTreeMap<_, _>>();
    let unresolved = unresolved_fields(&preset, &overrides)?;
    let resolved = if unresolved.is_empty() {
        preset.resolved(&overrides)?
    } else {
        preset.clone()
    };

    let mut theme = if let Some(theme_file) = cli.theme_file {
        Theme::from_path(&theme_file)?
    } else if let Some(theme_name) = &cli.theme {
        Theme::named(theme_name)?
    } else {
        Theme::named("default")?
    };
    if let Some(accent) = &resolved.accent {
        theme.file.accent = accent.clone();
        theme.accent = parse_hex(accent)?;
        theme.border_focused = parse_hex(accent)?;
    }

    let var_fields = unresolved
        .into_iter()
        .map(|(name, value, required)| VarField {
            name,
            input: crate::app::TextInput::new(value),
            required,
        })
        .collect::<Vec<_>>();

    if resolved.panes.is_empty() {
        preset.panes = vec![PanePreset {
            title: "Pane 1".into(),
            cmd: String::new(),
            interval_ms: Some(1000),
            paused: false,
        }];
    }
    let mut app = App::from_preset(resolved, theme, loaded_path, var_fields);
    if let Some(name) = cli.preset {
        app.toast = Some(Toast {
            message: format!("loaded preset `{name}`"),
            level: ToastLevel::Info,
            created: std::time::Instant::now(),
        });
    }
    run_tui(app)
}

fn unresolved_fields(
    preset: &Preset,
    overrides: &BTreeMap<String, String>,
) -> Result<Vec<(String, String, bool)>> {
    let invalid = vars::invalid_var_names(&preset.vars);
    if !invalid.is_empty() {
        bail!(
            "invalid variable names: {}. use snake_case like log_file, run_name, gpu_id",
            invalid.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    for key in overrides.keys() {
        if !preset.vars.contains_key(key) {
            bail!("unknown preset variable `{key}`");
        }
        if !vars::is_valid_var_name(key) {
            bail!("invalid variable name `{key}`. use snake_case like log_file");
        }
    }
    let mut fields = Vec::new();
    for (name, default) in &preset.vars {
        let value = overrides
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.clone());
        let required = default.trim().is_empty() && value.trim().is_empty();
        if required || !preset.vars.is_empty() {
            fields.push((name.clone(), value, required));
        }
    }
    Ok(fields)
}

fn edit_preset(name: &str) -> Result<()> {
    let path = ensure_editable_preset(name)?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let status = Command::new(editor)
        .arg(&path)
        .status()
        .with_context(|| format!("failed to open editor for {}", path.display()))?;
    if !status.success() {
        bail!("editor exited with status {status}");
    }
    Ok(())
}

fn parse_var(value: &str) -> Result<(String, String), String> {
    let (key, val) = value
        .split_once('=')
        .ok_or_else(|| "expected key=value".to_string())?;
    if key.trim().is_empty() {
        return Err("variable name cannot be empty".into());
    }
    let key = key.trim().to_string();
    if !vars::is_valid_var_name(&key) {
        return Err("variable name must use snake_case letters, numbers, and _".into());
    }
    Ok((key, val.to_string()))
}
