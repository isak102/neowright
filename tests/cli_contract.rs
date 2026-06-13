use assert_cmd::Command;
use predicates::prelude::*;

fn neowright() -> Command {
    Command::cargo_bin("neowright").expect("binary exists")
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
        &["open"],
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
