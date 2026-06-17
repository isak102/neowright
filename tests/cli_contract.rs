use neowright::cli::TERMINAL_PRESETS;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

mod support;

use support::{
    SupervisorCleanup, neowright, registry_records, require_nvim, run_neowright_with_timeout,
    wait_until,
};

fn assert_contains(actual: &str, expected: &str) {
    assert!(
        actual.contains(expected),
        "expected text to contain {expected:?}\nactual:\n{actual}"
    );
}

fn assert_not_contains(actual: &str, unexpected: &str) {
    assert!(
        !actual.contains(unexpected),
        "expected text not to contain {unexpected:?}\nactual:\n{actual}"
    );
}

fn assert_starts_with(actual: &str, expected_prefix: &str) {
    assert!(
        actual.starts_with(expected_prefix),
        "expected text to start with {expected_prefix:?}\nactual: {actual:?}"
    );
}

fn assert_ends_with(actual: &str, expected_suffix: &str) {
    assert!(
        actual.ends_with(expected_suffix),
        "expected text to end with {expected_suffix:?}\nactual: {actual:?}"
    );
}

fn assert_is_dir(path: impl AsRef<std::path::Path>) {
    let path = path.as_ref();
    assert!(
        path.is_dir(),
        "expected path to be a directory: {}",
        path.display()
    );
}

fn assert_not_exists(path: impl AsRef<std::path::Path>) {
    let path = path.as_ref();
    assert!(
        !path.exists(),
        "expected path not to exist: {}",
        path.display()
    );
}

