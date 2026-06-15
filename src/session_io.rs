use std::fs;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::screen;
use crate::session::{SessionRecord, SizeRecord};

const SCREEN_SETTLE_TIMEOUT: Duration = Duration::from_secs(1);
const SCREEN_SETTLE_AGE: Duration = Duration::from_millis(100);
const SOCKET_DIR: &str = "/tmp";

#[derive(Clone)]
pub(crate) struct SessionRuntimeLayout {
    id: String,
    artifact_dir: PathBuf,
}

impl SessionRuntimeLayout {
    pub(crate) fn new(id: impl Into<String>, artifact_dir: impl Into<PathBuf>) -> Self {
        Self {
            id: id.into(),
            artifact_dir: artifact_dir.into(),
        }
    }

    pub(crate) fn runtime_dir(&self) -> PathBuf {
        self.artifact_dir.join("sessions").join(&self.id)
    }

    pub(crate) fn screen_path(&self) -> PathBuf {
        self.runtime_dir().join("screen.txt")
    }

    pub(crate) fn ready_file(&self) -> PathBuf {
        self.runtime_dir().join("ready")
    }

    pub(crate) fn supervisor_log(&self) -> PathBuf {
        self.runtime_dir().join("supervisor.log")
    }

    pub(crate) fn desired_size_path(&self) -> PathBuf {
        self.runtime_dir().join("desired-size.json")
    }

    pub(crate) fn snapshot_dir(&self) -> PathBuf {
        self.artifact_dir.join("snapshots")
    }

    pub(crate) fn nvim_listen_path(&self) -> PathBuf {
        runtime_socket_path(&self.id, ".sock")
    }

    pub(crate) fn pty_input_path(&self) -> PathBuf {
        runtime_socket_path(&self.id, "-pty.sock")
    }
}

fn runtime_socket_path(session_id: &str, suffix: &str) -> PathBuf {
    // Unix socket paths are constrained by sockaddr_un.sun_path. macOS has a
    // particularly small limit, while $TMPDIR often points at a long
    // /var/folders/... path. Keep sockets under a deliberately short base.
    PathBuf::from(SOCKET_DIR).join(format!("neowright-{session_id}{suffix}"))
}

#[derive(Clone)]
pub(crate) struct SessionIo {
    layout: SessionRuntimeLayout,
}

impl SessionIo {
    pub(crate) fn new(id: impl Into<String>, artifact_dir: impl Into<PathBuf>) -> Self {
        Self {
            layout: SessionRuntimeLayout::new(id, artifact_dir),
        }
    }

    pub(crate) fn for_record(record: &SessionRecord) -> Self {
        Self::new(record.id.clone(), record.artifact_dir.clone())
    }

    pub(crate) fn runtime_dir(&self) -> PathBuf {
        self.layout.runtime_dir()
    }

    pub(crate) fn screen_path(&self) -> PathBuf {
        self.layout.screen_path()
    }

    pub(crate) fn ready_file(&self) -> PathBuf {
        self.layout.ready_file()
    }

    pub(crate) fn supervisor_log(&self) -> PathBuf {
        self.layout.supervisor_log()
    }

    pub(crate) fn nvim_listen_path(&self) -> PathBuf {
        self.layout.nvim_listen_path()
    }

    pub(crate) fn pty_input_path(&self) -> PathBuf {
        self.layout.pty_input_path()
    }

    pub(crate) fn desired_size_path(&self) -> PathBuf {
        self.layout.desired_size_path()
    }

    pub(crate) fn write_latest_screen(&self, contents: &str) -> Result<(), String> {
        screen::write_latest(&self.screen_path(), contents)
    }

    pub(crate) fn read_settled_screen(&self, size: SizeRecord) -> Result<String, String> {
        let path = self.screen_path();
        let snapshot = read_settled_file(&path)?;
        Ok(screen::normalize_text(&snapshot, size))
    }

    pub(crate) fn write_snapshot_artifact(&self, contents: &str) -> Result<PathBuf, String> {
        let snapshot_dir = self.layout.snapshot_dir();
        fs::create_dir_all(&snapshot_dir).map_err(|error| {
            format!(
                "failed to create Snapshot directory `{}`: {error}",
                snapshot_dir.display()
            )
        })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before UNIX epoch: {error}"))?
            .as_millis();
        let filename = format!("snapshot-{timestamp}-{}.txt", &self.layout.id[..8]);
        let path = snapshot_dir.join(filename);
        fs::write(&path, contents)
            .map_err(|error| format!("failed to write Snapshot `{}`: {error}", path.display()))?;
        Ok(path)
    }

