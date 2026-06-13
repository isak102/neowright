use crate::cli::EvalArgs;

pub fn run(_args: EvalArgs) -> Result<String, String> {
    Ok("`eval` parsed successfully. Lua evaluation is not implemented yet.".to_string())
}
