use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use unicode_width::UnicodeWidthChar;

use crate::session::{SessionRecord, SizeRecord};

const SOCKET_DIR: &str = "/tmp";

pub fn runtime_dir(artifact_dir: &Path, session_id: &str) -> PathBuf {
    artifact_dir.join("sessions").join(session_id)
}

pub fn screen_path(record: &SessionRecord) -> PathBuf {
    runtime_dir(&record.artifact_dir, &record.id).join("screen.txt")
}

pub fn desired_size_path(artifact_dir: &Path, session_id: &str) -> PathBuf {
    runtime_dir(artifact_dir, session_id).join("desired-size.json")
}

pub fn nvim_listen_path(session_id: &str) -> PathBuf {
    runtime_socket_path(session_id, ".sock")
}

pub fn pty_input_path(_artifact_dir: &Path, session_id: &str) -> PathBuf {
    runtime_socket_path(session_id, "-pty.sock")
}

fn runtime_socket_path(session_id: &str, suffix: &str) -> PathBuf {
    // Unix socket paths are constrained by sockaddr_un.sun_path. macOS has a
    // particularly small limit, while $TMPDIR often points at a long
    // /var/folders/... path. Keep sockets under a deliberately short base.
    PathBuf::from(SOCKET_DIR).join(format!("neowright-{session_id}{suffix}"))
}

pub fn restrict_socket_permissions(path: &Path) -> Result<(), String> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "failed to restrict Session socket permissions `{}`: {error}",
            path.display()
        )
    })
}

pub fn parser_for(size: SizeRecord) -> vt100::Parser {
    vt100::Parser::new(size.rows, size.cols, 0)
}

pub fn snapshot_text(parser: &vt100::Parser, size: SizeRecord) -> String {
    normalize_lines(parser.screen().rows(0, size.cols), size)
}

pub fn normalize_text(contents: &str, size: SizeRecord) -> String {
    normalize_lines(contents.lines().map(str::to_string), size)
}

pub fn write_latest(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create Screen directory `{}`: {error}",
                parent.display()
            )
        })?;
    }

    let tmp_path = path.with_extension(format!("txt.{}.tmp", crate::session::generate_id()));
    fs::write(&tmp_path, contents)
        .map_err(|error| format!("failed to write Screen `{}`: {error}", tmp_path.display()))?;
    fs::rename(&tmp_path, path)
        .map_err(|error| format!("failed to update Screen `{}`: {error}", path.display()))
}

fn normalize_lines(lines: impl IntoIterator<Item = String>, size: SizeRecord) -> String {
    let rows = usize::from(size.rows);
    let cols = usize::from(size.cols);
    let mut output = Vec::with_capacity(rows);

    for line in lines.into_iter().take(rows) {
        output.push(normalize_line(&line, cols));
    }

    while output.len() < rows {
        output.push(" ".repeat(cols));
    }

    output.join("\n")
}

fn normalize_line(line: &str, cols: usize) -> String {
    let mut normalized = String::new();
    let mut width = 0;

    for char in line.chars() {
        let char_width = char.width().unwrap_or(0);
        if width + char_width > cols {
            break;
        }

        normalized.push(char);
        width += char_width;
    }

    if width < cols {
        normalized.push_str(&" ".repeat(cols - width));
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_escape_sequences_to_fixed_plain_text() {
        let size = SizeRecord { cols: 5, rows: 2 };
        let mut parser = parser_for(size);

        parser.process(b"hello\x1b[2D!!");

        assert_eq!(snapshot_text(&parser, size), "hel!!\n     ");
    }

    #[test]
    fn normalizes_text_to_exact_dimensions() {
        let size = SizeRecord { cols: 4, rows: 3 };

        assert_eq!(normalize_text("abcdef\nx", size), "abcd\nx   \n    ");
    }

    #[test]
    fn normalizes_wide_text_to_terminal_cell_dimensions() {
        let size = SizeRecord { cols: 4, rows: 2 };

        assert_eq!(normalize_text("ab表c\n表表x", size), "ab表\n表表");
    }

    #[test]
    fn runtime_socket_paths_use_short_temp_base() {
        let session_id = "1234567890abcdef1234567890abcdef";

        assert_eq!(
            nvim_listen_path(session_id),
            PathBuf::from("/tmp/neowright-1234567890abcdef1234567890abcdef.sock")
        );
        assert_eq!(
            pty_input_path(Path::new("/very/long/project/path"), session_id),
            PathBuf::from("/tmp/neowright-1234567890abcdef1234567890abcdef-pty.sock")
        );
        assert!(nvim_listen_path(session_id).as_os_str().len() < 104);
        assert!(
            pty_input_path(Path::new("/ignored"), session_id)
                .as_os_str()
                .len()
                < 104
        );
    }

    #[test]
    fn restrict_socket_permissions_sets_owner_only_mode() {
        use std::os::unix::net::UnixListener;

        let path = runtime_socket_path(&crate::session::generate_id(), ".test.sock");
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path).expect("socket binds");

        restrict_socket_permissions(&path).expect("socket permissions restricted");

        let mode = fs::metadata(&path)
            .expect("socket metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        drop(listener);
        let _ = fs::remove_file(path);
    }
}
