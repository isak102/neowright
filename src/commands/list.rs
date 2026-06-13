use crate::session::active_records;

pub fn run() -> Result<String, String> {
    let records = active_records()?;

    if records.is_empty() {
        return Ok("No active Sessions.".to_string());
    }

    let mut output = String::from("Active Sessions:");
    for record in records {
        output.push_str(&format!(
            "\n- Session ID: `{}`\n  Session Name: `{}`\n  Opened From: `{}`\n  Size: `{}`",
            record.id,
            record.name.as_deref().unwrap_or("(unnamed)"),
            record.cwd.display(),
            record.size
        ));
    }

    Ok(output)
}
