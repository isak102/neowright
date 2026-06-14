use crate::output;
use crate::session::SessionRegistry;

pub fn run() -> Result<String, String> {
    let records = SessionRegistry::load_global()?.active_sessions()?;

    Ok(output::active_sessions(&records))
}
