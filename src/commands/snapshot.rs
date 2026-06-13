use crate::cli::TargetArgs;
use crate::session;

pub fn run(args: TargetArgs) -> Result<String, String> {
    if args.target.session.is_none() && args.target.name.is_none() {
        let _ = session::resolve_target(&args.target)?;
    }

    Ok("`snapshot` parsed successfully. Snapshot capture is not implemented yet.".to_string())
}
