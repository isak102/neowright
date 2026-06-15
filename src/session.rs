use std::env;
use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cli::{Size, TargetSelector};

pub const DEFAULT_SIZE: Size = Size {
    cols: 240,
    rows: 70,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: String,
    pub name: Option<String>,
    pub cwd: PathBuf,
    pub artifact_dir: PathBuf,
    pub size: SizeRecord,
    pub supervisor_pid: u32,
    #[serde(default)]
    pub child_pid: Option<u32>,
    pub listen: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SizeRecord {
    pub cols: u16,
    pub rows: u16,
}

impl From<Size> for SizeRecord {
    fn from(size: Size) -> Self {
        Self {
            cols: size.cols,
            rows: size.rows,
        }
    }
}

impl std::fmt::Display for SizeRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.cols, self.rows)
    }
}

pub fn generate_id() -> String {
    Uuid::new_v4().simple().to_string()
}

pub fn artifact_dir_for(cwd: &Path) -> PathBuf {
    cwd.join(".neowright")
}

pub fn ensure_artifact_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| {
        format!(
            "failed to create Artifact Directory `{}`: {error}",
            path.display()
        )
    })
}

pub struct SessionRegistry {
    path: PathBuf,
}

struct RegistryLock {
    file: File,
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

impl SessionRegistry {
    pub fn load_global() -> Result<Self, String> {
        Ok(Self {
            path: registry_path()?,
        })
    }

    pub fn active_sessions(&self) -> Result<Vec<SessionRecord>, String> {
        self.with_active_records(|records| Ok(records.to_vec()))
    }

    pub fn insert(&self, record: SessionRecord) -> Result<(), String> {
        self.with_active_records(|records| {
            ensure_name_available(records, record.name.as_deref(), Some(&record.id))?;

            records.retain(|existing| existing.id != record.id);
            records.push(record);
            Ok(())
        })
    }

    pub fn ensure_name_available(&self, name: Option<&str>) -> Result<(), String> {
        self.with_active_records(|records| ensure_name_available(records, name, None))
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        self.with_records(|records| {
            records.retain(|record| record.id != id);
            Ok(())
        })
    }

    pub fn update(&self, updated: SessionRecord) -> Result<(), String> {
        self.with_active_records(|records| {
            let Some(existing) = records.iter_mut().find(|record| record.id == updated.id) else {
                return Err(format!(
                    "no active Session found with Session ID `{}`",
                    updated.id
                ));
            };
            *existing = updated;
            Ok(())
        })
    }

    pub fn resolve_target(&self, selector: &TargetSelector) -> Result<SessionRecord, String> {
        let records = self.active_sessions()?;

        if let Some(id) = &selector.session {
            return records
                .into_iter()
                .find(|record| record.id == *id)
                .ok_or_else(|| format!("no active Session found with Session ID `{id}`"));
        }

        if let Some(name) = &selector.name {
            return records
                .into_iter()
                .find(|record| record.name.as_deref() == Some(name.as_str()))
                .ok_or_else(|| format!("no active Session found with Session Name `{name}`"));
        }

        match records.len() {
            0 => Err("no active Sessions; pass --session or --name after opening one".to_string()),
            1 => Ok(records.into_iter().next().expect("one record exists")),
            _ => Err(format!(
                "multiple active Sessions; pass --session or --name\n\n{}",
                active_session_list(&records)
            )),
        }
    }

    fn with_active_records<T>(
        &self,
        update: impl FnOnce(&mut Vec<SessionRecord>) -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_records(|records| {
            records.retain(|record| process_is_alive(record.supervisor_pid));
            update(records)
        })
    }

    fn with_records<T>(
        &self,
        update: impl FnOnce(&mut Vec<SessionRecord>) -> Result<T, String>,
    ) -> Result<T, String> {
        let _lock = self.lock()?;
        let mut records = self.read_records()?;
        let result = update(&mut records)?;
        self.write_records(&records)?;
        Ok(result)
    }

    fn lock(&self) -> Result<RegistryLock, String> {
        let lock_path = self.path.with_extension("lock");
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create Session Registry directory `{}`: {error}",
                    parent.display()
                )
            })?;
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| {
                format!(
                    "failed to open Session Registry lock `{}`: {error}",
                    lock_path.display()
                )
            })?;

        let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if locked != 0 {
            return Err(format!(
                "failed to lock Session Registry `{}`: {}",
                lock_path.display(),
                std::io::Error::last_os_error()
            ));
        }

        Ok(RegistryLock { file })
    }

    fn read_records(&self) -> Result<Vec<SessionRecord>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&self.path).map_err(|error| {
            format!(
                "failed to read Session Registry `{}`: {error}",
                self.path.display()
            )
        })?;

        serde_json::from_str(&contents).map_err(|error| {
            format!(
                "failed to parse Session Registry `{}`: {error}",
                self.path.display()
            )
        })
    }

    fn write_records(&self, records: &[SessionRecord]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create Session Registry directory `{}`: {error}",
                    parent.display()
                )
            })?;
        }

        let tmp_path = self
            .path
            .with_extension(format!("json.{}.tmp", generate_id()));
        let contents = serde_json::to_string_pretty(records)
            .map_err(|error| format!("failed to serialize Session Registry: {error}"))?;
        fs::write(&tmp_path, contents).map_err(|error| {
            format!(
                "failed to write Session Registry `{}`: {error}",
                tmp_path.display()
            )
        })?;
        fs::rename(&tmp_path, &self.path).map_err(|error| {
            format!(
                "failed to update Session Registry `{}`: {error}",
                self.path.display()
            )
        })
    }
}

