use crate::cli::ResizeArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::output::MarkdownDocument;
use crate::screen;
use crate::session;

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
    write_desired_size(&record)?;

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

fn write_desired_size(record: &session::SessionRecord) -> Result<(), String> {
    let path = screen::desired_size_path(&record.artifact_dir, &record.id);
    let contents = serde_json::to_string(&record.size)
        .map_err(|error| format!("failed to serialize desired Session size: {error}"))?;
    screen::write_latest(&path, &contents)
}
