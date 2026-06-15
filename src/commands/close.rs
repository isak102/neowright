use crate::cli::CloseArgs;
use crate::commands::CommandOutput;
use crate::output::MarkdownDocument;
use crate::session;
use crate::session_control::{LiveSessionControl, SessionControl};

pub fn run(args: CloseArgs) -> Result<CommandOutput, String> {
    let records = if args.all {
        session::SessionRegistry::load_global()?.active_sessions()?
    } else {
        vec![session::SessionRegistry::load_global()?.resolve_target(&args.target)?]
    };

    if records.is_empty() {
        let mut markdown = MarkdownDocument::new();
        markdown
            .section("Closed Sessions")
            .text("No active Sessions.");
        return Ok(CommandOutput::Markdown(markdown.finish()));
    }

    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for record in records {
        match close_one(record.clone(), args.force) {
            Ok(()) => successes.push(record),
            Err(error) => failures.push((record, error)),
        }
    }

    let mut markdown = MarkdownDocument::new();
    markdown.section("Closed Sessions");
    if successes.is_empty() {
        markdown.text("None.");
    } else {
        for record in &successes {
            markdown.field("Session ID", &record.id).continuation_field(
                "Session Name",
                record.name.as_deref().unwrap_or("(unnamed)"),
            );
        }
    }

    if !failures.is_empty() {
        markdown.section("Failed Sessions");
        for (record, error) in failures {
            markdown
                .field("Session ID", &record.id)
                .continuation_field(
                    "Session Name",
                    record.name.as_deref().unwrap_or("(unnamed)"),
                )
                .continuation_text("Error", error);
        }
        return Err(markdown.finish());
    }

    Ok(CommandOutput::Markdown(markdown.finish()))
}

fn close_one(record: session::SessionRecord, force: bool) -> Result<(), String> {
    LiveSessionControl::for_record(record)?.close(force)
}
