use crate::cli::WaitArgs;
use crate::session;

pub fn run(args: WaitArgs) -> Result<String, String> {
    if args.target.session.is_none() && args.target.name.is_none() {
        let _ = session::resolve_target(&args.target)?;
    }

    Ok("`wait` parsed successfully. Condition waiting is not implemented yet.".to_string())
}
