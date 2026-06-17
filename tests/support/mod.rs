#![allow(dead_code)]

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

pub struct SupervisorCleanup<'a> {
    pub state_home: &'a Path,
}

impl<'a> SupervisorCleanup<'a> {
    pub fn new(state_home: &'a Path) -> Self {
        Self { state_home }
    }
}

impl Drop for SupervisorCleanup<'_> {
    fn drop(&mut self) {
        cleanup_supervisors(self.state_home);
    }
}

pub fn neowright() -> Command {
    Command::cargo_bin("neowright").expect("binary exists")
}

pub fn require_nvim() {
    let status = std::process::Command::new("nvim")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    assert!(
        status.is_ok_and(|status| status.success()),
        "nvim must be installed and runnable for Neowright integration tests"
    );
}

pub fn open_session(state: &Path, project: &Path, name: &str, args: &[&str]) {
    let mut command_args = vec!["open", "--name", name, "--size", "80x20", "--"];
    command_args.extend_from_slice(args);

    neowright()
        .args(command_args)
        .env("XDG_STATE_HOME", state)
        .current_dir(project)
        .assert()
        .success()
        .stdout(predicate::str::contains("### Status"))
        .stdout(predicate::str::contains("Session opened."));
}

pub fn eval_raw(state: &Path, name: &str, lua: &str) -> String {
    let output = neowright()
        .args(["eval", "--name", name, "--raw", lua])
        .env("XDG_STATE_HOME", state)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    String::from_utf8(output).expect("utf8 stdout")
}

pub fn wait_for(state: &Path, name: &str, lua: &str) {
    neowright()
        .args(["wait", "--name", name, "--timeout", "10s", lua])
        .env("XDG_STATE_HOME", state)
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"));
}

pub fn snapshot_output(state: &Path, name: &str) -> String {
    let output = neowright()
        .args(["snapshot", "--name", name])
        .env("XDG_STATE_HOME", state)
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"))
        .get_output()
        .stdout
        .clone();

    String::from_utf8(output).expect("utf8 stdout")
}

pub fn registry_records(state_home: &Path) -> Vec<Value> {
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

pub fn run_neowright_with_timeout(args: &[&str], state_home: &Path, timeout: Duration) -> Output {
    let binary = std::env::var_os("CARGO_BIN_EXE_neowright")
        .map(PathBuf::from)
        .unwrap_or_else(|| assert_cmd::cargo::cargo_bin("neowright"));
    let mut child = std::process::Command::new(binary)
        .args(args)
        .env("XDG_STATE_HOME", state_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn neowright");

    let start = Instant::now();
    while start.elapsed() < timeout {
        if child.try_wait().expect("poll neowright").is_some() {
            return child.wait_with_output().expect("neowright output");
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("timed out neowright output");
    panic!(
        "neowright {:?} timed out after {:?}\nstdout:\n{}\nstderr:\n{}",
        args,
        timeout,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn wait_until(timeout: Duration, description: &str, mut condition: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    panic!("timed out after {timeout:?} waiting for {description}");
}

fn cleanup_supervisors(state_home: &Path) {
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

    std::thread::sleep(Duration::from_millis(200));

    for record in records {
        if let Some(pid) = record.get("child_pid").and_then(Value::as_u64) {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }
}
