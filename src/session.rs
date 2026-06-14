use std::env;
use std::fs;
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

impl SessionRegistry {
    pub fn load_global() -> Result<Self, String> {
        Ok(Self {
            path: registry_path()?,
        })
    }

    pub fn active_sessions(&self) -> Result<Vec<SessionRecord>, String> {
        let active = self
            .read_records()?
            .into_iter()
            .filter(|record| process_is_alive(record.supervisor_pid))
            .collect::<Vec<_>>();
        self.write_records(&active)?;
        Ok(active)
    }

    pub fn insert(&self, record: SessionRecord) -> Result<(), String> {
        let mut records = self.active_sessions()?;
        records.retain(|existing| existing.id != record.id);
        records.push(record);
        self.write_records(&records)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut records = self.read_records()?;
        records.retain(|record| record.id != id);
        self.write_records(&records)
    }

    pub fn update(&self, updated: SessionRecord) -> Result<(), String> {
        let mut records = self.active_sessions()?;
        let Some(existing) = records.iter_mut().find(|record| record.id == updated.id) else {
            return Err(format!(
                "no active Session found with Session ID `{}`",
                updated.id
            ));
        };
        *existing = updated;
        self.write_records(&records)
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
}
