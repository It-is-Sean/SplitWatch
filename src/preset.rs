use crate::vars;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanePreset {
    pub title: String,
    pub cmd: String,
    pub interval_ms: Option<u64>,
    #[serde(default)]
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    pub name: Option<String>,
    pub layout: Option<String>,
    pub default_interval_ms: Option<u64>,
    pub theme: Option<String>,
    pub accent: Option<String>,
    #[serde(default)]
    pub focused: Option<usize>,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    #[serde(default)]
    pub panes: Vec<PanePreset>,
}

impl Preset {
    pub fn empty(count: usize, theme: Option<String>, accent: Option<String>) -> Self {
        let panes = (0..count)
            .map(|index| PanePreset {
                title: format!("Pane {}", index + 1),
                cmd: String::new(),
                interval_ms: Some(1000),
                paused: false,
            })
            .collect();
        Self {
            name: None,
            layout: Some("grid".into()),
            default_interval_ms: Some(1000),
            theme,
            accent,
            focused: Some(0),
            vars: BTreeMap::new(),
            panes,
        }
    }

    pub fn resolved(&self, overrides: &BTreeMap<String, String>) -> Result<Self> {
        let mut resolved = self.clone();
        for (key, value) in overrides {
            if !resolved.vars.contains_key(key) {
                bail!("unknown preset variable `{key}`");
            }
            resolved.vars.insert(key.clone(), value.clone());
        }

        let values: Vec<String> = resolved
            .panes
            .iter()
            .flat_map(|pane| [pane.title.clone(), pane.cmd.clone()])
            .collect();
        let missing = vars::missing_vars(&values, &resolved.vars);
        if !missing.is_empty() {
            bail!(
                "missing variable declarations for: {}",
                missing.into_iter().collect::<Vec<_>>().join(", ")
            );
        }

        for pane in &mut resolved.panes {
            pane.title = vars::substitute(&pane.title, &resolved.vars)?;
            pane.cmd = vars::substitute(&pane.cmd, &resolved.vars)?;
        }
        Ok(resolved)
    }
}

pub fn preset_dir_with(xdg_config_home: Option<PathBuf>, home: PathBuf) -> PathBuf {
    xdg_config_home
        .unwrap_or_else(|| home.join(".config"))
        .join("split-watch")
        .join("presets")
}

pub fn state_dir_with(xdg_state_home: Option<PathBuf>, home: PathBuf) -> PathBuf {
    xdg_state_home
        .unwrap_or_else(|| home.join(".local").join("state"))
        .join("split-watch")
}

pub fn preset_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(preset_dir_with(env_path("XDG_CONFIG_HOME"), home))
}

pub fn state_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(state_dir_with(env_path("XDG_STATE_HOME"), home))
}

pub fn resume_path() -> Result<PathBuf> {
    Ok(state_dir()?.join("resume.toml"))
}

pub fn load_from_path(path: &Path) -> Result<Preset> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("failed to read preset {}", path.display()))?;
    let preset = toml::from_str::<Preset>(&data)
        .with_context(|| format!("failed to parse preset {}", path.display()))?;
    Ok(preset)
}

pub fn save_to_path(path: &Path, preset: &Preset) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = toml::to_string_pretty(preset)?;
    fs::write(path, data)?;
    Ok(())
}

pub fn resolve_preset_path(name: &str) -> Result<PathBuf> {
    let mut file = preset_dir()?;
    file.push(format!("{name}.toml"));
    Ok(file)
}

pub fn list_presets() -> Result<Vec<String>> {
    let dir = preset_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            if let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) {
                names.insert(name.to_string());
            }
        }
    }
    Ok(names.into_iter().collect())
}

pub fn ensure_editable_preset(name: &str) -> Result<PathBuf> {
    let path = resolve_preset_path(name)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        save_to_path(
            &path,
            &Preset {
                name: Some(name.to_string()),
                layout: Some("grid".into()),
                default_interval_ms: Some(1000),
                theme: Some("default".into()),
                accent: None,
                focused: Some(0),
                vars: BTreeMap::new(),
                panes: vec![PanePreset {
                    title: "Pane 1".into(),
                    cmd: String::new(),
                    interval_ms: Some(1000),
                    paused: false,
                }],
            },
        )?;
    }
    Ok(path)
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

#[cfg(test)]
mod tests {
    use super::{
        PanePreset, Preset, load_from_path, preset_dir_with, save_to_path, state_dir_with,
    };
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn resolves_xdg_dirs() {
        let home = tempdir().unwrap();
        let xdg = tempdir().unwrap();
        assert_eq!(
            preset_dir_with(Some(xdg.path().to_path_buf()), home.path().to_path_buf()),
            xdg.path().join("split-watch").join("presets")
        );
        assert_eq!(
            state_dir_with(None, home.path().to_path_buf()),
            home.path().join(".local").join("state").join("split-watch")
        );
    }

    #[test]
    fn preset_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("preset.toml");
        let preset = Preset {
            name: Some("gpu-dev".into()),
            layout: Some("grid".into()),
            default_interval_ms: Some(1000),
            theme: Some("catppuccin".into()),
            accent: Some("#89b4fa".into()),
            focused: Some(1),
            vars: BTreeMap::from([("gpu".into(), "0".into())]),
            panes: vec![PanePreset {
                title: "GPU".into(),
                cmd: "nvidia-smi".into(),
                interval_ms: Some(1000),
                paused: false,
            }],
        };
        save_to_path(&path, &preset).unwrap();
        let loaded = load_from_path(&path).unwrap();
        assert_eq!(loaded, preset);
    }
}
