use crate::cli::EvalArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::output;
use crate::session;

pub fn run(args: EvalArgs) -> Result<CommandOutput, String> {
    let record = session::SessionRegistry::load_global()?.resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    let result = client.eval_lua(&args.lua)?;

    if args.raw {
        return Ok(CommandOutput::Raw(format!("{}\n", result.format_raw())));
    }

    Ok(CommandOutput::Markdown(output::result_with_lua(
        &result.format_display(),
        &args.lua,
    )))
}
