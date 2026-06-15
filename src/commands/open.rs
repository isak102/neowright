use crate::attached_ui;
use crate::cli::{OpenArgs, SessionSupervisorArgs};
use crate::commands::CommandFailure;
use crate::output;
use crate::session_launch::{OpenSessionRequest, open_session};
use crate::session_supervisor;

pub fn run(args: OpenArgs) -> Result<String, CommandFailure> {
    let launch = if args.headed {
        Some(attached_ui::TerminalLaunch::resolve(
            args.terminal_cmd.as_deref(),
            args.terminal_preset,
        )?)
    } else {
        None
    };

    let record = open_session(OpenSessionRequest {
        name: args.name,
        size: args.size.unwrap_or_default(),
        neovim_args: args.neovim_args,
    })?;

    let opened = output::opened_session(&record);
    if let Some(launch) = launch {
        match launch.launch_for_record(&record) {
            Ok(()) => {}
            Err(error) => {
                return Err(CommandFailure {
                    message: format!("Headed UI launch failed.\n\n{error}"),
                    stdout: Some(crate::output::status_document(&opened)),
                });
            }
        };

        return Ok(format!(
            "{opened}\n\n{}",
            attached_ui::launch_summary(&launch)
        ));
    }

    Ok(opened)
}

pub fn run_supervisor(args: SessionSupervisorArgs) -> Result<String, String> {
    session_supervisor::run(args)
}
