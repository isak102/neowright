use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::SnapshotArgs;
use crate::commands::CommandOutput;
use crate::nvim::{NvimClient, NvimValue};
use crate::session;

pub fn run(args: SnapshotArgs) -> Result<CommandOutput, String> {
    let record = session::resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    client.command("redraw!")?;

    let lua = format!(
        r#"
local rows = {}
local cols = {}
local lines = {{}}
for row = 1, rows do
  local chars = {{}}
  for col = 1, cols do
    local char = vim.fn.screenstring(row, col)
    if char == "" then
      char = " "
    end
    chars[col] = char
  end
  lines[row] = table.concat(chars)
end
return table.concat(lines, "\n")
"#,
        record.size.rows, record.size.cols
    );
    let NvimValue::String(snapshot) = client.eval_lua(&lua)? else {
        return Err("Neovim did not return Snapshot text".to_string());
    };

    let snapshot_dir = record.artifact_dir.join("snapshots");
    fs::create_dir_all(&snapshot_dir).map_err(|error| {
        format!(
            "failed to create Snapshot directory `{}`: {error}",
            snapshot_dir.display()
        )
    })?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before UNIX epoch: {error}"))?
        .as_millis();
    let filename = format!("snapshot-{timestamp}-{}.txt", &record.id[..8]);
    let path = snapshot_dir.join(filename);
    fs::write(&path, &snapshot)
        .map_err(|error| format!("failed to write Snapshot `{}`: {error}", path.display()))?;

    let mut markdown = format!(
        "### Snapshot\n- Session ID: `{}`\n- Session Name: `{}`\n- Size: `{}`\n- Artifact: [{}]({})\n",
        record.id,
        record.name.as_deref().unwrap_or("(unnamed)"),
        record.size,
        path.display(),
        path.display()
    );
    if args.inline {
        markdown.push_str("\n### Contents\n```text\n");
        markdown.push_str(&snapshot);
        if !snapshot.ends_with('\n') {
            markdown.push('\n');
        }
        markdown.push_str("```\n");
    }

    Ok(CommandOutput::Markdown(markdown))
}
