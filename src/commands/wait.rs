use crate::cli::WaitArgs;

pub fn run(_args: WaitArgs) -> Result<String, String> {
    Ok("`wait` parsed successfully. Condition waiting is not implemented yet.".to_string())
}
