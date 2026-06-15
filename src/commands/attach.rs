use std::ffi::OsString;

use crate::attached_ui;
use crate::cli::AttachArgs;
use crate::commands::CommandOutput;
use crate::output::MarkdownDocument;
use crate::session::{SessionRecord, SessionRegistry};

pub fn run(args: AttachArgs) -> Result<CommandOutput, String> {
    let launch = if args.print_command {
        None
    } else {
        Some(attached_ui::TerminalLaunch::resolve(
            args.terminal_cmd.as_deref(),
            args.terminal_preset,
        )?)
    };

    let record = SessionRegistry::load_global()?.resolve_target(&args.target)?;
    attached_ui::ensure_listen_socket(&record)?;
    let remote = attached_ui::remote_ui_command(&record);

    if args.print_command {
        return Ok(CommandOutput::Markdown(print_command_output(
            &record, &remote,
        )));
    }

    let launch = launch.expect("launch exists when not printing");
    attached_ui::launch_for_record(&record, args.terminal_cmd.as_deref(), args.terminal_preset)?;
    Ok(CommandOutput::Markdown(attached_output(&record, &launch)))
}

fn print_command_output(record: &SessionRecord, remote: &[OsString]) -> String {
    let mut markdown = MarkdownDocument::new();
    markdown
        .section("Attach Command")
        .field("Session ID", &record.id)
        .field(
            "Session Name",
            record.name.as_deref().unwrap_or("(unnamed)"),
        )
        .code_block("bash", &attached_ui::shell_join(remote));
    markdown.finish()
}

fn attached_output(record: &SessionRecord, launch: &attached_ui::TerminalLaunch) -> String {
    let mut markdown = MarkdownDocument::new();
    markdown
        .section("Attached UI")
        .field("Session ID", &record.id)
        .field(
            "Session Name",
            record.name.as_deref().unwrap_or("(unnamed)"),
        )
        .field("Terminal Command", &launch.command)
        .field("Terminal Source", launch.source_label())
        .text("Headed UI process launched.");
    markdown.finish()
}
