use crate::cli::EvalArgs;
use crate::session;

pub fn run(args: EvalArgs) -> Result<String, String> {
    if args.target.session.is_none() && args.target.name.is_none() {
        let _ = session::resolve_target(&args.target)?;
    }

    Ok("`eval` parsed successfully. Lua evaluation is not implemented yet.".to_string())
}
