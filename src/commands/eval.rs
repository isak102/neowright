use crate::cli::EvalArgs;
use crate::commands::CommandOutput;
use crate::commands::target_session::TargetSession;
use crate::output;

pub fn run(args: EvalArgs) -> Result<CommandOutput, String> {
    let target = TargetSession::resolve(&args.target)?;
    let mut client = target.client()?;
    let result = client.eval_lua(&args.lua)?;

    if args.raw {
        return Ok(CommandOutput::Raw(format!("{}\n", result.format_raw())));
    }

    Ok(CommandOutput::Markdown(output::result_with_lua(
        &result.format_display(),
        &args.lua,
    )))
}
