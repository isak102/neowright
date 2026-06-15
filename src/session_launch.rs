use std::fs::{self, File};
use std::process::{Command, Stdio};

use crate::cli::{SessionSupervisorArgs, Size};
use crate::session::{
    SessionRecord, SessionRegistry, SizeRecord, artifact_dir_for, ensure_artifact_dir, generate_id,
};
use crate::session_io::SessionIo;
use crate::session_supervisor;

pub(crate) struct OpenSessionRequest {
    pub(crate) name: Option<String>,
    pub(crate) size: Size,
    pub(crate) neovim_args: Vec<String>,
}

pub(crate) fn open_session(request: OpenSessionRequest) -> Result<SessionRecord, String> {
    let cwd = std::env::current_dir().map_err(|error| format!("failed to resolve cwd: {error}"))?;
    SessionRegistry::load_global()?.ensure_name_available(request.name.as_deref())?;

    let id = generate_id();
    let artifact_dir = artifact_dir_for(&cwd);
    ensure_artifact_dir(&artifact_dir)?;

    let io = SessionIo::new(id.clone(), artifact_dir.clone());
    let runtime_dir = io.runtime_dir();
    fs::create_dir_all(&runtime_dir).map_err(|error| {
        format!(
            "failed to create Session runtime directory `{}`: {error}",
            runtime_dir.display()
        )
    })?;

    let listen = io.nvim_listen_path();
    let ready_file = io.ready_file();
    let supervisor_log = io.supervisor_log();
    let supervisor_args = SessionSupervisorArgs {
        session: id.clone(),
        name: request.name.clone(),
        cwd: cwd.clone(),
        size: request.size,
        artifact_dir: artifact_dir.clone(),
        listen: listen.clone(),
        ready_file: ready_file.clone(),
        neovim_args: request.neovim_args,
    };
    let mut supervisor = spawn_supervisor(&supervisor_args, &supervisor_log)?;

    if let Err(error) = session_supervisor::wait_until_ready(&listen, &ready_file, &supervisor_log)
    {
        let _ = supervisor.kill();
        let _ = supervisor.wait();
        return Err(error);
    }

    Ok(SessionRecord {
        id,
        name: request.name,
        cwd,
        artifact_dir,
        size: SizeRecord::from(request.size),
        supervisor_pid: supervisor.id(),
        child_pid: None,
        listen,
    })
}

fn spawn_supervisor(
    args: &SessionSupervisorArgs,
    supervisor_log: &std::path::Path,
) -> Result<std::process::Child, String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("failed to resolve neowright executable: {error}"))?;
    let log = File::create(supervisor_log).map_err(|error| {
        format!(
            "failed to create supervisor log `{}`: {error}",
            supervisor_log.display()
        )
    })?;

    let mut command = Command::new(current_exe);
    command
        .arg("__session-supervisor")
        .arg("--session")
        .arg(&args.session)
        .arg("--cwd")
        .arg(&args.cwd)
        .arg("--size")
        .arg(args.size.to_string())
        .arg("--artifact-dir")
        .arg(&args.artifact_dir)
        .arg("--listen")
        .arg(&args.listen)
        .arg("--ready-file")
        .arg(&args.ready_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);

    if let Some(name) = &args.name {
        command.arg("--name").arg(name);
    }

    if !args.neovim_args.is_empty() {
        command.arg("--").args(&args.neovim_args);
    }

    command
        .spawn()
        .map_err(|error| format!("failed to start Session supervisor: {error}"))
}
