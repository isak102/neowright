use std::thread;
use std::time::Instant;

use crate::cli::WaitArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::output;
use crate::session;

pub fn run(args: WaitArgs) -> Result<CommandOutput, String> {
    let record = session::SessionRegistry::load_global()?.resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
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
