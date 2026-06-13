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
fn short_help_works_for_all_commands() {
    let commands = [
        vec!["-h"],
        vec!["open", "-h"],
        vec!["list", "-h"],
        vec!["close", "-h"],
        vec!["keys", "-h"],
        vec!["exec", "-h"],
        vec!["eval", "-h"],
        vec!["wait", "-h"],
        vec!["snapshot", "-h"],
        vec!["resize", "-h"],
        vec!["skills", "-h"],
        vec!["skills", "install", "-h"],
    ];

    for command in commands {
        neowright()
            .args(command)
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:"))
            .stdout(predicate::str::contains("### Error").not())
            .stderr(predicate::str::is_empty());
    }
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
    neowright()
        .assert()
        .success()
        .stdout(predicate::str::contains("open"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("close"))
        .stdout(predicate::str::contains("keys"))
        .stdout(predicate::str::contains("exec"))
        .stdout(predicate::str::contains("eval"))
        .stdout(predicate::str::contains("wait"))
        .stdout(predicate::str::contains("snapshot"))
        .stdout(predicate::str::contains("resize"))
        .stdout(predicate::str::contains("skills"));
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
        .stdout(predicate::str::contains("### Snapshot"))
        .stdout(predicate::str::contains("Size: `100x30`"));
}

#[test]
fn snapshot_writes_timestamped_plain_text_artifact_when_nvim_exists() {
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
            "open", "--name", "main", "--size", "40x10", "--", "-u", "NONE",
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["keys", "--name", "main", "ihello<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args(["snapshot", "--name", "main"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"))
        .stdout(predicate::str::contains("Artifact:"))
        .stdout(predicate::str::contains("hello").not());

    let snapshot_dir = project.path().join(".neowright/snapshots");
    let snapshots = snapshot_files(&snapshot_dir);
    assert_eq!(snapshots.len(), 1);
    let filename = snapshots[0]
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("snapshot filename");
    assert!(filename.starts_with("snapshot-"));
    assert!(filename.ends_with(".txt"));
    assert!(!snapshot_dir.join("snapshot-latest.txt").exists());

    let contents = std::fs::read_to_string(&snapshots[0]).expect("snapshot contents");
    assert!(contents.contains("hello"));
    assert!(!contents.contains('\u{1b}'));
    assert_snapshot_dimensions(&contents, 40, 10);

    neowright()
        .args(["snapshot", "--name", "main", "--inline"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Contents"))
        .stdout(predicate::str::contains("hello"));

    assert_eq!(snapshot_files(&snapshot_dir).len(), 2);
}

#[test]
fn resize_updates_metadata_and_snapshot_dimensions_when_nvim_exists() {
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
            "open", "--name", "main", "--size", "40x10", "--", "-u", "NONE",
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["resize", "--name", "main", "50x12"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Resized Session"))
        .stdout(predicate::str::contains("Size: `50x12`"));

    neowright()
        .arg("list")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Size: `50x12`"));

    let records = registry_records(state.path());
    assert_eq!(
        records[0].pointer("/size/cols").and_then(Value::as_u64),
        Some(50)
    );
    assert_eq!(
        records[0].pointer("/size/rows").and_then(Value::as_u64),
        Some(12)
    );

    neowright()
        .args(["snapshot", "--name", "main"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    let snapshots = snapshot_files(&project.path().join(".neowright/snapshots"));
    let contents = std::fs::read_to_string(&snapshots[0]).expect("snapshot contents");
    assert_snapshot_dimensions(&contents, 50, 12);
}

#[test]
fn close_handles_graceful_force_all_and_partial_failures_when_nvim_exists() {
    if !nvim_is_available() {
        return;
    }

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--name", "clean", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();
    neowright()
        .args(["close", "--name", "clean"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Closed Sessions"));
    assert_eq!(registry_records(state.path()), Vec::<Value>::new());

    neowright()
        .args(["open", "--name", "dirty", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();
    neowright()
        .args(["keys", "--name", "dirty", "idirty<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args(["close", "--name", "dirty"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsaved changes"));
    assert_eq!(registry_records(state.path()).len(), 1);
    neowright()
        .args(["close", "--name", "dirty", "--force"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    assert_eq!(registry_records(state.path()), Vec::<Value>::new());

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
        .args(["close", "--all"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    assert_eq!(registry_records(state.path()), Vec::<Value>::new());

    neowright()
        .args(["open", "--name", "clean-all", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();
    neowright()
        .args(["open", "--name", "dirty-all", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();
    neowright()
        .args(["keys", "--name", "dirty-all", "idirty<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args(["close", "--all"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("clean-all"))
        .stderr(predicate::str::contains("dirty-all"));
    let records = registry_records(state.path());
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].get("name").and_then(Value::as_str),
        Some("dirty-all")
    );
    neowright()
        .args(["close", "--all", "--force"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    assert_eq!(registry_records(state.path()), Vec::<Value>::new());
}

#[test]
fn supervisor_sigterm_terminates_child_nvim_when_nvim_exists() {
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

    let records = registry_records(state.path());
    let supervisor_pid = records[0]
        .get("supervisor_pid")
        .and_then(Value::as_u64)
        .expect("supervisor pid");
    let listen = records[0]
        .get("listen")
        .and_then(Value::as_str)
        .expect("listen path");

    assert!(neowright_socket_process_exists(listen));
    unsafe {
        libc::kill(supervisor_pid as libc::pid_t, libc::SIGTERM);
    }

    wait_until(std::time::Duration::from_secs(5), || {
        !neowright_socket_process_exists(listen)
    });
    assert!(!neowright_socket_process_exists(listen));
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
    assert!(record.get("child_pid").and_then(Value::as_u64).is_some());
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
        .stderr(predicate::str::contains("multiple active Sessions"))
        .stderr(predicate::str::contains("Active Sessions:"))
        .stderr(predicate::str::contains("Session Name: `one`"))
        .stderr(predicate::str::contains("Session Name: `two`"));
}

#[test]
fn list_cleans_stale_registry_entries() {
    let state = TempDir::new().expect("state dir");
    let registry_dir = state.path().join("neowright");
    std::fs::create_dir_all(&registry_dir).expect("registry dir");
    std::fs::write(
        registry_dir.join("registry.json"),
        r#"[
          {
            "id": "stale",
            "name": "stale-name",
            "cwd": "/tmp",
            "artifact_dir": "/tmp/.neowright",
            "size": { "cols": 80, "rows": 24 },
            "supervisor_pid": 0,
            "listen": "/tmp/neowright-stale.sock"
          }
        ]"#,
    )
    .expect("registry contents");

    neowright()
        .arg("list")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No active Sessions."));

    assert_eq!(registry_records(state.path()), Vec::<Value>::new());
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

#[test]
fn target_resolution_supports_session_id_and_name_when_nvim_exists() {
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

    let records = registry_records(state.path());
    let id = records[0].get("id").and_then(Value::as_str).expect("id");

    neowright()
        .args(["snapshot", "--session", id])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args(["snapshot", "--name", "main"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
}

#[test]
fn eval_exec_keys_and_wait_drive_real_session_when_nvim_exists() {
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
        .args(["eval", "--name", "main", "return { answer = 42 }"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"))
        .stdout(predicate::str::contains("```text"))
        .stdout(predicate::str::contains("answer = 42"))
        .stdout(predicate::str::contains("### Ran Lua"));

    neowright()
        .args(["eval", "--name", "main", "return 'hello\\nworld'"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("```text\nhello\nworld\n```"))
        .stdout(predicate::str::contains(r#"\"hello\\nworld\""#).not());

    neowright()
        .args(["eval", "--name", "main", "vim.g.neowright_side_effect = 42"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"));

    neowright()
        .args([
            "eval",
            "--name",
            "main",
            "--raw",
            "return vim.g.neowright_side_effect",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^42\n$").unwrap());

    neowright()
        .args(["eval", "--name", "main", "--raw", "return 'hello'"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^hello\n$").unwrap());

    neowright()
        .args(["eval", "--name", "main", "--raw", "return { answer = 42 }"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match(r#"^\{"answer":42\}\n$"#).unwrap());

    neowright()
        .args(["eval", "--name", "main", "error('boom')"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("boom"));

    neowright()
        .args(["exec", "--name", "main", ":echo 'from exec'"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Output"))
        .stdout(predicate::str::contains("from exec"))
        .stdout(predicate::str::contains("### Ran Command"));

    neowright()
        .args(["exec", "--name", "main", "NoSuchCommand"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("NoSuchCommand"));

    neowright()
        .args(["keys", "--name", "main", "ihello<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Sent Keys"));

    neowright()
        .args(["keys", "--name", "main", "<C-w>v"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Sent Keys"));

    neowright()
        .args([
            "eval",
            "--name",
            "main",
            "--raw",
            "return #vim.api.nvim_list_wins()",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^2\n$").unwrap());

    assert!(!project.path().join(".neowright/snapshots").exists());

    neowright()
        .args([
            "wait",
            "--name",
            "main",
            "--timeout",
            "2s",
            "return vim.api.nvim_get_current_line() == 'hello'",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"));
}

#[test]
fn canonical_mvp_agent_debugging_loop_when_nvim_exists() {
    if !nvim_is_available() {
        return;
    }

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let passthrough_marker = project.path().join("passthrough-marker");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args([
            "open",
            "--name",
            "demo",
            "--size",
            "40x10",
            "--",
            "-u",
            "NONE",
            "--cmd",
            &format!(
                "call writefile(['passthrough-ok'], '{}')",
                passthrough_marker.display()
            ),
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Status"))
        .stdout(predicate::str::contains("Session opened."))
        .stdout(predicate::str::contains("Session Name: `demo`"))
        .stdout(predicate::str::contains("Size: `40x10`"))
        .stdout(predicate::str::contains("Artifact Directory:"));
    assert_eq!(
        std::fs::read_to_string(&passthrough_marker).expect("passthrough marker file"),
        "passthrough-ok\n"
    );

    neowright()
        .args(["keys", "--name", "demo", "ihello from keys<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Sent Keys"));

    neowright()
        .args(["exec", "--name", "demo", ":let g:neowright_exec = 'ok'"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Ran Command"));

    neowright()
        .args([
            "eval",
            "--name",
            "demo",
            "vim.g.neowright_eval = vim.g.neowright_exec .. '-lua'",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"));

    neowright()
        .args([
            "wait",
            "--name",
            "demo",
            "--timeout",
            "2s",
            "return vim.api.nvim_get_current_line() == 'hello from keys' and vim.g.neowright_eval == 'ok-lua'",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"));

    neowright()
        .args(["resize", "--name", "demo", "50x12"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Resized Session"))
        .stdout(predicate::str::contains("Size: `50x12`"));
    let records = registry_records(state.path());
    assert_eq!(
        records[0].pointer("/size/cols").and_then(Value::as_u64),
        Some(50)
    );
    assert_eq!(
        records[0].pointer("/size/rows").and_then(Value::as_u64),
        Some(12)
    );

    neowright()
        .args(["snapshot", "--name", "demo"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"))
        .stdout(predicate::str::contains("Artifact:"))
        .stdout(predicate::str::contains("hello from keys").not());
    let snapshot_dir = project.path().join(".neowright/snapshots");
    let snapshots = snapshot_files(&snapshot_dir);
    assert_eq!(snapshots.len(), 1);
    let filename = snapshots[0]
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("snapshot filename");
    assert!(filename.starts_with("snapshot-"));
    assert!(filename.ends_with(".txt"));
    assert!(!snapshot_dir.join("snapshot-latest.txt").exists());
    let contents = std::fs::read_to_string(&snapshots[0]).expect("snapshot contents");
    assert!(contents.contains("hello from keys"));
    assert!(!contents.contains('\u{1b}'));
    assert_snapshot_dimensions(&contents, 50, 12);

    neowright()
        .args(["eval", "--name", "demo", "error('acceptance failure')"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("acceptance failure"));

    neowright()
        .args(["close", "--name", "demo", "--force"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Closed Sessions"));
    assert_eq!(registry_records(state.path()), Vec::<Value>::new());
}

#[test]
fn wait_timeout_reports_last_result_when_nvim_exists() {
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
        .args([
            "wait",
            "--name",
            "main",
            "--timeout",
            "500ms",
            "--interval",
            "100ms",
            "return false",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("### Last Result"));
}

#[test]
fn skills_install_defaults_to_global_skill_directory() {
    let home = TempDir::new().expect("home dir");
    let project = TempDir::new().expect("project dir");
    let skill_path = home.path().join(".agents/skills/neowright");

    neowright()
        .args(["skills", "install"])
        .current_dir(project.path())
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Status"))
        .stdout(predicate::str::contains("Installed Neowright Agent Skill."))
        .stdout(predicate::str::contains("Scope: `global`"))
        .stdout(predicate::str::contains("Path: `"))
        .stdout(predicate::str::contains(format!(
            "Path: `{}`",
            skill_path.display()
        )));

    assert_neowright_skill_installed(&skill_path);
    assert!(!project.path().join(".agents").exists());
}

#[test]
fn skills_install_global_uses_home_agents_directory() {
    let home = TempDir::new().expect("home dir");
    let project = TempDir::new().expect("project dir");
    let skill_path = home.path().join(".agents/skills/neowright");

    neowright()
        .args(["skills", "install", "--global"])
        .current_dir(project.path())
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Status"))
        .stdout(predicate::str::contains("Scope: `global`"))
        .stdout(predicate::str::contains(format!(
            "Path: `{}`",
            skill_path.display()
        )));

    assert_neowright_skill_installed(&skill_path);
    assert!(!project.path().join(".agents").exists());
}

#[test]
fn skills_install_local_uses_project_agents_directory() {
    let home = TempDir::new().expect("home dir");
    let project = TempDir::new().expect("project dir");
    let skill_path = project.path().join(".agents/skills/neowright");

    neowright()
        .args(["skills", "install", "--local"])
        .current_dir(project.path())
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Status"))
        .stdout(predicate::str::contains("Scope: `local`"))
        .stdout(predicate::str::contains(".agents/skills/neowright`"));

    assert_neowright_skill_installed(&skill_path);
    assert!(!home.path().join(".agents").exists());
}

#[test]
fn skills_install_does_not_overwrite_existing_skill() {
    let home = TempDir::new().expect("home dir");
    let project = TempDir::new().expect("project dir");
    let skill_path = home.path().join(".agents/skills/neowright");
    std::fs::create_dir_all(&skill_path).expect("existing skill dir");
    std::fs::write(skill_path.join("SKILL.md"), "custom skill").expect("custom skill");

    neowright()
        .args(["skills", "install"])
        .current_dir(project.path())
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("--force"));

    let contents = std::fs::read_to_string(skill_path.join("SKILL.md")).expect("skill contents");
    assert_eq!(contents, "custom skill");
}

#[test]
fn skills_install_force_overwrites_existing_skill() {
    let home = TempDir::new().expect("home dir");
    let project = TempDir::new().expect("project dir");
    let skill_path = home.path().join(".agents/skills/neowright");
    std::fs::create_dir_all(&skill_path).expect("existing skill dir");
    std::fs::write(skill_path.join("SKILL.md"), "custom skill").expect("custom skill");
    std::fs::write(skill_path.join("CUSTOM.md"), "custom file").expect("custom file");

    neowright()
        .args(["skills", "install", "--force"])
        .current_dir(project.path())
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Scope: `global`"));

    assert_neowright_skill_installed(&skill_path);
    assert!(!skill_path.join("CUSTOM.md").exists());
}

fn registry_records(state_home: &std::path::Path) -> Vec<Value> {
    let registry = state_home.join("neowright/registry.json");
    if !registry.exists() {
        return Vec::new();
    }
    let contents = std::fs::read_to_string(registry).expect("registry contents");
    let Value::Array(records) = serde_json::from_str::<Value>(&contents).expect("registry json")
    else {
        panic!("registry must be an array")
    };
    records
}

fn snapshot_files(snapshot_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = std::fs::read_dir(snapshot_dir)
        .expect("snapshot dir")
        .map(|entry| entry.expect("snapshot dir entry").path())
        .filter(|path| path.extension().and_then(std::ffi::OsStr::to_str) == Some("txt"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn assert_snapshot_dimensions(contents: &str, cols: usize, rows: usize) {
    let lines = contents.split('\n').collect::<Vec<_>>();
    assert_eq!(lines.len(), rows);
    for line in lines {
        assert_eq!(line.chars().count(), cols, "snapshot line has wrong width");
    }
}

fn neowright_socket_process_exists(listen: &str) -> bool {
    let output = std::process::Command::new("ps")
        .args(["-ax", "-o", "command="])
        .output()
        .expect("ps output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .any(|line| line.contains("nvim --embed --listen") && line.contains(listen))
}

fn assert_neowright_skill_installed(skill_path: &std::path::Path) {
    assert!(skill_path.is_dir(), "skill path should be a directory");
    let contents = std::fs::read_to_string(skill_path.join("SKILL.md")).expect("skill contents");
    assert!(contents.contains("name: neowright"));
    assert!(contents.contains("standalone CLI harness"));
    assert!(contents.contains("neowright open"));
    assert!(contents.contains("neowright keys"));
    assert!(contents.contains("neowright wait"));
    assert!(contents.contains("neowright snapshot"));
    assert!(contents.contains("neowright close"));
    assert!(!contents.contains("-- --clean"));
    assert!(!contents.contains("test fixtures"));
    assert!(!contents.contains("force-close"));
}

fn wait_until(timeout: std::time::Duration, mut condition: impl FnMut() -> bool) {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn cleanup_supervisors(state_home: &std::path::Path) {
    let registry = state_home.join("neowright/registry.json");
    let Ok(contents) = std::fs::read_to_string(registry) else {
        return;
    };
    let Ok(Value::Array(records)) = serde_json::from_str::<Value>(&contents) else {
        return;
    };

    for record in &records {
        if let Some(pid) = record.get("supervisor_pid").and_then(Value::as_u64) {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(200));

    for record in records {
        if let Some(pid) = record.get("child_pid").and_then(Value::as_u64) {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }
}
