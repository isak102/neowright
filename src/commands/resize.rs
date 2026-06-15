use crate::cli::ResizeArgs;
use crate::commands::CommandOutput;
use crate::output::MarkdownDocument;
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
