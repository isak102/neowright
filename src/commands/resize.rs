use crate::cli::ResizeArgs;
use crate::session;

pub fn run(args: ResizeArgs) -> Result<String, String> {
    if args.target.session.is_none() && args.target.name.is_none() {
        let _ = session::resolve_target(&args.target)?;
    }

    Ok("`resize` parsed successfully. Session resizing is not implemented yet.".to_string())
}
