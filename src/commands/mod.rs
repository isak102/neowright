use crate::cli::Command;

mod close;
mod eval;
mod exec;
mod keys;
mod list;
mod open;
mod resize;
mod skills;
mod snapshot;
mod wait;

pub enum CommandOutput {
    Status(String),
    Markdown(String),
    Raw(String),
}

impl From<String> for CommandOutput {
    fn from(value: String) -> Self {
        Self::Status(value)
    }
}

pub fn dispatch(command: Command) -> Result<CommandOutput, String> {
    match command {
        Command::Open(args) => open::run(args).map(Into::into),
        Command::List => list::run().map(Into::into),
        Command::SessionSupervisor(args) => open::run_supervisor(args).map(Into::into),
        Command::Close(args) => close::run(args),
        Command::Keys(args) => keys::run(args),
        Command::Exec(args) => exec::run(args),
        Command::Eval(args) => eval::run(args),
        Command::Wait(args) => wait::run(args),
        Command::Snapshot(args) => snapshot::run(args),
        Command::Resize(args) => resize::run(args),
        Command::Skills(args) => skills::run(args).map(Into::into),
    }
}
