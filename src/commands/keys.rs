use crate::cli::KeysArgs;
use crate::commands::CommandOutput;
use crate::output;
use crate::session_control::{LiveSessionControl, SessionControl};

pub fn run(args: KeysArgs) -> Result<CommandOutput, String> {
    let mut session = LiveSessionControl::resolve(&args.target)?;
    run_with_control(args, &mut session)
}

fn run_with_control(
    args: KeysArgs,
    session: &mut impl SessionControl,
) -> Result<CommandOutput, String> {
    if args.pty {
        session.send_pty_keys(&args.keys)?;

        return Ok(CommandOutput::Markdown(output::sent_keys(
            "Sent PTY Keys",
            &args.keys,
        )));
    }

    session.send_keys(&args.keys)?;

    Ok(CommandOutput::Markdown(output::sent_keys(
        "Sent Keys",
        &args.keys,
    )))
}
