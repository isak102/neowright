use crate::cli::ResizeArgs;
use crate::commands::CommandOutput;
use crate::output;
use crate::session_control::{LiveSessionControl, SessionControl};

pub fn run(args: ResizeArgs) -> Result<CommandOutput, String> {
    let mut session = LiveSessionControl::resolve(&args.target)?;
    run_with_control(args, &mut session)
}

fn run_with_control(
    args: ResizeArgs,
    session: &mut impl SessionControl,
) -> Result<CommandOutput, String> {
    session.resize(args.size)?;
    let record = session.record();

    Ok(CommandOutput::Markdown(output::resized_session(record)))
}
