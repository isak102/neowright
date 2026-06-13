use crate::cli::KeysArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::session;

pub fn run(args: KeysArgs) -> Result<CommandOutput, String> {
    let record = session::resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    client.feed_keys(&args.keys)?;

    Ok(CommandOutput::Markdown(format!(
        "### Sent Keys\n```\n{}\n```\n",
        args.keys
    )))
}
