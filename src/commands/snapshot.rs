use std::fs;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::SnapshotArgs;
use crate::commands::CommandOutput;
use crate::screen;
use crate::session;

const SCREEN_SETTLE_TIMEOUT: Duration = Duration::from_secs(1);
const SCREEN_SETTLE_AGE: Duration = Duration::from_millis(100);

pub fn run(args: SnapshotArgs) -> Result<CommandOutput, String> {
    let record = session::resolve_target(&args.target)?;
    let _ = args.inline;
    let current_screen = screen::screen_path(&record);
    let snapshot = read_settled_screen(&current_screen)?;
    let snapshot = screen::normalize_text(&snapshot, record.size);

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
        "### Snapshot\n- Session ID: `{}`\n- Session Name: `{}`\n- Size: `{}`\n- Artifact: `{}`\n\n### Contents\n```text\n",
        record.id,
        record.name.as_deref().unwrap_or("(unnamed)"),
        record.size,
        path.display()
    );
    markdown.push_str(&snapshot);
    if !snapshot.ends_with('\n') {
        markdown.push('\n');
    }
    markdown.push_str("```\n");

    Ok(CommandOutput::Markdown(markdown))
}

fn read_settled_screen(path: &std::path::Path) -> Result<String, String> {
    let start = SystemTime::now();
    let mut last_contents = read_screen(path)?;

    loop {
        let metadata = fs::metadata(path).map_err(|error| {
            format!(
                "failed to stat Session Screen `{}`: {error}",
                path.display()
            )
        })?;
        let modified = metadata.modified().map_err(|error| {
            format!(
                "failed to read Session Screen modified time `{}`: {error}",
                path.display()
            )
        })?;

        if modified.elapsed().unwrap_or_default() >= SCREEN_SETTLE_AGE {
            return read_screen(path);
        }
        if start.elapsed().unwrap_or_default() >= SCREEN_SETTLE_TIMEOUT {
            return Ok(last_contents);
        }

        thread::sleep(Duration::from_millis(25));
        last_contents = read_screen(path)?;
    }
}

fn read_screen(path: &std::path::Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read Session Screen `{}`: {error}",
            path.display()
        )
    })
}
