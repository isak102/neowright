use crate::cli::TargetArgs;
use crate::session;

pub fn run(args: TargetArgs) -> Result<String, String> {
    let _ = session::resolve_target(&args.target)?;

    Ok("`snapshot` parsed successfully. Snapshot capture is not implemented yet.".to_string())
}
