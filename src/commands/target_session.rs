use crate::cli::TargetSelector;
use crate::session::{SessionRecord, SessionRegistry};
use crate::session_io::SessionIo;

pub(super) struct TargetSession {
    record: SessionRecord,
}

impl TargetSession {
    pub(super) fn resolve(selector: &TargetSelector) -> Result<Self, String> {
        let registry = SessionRegistry::load_global()?;
        let record = registry.resolve_target(selector)?;
        Ok(Self { record })
    }

    pub(super) fn record(&self) -> &SessionRecord {
        &self.record
    }

    pub(super) fn io(&self) -> SessionIo {
        SessionIo::for_record(&self.record)
    }
}
