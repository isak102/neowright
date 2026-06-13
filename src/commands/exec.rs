use crate::cli::ExecArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::session;

pub fn run(args: ExecArgs) -> Result<CommandOutput, String> {
    let record = session::resolve_target(&args.target)?;
    let mut client = NvimClient::connect(&record)?;
    let command = args.command.strip_prefix(':').unwrap_or(&args.command);
    let output = client.exec(command)?;

    let mut markdown = String::new();
    if !output.trim().is_empty() {
        markdown.push_str("### Output\n```\n");
        markdown.push_str(&output);
        if !output.ends_with('\n') {
            markdown.push('\n');
        }
        markdown.push_str("```\n\n");
    }
    markdown.push_str("### Ran Command\n```vim\n");
    markdown.push_str(command);
    markdown.push_str("\n```\n");

    Ok(CommandOutput::Markdown(markdown))
}
