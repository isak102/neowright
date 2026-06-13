# Neowright

Neowright is a CLI tool for agents to interact with a real Neovim TUI session. Think [Playwright](https://playwright.dev/), but for Neovim.

It exists so coding agents can debug your actual Neovim configuration, reproduce UI behavior, execute commands, press mappings, inspect editor state, and capture terminal snapshots without pretending that headless Neovim is the same thing as your real editor.

This is mostly not a human-facing tool. Humans can run it, but the primary user is an agent that needs a reliable way to drive Neovim from the outside.

## Installation

### GitHub Releases

This installs `neowright` to `$HOME/.local/bin` by default. Set `NEOWRIGHT_INSTALL_DIR` to choose a different directory:

```bash
curl -fsSL https://raw.githubusercontent.com/isak102/neowright/master/install.sh | sh
```

### Cargo

Install the latest release from git with Cargo:

```bash
cargo install --git https://github.com/isak102/neowright --tag v0.1.0 neowright
```

## Why

Agents are good at reading files and running tests. They are much worse at understanding what happens inside an interactive Neovim UI: floating windows, completion menus, diagnostics, splits, keymaps, plugin startup, redraw timing, and configuration issues that only appear in a real TUI.

Neowright gives agents a small command surface for that missing layer.

## Agent Skill

Neowright ships with an agent skill that teaches agents when and how to use it.

Install it globally:

```bash
neowright skills install --global
```

Install it into the current project:

```bash
neowright skills install --local
```

The skill tells agents to use Neowright for real Neovim UI behavior: plugin debugging, config debugging, mappings, floating windows, diagnostics, completion, snapshots, and session inspection. It also tells them not to use Neowright when normal file reads or commands are enough.

## Use Case

You update a plugin and something in your Neovim UI breaks. Maybe a picker no longer opens, diagnostics render in the wrong place, a mapping stops working, or a floating window layout changes.

Instead of describing the broken state from memory, tell your agent:

```text
The latest plugin update broke my picker UI. Use Neowright to reproduce it in my real Neovim config, inspect what changed, patch the config, and keep using Neowright as the feedback loop until it works again.
```

The agent can then open Neovim, press the same mappings you use, inspect messages and Lua state, capture snapshots, edit your config, and repeat until the real TUI behavior is fixed.

## Examples

Open Neovim with your real config:

```bash
neowright open -- path/to/file.lua
```

Drive the session like a Neovim user would:

```bash
neowright keys "<leader>ff"
neowright exec "messages"
neowright eval "return vim.inspect(vim.api.nvim_list_wins())"
```

Wait for async UI state and capture what the agent can see:

```bash
neowright wait "return vim.fn.mode() == 'n'"
neowright snapshot
```

Close the session when done:

```bash
neowright close
```


## Current Shape

Neowright runs Neovim in a PTY-backed session. Snapshots are text captures of the visible terminal grid, not pixel screenshots.

Session metadata is stored globally so agents can find active sessions from any working directory. Snapshot artifacts are written under `.neowright/` in the project where the session was opened.

## Development

```bash
cargo fmt
cargo test
```

## Future Work

- Multiple backends, including higher-fidelity terminal integrations such as Ghostty and Alacritty.
- Real screenshots in addition to terminal-grid snapshots.
