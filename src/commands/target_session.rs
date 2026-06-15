use crate::cli::{Size, TargetSelector};
use crate::nvim::NvimClient;
use crate::session::{SessionRecord, SessionRegistry};
use crate::session_io::SessionIo;

pub(super) struct TargetSession {
    registry: SessionRegistry,
    record: SessionRecord,
}

impl TargetSession {
    pub(super) fn resolve(selector: &TargetSelector) -> Result<Self, String> {
        let registry = SessionRegistry::load_global()?;
        let record = registry.resolve_target(selector)?;
        Ok(Self { registry, record })
    }

    pub(super) fn record(&self) -> &SessionRecord {
        &self.record
    }

    pub(super) fn client(&self) -> Result<NvimClient, String> {
        NvimClient::connect(&self.record)
    }

    pub(super) fn io(&self) -> SessionIo {
        SessionIo::for_record(&self.record)
    }

    pub(super) fn resize(&mut self, size: Size) -> Result<(), String> {
        let mut client = self.client()?;
        client.command(&format!("set columns={} lines={}", size.cols, size.rows))?;

        self.record.size = size.into();
        self.registry.update(self.record.clone())?;
        self.io().write_desired_size(self.record.size)
    }
}
