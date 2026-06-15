# `neowright`

`neowright` is a CLI tool for agents to interact with a real Neovim TUI session. Think [Playwright CLI](https://playwright.dev/docs/test-cli), but for Neovim.

It helps your coding agents debug your actual Neovim configuration, reproduce UI behavior, execute commands, press mappings, inspect editor state, and capture terminal snapshots inside a real Neovim TUI session.

This is mostly not a human-facing tool. Humans can run it, but the primary user is an agent that needs a reliable way to drive Neovim from the outside. The bundled agent skill teaches agents when to use the CLI and how to drive the Neovim feedback loop.

## Demo

https://github.com/user-attachments/assets/479f815e-d205-4d94-b4f8-45a3c0ddb343

Above you can see `neowright` in action with the optional `--headed` flag. 

## Why

Agents are good at reading files and running tests. They are much worse at understanding what happens inside an interactive Neovim UI: floating windows, completion menus, diagnostics, splits, keymaps, plugin startup, redraw timing, and configuration issues.

Neowright gives agents a small yet powerful command surface for that missing layer.

## Use Case

You update a plugin and something in your Neovim UI breaks. Maybe a picker no longer opens, diagnostics render in the wrong place, a mapping stops working, or a floating window layout changes.

Instead of describing the broken state from memory, tell your agent:

```text
The latest plugin update broke my picker UI. Use Neowright to reproduce it in my real Neovim config, inspect what changed, patch the config, and keep using Neowright as the feedback loop until it works again.
```

The agent can then open Neovim, press the same mappings you use, inspect messages and Lua state, capture snapshots, edit your config, and repeat until the behavior is fixed.

## Installation

### Install Neowright

#### From GitHub Releases

This installs `neowright` to `$HOME/.local/bin` by default. Set `NEOWRIGHT_INSTALL_DIR` to choose a different directory:

```bash
curl -fsSL https://raw.githubusercontent.com/isak102/neowright/master/install.sh | sh
```

#### Using Cargo

Install the latest release from git with Cargo:

```bash
cargo install --git https://github.com/isak102/neowright --tag v0.2.0 neowright
```

### Install the Agent Skill

`neowright` ships with an agent skill that teaches agents when and how to use it.

Install it globally:

```bash
neowright skills install --global
```

Install it into the current project:

```bash
neowright skills install --local
```

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

`neowright keys` uses Neovim RPC by default, so Neovim key notation and mappings work. If Neovim is blocked and cannot answer RPC, use the PTY escape hatch with terminal-level notation:

```bash
neowright keys --pty "<CR>"
```

PTY mode writes bytes directly to the Session PTY. It supports plain text plus a small syntax such as `<Esc>`, `<CR>`, `<Tab>`, `<BS>`, `<C-c>`, and `<M-x>`; unsupported notation fails instead of guessing.

Wait for async UI state and capture what the agent can see:

```bash
neowright wait "return vim.fn.mode() == 'n'"
neowright snapshot
```

Open a visible remote UI for demos or human debugging:

```bash
neowright open --name demo --headed -- -u NONE
neowright attach --name demo
neowright attach --name demo --terminal-preset <preset>
neowright attach --name demo --terminal-cmd "<terminal-command>"
neowright attach --name demo --print-command
```

`neowright` can auto-detect known terminal presets from the current terminal environment, so `--terminal-preset` and `--terminal-cmd` are optional when running from a supported terminal. Use `neowright attach -h` to see the current preset flag help, `--terminal-preset` to force a known launch command, or `--terminal-cmd` for an arbitrary terminal command. Without `{}`, `neowright` appends `nvim --server <socket> --remote-ui` as arguments. With `{}`, `neowright` replaces the placeholder with one shell-quoted remote UI command string.

Headed UIs are optional clients attached to the same Neovim instance. The original PTY-backed Session remains authoritative for `keys`, `eval`, `wait`, `resize`, and `snapshot`.

Close the session when done:

```bash
neowright close
```

## Current Shape

`neowright` runs Neovim in a PTY-backed session. Snapshots are text captures of the visible terminal grid, not pixel screenshots.

Session metadata is stored globally so agents can find active sessions from any working directory. Snapshot artifacts are written under `.neowright/` in the project where the session was opened.

Visible attached UIs share editor state with the Session. Multiple UIs can affect layout, especially when the visible terminal is smaller than the `neowright` PTY. Human input can race with agent input, resize events can change snapshots, focus-related plugins may observe extra UI transitions, and `:qa!` from any UI exits the shared Neovim instance.

## Development

```bash
cargo fmt
cargo test
```
