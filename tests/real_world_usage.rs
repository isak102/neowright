use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;
use unicode_width::UnicodeWidthStr;

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

fn require_nvim() {
    let status = std::process::Command::new("nvim")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    assert!(
        status.is_ok_and(|status| status.success()),
        "nvim must be installed and runnable for Neowright integration tests"
    );
}

fn open_session(state: &std::path::Path, project: &std::path::Path, name: &str, args: &[&str]) {
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

fn eval_raw(state: &std::path::Path, name: &str, lua: &str) -> String {
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

fn wait_for(state: &std::path::Path, name: &str, lua: &str) {
    neowright()
        .args(["wait", "--name", name, "--timeout", "3s", lua])
        .env("XDG_STATE_HOME", state)
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"));
}

fn snapshot_inline(state: &std::path::Path, name: &str) -> String {
    let output = neowright()
        .args(["snapshot", "--name", name, "--inline"])
        .env("XDG_STATE_HOME", state)
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"))
        .get_output()
        .stdout
        .clone();

    String::from_utf8(output).expect("utf8 stdout")
}

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

fn assert_contains_any(actual: &str, expected: &[&str]) {
    assert!(
        expected.iter().any(|value| actual.contains(value)),
        "expected text to contain one of {expected:?}\nactual:\n{actual}"
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

fn wait_for_snapshot_contains(state: &std::path::Path, name: &str, expected: &str) -> String {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(3);
    let mut snapshot = snapshot_inline(state, name);

    while start.elapsed() < timeout {
        if snapshot.contains(expected) {
            return snapshot;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        snapshot = snapshot_inline(state, name);
    }

    panic!("snapshot for {name:?} did not contain {expected:?}\n{snapshot}");
}

fn assert_snapshot_dimensions(contents: &str, cols: usize, rows: usize) {
    let body = contents
        .split("```text\n")
        .nth(1)
        .and_then(|contents| contents.split("\n```").next())
        .expect("inline snapshot text block");
    let lines = body.split('\n').collect::<Vec<_>>();
    assert_eq!(
        lines.len(),
        rows,
        "snapshot has wrong row count\nexpected rows: {rows}\nactual rows: {}\nsnapshot:\n{contents}",
        lines.len()
    );
    for (index, line) in lines.iter().enumerate() {
        let width = UnicodeWidthStr::width(*line);
        assert_eq!(
            width, cols,
            "snapshot line {index} has wrong width\nexpected cols: {cols}\nactual cols: {width}\nline: {line:?}\nsnapshot:\n{contents}"
        );
    }
}

#[test]
fn agent_can_edit_save_and_inspect_a_real_project_file() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let file = project.path().join("notes.txt");
    std::fs::write(&file, "alpha\n").expect("seed file");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(
        state.path(),
        project.path(),
        "edit",
        &["-u", "NONE", "notes.txt"],
    );
    wait_for(
        state.path(),
        "edit",
        "return vim.api.nvim_buf_get_name(0):match('notes%.txt$') ~= nil",
    );

    neowright()
        .args(["keys", "--name", "edit", "Goagent-added line<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Sent Keys"));

    neowright()
        .args(["exec", "--name", "edit", "write"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Ran Command"));

    assert_eq!(
        std::fs::read_to_string(&file).expect("saved file"),
        "alpha\nagent-added line\n"
    );
    assert_contains(&snapshot_inline(state.path(), "edit"), "agent-added line");
}

#[test]
fn agent_can_inspect_a_deterministic_floating_window() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "float", &["-u", "NONE"]);
    neowright()
        .args([
            "eval",
            "--name",
            "float",
            "local b=vim.api.nvim_create_buf(false,true); vim.api.nvim_buf_set_lines(b,0,-1,false,{'NEOWRIGHT FLOAT','stable content'}); local w=vim.api.nvim_open_win(b,false,{relative='editor',row=2,col=6,width=24,height=3,style='minimal',border='single'}); vim.g.neowright_float_win=w",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    wait_for(
        state.path(),
        "float",
        "return vim.g.neowright_float_win ~= nil and vim.api.nvim_win_is_valid(vim.g.neowright_float_win)",
    );

    let config = eval_raw(
        state.path(),
        "float",
        "local c=vim.api.nvim_win_get_config(vim.g.neowright_float_win); return {relative=c.relative,width=c.width,height=c.height}",
    );
    assert_eq!(
        config,
        "{\"height\":3,\"relative\":\"editor\",\"width\":24}\n"
    );
    let snapshot = snapshot_inline(state.path(), "float");
    assert_contains(&snapshot, "NEOWRIGHT FLOAT");
    assert_contains(&snapshot, "stable content");
}

#[test]
fn agent_can_debug_diagnostics_without_lsp_or_plugins() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    std::fs::write(project.path().join("broken.lua"), "local answer =\n").expect("lua file");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(
        state.path(),
        project.path(),
        "diag",
        &["-u", "NONE", "broken.lua"],
    );
    neowright()
        .args([
            "eval",
            "--name",
            "diag",
            "vim.diagnostic.config({virtual_text=true,signs=false,underline=false}); local ns=vim.api.nvim_create_namespace('neowright-test'); vim.diagnostic.set(ns,0,{{lnum=0,col=13,severity=vim.diagnostic.severity.ERROR,message='expected expression after equals'}})",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    wait_for(state.path(), "diag", "return #vim.diagnostic.get(0) == 1");

    let diagnostic = eval_raw(
        state.path(),
        "diag",
        "local d=vim.diagnostic.get(0)[1]; return {message=d.message,severity=d.severity,lnum=d.lnum,col=d.col}",
    );
    assert_eq!(
        diagnostic,
        "{\"col\":13,\"lnum\":0,\"message\":\"expected expression after equals\",\"severity\":1}\n"
    );
    assert_contains(
        &snapshot_inline(state.path(), "diag"),
        "expected expression after equals",
    );
}

#[test]
fn global_registry_commands_can_run_from_another_directory_but_artifacts_stay_with_project() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project_a = TempDir::new().expect("project a");
    let project_b = TempDir::new().expect("project b");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project_a.path(), "remote", &["-u", "NONE"]);

    neowright()
        .args(["keys", "--name", "remote", "iremote project text<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project_b.path())
        .assert()
        .success();

    neowright()
        .args(["snapshot", "--name", "remote"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project_b.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"));

    assert_is_dir(project_a.path().join(".neowright/snapshots"));
    assert_not_exists(project_b.path().join(".neowright/snapshots"));

    let records = registry_records(state.path());
    assert_eq!(records.len(), 1);
    let recorded_cwd = records[0]
        .get("cwd")
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        .expect("recorded cwd")
        .canonicalize()
        .expect("canonical recorded cwd");
    assert_eq!(
        recorded_cwd,
        project_a
            .path()
            .canonicalize()
            .expect("canonical project path")
    );
}

#[test]
fn failed_exploratory_commands_do_not_poison_the_session() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "recover", &["-u", "NONE"]);

    neowright()
        .args(["eval", "--name", "recover", "error('boom')"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"))
        .stderr(predicate::str::contains("boom"));

    assert_eq!(
        eval_raw(state.path(), "recover", "return 'still alive'"),
        "still alive\n"
    );

    neowright()
        .args(["exec", "--name", "recover", "NoSuchCommand"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"));

    neowright()
        .args(["keys", "--name", "recover", "irecovered after failure<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "recover",
        "return vim.api.nvim_get_current_line() == 'recovered after failure'",
    );
}

#[test]
fn temp_init_lua_config_mapping_autocmd_and_command_work_end_to_end() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let init = project.path().join("init.lua");
    std::fs::write(
        &init,
        r#"
vim.g.neowright_init_loaded = true
vim.o.number = true
vim.keymap.set('n', '<leader>x', function() vim.g.neowright_mapping_seen = 'yes' end)
vim.api.nvim_create_user_command('NeowrightMark', function() vim.g.neowright_command_seen = 'ok' end, {})
"#,
    )
    .expect("init lua");
    let file = project.path().join("configured.txt");
    std::fs::write(&file, "configured\n").expect("configured file");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(
        state.path(),
        project.path(),
        "config",
        &[
            "-u",
            "NONE",
            "--cmd",
            &format!("luafile {}", init.display()),
            "configured.txt",
        ],
    );
    wait_for(
        state.path(),
        "config",
        "return vim.g.neowright_init_loaded == true and vim.api.nvim_buf_get_name(0):match('configured%.txt$') ~= nil",
    );

    neowright()
        .args(["keys", "--name", "config", "<leader>x"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args(["exec", "--name", "config", "NeowrightMark"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    assert_eq!(
        eval_raw(
            state.path(),
            "config",
            "return {mapping=vim.g.neowright_mapping_seen, command=vim.g.neowright_command_seen, number=vim.o.number}"
        ),
        "{\"command\":\"ok\",\"mapping\":\"yes\",\"number\":true}\n"
    );
}

#[test]
fn vnew_blank_buffer_accepts_text_from_neowright_keys() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "vnew", &["-u", "NONE"]);
    neowright()
        .args(["exec", "--name", "vnew", ":vnew"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "vnew",
        "return #vim.api.nvim_tabpage_list_wins(0) == 2 and vim.fn.expand('%:t') == ''",
    );

    neowright()
        .args(["keys", "--name", "vnew", "i"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "vnew",
        "return vim.api.nvim_get_mode().mode == 'i'",
    );

    neowright()
        .args(["keys", "--name", "vnew", "helloworld"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    wait_for(
        state.path(),
        "vnew",
        "return #vim.api.nvim_tabpage_list_wins(0) == 2 and vim.api.nvim_get_current_line() == 'helloworld'",
    );

    assert_eq!(
        eval_raw(
            state.path(),
            "vnew",
            "return {wins=#vim.api.nvim_tabpage_list_wins(0), line=vim.api.nvim_get_current_line(), file=vim.fn.expand('%:t')}"
        ),
        "{\"file\":\"\",\"line\":\"helloworld\",\"wins\":2}\n"
    );
}

#[test]
fn separated_neowright_keys_preserve_insert_mode_between_calls() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "split-keys", &["-u", "NONE"]);
    neowright()
        .args(["keys", "--name", "split-keys", "i"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "split-keys",
        "return vim.api.nvim_get_mode().mode == 'i'",
    );

    neowright()
        .args(["keys", "--name", "split-keys", "hello"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "split-keys",
        "return vim.api.nvim_get_current_line() == 'hello' and vim.api.nvim_get_mode().mode == 'i'",
    );

    neowright()
        .args(["keys", "--name", "split-keys", "world"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "split-keys",
        "return vim.api.nvim_get_current_line() == 'helloworld' and vim.api.nvim_get_mode().mode == 'i'",
    );
}

#[test]
fn separated_neowright_escape_and_normal_keys_take_effect() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "normal-keys", &["-u", "NONE"]);
    neowright()
        .args(["keys", "--name", "normal-keys", "i"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "normal-keys",
        "return vim.api.nvim_get_mode().mode == 'i'",
    );

    neowright()
        .args(["keys", "--name", "normal-keys", "abc"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "normal-keys",
        "return vim.api.nvim_get_current_line() == 'abc'",
    );

    neowright()
        .args(["keys", "--name", "normal-keys", "<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "normal-keys",
        "return vim.api.nvim_get_mode().mode == 'n'",
    );

    neowright()
        .args(["keys", "--name", "normal-keys", "0x"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "normal-keys",
        "return vim.api.nvim_get_current_line() == 'bc' and vim.api.nvim_get_mode().mode == 'n'",
    );
}

#[test]
fn tabs_splits_and_buffers_can_be_navigated_and_inspected() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    std::fs::write(project.path().join("one.txt"), "one\n").expect("one file");
    std::fs::write(project.path().join("two.txt"), "two\n").expect("two file");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(
        state.path(),
        project.path(),
        "layout",
        &["-u", "NONE", "one.txt"],
    );
    neowright()
        .args(["exec", "--name", "layout", "tabnew two.txt"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args(["keys", "--name", "layout", "<C-w>v"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "layout",
        "return vim.fn.tabpagenr('$') == 2 and #vim.api.nvim_tabpage_list_wins(0) == 2",
    );

    assert_eq!(
        eval_raw(
            state.path(),
            "layout",
            "return {tabs=vim.fn.tabpagenr('$'), wins=#vim.api.nvim_tabpage_list_wins(0), file=vim.fn.expand('%:t')}"
        ),
        "{\"file\":\"two.txt\",\"tabs\":2,\"wins\":2}\n"
    );
}

#[test]
fn quickfix_search_workflow_is_visible_and_inspectable() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    std::fs::write(project.path().join("first.txt"), "needle one\n").expect("first file");
    std::fs::write(project.path().join("second.txt"), "needle two\n").expect("second file");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(
        state.path(),
        project.path(),
        "quickfix",
        &["-u", "NONE", "first.txt"],
    );
    neowright()
        .args([
            "exec",
            "--name",
            "quickfix",
            "vimgrep /needle/ *.txt | copen",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "quickfix",
        "return #vim.fn.getqflist() == 2 and vim.fn.getwininfo(vim.fn.getqflist({winid=1}).winid)[1] ~= nil",
    );

    assert_eq!(
        eval_raw(state.path(), "quickfix", "return #vim.fn.getqflist()"),
        "2\n"
    );
    let snapshot = snapshot_inline(state.path(), "quickfix");
    assert_contains_any(&snapshot, &["first.txt", "second.txt"]);
}

#[test]
fn terminal_buffer_output_can_be_captured() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "terminal", &["-u", "NONE"]);
    neowright()
        .args([
            "exec",
            "--name",
            "terminal",
            "terminal printf 'neowright-terminal-ok\\n'",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "terminal",
        "return vim.bo.buftype == 'terminal' and table.concat(vim.api.nvim_buf_get_lines(0,0,-1,false),'\\n'):find('neowright%-terminal%-ok') ~= nil",
    );

    assert_contains(
        &snapshot_inline(state.path(), "terminal"),
        "neowright-terminal-ok",
    );
}

#[test]
fn markdown_sections_remain_stable_across_a_full_agent_workflow() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "markdown", &["-u", "NONE"]);
    neowright()
        .args(["keys", "--name", "markdown", "imarkdown workflow<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Sent Keys"));
    neowright()
        .args(["exec", "--name", "markdown", "let g:markdown_exec = 'ok'"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Ran Command"));
    neowright()
        .args(["eval", "--name", "markdown", "return vim.g.markdown_exec"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"))
        .stdout(predicate::str::contains("### Ran Lua"));
    neowright()
        .args([
            "wait",
            "--name",
            "markdown",
            "return vim.g.markdown_exec == 'ok'",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Result"));
    neowright()
        .args(["resize", "--name", "markdown", "60x14"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Resized Session"));
    neowright()
        .args(["snapshot", "--name", "markdown"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Snapshot"));
    neowright()
        .args(["close", "--name", "markdown", "--force"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Closed Sessions"));
}

#[test]
fn unicode_wide_characters_survive_the_real_snapshot_path() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "unicode", &["-u", "NONE"]);
    neowright()
        .args(["keys", "--name", "unicode", "iascii 表 text<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    let snapshot = wait_for_snapshot_contains(state.path(), "unicode", "ascii 表 text");
    assert_not_contains(&snapshot, "\u{1b}");
    assert_snapshot_dimensions(&snapshot, 80, 20);
}

#[test]
fn session_can_be_targeted_by_id_from_another_directory() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project_a = TempDir::new().expect("project a");
    let project_b = TempDir::new().expect("project b");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project_a.path(), "by-id", &["-u", "NONE"]);
    let records = registry_records(state.path());
    let id = records[0]
        .get("id")
        .and_then(Value::as_str)
        .expect("session id")
        .to_owned();

    neowright()
        .args(["keys", "--session", &id, "itargeted by id<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project_b.path())
        .assert()
        .success();
    assert_eq!(
        eval_raw(
            state.path(),
            "by-id",
            "return vim.api.nvim_get_current_line()"
        ),
        "targeted by id\n"
    );

    neowright()
        .args(["snapshot", "--session", &id])
        .env("XDG_STATE_HOME", state.path())
        .current_dir(project_b.path())
        .assert()
        .success();
    assert_is_dir(project_a.path().join(".neowright/snapshots"));
    assert_not_exists(project_b.path().join(".neowright/snapshots"));
}

#[test]
fn pty_keys_drive_insert_mode_text_like_a_terminal_user() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "pty-text", &["-u", "NONE"]);
    neowright()
        .args([
            "keys",
            "--name",
            "pty-text",
            "--pty",
            "ihello from pty<Esc>",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("### Sent PTY Keys"));
    wait_for(
        state.path(),
        "pty-text",
        "return vim.api.nvim_get_current_line() == 'hello from pty'",
    );
}

#[test]
fn pty_keys_dismiss_hit_enter_prompt_and_restore_rpc() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(
        state.path(),
        project.path(),
        "pty-hit-enter",
        &["-u", "NONE"],
    );
    neowright()
        .args([
            "keys",
            "--name",
            "pty-hit-enter",
            ":echoerr 'blocked by hit-enter'<CR>",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for_snapshot_contains(state.path(), "pty-hit-enter", "blocked by hit-enter");
    neowright()
        .args(["keys", "--name", "pty-hit-enter", "--pty", "<CR>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    assert_eq!(
        eval_raw(state.path(), "pty-hit-enter", "return 'rpc restored'"),
        "rpc restored\n"
    );
}

#[test]
fn pty_keys_translate_backspace_ctrl_c_commandline_and_alt_notation() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(
        state.path(),
        project.path(),
        "pty-notation",
        &["-u", "NONE"],
    );
    neowright()
        .args([
            "eval",
            "--name",
            "pty-notation",
            "vim.keymap.set('n', '<M-x>', function() vim.g.pty_alt_seen = true end)",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args(["keys", "--name", "pty-notation", "--pty", "iabc<BS>d<Esc>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args([
            "keys",
            "--name",
            "pty-notation",
            "--pty",
            "iunfinished<C-c>",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args([
            "keys",
            "--name",
            "pty-notation",
            "--pty",
            ":let g:pty_cmdline = 'ok'<CR>",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args(["keys", "--name", "pty-notation", "--pty", "<M-x>"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();

    wait_for(
        state.path(),
        "pty-notation",
        "return vim.fn.mode() == 'n' and vim.g.pty_cmdline == 'ok' and vim.g.pty_alt_seen == true",
    );
    assert_eq!(
        eval_raw(
            state.path(),
            "pty-notation",
            "return vim.api.nvim_get_current_line()"
        ),
        "abd\n"
    );
}

#[test]
fn pty_keys_reject_unsupported_notation_without_buffer_mutation() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "pty-reject", &["-u", "NONE"]);
    neowright()
        .args(["keys", "--name", "pty-reject", "--pty", "<leader>x"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("### Error"));

    assert_eq!(
        eval_raw(
            state.path(),
            "pty-reject",
            "return vim.api.nvim_get_current_line()"
        ),
        "\n"
    );
}

#[test]
fn pty_keys_still_work_after_resize() {
    require_nvim();

    let state = TempDir::new().expect("state dir");
    let project = TempDir::new().expect("project dir");
    let _cleanup = SupervisorCleanup {
        state_home: state.path(),
    };

    open_session(state.path(), project.path(), "pty-resize", &["-u", "NONE"]);
    neowright()
        .args(["resize", "--name", "pty-resize", "50x12"])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    neowright()
        .args([
            "keys",
            "--name",
            "pty-resize",
            "--pty",
            "ipty after resize<Esc>",
        ])
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success();
    wait_for(
        state.path(),
        "pty-resize",
        "return vim.api.nvim_get_current_line() == 'pty after resize'",
    );
    let snapshot = snapshot_inline(state.path(), "pty-resize");
    assert_contains(&snapshot, "pty after resize");
    assert_snapshot_dimensions(&snapshot, 50, 12);
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
