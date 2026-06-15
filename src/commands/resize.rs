use crate::cli::ResizeArgs;
use crate::commands::CommandOutput;
use crate::commands::target_session::TargetSession;
use crate::output::MarkdownDocument;

pub fn run(args: ResizeArgs) -> Result<CommandOutput, String> {
    let mut target = TargetSession::resolve(&args.target)?;
    target.resize(args.size)?;
    let record = target.record();

    let mut markdown = MarkdownDocument::new();
    markdown
        .section("Resized Session")
        .field("Session ID", &record.id)
        .field(
            "Session Name",
            record.name.as_deref().unwrap_or("(unnamed)"),
        )
        .field("Size", record.size);

    Ok(CommandOutput::Markdown(markdown.finish()))
}
