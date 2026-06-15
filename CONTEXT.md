# Neowright

Neowright is a tool for automating and debugging Neovim user interfaces from outside Neovim.

## Language

**Neowright**:
A standalone CLI harness for launching, driving, inspecting, and capturing a real Neovim TUI session.
_Avoid_: MCP server, Neovim plugin, terminal automation framework

**Agent Skill**:
Documentation and workflow guidance that teaches agents how to use Neowright safely and effectively.
_Avoid_: runtime, server, integration layer

**Session**:
One managed Neovim TUI instance that Neowright controls, inspects, captures, and closes. A user can have multiple Sessions open at the same time.
_Avoid_: process, server, connection

**Session ID**:
A generated identifier for a Session. Every Session has a Session ID.
_Avoid_: name, alias

**Session Name**:
An optional user-provided alias for a Session, used as a memorable way to target that Session.
_Avoid_: ID, handle

**Agent-Readable Markdown**:
The default Neowright output style: structured Markdown sections intended to be readable by both people and agents.
_Avoid_: JSON-first output, prose-only output

**Snapshot**:
An agent-readable capture of the current Neovim TUI state. In the MVP, a Snapshot means a terminal-style capture rather than a pixel image.
_Avoid_: screenshot, image capture

**Attached UI**:
A visible Neovim UI client attached to an existing Session for human observation or interaction. An Attached UI shares editor state with the Session but is not authoritative for Neowright automation or Snapshots.
_Avoid_: Session, terminal backend, snapshot source

**Eval**:
A command that runs Lua in a Session and returns the result. Eval may inspect or mutate the Session.
_Avoid_: read-only inspection

**Exec**:
A command that runs a Neovim command-line command in a Session. Exec accepts commands with or without a leading colon.
_Avoid_: Lua execution, structured inspection

**Keys**:
A command that sends Neovim-style key notation to a Session, such as `<leader>ff`, `<Esc>`, or `<C-w>v`.
_Avoid_: literal text entry, browser-style press

**Wait**:
A command that repeatedly runs a Lua condition in a Session until it becomes true or times out.
_Avoid_: sleep, delay

**Real Config**:
The user's normal Neovim configuration as Neovim would load it outside Neowright.
_Avoid_: project config

**Passthrough Arguments**:
Arguments provided after `--` that Neowright passes to Neovim when opening a Session.
_Avoid_: config flags, startup aliases

**Session Registry**:
The global record Neowright uses to discover and target active Sessions from any working directory.
_Avoid_: project workspace, artifact directory

**Artifact Directory**:
The project-local `.neowright/` directory associated with the working directory where a Session was opened.
_Avoid_: session registry, global state
