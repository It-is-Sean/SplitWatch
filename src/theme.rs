use anyhow::{Result, anyhow, bail};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeFile {
    pub name: String,
    pub accent: String,
    pub accent_muted: String,
    pub foreground: String,
    pub muted: String,
    pub background: String,
    pub panel: String,
    pub border: String,
    pub border_focused: String,
    pub success: String,
    pub warning: String,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub file: ThemeFile,
    pub accent: Color,
    pub accent_muted: Color,
    pub foreground: Color,
    pub muted: Color,
    pub background: Color,
    pub panel: Color,
    pub border: Color,
    pub border_focused: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Theme {
    pub fn from_file(file: ThemeFile) -> Result<Self> {
        Ok(Self {
            accent: parse_hex(&file.accent)?,
            accent_muted: parse_hex(&file.accent_muted)?,
            foreground: parse_hex(&file.foreground)?,
            muted: parse_hex(&file.muted)?,
            background: parse_hex(&file.background)?,
            panel: parse_hex(&file.panel)?,
            border: parse_hex(&file.border)?,
            border_focused: parse_hex(&file.border_focused)?,
            success: parse_hex(&file.success)?,
            warning: parse_hex(&file.warning)?,
            error: parse_hex(&file.error)?,
            file,
        })
    }

    pub fn named(name: &str) -> Result<Self> {
        let file = match name {
            "default" => ThemeFile {
                name: "default".into(),
                accent: "#7dd3fc".into(),
                accent_muted: "#155e75".into(),
                foreground: "#e2e8f0".into(),
                muted: "#94a3b8".into(),
                background: "#0f172a".into(),
                panel: "#111827".into(),
                border: "#334155".into(),
                border_focused: "#7dd3fc".into(),
                success: "#86efac".into(),
                warning: "#fde68a".into(),
                error: "#fca5a5".into(),
            },
            "catppuccin" => ThemeFile {
                name: "catppuccin".into(),
                accent: "#89b4fa".into(),
                accent_muted: "#585b70".into(),
                foreground: "#cdd6f4".into(),
                muted: "#6c7086".into(),
                background: "#11111b".into(),
                panel: "#181825".into(),
                border: "#313244".into(),
                border_focused: "#89b4fa".into(),
                success: "#a6e3a1".into(),
                warning: "#f9e2af".into(),
                error: "#f38ba8".into(),
            },
            "nord" => ThemeFile {
                name: "nord".into(),
                accent: "#88c0d0".into(),
                accent_muted: "#4c566a".into(),
                foreground: "#e5e9f0".into(),
                muted: "#81a1c1".into(),
                background: "#2e3440".into(),
                panel: "#3b4252".into(),
                border: "#4c566a".into(),
                border_focused: "#88c0d0".into(),
                success: "#a3be8c".into(),
                warning: "#ebcb8b".into(),
                error: "#bf616a".into(),
            },
            "dracula" => ThemeFile {
                name: "dracula".into(),
                accent: "#bd93f9".into(),
                accent_muted: "#6272a4".into(),
                foreground: "#f8f8f2".into(),
                muted: "#6272a4".into(),
                background: "#282a36".into(),
                panel: "#1f2330".into(),
                border: "#44475a".into(),
                border_focused: "#bd93f9".into(),
                success: "#50fa7b".into(),
                warning: "#f1fa8c".into(),
                error: "#ff5555".into(),
            },
            "monochrome" => ThemeFile {
                name: "monochrome".into(),
                accent: "#f5f5f5".into(),
                accent_muted: "#737373".into(),
                foreground: "#f5f5f5".into(),
                muted: "#a3a3a3".into(),
                background: "#171717".into(),
                panel: "#262626".into(),
                border: "#525252".into(),
                border_focused: "#f5f5f5".into(),
                success: "#d4d4d4".into(),
                warning: "#a3a3a3".into(),
                error: "#fafafa".into(),
            },
            other => bail!("unknown theme `{other}`"),
        };
        Self::from_file(file)
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let data = fs::read_to_string(path)?;
        let file: ThemeFile = toml::from_str(&data)?;
        Self::from_file(file)
    }
}

pub fn parse_hex(value: &str) -> Result<Color> {
    let value = value.trim();
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| anyhow!("color must start with #"))?;
    if hex.len() != 6 {
        bail!("color must be #RRGGBB");
    }
    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok(Color::Rgb(r, g, b))
}
