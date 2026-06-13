use crate::cli::KeysArgs;
use crate::session;

pub fn run(args: KeysArgs) -> Result<String, String> {
    if args.target.session.is_none() && args.target.name.is_none() {
        let _ = session::resolve_target(&args.target)?;
    }

    Ok("`keys` parsed successfully. Key sending is not implemented yet.".to_string())
}
