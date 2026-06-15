use crate::cli::EvalArgs;
use crate::commands::CommandOutput;
use crate::output;
use crate::session_control::{LiveSessionControl, SessionControl};

pub fn run(args: EvalArgs) -> Result<CommandOutput, String> {
    let mut session = LiveSessionControl::resolve(&args.target)?;
    run_with_control(args, &mut session)
}

fn run_with_control(
    args: EvalArgs,
    session: &mut impl SessionControl,
) -> Result<CommandOutput, String> {
    let result = session.eval_lua(&args.lua)?;

    if args.raw {
        return Ok(CommandOutput::Raw(format!("{}\n", result.format_raw())));
    }

    Ok(CommandOutput::Markdown(output::result_with_lua(
        &result.format_display(),
        &args.lua,
    )))
}
