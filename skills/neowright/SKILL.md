---
name: neowright
description: Use this skill when debugging, reproducing, automating, or inspecting Neovim UI behavior with Neowright. Trigger whenever the user mentions Neovim TUI issues, plugin or config debugging, floating windows, completion menus, keymaps, diagnostics, snapshots, sessions, or asks an agent to drive or inspect a real Neovim instance.
---

# Neowright

Use Neowright to automate and inspect a real Neovim TUI session from outside Neovim. Neowright is a standalone CLI harness, not a Neovim plugin, MCP server, or general-purpose terminal automation framework.

## When To Use

Use this skill when the task depends on real interactive Neovim behavior:

- Reproducing plugin or configuration issues that only appear in the TUI.
- Inspecting floating windows, completion menus, diagnostics, messages, splits, or layout behavior.
- Driving mappings or key sequences with Neovim-style key notation.
- Waiting for asynchronous UI state before inspecting results.
- Capturing a Snapshot of the visible terminal grid for later review.

Do not use Neowright for tasks that can be answered by reading files or running headless commands alone.

## Core Workflow

Open a named Session so later commands can target it reliably:

```bash
neowright open --name debug -- <nvim-args>
```

Send Neovim-style keys:

```bash
neowright keys --name debug "<leader>ff"
```

Run an Ex command:

```bash
neowright exec --name debug "messages"
```

Inspect or mutate Neovim state with Lua:

```bash
neowright eval --name debug "return vim.api.nvim_get_current_line()"
```

Wait for UI or editor state instead of sleeping:

```bash
neowright wait --name debug "return vim.fn.mode() == 'n'"
```

Capture the visible TUI grid:

```bash
neowright snapshot --name debug
```

Close Sessions opened for the task when the workflow is complete:

```bash
neowright close --name debug
```

## Working Practices

- Be explicit about which Session is being driven.
- Use `--name` for repeatable targeting across commands.
- Read Neowright output as Agent-Readable Markdown; important values such as Session IDs, paths, and results are reported as structured Markdown fields.
- Use Snapshot artifact paths from command output when referring to saved captures.
- Prefer `wait` for state changes that may be asynchronous, such as plugin startup, diagnostics, completion, or UI redraws.
- Close Sessions the agent opened when the task is finished.

## Common Patterns

Open a file and capture the initial UI:

```bash
neowright open --name inspect -- path/to/file.lua
neowright wait --name inspect "return vim.api.nvim_buf_get_name(0):match('file%.lua') ~= nil"
neowright snapshot --name inspect
```

Trigger a mapping and inspect the result:

```bash
neowright keys --name inspect "<leader>x"
neowright wait --name inspect "return true"
neowright snapshot --name inspect
```

Check structured Neovim state:

```bash
neowright eval --name inspect "return vim.inspect(vim.api.nvim_list_wins())"
```
