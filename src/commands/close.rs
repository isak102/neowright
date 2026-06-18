use crate::cli::CloseArgs;
use crate::commands::CommandOutput;
use crate::output;
use crate::session;
use crate::session_control::LiveSessionControl;

pub fn run(args: CloseArgs) -> Result<CommandOutput, String> {
    let records = if args.all {
        session::SessionRegistry::load_global()?.active_sessions()?
    } else {
        vec![session::SessionRegistry::load_global()?.resolve_target(&args.target)?]
    };

    if records.is_empty() {
        return Ok(CommandOutput::Markdown(output::no_closed_sessions()));
    }

    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for record in records {
        match close_one(record.clone(), args.force) {
            Ok(()) => successes.push(record),
            Err(error) => failures.push((record, error)),
        }
    }

    let markdown = output::close_report(&successes, &failures);
    if !failures.is_empty() {
        return Err(markdown);
    }

    Ok(CommandOutput::Markdown(markdown))
}

fn close_one(record: session::SessionRecord, force: bool) -> Result<(), String> {
    LiveSessionControl::for_record(record)?.close(force)
}
