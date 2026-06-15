use std::thread;
use std::time::Instant;

use crate::cli::WaitArgs;
use crate::commands::CommandOutput;
use crate::output;
use crate::session_control::{LiveSessionControl, SessionControl};

pub fn run(args: WaitArgs) -> Result<CommandOutput, String> {
    let mut session = LiveSessionControl::resolve(&args.target)?;
    run_with_control(args, &mut session)
}

fn run_with_control(
    args: WaitArgs,
    session: &mut impl SessionControl,
) -> Result<CommandOutput, String> {
    let start = Instant::now();
    loop {
        let last_result = session.eval_lua(&args.condition)?;
        if last_result.is_truthy() {
            return Ok(CommandOutput::Markdown(output::result_with_lua(
                &last_result.format_display(),
                &args.condition,
            )));
        }

        if start.elapsed() >= args.timeout {
            return Err(output::timed_out_lua_condition(
                &last_result.format_display(),
                &args.condition,
            ));
        }

        thread::sleep(args.interval);
    }
}
