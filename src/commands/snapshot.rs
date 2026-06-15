use crate::cli::SnapshotArgs;
use crate::commands::CommandOutput;
use crate::output::MarkdownDocument;
use crate::session;
use crate::session_io::SessionIo;

pub fn run(args: SnapshotArgs) -> Result<CommandOutput, String> {
    let record = session::SessionRegistry::load_global()?.resolve_target(&args.target)?;
    let io = SessionIo::for_record(&record);
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
