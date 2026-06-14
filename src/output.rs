use std::io::{self, Write};

use crate::session::SessionRecord;

pub fn write_success(writer: &mut impl Write, message: &str) -> io::Result<()> {
    write!(writer, "{}", status_document(message))
}

pub fn status_document(message: &str) -> String {
    let mut document = MarkdownDocument::new();
    document.section("Status").text(message);
    document.finish()
}

pub fn write_error(writer: &mut impl Write, message: impl AsRef<str>) -> io::Result<()> {
    write!(writer, "{}", error_document(message.as_ref()))
}

pub fn error_document(message: &str) -> String {
    let mut document = MarkdownDocument::new();
    document.section("Error").text(message.trim());
    document.finish()
}

#[derive(Default)]
pub struct MarkdownDocument {
    contents: String,
}

impl MarkdownDocument {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn section(&mut self, title: &str) -> &mut Self {
        if !self.contents.is_empty() {
            self.contents.push('\n');
        }
        self.contents.push_str("### ");
        self.contents.push_str(title);
        self.contents.push('\n');
        self
    }

    pub fn text(&mut self, text: &str) -> &mut Self {
        self.contents.push_str(text.trim_end_matches('\n'));
        self.contents.push('\n');
        self
    }

    pub fn field(&mut self, name: &str, value: impl std::fmt::Display) -> &mut Self {
        self.contents.push_str("- ");
        self.contents.push_str(name);
        self.contents.push_str(": `");
        self.contents.push_str(&value.to_string());
        self.contents.push_str("`\n");
        self
    }

    pub fn continuation_field(&mut self, name: &str, value: impl std::fmt::Display) -> &mut Self {
        self.contents.push_str("  ");
        self.contents.push_str(name);
        self.contents.push_str(": `");
        self.contents.push_str(&value.to_string());
        self.contents.push_str("`\n");
        self
    }

    pub fn continuation_text(&mut self, name: &str, value: impl std::fmt::Display) -> &mut Self {
        self.contents.push_str("  ");
        self.contents.push_str(name);
        self.contents.push_str(": ");
        self.contents.push_str(&value.to_string());
        self.contents.push('\n');
        self
    }

    pub fn code_block(&mut self, language: &str, contents: &str) -> &mut Self {
        let fence = fence_for(contents);
        self.contents.push_str(&fence);
        if !language.is_empty() {
            self.contents.push_str(language);
        }
        self.contents.push('\n');
        self.contents.push_str(contents);
        if !contents.ends_with('\n') {
            self.contents.push('\n');
        }
        self.contents.push_str(&fence);
        self.contents.push('\n');
        self
    }

    pub fn finish(self) -> String {
        self.contents
    }
}

pub fn result_with_lua(result: &str, lua: &str) -> String {
    let mut document = MarkdownDocument::new();
    document
        .section("Result")
        .code_block("text", result)
        .section("Ran Lua")
        .code_block("lua", lua);
    document.finish()
}

pub fn timed_out_lua_condition(result: &str, lua: &str) -> String {
    let mut document = MarkdownDocument::new();
    document
        .text("timed out waiting for Lua condition")
        .section("Last Result")
        .code_block("text", result)
        .section("Ran Lua")
        .code_block("lua", lua);
    document.finish()
}

pub fn ran_command(output: &str, command: &str) -> String {
    let mut document = MarkdownDocument::new();
    if !output.trim().is_empty() {
        document.section("Output").code_block("", output);
    }
    document.section("Ran Command").code_block("vim", command);
    document.finish()
}

pub fn sent_keys(title: &str, keys: &str) -> String {
    let mut document = MarkdownDocument::new();
    document.section(title).code_block("", keys);
    document.finish()
}

pub fn opened_session(record: &SessionRecord) -> String {
    let mut output = String::from("Session opened.");
    output.push_str(&format!("\n- Session ID: `{}`", record.id));
    output.push_str(&format!(
        "\n- Session Name: `{}`",
        record.name.as_deref().unwrap_or("(unnamed)")
    ));
    output.push_str(&format!("\n- Opened From: `{}`", record.cwd.display()));
    output.push_str(&format!("\n- Size: `{}`", record.size));
    output.push_str(&format!(
        "\n- Artifact Directory: `{}`",
        record.artifact_dir.display()
    ));
    output
}

pub fn active_sessions(records: &[SessionRecord]) -> String {
    if records.is_empty() {
        return "No active Sessions.".to_string();
    }

    let mut output = String::from("Active Sessions:");
    for record in records {
        output.push_str(&format!("\n- Session ID: `{}`", record.id));
        output.push_str(&format!(
            "\n  Session Name: `{}`",
            record.name.as_deref().unwrap_or("(unnamed)")
        ));
        output.push_str(&format!("\n  Opened From: `{}`", record.cwd.display()));
        output.push_str(&format!("\n  Size: `{}`", record.size));
    }
    output
}

fn fence_for(contents: &str) -> String {
    let mut longest = 0;
    let mut current = 0;
    for char in contents.chars() {
        if char == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    "`".repeat(longest.max(2) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_and_error_documents_use_stable_sections() {
        assert_eq!(status_document("ok"), "### Status\nok\n");
        assert_eq!(error_document(" bad\n"), "### Error\nbad\n");
    }

    #[test]
    fn document_separates_sections_with_one_blank_line() {
        let mut document = MarkdownDocument::new();
        document.section("One").text("a").section("Two").text("b");

        assert_eq!(document.finish(), "### One\na\n\n### Two\nb\n");
    }

    #[test]
    fn document_separates_intro_text_from_first_section() {
        let mut document = MarkdownDocument::new();
        document.text("intro").section("Details").text("body");

        assert_eq!(document.finish(), "intro\n\n### Details\nbody\n");
    }

    #[test]
    fn fields_render_as_agent_readable_metadata() {
        let mut document = MarkdownDocument::new();
        document
            .section("Snapshot")
            .field("Session ID", "abc")
            .continuation_field("Session Name", "main");

        assert_eq!(
            document.finish(),
            "### Snapshot\n- Session ID: `abc`\n  Session Name: `main`\n"
        );
    }

    #[test]
    fn code_block_adds_trailing_newline_before_closing_fence() {
        let mut document = MarkdownDocument::new();
        document.section("Result").code_block("text", "hello");

        assert_eq!(document.finish(), "### Result\n```text\nhello\n```\n");
    }

    #[test]
    fn code_block_uses_longer_fence_when_contents_contain_backticks() {
        let mut document = MarkdownDocument::new();
        document.section("Result").code_block("text", "a ``` b");

        assert_eq!(document.finish(), "### Result\n````text\na ``` b\n````\n");
    }

    #[test]
    fn active_sessions_status_has_stable_bullet_shape() {
        let record = SessionRecord {
            id: "abc".to_string(),
            name: Some("main".to_string()),
            cwd: "/tmp/project".into(),
            artifact_dir: "/tmp/project/.neowright".into(),
            size: crate::session::SizeRecord { cols: 80, rows: 24 },
            supervisor_pid: 1,
            child_pid: Some(2),
            listen: "/tmp/neowright-abc.sock".into(),
        };

        assert_eq!(
            active_sessions(&[record]),
            "Active Sessions:\n- Session ID: `abc`\n  Session Name: `main`\n  Opened From: `/tmp/project`\n  Size: `80x24`"
        );
    }
}
