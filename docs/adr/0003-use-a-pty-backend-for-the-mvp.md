# Use A PTY Backend For The MVP

The MVP will run Neovim as a real TUI inside a PTY-backed Session. This gives Neowright real interactive Neovim behavior without requiring a specific terminal emulator, tmux, a manually opened window, or headless-only limitations; higher-fidelity terminal backends can be added later behind the same command model.
