use crate::cli::EvalArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::session;

pub fn run(args: EvalArgs) -> Result<CommandOutput, String> {
    let record = session::resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    let result = client.eval_lua(&args.lua)?;

    if args.raw {
        return Ok(CommandOutput::Raw(format!("{}\n", result.format_raw())));
    }

    Ok(CommandOutput::Markdown(format!(
        "### Result\n```text\n{}\n```\n\n### Ran Lua\n```lua\n{}\n```\n",
        result.format_display(),
        args.lua
    )))
}
