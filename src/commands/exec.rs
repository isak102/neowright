use crate::cli::ExecArgs;
use crate::commands::CommandOutput;
use crate::commands::target_session::TargetSession;
use crate::output;

pub fn run(args: ExecArgs) -> Result<CommandOutput, String> {
    let target = TargetSession::resolve(&args.target)?;
    let mut client = target.client()?;
    let command = args.command.strip_prefix(':').unwrap_or(&args.command);
    let output = client.exec(command)?;

    Ok(CommandOutput::Markdown(output::ran_command(
        &output, command,
    )))
}
