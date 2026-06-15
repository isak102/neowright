use std::thread;
use std::time::Instant;

use crate::cli::WaitArgs;
use crate::commands::CommandOutput;
use crate::commands::target_session::TargetSession;
use crate::output;

pub fn run(args: WaitArgs) -> Result<CommandOutput, String> {
    let target = TargetSession::resolve(&args.target)?;
    let mut client = target.client()?;
    let start = Instant::now();
    loop {
        let last_result = client.eval_lua(&args.condition)?;
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
