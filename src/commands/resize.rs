use crate::cli::ResizeArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::session;

pub fn run(args: ResizeArgs) -> Result<CommandOutput, String> {
    let mut record = session::resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    client.command(&format!(
        "set columns={} lines={}",
        args.size.cols, args.size.rows
    ))?;
    record.size = args.size.into();
    session::update_record(record.clone())?;

    Ok(CommandOutput::Markdown(format!(
        "### Resized Session\n- Session ID: `{}`\n- Session Name: `{}`\n- Size: `{}`\n",
        record.id,
        record.name.as_deref().unwrap_or("(unnamed)"),
        record.size
    )))
}
