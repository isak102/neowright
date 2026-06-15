use crate::cli::ExecArgs;
use crate::commands::CommandOutput;
use crate::output;
use crate::session_control::{LiveSessionControl, SessionControl};

pub fn run(args: ExecArgs) -> Result<CommandOutput, String> {
    let mut session = LiveSessionControl::resolve(&args.target)?;
    run_with_control(args, &mut session)
}

fn run_with_control(
    args: ExecArgs,
    session: &mut impl SessionControl,
) -> Result<CommandOutput, String> {
    let command = args.command.strip_prefix(':').unwrap_or(&args.command);
    let output = session.exec(command)?;

    Ok(CommandOutput::Markdown(output::ran_command(
        &output, command,
    )))
}
