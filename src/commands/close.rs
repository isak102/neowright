use crate::cli::TargetArgs;

pub fn run(_args: TargetArgs) -> Result<String, String> {
    Ok("`close` parsed successfully. Session closing is not implemented yet.".to_string())
}
