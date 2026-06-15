use crate::cli::SnapshotArgs;
use crate::commands::CommandOutput;
use crate::commands::target_session::TargetSession;
use crate::output::MarkdownDocument;

pub fn run(args: SnapshotArgs) -> Result<CommandOutput, String> {
    let target = TargetSession::resolve(&args.target)?;
    let record = target.record();
    let io = target.io();
    let snapshot = io.read_settled_screen(record.size)?;
    let path = io.write_snapshot_artifact(&snapshot)?;

    let mut markdown = MarkdownDocument::new();
    markdown
        .section("Snapshot")
        .field("Session ID", &record.id)
        .field(
            "Session Name",
            record.name.as_deref().unwrap_or("(unnamed)"),
        )
        .field("Size", record.size)
        .field("Artifact", path.display())
        .section("Contents")
        .code_block("text", &snapshot);

    Ok(CommandOutput::Markdown(markdown.finish()))
}
