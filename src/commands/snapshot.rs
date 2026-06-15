use crate::cli::SnapshotArgs;
use crate::commands::CommandOutput;
use crate::output;
use crate::session_control::{LiveSessionControl, SessionControl};

pub fn run(args: SnapshotArgs) -> Result<CommandOutput, String> {
    let session = LiveSessionControl::resolve(&args.target)?;
    run_with_control(&session)
}

fn run_with_control(session: &impl SessionControl) -> Result<CommandOutput, String> {
    let record = session.record();
    let snapshot = session.capture_snapshot()?;

    Ok(CommandOutput::Markdown(output::snapshot(record, &snapshot)))
}
