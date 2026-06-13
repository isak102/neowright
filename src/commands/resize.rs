use crate::cli::ResizeArgs;
use crate::session;

pub fn run(args: ResizeArgs) -> Result<String, String> {
    let _ = session::resolve_target(&args.target)?;

    Ok("`resize` parsed successfully. Session resizing is not implemented yet.".to_string())
}
