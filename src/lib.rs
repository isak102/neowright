use std::ffi::OsString;
use std::io::{self, Write};

use clap::{CommandFactory, Parser};

pub mod cli;
mod commands;
mod nvim;
mod output;
mod session;

pub fn run(args: impl IntoIterator<Item = OsString>) -> i32 {
    run_with_io(args, &mut io::stdout(), &mut io::stderr())
}

pub fn run_with_io(
    args: impl IntoIterator<Item = OsString>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let args = args.into_iter().collect::<Vec<_>>();

    if args.len() == 1 {
        let _ = cli::Cli::command().write_long_help(stdout);
        let _ = writeln!(stdout);
        return 0;
    }

    let cli = match cli::Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = output::write_error(stderr, error.to_string());
            return 2;
        }
    };

    match commands::dispatch(cli.command) {
        Ok(commands::CommandOutput::Status(message)) => {
            let _ = output::write_success(stdout, &message);
            0
        }
        Ok(
            commands::CommandOutput::Markdown(markdown) | commands::CommandOutput::Raw(markdown),
        ) => {
            let _ = write!(stdout, "{markdown}");
            0
        }
        Err(error) => {
            let _ = output::write_error(stderr, error);
            1
        }
    }
}
