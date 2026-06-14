use std::thread;
use std::time::Instant;

use crate::cli::WaitArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::session;

pub fn run(args: WaitArgs) -> Result<CommandOutput, String> {
    let record = session::SessionRegistry::load_global()?.resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    let start = Instant::now();
    loop {
        let last_result = client.eval_lua(&args.condition)?;
        if last_result.is_truthy() {
            return Ok(CommandOutput::Markdown(format!(
                "### Result\n```text\n{}\n```\n\n### Ran Lua\n```lua\n{}\n```\n",
                last_result.format_display(),
                args.condition
            )));
        }

        if start.elapsed() >= args.timeout {
            return Err(format!(
                "timed out waiting for Lua condition\n\n### Last Result\n```text\n{}\n```\n\n### Ran Lua\n```lua\n{}\n```",
                last_result.format_display(),
                args.condition
            ));
        }

        thread::sleep(args.interval);
    }
}
