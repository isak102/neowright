use crate::cli::CloseArgs;
use crate::commands::CommandOutput;
use crate::nvim::{NvimClient, NvimValue};
use crate::output::MarkdownDocument;
use crate::session;

const GRACEFUL_CLOSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

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
        match close_one(&record, args.force) {
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

fn close_one(record: &session::SessionRecord, force: bool) -> Result<(), String> {
    let mut client = NvimClient::connect(record)?;
    if !force {
        let modified = client.eval_lua(
            r#"
local modified = {}
for _, buffer in ipairs(vim.api.nvim_list_bufs()) do
  if vim.bo[buffer].modified then
    table.insert(modified, vim.api.nvim_buf_get_name(buffer))
  end
end
return modified
"#,
        )?;
        if let NvimValue::Array(buffers) = modified
            && !buffers.is_empty()
        {
            return Err(format!(
                "Session has unsaved changes; use `--force` to discard them ({})",
                buffers.len()
            ));
        }
    }

    let command = if force { "qall!" } else { "qall" };
    client.notify_command(command)?;
    let exited = session::wait_for_record_exit(record, GRACEFUL_CLOSE_TIMEOUT);
    session::SessionRegistry::load_global()?.remove(&record.id)?;
    if !exited {
        session::kill_record_processes(record);
    }
    Ok(())
}
