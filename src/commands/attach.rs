use crate::attached_ui;
use crate::cli::AttachArgs;
use crate::commands::CommandOutput;
use crate::output;
use crate::session_control::LiveSessionControl;

pub fn run(args: AttachArgs) -> Result<CommandOutput, String> {
    let launch = if args.print_command {
        None
    } else {
        Some(attached_ui::TerminalLaunch::resolve(
            args.terminal_cmd.as_deref(),
            args.terminal_preset,
        )?)
    };

    let session = LiveSessionControl::resolve(&args.target)?;
    session.ensure_attachable()?;
    let record = session.record();
    let remote = session.remote_ui_command();

    if args.print_command {
        return Ok(CommandOutput::Markdown(output::attach_command(
            record, &remote,
        )));
    }

    let launch = launch.expect("launch exists when not printing");
    launch.launch_for_record(record)?;
    Ok(CommandOutput::Markdown(output::attached_ui(
        record,
        &launch.command,
        &launch.source_label(),
    )))
}
