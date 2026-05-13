<img width="1510" height="844" alt="image" src="https://github.com/user-attachments/assets/f8ad8070-c701-451b-acc5-aa65355ba735" />


# Split Watch

`swatch` is a lightweight terminal dashboard made with rust for periodically running shell commands in multiple panes and showing the latest output.

It is not a terminal multiplexer. It does not manage interactive shells, tabs, SSH sessions, or persistent shell processes. It runs configured commands on an interval and renders the latest results in a polished TUI.

## Features

- Grid dashboard with rounded pane borders
- Vim keys and arrow keys for pane navigation
- Mouse click focus and wheel scrolling
- Per-pane command, title, pause, rerun, and interval controls
- Preset load/save with TOML
- Startup variables with `{{name}}` substitution
- Built-in themes: `default`, `catppuccin`, `nord`, `dracula`, `monochrome`
- Custom theme files
- Save current view and resume it later with `swatch resume`

## Install

If Rust is already available through Homebrew:

```bash
cargo build --release
cp target/release/swatch /usr/local/bin/swatch
```

If you still need the toolchain:

```bash
brew install rust
```

## Usage

Start a temporary dashboard:

```bash
swatch -n 4
swatch -n 4 --theme catppuccin
swatch -n 4 --accent "#89b4fa"
```

Load a preset:

```bash
swatch gpu
swatch logs
swatch -f ./examples/gpu-dev.toml
swatch --file ./examples/gpu-dev.toml
```

List presets:

```bash
swatch list
```

Edit a preset:

```bash
swatch edit gpu
```

Resume the last saved view:

```bash
swatch resume
```

Inside the TUI, press `z` to save the current view and exit. `swatch resume` loads that temporary view as a resume preset from:

- `$XDG_STATE_HOME/split-watch/resume.toml`
- fallback: `~/.local/state/split-watch/resume.toml`

## Presets

Default preset lookup uses:

- `$XDG_CONFIG_HOME/split-watch/presets/`
- fallback: `~/.config/split-watch/presets/`

Example:

```toml
name = "gpu-dev"
layout = "grid"
default_interval_ms = 1000
theme = "catppuccin"
accent = "#89b4fa"

[[panes]]
title = "GPU"
cmd = "nvidia-smi"
interval_ms = 1000

[[panes]]
title = "Git"
cmd = "git status --short"
interval_ms = 2000
```

## Variables

Presets may declare variables under `[vars]` and reference them with `{{name}}`.

```toml
name = "train-debug"
theme = "nord"

[vars]
log_file = ""
pattern = "ERROR"

[[panes]]
title = "Log {{log_file}}"
cmd = "tail -n 200 {{log_file}} | grep {{pattern}}"
interval_ms = 1000
```

Run with overrides:

```bash
swatch train-debug --var log_file=/tmp/train.log --var pattern=CUDA
```

If required values are still empty, `swatch` opens a startup variable modal before running commands.

Variable naming convention:

- Use `snake_case`
- Start with a letter or `_`
- Only use letters, numbers, and `_`
- Good: `log_file`, `run_name`, `gpu_id`, `work_dir`
- Avoid: `log-file`, `run name`, `1gpu`

## Themes

Built-in themes:

- `default`
- `catppuccin`
- `nord`
- `dracula`
- `monochrome`

Load a custom theme file:

```bash
swatch -n 4 --theme-file ./examples/custom-theme.toml
```

## Keys

- `h` `j` `k` `l` or arrow keys: move focus
- `H` `L`: shrink or grow the focused pane width
- `K` `J`: shrink or grow the focused pane height
- `i`: set or edit pane command
- `t`: rename pane
- `r`: rerun focused pane
- `R`: rerun all panes
- `space`: pause or resume focused pane
- `p`: pause or resume all panes
- `+` / `-`: adjust focused pane interval
- `v` / `b`: split focused pane vertically or horizontally
- `x`: delete focused pane
- `s`: save current dashboard as a preset
- `?`: toggle help
- `z`: save current view and exit
- `q` or `Ctrl-C`: quit

## Mouse

- Left click focuses a pane
- Mouse wheel scrolls output in the focused pane

## Security

Presets execute shell commands through your shell. Only load preset files you trust.

## Current MVP Notes

- Commands run periodically and do not overlap within the same pane
- On exit, the app attempts to kill running child processes
- Output is refreshed after each command finishes
- ANSI passthrough is currently minimal; this version focuses on stable command execution and preset flow first
