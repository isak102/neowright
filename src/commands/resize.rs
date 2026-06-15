use crate::cli::ResizeArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::output::MarkdownDocument;
use crate::session;
use crate::session_io::SessionIo;

pub fn run(args: ResizeArgs) -> Result<CommandOutput, String> {
    let registry = session::SessionRegistry::load_global()?;
    let mut record = registry.resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    client.command(&format!(
        "set columns={} lines={}",
        args.size.cols, args.size.rows
    ))?;
    record.size = args.size.into();
    registry.update(record.clone())?;
    SessionIo::for_record(&record).write_desired_size(record.size)?;

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
