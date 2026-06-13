use crate::cli::CloseArgs;
use crate::commands::CommandOutput;
use crate::nvim::{NvimClient, NvimValue};
use crate::session;

pub fn run(args: CloseArgs) -> Result<CommandOutput, String> {
    let records = if args.all {
        session::active_records()?
    } else {
        vec![session::resolve_target(&args.target)?]
    };

    if records.is_empty() {
        return Ok(CommandOutput::Markdown(
            "### Closed Sessions\nNo active Sessions.\n".to_string(),
        ));
    }

    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for record in records {
        match close_one(&record, args.force) {
            Ok(()) => successes.push(record),
            Err(error) => failures.push((record, error)),
        }
    }

    let mut markdown = String::from("### Closed Sessions\n");
    if successes.is_empty() {
        markdown.push_str("None.\n");
    } else {
        for record in &successes {
            markdown.push_str(&format!(
                "- Session ID: `{}`\n  Session Name: `{}`\n",
                record.id,
                record.name.as_deref().unwrap_or("(unnamed)")
            ));
        }
    }

    if !failures.is_empty() {
        markdown.push_str("\n### Failed Sessions\n");
        for (record, error) in failures {
            markdown.push_str(&format!(
                "- Session ID: `{}`\n  Session Name: `{}`\n  Error: {}\n",
                record.id,
                record.name.as_deref().unwrap_or("(unnamed)"),
                error
            ));
        }
        return Err(markdown);
    }

    Ok(CommandOutput::Markdown(markdown))
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
        if let NvimValue::Array(buffers) = modified {
            if !buffers.is_empty() {
                return Err(format!(
                    "Session has unsaved changes; use `--force` to discard them ({})",
                    buffers.len()
                ));
            }
        }
    }

    let command = if force { "qall!" } else { "confirm qall" };
    if force {
        let _ = client.command(command);
    } else {
        match client.command(command) {
            Ok(()) => {}
            Err(error) if is_expected_close_disconnect(&error) => {}
            Err(error) => return Err(error),
        }
    }
    session::remove_record(&record.id)?;
    session::kill_record_processes(record);
    Ok(())
}

fn is_expected_close_disconnect(error: &str) -> bool {
    error.contains("failed to read Neovim RPC response for `nvim_command`")
        && error.contains("failed to fill whole buffer")
}