    pub(crate) fn write_pty_input(&self, bytes: &[u8]) -> Result<(), String> {
        let path = self.pty_input_path();
        let mut stream = UnixStream::connect(&path).map_err(|error| {
            format!(
                "failed to connect to Session PTY input socket `{}`: {error}",
                path.display()
            )
        })?;
        stream.write_all(bytes).map_err(|error| {
            format!(
                "failed to write Session PTY input socket `{}`: {error}",
                path.display()
            )
        })?;
        stream.flush().map_err(|error| {
            format!(
                "failed to flush Session PTY input socket `{}`: {error}",
                path.display()
            )
        })
    }

    pub(crate) fn write_desired_size(&self, size: SizeRecord) -> Result<(), String> {
        let contents = serde_json::to_string(&size)
            .map_err(|error| format!("failed to serialize desired Session size: {error}"))?;
        screen::write_latest(&self.desired_size_path(), &contents)
    }

    pub(crate) fn read_desired_size(&self) -> Result<Option<SizeRecord>, String> {
        let path = self.desired_size_path();
        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path).map_err(|error| {
            format!(
                "failed to read desired Session size `{}`: {error}",
                path.display()
            )
        })?;
        serde_json::from_str(&contents).map(Some).map_err(|error| {
            format!(
                "failed to parse desired Session size `{}`: {error}",
                path.display()
            )
        })
    }
}

pub(crate) fn restrict_socket_permissions(path: &Path) -> Result<(), String> {
    screen::restrict_socket_permissions(path)
}

fn read_settled_file(path: &Path) -> Result<String, String> {
    let start = SystemTime::now();
    let mut last_contents = read_screen(path)?;

    loop {
        let metadata = fs::metadata(path).map_err(|error| {
            format!(
                "failed to stat Session Screen `{}`: {error}",
                path.display()
            )
        })?;
        let modified = metadata.modified().map_err(|error| {
            format!(
                "failed to read Session Screen modified time `{}`: {error}",
                path.display()
            )
        })?;

        if modified.elapsed().unwrap_or_default() >= SCREEN_SETTLE_AGE {
            return read_screen(path);
        }
        if start.elapsed().unwrap_or_default() >= SCREEN_SETTLE_TIMEOUT {
            return Ok(last_contents);
        }

        thread::sleep(Duration::from_millis(25));
        last_contents = read_screen(path)?;
    }
}

fn read_screen(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read Session Screen `{}`: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_layout_keeps_project_files_under_artifact_directory() {
        let layout = SessionRuntimeLayout::new("1234567890abcdef", "/project/.neowright");

        assert_eq!(
            layout.runtime_dir(),
            PathBuf::from("/project/.neowright/sessions/1234567890abcdef")
        );
        assert_eq!(
            layout.screen_path(),
            PathBuf::from("/project/.neowright/sessions/1234567890abcdef/screen.txt")
        );
        assert_eq!(
            layout.ready_file(),
            PathBuf::from("/project/.neowright/sessions/1234567890abcdef/ready")
        );
        assert_eq!(
            layout.supervisor_log(),
            PathBuf::from("/project/.neowright/sessions/1234567890abcdef/supervisor.log")
        );
        assert_eq!(
            layout.desired_size_path(),
            PathBuf::from("/project/.neowright/sessions/1234567890abcdef/desired-size.json")
        );
        assert_eq!(
            layout.snapshot_dir(),
            PathBuf::from("/project/.neowright/snapshots")
        );
    }

    #[test]
    fn runtime_layout_keeps_socket_paths_short() {
        let layout = SessionRuntimeLayout::new(
            "1234567890abcdef1234567890abcdef",
            "/very/long/project/path/.neowright",
        );

        assert_eq!(
            layout.nvim_listen_path(),
            PathBuf::from("/tmp/neowright-1234567890abcdef1234567890abcdef.sock")
        );
        assert_eq!(
            layout.pty_input_path(),
            PathBuf::from("/tmp/neowright-1234567890abcdef1234567890abcdef-pty.sock")
        );
        assert!(layout.nvim_listen_path().as_os_str().len() < 104);
        assert!(layout.pty_input_path().as_os_str().len() < 104);
    }
}