fn registry_path() -> Result<PathBuf, String> {
    let base = if let Some(xdg_state_home) = env::var_os("XDG_STATE_HOME") {
        PathBuf::from(xdg_state_home)
    } else if let Some(home) = env::var_os("HOME") {
        PathBuf::from(home).join(".local/state")
    } else {
        return Err("HOME or XDG_STATE_HOME must be set for the Session Registry".to_string());
    };

    Ok(base.join("neowright/registry.json"))
}

fn active_session_list(records: &[SessionRecord]) -> String {
    let mut output = String::from("Active Sessions:");
    for record in records {
        output.push_str(&format!(
            "\n- Session ID: `{}`\n  Session Name: `{}`\n  Opened From: `{}`\n  Size: `{}`",
            record.id,
            record.name.as_deref().unwrap_or("(unnamed)"),
            record.cwd.display(),
            record.size
        ));
    }
    output
}

fn ensure_name_available(
    records: &[SessionRecord],
    name: Option<&str>,
    replacing_id: Option<&str>,
) -> Result<(), String> {
    let Some(name) = name else {
        return Ok(());
    };

    if records.iter().any(|existing| {
        replacing_id != Some(existing.id.as_str()) && existing.name.as_deref() == Some(name)
    }) {
        return Err(format!("Session Name `{name}` is already active"));
    }

    Ok(())
}

pub fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }

    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

pub fn kill_record_processes(record: &SessionRecord) {
    if let Some(child_pid) = record.child_pid {
        kill_pid(child_pid, libc::SIGKILL);
    }
    kill_pid(record.supervisor_pid, libc::SIGTERM);
}

pub fn wait_for_record_exit(record: &SessionRecord, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let supervisor_alive = process_is_alive(record.supervisor_pid);
        let child_alive = record.child_pid.is_some_and(process_is_alive);
        if !supervisor_alive && !child_alive {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

fn kill_pid(pid: u32, signal: libc::c_int) {
    unsafe {
        libc::kill(pid as libc::pid_t, signal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, name: Option<&str>, supervisor_pid: u32) -> SessionRecord {
        SessionRecord {
            id: id.to_string(),
            name: name.map(str::to_string),
            cwd: PathBuf::from("/tmp/project"),
            artifact_dir: PathBuf::from("/tmp/project/.neowright"),
            size: DEFAULT_SIZE.into(),
            supervisor_pid,
            child_pid: None,
            listen: PathBuf::from(format!("/tmp/{id}.sock")),
        }
    }

    #[test]
    fn generated_ids_are_non_empty_and_distinct() {
        let first = generate_id();
        let second = generate_id();

        assert_eq!(first.len(), 32);
        assert_ne!(first, second);
    }

    #[test]
    fn artifact_dir_is_project_local() {
        assert_eq!(
            artifact_dir_for(Path::new("/tmp/project")),
            PathBuf::from("/tmp/project/.neowright")
        );
    }

    #[test]
    fn registry_rejects_duplicate_active_session_names() {
        let tempdir = tempfile::tempdir().expect("create tempdir");
        let registry = SessionRegistry {
            path: tempdir.path().join("registry.json"),
        };
        let pid = std::process::id();

        registry
            .insert(record("first", Some("work"), pid))
            .expect("insert first record");
        let error = registry
            .insert(record("second", Some("work"), pid))
            .expect_err("duplicate active name should be rejected");

        assert_eq!(error, "Session Name `work` is already active");
        assert_eq!(registry.active_sessions().expect("read records").len(), 1);
    }

    #[test]
    fn registry_allows_reusing_stale_session_names() {
        let tempdir = tempfile::tempdir().expect("create tempdir");
        let registry = SessionRegistry {
            path: tempdir.path().join("registry.json"),
        };

        registry
            .insert(record("stale", Some("work"), 0))
            .expect("insert stale record");
        registry
            .insert(record("active", Some("work"), std::process::id()))
            .expect("reuse stale name");

        let records = registry.active_sessions().expect("read active records");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "active");
    }
}
