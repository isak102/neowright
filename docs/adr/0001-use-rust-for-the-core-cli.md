# Use Rust For The Core CLI

Neowright is a durable standalone command-line tool that needs reliable process control, PTY integration, agent-readable behavior, and predictable distribution as a single binary. We will implement the core CLI in Rust rather than Lua, TypeScript, or Go because Rust gives strong state modeling and good packaging while avoiding runtime dependencies for agent workflows.