fn assert_output_success(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
        vec!["attach", "-h"],
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
fn keys_help_documents_rpc_and_pty_modes() {
    neowright()
        .args(["keys", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--pty"))
        .stdout(predicate::str::contains("Neovim RPC"))
        .stdout(predicate::str::contains("<Esc>"))
        .stdout(predicate::str::contains("<CR>"))
        .stdout(predicate::str::contains("<C-c>"))
        .stdout(predicate::str::contains("<M-x>"));
}

#[test]
fn attach_help_lists_terminal_presets() {
    let mut assertion = neowright()
        .args(["attach", "-h"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--terminal-preset"));

    for preset in TERMINAL_PRESETS {
        assertion = assertion.stdout(predicate::str::contains(preset.name));
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
fn invalid_resize_sizes_return_markdown_errors() {
    for size in ["0x10", "10x0", "-1x10", "10x", "10x10x10"] {
        neowright()
            .args(["resize", "--session", "abc", size])
            .assert()
            .failure()
            .stderr(predicate::str::contains("### Error"));
    }
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
        .stdout(predicate::str::contains("attach"))
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
fn open_headed_requires_launch_command_preset_or_detected_terminal() {
    let state = TempDir::new().expect("state dir");

    neowright()
        .args(["open", "--headed"])
        .env("XDG_STATE_HOME", state.path())
        .env("TERM_PROGRAM", "unknown")
        .env("TERMINAL", "unknown")
        .env_remove("ALACRITTY_WINDOW_ID")
        .env_remove("GHOSTTY_BIN_DIR")
        .env_remove("GHOSTTY_RESOURCES_DIR")
        .env_remove("__CFBundleIdentifier")
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("--terminal-cmd"))
        .stderr(predicate::str::contains("--terminal-preset"));
}

#[test]
fn open_terminal_cmd_requires_headed() {
    neowright()
        .args(["open", "--terminal-cmd", TERMINAL_PRESETS[0].command])
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("--headed"));
}

#[test]
fn open_terminal_preset_requires_headed() {
    neowright()
        .args(["open", "--terminal-preset", TERMINAL_PRESETS[0].name])
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("--headed"));
}

#[test]
fn attach_requires_terminal_or_print_command() {
    let state = TempDir::new().expect("state dir");

    neowright()
        .args(["attach"])
        .env("XDG_STATE_HOME", state.path())
        .env("TERM_PROGRAM", "unknown")
        .env("TERMINAL", "unknown")
        .env_remove("ALACRITTY_WINDOW_ID")
        .env_remove("GHOSTTY_BIN_DIR")
        .env_remove("GHOSTTY_RESOURCES_DIR")
        .env_remove("__CFBundleIdentifier")
        .assert()
        .failure()
        .stderr(predicate::str::contains("attach requires --terminal-cmd"))
        .stderr(predicate::str::contains("--terminal-preset"))
        .stderr(predicate::str::contains("--print-command"));
}

#[test]
fn attach_print_command_resolves_session_socket_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup::new(state.path());

    neowright()
        .args(["open", "--name", "main", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["attach", "--name", "main", "--print-command"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Attach Command"))
        .stdout(predicate::str::contains("Session Name: `main`"))
        .stdout(predicate::str::contains("nvim --server"))
        .stdout(predicate::str::contains("--remote-ui"));
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
    require_nvim();

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
    require_nvim();

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

    let output = neowright()
        .args(["snapshot", "--name", "main"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"))
        .stdout(predicate::str::contains("Artifact:"))
        .stdout(predicate::str::contains("### Contents"))
        .stdout(predicate::str::contains("hello"))
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("snapshot stdout");
    assert_eq!(stdout.matches(".neowright/snapshots/").count(), 1);

    let snapshot_dir = project.path().join(".neowright/snapshots");
    let snapshots = snapshot_files(&snapshot_dir);
    assert_eq!(snapshots.len(), 1);
    let filename = snapshots[0]
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("snapshot filename");
    assert_starts_with(filename, "snapshot-");
    assert_ends_with(filename, ".txt");
    assert_not_exists(snapshot_dir.join("snapshot-latest.txt"));

    let contents = std::fs::read_to_string(&snapshots[0]).expect("snapshot contents");
    assert_contains(&contents, "hello");
    assert_not_contains(&contents, "\u{1b}");
    assert_snapshot_dimensions(&contents, 40, 10);

    neowright()
        .args(["snapshot", "--name", "main"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Contents"))
        .stdout(predicate::str::contains("hello"));

    assert_eq!(snapshot_files(&snapshot_dir).len(), 2);
}

#[test]
fn snapshot_succeeds_while_nvim_is_blocked_at_hit_enter_prompt() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args([
            "open", "--name", "main", "--size", "60x12", "--", "-u", "NONE",
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["keys", "--name", "main", ":echoerr 'snapshot blocked'<CR>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    wait_until(
        std::time::Duration::from_secs(5),
        "snapshot blocked hit-enter prompt to appear",
        || {
            let output = run_neowright_with_timeout(
                &["snapshot", "--name", "main"],
                state.path(),
                std::time::Duration::from_secs(2),
            );
            String::from_utf8_lossy(&output.stdout).contains("snapshot blocked")
        },
    );

    let output = run_neowright_with_timeout(
        &["snapshot", "--name", "main"],
        state.path(),
        std::time::Duration::from_secs(2),
    );
    assert_output_success(
        &output,
        "snapshot should succeed while Neovim is at hit-enter prompt",
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains(&stdout, "### Snapshot");
    assert_contains(&stdout, "snapshot blocked");
}

#[test]
fn pty_keys_drive_real_session_and_are_visible_in_snapshot_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args([
            "open", "--name", "main", "--size", "60x12", "--", "-u", "NONE",
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["keys", "--name", "main", "--pty", "ihello from pty<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Sent PTY Keys"));

    wait_until(
        std::time::Duration::from_secs(5),
        "PTY text to appear in snapshot",
        || {
            let output = run_neowright_with_timeout(
                &["snapshot", "--name", "main"],
                state.path(),
                std::time::Duration::from_secs(2),
            );
            String::from_utf8_lossy(&output.stdout).contains("hello from pty")
        },
    );

    neowright()
        .args(["snapshot", "--name", "main"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Contents"))
        .stdout(predicate::str::contains("hello from pty"));

    neowright()
        .args(["eval", "--name", "main", "--raw", "return vim.fn.mode()"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^n\\n$").unwrap());
}

#[test]
fn pty_keys_translate_terminal_input_inside_neovim_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args([
            "open", "--name", "main", "--size", "60x12", "--", "-u", "NONE",
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args([
            "keys",
            "--name",
            "main",
            "--pty",
            ":let g:neowright_pty_cr = 'ok'<CR>",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args([
            "eval",
            "--name",
            "main",
            "--raw",
            "return vim.g.neowright_pty_cr",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^ok\\n$").unwrap());

    neowright()
        .args([
            "eval",
            "--name",
            "main",
            "vim.keymap.set('n', '<M-x>', function() vim.g.neowright_pty_alt = 'ok' end)",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args(["keys", "--name", "main", "--pty", "<M-x>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args([
            "wait",
            "--name",
            "main",
            "--timeout",
            "2s",
            "return vim.g.neowright_pty_alt == 'ok'",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args([
            "keys",
            "--name",
            "main",
            "--pty",
            "ggddihello<Tab>world<Esc>",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args([
            "eval",
            "--name",
            "main",
            "--raw",
            "return vim.api.nvim_get_current_line()",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^hello\\tworld\\n$").unwrap());

    neowright()
        .args(["keys", "--name", "main", "--pty", "ggddiabc<BS>d<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args([
            "eval",
            "--name",
            "main",
            "--raw",
            "return vim.api.nvim_get_current_line()",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^abd\\n$").unwrap());

    neowright()
        .args(["keys", "--name", "main", "--pty", "iunfinished<C-c>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args(["eval", "--name", "main", "--raw", "return vim.fn.mode()"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^n\\n$").unwrap());
}

#[test]
fn pty_keys_reject_unsupported_notation_without_sending_bytes_when_nvim_exists() {
    require_nvim();

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
        .args(["keys", "--name", "main", "--pty", "i<leader>sent<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains(
            "unsupported PTY key notation: <leader>",
        ));

    neowright()
        .args([
            "eval",
            "--name",
            "main",
            "--raw",
            "return vim.api.nvim_get_current_line()",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^\\n$").unwrap());
}

#[test]
fn pty_keys_dismiss_hit_enter_prompt_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args([
            "open", "--name", "main", "--size", "60x12", "--", "-u", "NONE",
        ])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["keys", "--name", "main", ":echoerr 'pty blocked'<CR>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    wait_until(
        std::time::Duration::from_secs(5),
        "PTY hit-enter prompt to appear",
        || {
            let output = run_neowright_with_timeout(
                &["snapshot", "--name", "main"],
                state.path(),
                std::time::Duration::from_secs(2),
            );
            String::from_utf8_lossy(&output.stdout).contains("pty blocked")
        },
    );

    neowright()
        .args(["keys", "--name", "main", "--pty", "<CR>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args(["eval", "--name", "main", "--raw", "return 'rpc responsive'"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^rpc responsive\\n$").unwrap());
}

#[test]
fn resize_updates_metadata_and_snapshot_dimensions_when_nvim_exists() {
    require_nvim();

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
    require_nvim();

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
fn close_does_not_hang_when_shutdown_autocmd_blocks_nvim_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--name", "slow-close", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();
    neowright()
        .args(["exec", "--name", "slow-close", "autocmd QuitPre * sleep 10"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    let output = run_neowright_with_timeout(
        &["close", "--name", "slow-close"],
        state.path(),
        std::time::Duration::from_secs(5),
    );
    assert_output_success(&output, "close should succeed");
    assert_contains(
        &String::from_utf8_lossy(&output.stdout),
        "### Closed Sessions",
    );
    assert_eq!(registry_records(state.path()), Vec::<Value>::new());
}

#[test]
fn supervisor_sigterm_terminates_child_nvim_when_nvim_exists() {
    require_nvim();

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
    let child_pid = records[0]
        .get("child_pid")
        .and_then(Value::as_u64)
        .expect("child pid");

    assert!(
        process_exists(child_pid),
        "expected child process {child_pid} to exist"
    );
    unsafe {
        libc::kill(supervisor_pid as libc::pid_t, libc::SIGTERM);
    }

    wait_until(
        std::time::Duration::from_secs(5),
        "child Neovim process to exit after supervisor SIGTERM",
        || !process_exists(child_pid),
    );
    assert!(
        !process_exists(child_pid),
        "expected child process {child_pid} not to exist"
    );
}

#[test]
fn commands_report_markdown_error_after_child_nvim_exits_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--name", "crash", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    let records = registry_records(state.path());
    let child_pid = records[0]
        .get("child_pid")
        .and_then(Value::as_u64)
        .expect("child pid");
    unsafe {
        libc::kill(child_pid as libc::pid_t, libc::SIGKILL);
    }

    wait_until(
        std::time::Duration::from_secs(5),
        "commands against killed child to fail",
        || {
            let output = run_neowright_with_timeout(
                &["snapshot", "--name", "crash"],
                state.path(),
                std::time::Duration::from_secs(2),
            );
            !output.status.success()
        },
    );

    let output = run_neowright_with_timeout(
        &["snapshot", "--name", "crash"],
        state.path(),
        std::time::Duration::from_secs(2),
    );
    assert!(
        !output.status.success(),
        "snapshot should fail after child Neovim exits\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_contains(&String::from_utf8_lossy(&output.stderr), "### Error");
}

#[test]
#[cfg(unix)]
fn sessions_work_from_paths_with_spaces_unicode_and_symlinks_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let root = TempDir::new().expect("root dir");
    let project = root.path().join("project with spaces 表");
    let link = root.path().join("linked project");
    std::fs::create_dir(&project).expect("project dir");
    std::os::unix::fs::symlink(&project, &link).expect("project symlink");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--name", "odd-path", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(&link)
        .assert()
        .success();

    neowright()
        .args(["keys", "--name", "odd-path", "ipath survives<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(root.path())
        .assert()
        .success();

    neowright()
        .args(["snapshot", "--name", "odd-path"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(root.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("path survives"));

    let records = registry_records(state.path());
    assert_eq!(records.len(), 1);
    assert_eq!(
        std::path::Path::new(records[0].get("cwd").and_then(Value::as_str).expect("cwd"))
            .canonicalize()
            .expect("registry cwd canonicalizes"),
        project.canonicalize().expect("project canonicalizes")
    );
    assert_is_dir(project.join(".neowright/snapshots"));
}

#[test]
fn open_uses_default_size_and_writes_registry_when_nvim_exists() {
    require_nvim();

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
    assert!(
        record.get("child_pid").and_then(Value::as_u64).is_some(),
        "expected registry record to include child_pid: {record:#?}"
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
    assert_starts_with(
        record
            .get("listen")
            .and_then(Value::as_str)
            .expect("listen path"),
        "/tmp/neowright-",
    );
}

#[test]
fn open_rejects_duplicate_session_name_when_nvim_exists() {
    require_nvim();

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
    require_nvim();

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
    require_nvim();

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
    require_nvim();

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
fn single_unnamed_session_can_be_targeted_implicitly_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--size", "45x9", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["keys", "iimplicit target<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    neowright()
        .args(["eval", "--raw", "return vim.api.nvim_get_current_line()"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^implicit target\n$").unwrap());

    neowright()
        .arg("snapshot")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"))
        .stdout(predicate::str::contains("Size: `45x9`"))
        .stdout(predicate::str::contains("implicit target"));

    neowright()
        .args(["close", "--force"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    assert_eq!(registry_records(state.path()), Vec::<Value>::new());
}

#[test]
fn eval_formats_basic_edge_values_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--name", "values", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();

    neowright()
        .args(["eval", "--name", "values", "return nil"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("```text\nnil\n```"));

    neowright()
        .args(["eval", "--name", "values", "return false"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("```text\nfalse\n```"));

    neowright()
        .args([
            "eval",
            "--name",
            "values",
            "return { nested = { ok = true }, list = { 1, 'two' } }",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("nested ="))
        .stdout(predicate::str::contains("list ="))
        .stdout(predicate::str::contains("ok = true"));

    neowright()
        .args(["eval", "--name", "values", "--raw", "return 'payload only'"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match("^payload only\n$").unwrap());
}

#[test]
fn rapid_snapshots_write_unique_artifacts_when_nvim_exists() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    neowright()
        .args(["open", "--name", "snapshots", "--", "-u", "NONE"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project.path())
        .assert()
        .success();
    neowright()
        .args(["keys", "--name", "snapshots", "irapid snapshot text<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    for _ in 0..3 {
        neowright()
            .args(["snapshot", "--name", "snapshots"])
            .env("XDG_STATE_HOME", state.path())
            .assert()
            .success();
    }

    let snapshots = snapshot_files(&project.path().join(".neowright/snapshots"));
    assert_eq!(snapshots.len(), 3);
    let names = snapshots
        .iter()
        .map(|path| path.file_name().expect("filename").to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), 3, "snapshot artifact names must be unique");
}

#[test]
fn eval_exec_keys_and_wait_drive_real_session_when_nvim_exists() {
    require_nvim();

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

    assert_not_exists(project.path().join(".neowright/snapshots"));

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
    require_nvim();

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
            "10s",
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
    let screen_path = session_screen_path(&records[0]);
    wait_until(
        std::time::Duration::from_secs(5),
        "resized session screen to contain typed keys",
        || {
            std::fs::read_to_string(&screen_path)
                .is_ok_and(|contents| contents.contains("hello from keys"))
        },
    );

    neowright()
        .args(["snapshot", "--name", "demo"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"))
        .stdout(predicate::str::contains("Artifact:"))
        .stdout(predicate::str::contains("### Contents"))
        .stdout(predicate::str::contains("hello from keys"));
    let snapshot_dir = project.path().join(".neowright/snapshots");
    let snapshots = snapshot_files(&snapshot_dir);
    assert_eq!(snapshots.len(), 1);
    let filename = snapshots[0]
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("snapshot filename");
    assert_starts_with(filename, "snapshot-");
    assert_ends_with(filename, ".txt");
    assert_not_exists(snapshot_dir.join("snapshot-latest.txt"));
    let contents = std::fs::read_to_string(&snapshots[0]).expect("snapshot contents");
    assert_contains(&contents, "hello from keys");
    assert_not_contains(&contents, "\u{1b}");
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
    require_nvim();

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
    assert_not_exists(project.path().join(".agents"));
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
    assert_not_exists(project.path().join(".agents"));
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
    assert_not_exists(home.path().join(".agents"));
}

#[test]
fn skills_install_overwrites_existing_skill() {
    let home = TempDir::new().expect("home dir");
    let project = TempDir::new().expect("project dir");
    let skill_path = home.path().join(".agents/skills/neowright");
    std::fs::create_dir_all(&skill_path).expect("existing skill dir");
    std::fs::write(skill_path.join("SKILL.md"), "custom skill").expect("custom skill");
    std::fs::write(skill_path.join("CUSTOM.md"), "custom file").expect("custom file");

    neowright()
        .args(["skills", "install"])
        .current_dir(project.path())
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Scope: `global`"))
        .stdout(predicate::str::contains("Overwrote existing skill files"));

    assert_neowright_skill_installed(&skill_path);
    assert_not_exists(skill_path.join("CUSTOM.md"));
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

fn session_screen_path(record: &Value) -> std::path::PathBuf {
    let artifact_dir = record
        .get("artifact_dir")
        .and_then(Value::as_str)
        .expect("artifact dir");
    let session_id = record
        .get("id")
        .and_then(Value::as_str)
        .expect("session id");

    std::path::Path::new(artifact_dir)
        .join("sessions")
        .join(session_id)
        .join("screen.txt")
}

fn assert_snapshot_dimensions(contents: &str, cols: usize, rows: usize) {
    let lines = contents.split('\n').collect::<Vec<_>>();
    assert_eq!(
        lines.len(),
        rows,
        "snapshot has wrong row count\nexpected rows: {rows}\nactual rows: {}\nsnapshot:\n{contents}",
        lines.len()
    );
    for (index, line) in lines.iter().enumerate() {
        let width = line.chars().count();
        assert_eq!(
            width, cols,
            "snapshot line {index} has wrong width\nexpected cols: {cols}\nactual cols: {width}\nline: {line:?}\nsnapshot:\n{contents}"
        );
    }
}

fn process_exists(pid: u64) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

fn assert_neowright_skill_installed(skill_path: &std::path::Path) {
    assert_is_dir(skill_path);
    let contents = std::fs::read_to_string(skill_path.join("SKILL.md")).expect("skill contents");
    assert_contains(&contents, "name: neowright");
    assert_contains(&contents, "standalone CLI harness");
    assert_contains(&contents, "neowright open");
    assert_contains(&contents, "neowright keys");
    assert_contains(&contents, "neowright wait");
    assert_contains(&contents, "neowright snapshot");
    assert_contains(&contents, "neowright close");
    assert_not_contains(&contents, "-- --clean");
    assert_not_contains(&contents, "test fixtures");
    assert_not_contains(&contents, "force-close");
}
