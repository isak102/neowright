use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

struct SupervisorCleanup<'a> {
    state_home: &'a std::path::Path,
}

impl Drop for SupervisorCleanup<'_> {
    fn drop(&mut self) {
        cleanup_supervisors(self.state_home);
    }
}

fn neowright() -> Command {
    Command::cargo_bin("neowright").expect("binary exists")
}

fn nvim_is_available() -> bool {
    std::process::Command::new("nvim")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

#[test]
fn unknown_command_returns_markdown_error() {
    neowright()
        .arg("nope")
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"));
}

#[test]
fn no_args_prints_help_without_markdown_error() {
    neowright()
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: neowright <COMMAND>"))
        .stdout(predicate::str::contains("### Error").not())
        .stderr(predicate::str::is_empty());
}

#[test]
fn malformed_size_returns_markdown_error() {
    neowright()
        .args(["resize", "--session", "abc", "240"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"));
}

#[test]
fn malformed_duration_returns_markdown_error() {
    neowright()
        .args([
            "wait",
            "--session",
            "abc",
            "--timeout",
            "500",
            "return true",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"));
}

#[test]
fn conflicting_targets_return_markdown_error() {
    neowright()
        .args(["snapshot", "--session", "abc", "--name", "main"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"));
}

#[test]
fn required_commands_exist() {
    let cases: &[&[&str]] = &[
        &["list"],
        &["close", "--session", "abc"],
        &["keys", "--session", "abc", "<Esc>"],
        &["exec", "--session", "abc", "write"],
        &["eval", "--session", "abc", "return true"],
        &["wait", "--session", "abc", "return true"],
        &["snapshot", "--session", "abc"],
        &["resize", "--session", "abc", "240x70"],
        &["skills", "install"],
    ];

    for args in cases {
        neowright()
            .args(*args)
            .assert()
            .success()
            .stdout(predicate::str::contains("### Status"));
    }
}

#[test]
fn omitted_target_requires_one_active_session() {
    let state = TempDir::new().expect("state dir");

    neowright()
        .arg("snapshot")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no active Sessions"));
}

#[test]
fn list_reports_empty_registry() {
    let state = TempDir::new().expect("state dir");

    neowright()
        .arg("list")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No active Sessions."));
}

#[test]
fn open_starts_session_and_list_shows_it_when_nvim_exists() {
    if !nvim_is_available() {
        return;
    }

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args([
            "open", "--name", "main", "--size", "100x30", "--", "-u", "NONE",
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Session opened."))
        .stdout(predicate::str::contains("Session Name: `main`"))
        .stdout(predicate::str::contains("Size: `100x30`"));

    neowright()
        .arg("list")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Active Sessions:"))
        .stdout(predicate::str::contains("Active Sessions:\n\n-").not())
        .stdout(predicate::str::contains("Session Name: `main`"))
        .stdout(predicate::str::contains("Size: `100x30`"));

    neowright()
        .arg("snapshot")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Snapshot capture is not implemented yet.",
        ));
}

#[test]
fn open_uses_default_size_and_writes_registry_when_nvim_exists() {
    if !nvim_is_available() {
        return;
    }

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Session ID: `"))
        .stdout(predicate::str::contains("Size: `240x70`"))
        .stdout(predicate::str::contains("Artifact Directory:"));

    let records = registry_records(state.path());
    assert_eq!(records.len(), 1);

    let record = &records[0];
    let id = record.get("id").and_then(Value::as_str).expect("id string");
    assert_eq!(id.len(), 32);
    assert_eq!(record.get("name"), Some(&Value::Null));
    assert_eq!(
        std::path::Path::new(record.get("cwd").and_then(Value::as_str).expect("cwd"))
            .canonicalize()
            .expect("registry cwd canonicalizes"),
        project
            .path()
            .canonicalize()
            .expect("project canonicalizes")
    );
    assert_eq!(
        record.pointer("/size/cols").and_then(Value::as_u64),
        Some(240)
    );
    assert_eq!(
        record.pointer("/size/rows").and_then(Value::as_u64),
        Some(70)
    );
    assert_eq!(
        std::path::Path::new(
            record
                .get("artifact_dir")
                .and_then(Value::as_str)
                .expect("artifact dir"),
        )
        .canonicalize()
        .expect("registry artifact dir canonicalizes"),
        project
            .path()
            .join(".neowright")
            .canonicalize()
            .expect("artifact dir canonicalizes")
    );
    assert!(
        record
            .get("listen")
            .and_then(Value::as_str)
            .expect("listen path")
            .starts_with("/tmp/neowright-")
    );
}

#[test]
fn open_rejects_duplicate_session_name_when_nvim_exists() {
    if !nvim_is_available() {
        return;
    }

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--name", "main", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["open", "--name", "main", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Session Name `main` is already active",
        ));
}

#[test]
fn omitted_target_fails_when_multiple_sessions_are_active_and_nvim_exists() {
    if !nvim_is_available() {
        return;
    }

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--name", "one", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["open", "--name", "two", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .arg("snapshot")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("multiple active Sessions"));
}

#[test]
fn passthrough_args_are_forwarded_after_owned_listen_when_nvim_exists() {
    if !nvim_is_available() {
        return;
    }

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let marker = project.path().join("passthrough-marker");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args([
            "open",
            "--",
            "-u",
            "NONE",
            "--cmd",
            &format!("call writefile(['ok'], '{}')", marker.display()),
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(marker).expect("passthrough marker file"),
        "ok\n"
    );
}

fn registry_records(state_home: &std::path::Path) -> Vec<Value> {
    let registry = state_home.join("neowright/registry.json");
    let contents = std::fs::read_to_string(registry).expect("registry contents");
    let Value::Array(records) = serde_json::from_str::<Value>(&contents).expect("registry json")
    else {
        panic!("registry must be an array")
    };
    records
}

fn cleanup_supervisors(state_home: &std::path::Path) {
    let registry = state_home.join("neowright/registry.json");
    let Ok(contents) = std::fs::read_to_string(registry) else {
        return;
    };
    let Ok(Value::Array(records)) = serde_json::from_str::<Value>(&contents) else {
        return;
    };

    for record in records {
        if let Some(pid) = record.get("supervisor_pid").and_then(Value::as_u64) {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        }
    }
}
