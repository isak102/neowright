use crate::cli::ExecArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::output;
use crate::session;

pub fn run(args: ExecArgs) -> Result<CommandOutput, String> {
    let record = session::SessionRegistry::load_global()?.resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    let command = args.command.strip_prefix(':').unwrap_or(&args.command);
    let output = client.exec(command)?;

    Ok(CommandOutput::Markdown(output::ran_command(
        &output, command,
    )))
}
