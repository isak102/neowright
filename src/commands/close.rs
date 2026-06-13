use crate::cli::TargetArgs;
use crate::session;

pub fn run(args: TargetArgs) -> Result<String, String> {
    let _ = session::resolve_target(&args.target)?;

    Ok("`close` parsed successfully. Session closing is not implemented yet.".to_string())
}
