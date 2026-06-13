use crate::cli::ExecArgs;
use crate::session;

pub fn run(args: ExecArgs) -> Result<String, String> {
    if args.target.session.is_none() && args.target.name.is_none() {
        let _ = session::resolve_target(&args.target)?;
    }

    Ok("`exec` parsed successfully. Command execution is not implemented yet.".to_string())
}
