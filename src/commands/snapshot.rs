use crate::cli::SnapshotArgs;
use crate::commands::CommandOutput;
use crate::output::MarkdownDocument;
use crate::session_control::{LiveSessionControl, SessionControl};

pub fn run(args: SnapshotArgs) -> Result<CommandOutput, String> {
    let session = LiveSessionControl::resolve(&args.target)?;
    run_with_control(&session)
}

fn run_with_control(session: &impl SessionControl) -> Result<CommandOutput, String> {
    let record = session.record();
    let snapshot = session.capture_snapshot()?;

    let mut markdown = MarkdownDocument::new();
    markdown
        .section("Snapshot")
        .field("Session ID", &record.id)
        .field(
            "Session Name",
            record.name.as_deref().unwrap_or("(unnamed)"),
        )
        .field("Size", record.size)
        .field("Artifact", snapshot.artifact_path.display())
        .section("Contents")
        .code_block("text", &snapshot.contents);

    Ok(CommandOutput::Markdown(markdown.finish()))
}
