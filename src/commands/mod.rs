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

pub fn dispatch(command: Command) -> Result<String, String> {
    match command {
        Command::Open(args) => open::run(args),
        Command::List => list::run(),
        Command::Close(args) => close::run(args),
        Command::Keys(args) => keys::run(args),
        Command::Exec(args) => exec::run(args),
        Command::Eval(args) => eval::run(args),
        Command::Wait(args) => wait::run(args),
        Command::Snapshot(args) => snapshot::run(args),
        Command::Resize(args) => resize::run(args),
        Command::Skills(args) => skills::run(args),
    }
}
