use crate::cli::Command;

mod attach;
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

pub struct CommandFailure {
    pub message: String,
    pub stdout: Option<String>,
}

impl From<String> for CommandFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            stdout: None,
        }
    }
}

impl From<String> for CommandOutput {
    fn from(value: String) -> Self {
        Self::Status(value)
    }
}

pub fn dispatch(command: Command) -> Result<CommandOutput, CommandFailure> {
    match command {
        Command::Open(args) => open::run(args).map(Into::into),
        Command::Attach(args) => attach::run(args).map_err(Into::into),
        Command::List => list::run().map(Into::into).map_err(Into::into),
        Command::SessionSupervisor(args) => open::run_supervisor(args)
            .map(Into::into)
            .map_err(Into::into),
        Command::Close(args) => close::run(args).map_err(Into::into),
        Command::Keys(args) => keys::run(args).map_err(Into::into),
        Command::Exec(args) => exec::run(args).map_err(Into::into),
        Command::Eval(args) => eval::run(args).map_err(Into::into),
        Command::Wait(args) => wait::run(args).map_err(Into::into),
        Command::Snapshot(args) => snapshot::run(args).map_err(Into::into),
        Command::Resize(args) => resize::run(args).map_err(Into::into),
        Command::Skills(args) => skills::run(args).map(Into::into).map_err(Into::into),
    }
}

pub(crate) use attach::{launch_for_record, launch_summary, validate_launch_options};
