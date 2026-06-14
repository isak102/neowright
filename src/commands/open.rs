use std::fs::{self, File};
use std::process::{Command, Stdio};

use crate::cli::{OpenArgs, SessionSupervisorArgs};
use crate::output;
use crate::screen;
use crate::session::{
    SessionRecord, SessionRegistry, SizeRecord, artifact_dir_for, ensure_artifact_dir, generate_id,
};
use crate::session_supervisor;

pub fn run(args: OpenArgs) -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|error| format!("failed to resolve cwd: {error}"))?;
    let size = args.size.unwrap_or_default();
    let registry = SessionRegistry::load_global()?;
    let records = registry.active_sessions()?;

    if let Some(name) = &args.name
        && records
            .iter()
            .any(|record| record.name.as_deref() == Some(name.as_str()))
    {
        return Err(format!("Session Name `{name}` is already active"));
    }

    let id = generate_id();
    let artifact_dir = artifact_dir_for(&cwd);
    ensure_artifact_dir(&artifact_dir)?;

    let runtime_dir = screen::runtime_dir(&artifact_dir, &id);
    fs::create_dir_all(&runtime_dir).map_err(|error| {
        format!(
            "failed to create Session runtime directory `{}`: {error}",
            runtime_dir.display()
        )
    })?;

    let listen = screen::nvim_listen_path(&id);
    let ready_file = runtime_dir.join("ready");
    let supervisor_log = runtime_dir.join("supervisor.log");
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("failed to resolve neowright executable: {error}"))?;

    let mut command = Command::new(current_exe);
    let log = File::create(&supervisor_log).map_err(|error| {
        format!(
            "failed to create supervisor log `{}`: {error}",
            supervisor_log.display()
        )
    })?;

    command
        .arg("__session-supervisor")
        .arg("--session")
        .arg(&id)
        .arg("--cwd")
        .arg(&cwd)
        .arg("--size")
        .arg(size.to_string())
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--listen")
        .arg(&listen)
        .arg("--ready-file")
        .arg(&ready_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);

    if let Some(name) = &args.name {
        command.arg("--name").arg(name);
    }

    if !args.neovim_args.is_empty() {
        command.arg("--").args(&args.neovim_args);
    }

    let mut supervisor = command
        .spawn()
        .map_err(|error| format!("failed to start Session supervisor: {error}"))?;

    if let Err(error) = session_supervisor::wait_until_ready(&listen, &ready_file, &supervisor_log)
    {
        let _ = supervisor.kill();
        let _ = supervisor.wait();
        return Err(error);
    }

    Ok(output::opened_session(&SessionRecord {
        id,
        name: args.name,
        cwd,
        artifact_dir,
        size: SizeRecord::from(size),
        supervisor_pid: supervisor.id(),
        child_pid: None,
        listen,
    }))
}

pub fn run_supervisor(args: SessionSupervisorArgs) -> Result<String, String> {
    session_supervisor::run(args)
}
