use std::fs::{self, File};
use std::io::Read;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use crate::cli::{OpenArgs, SessionSupervisorArgs};
use crate::session::{
    self, SessionRecord, SizeRecord, active_records, add_record, artifact_dir_for,
    ensure_artifact_dir, generate_id,
};

const READY_TIMEOUT: Duration = Duration::from_secs(10);

pub fn run(args: OpenArgs) -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|error| format!("failed to resolve cwd: {error}"))?;
    let size = args.size.unwrap_or_default();
    let records = active_records()?;

    if let Some(name) = &args.name {
        if records
            .iter()
            .any(|record| record.name.as_deref() == Some(name.as_str()))
        {
            return Err(format!("Session Name `{name}` is already active"));
        }
    }

    let id = generate_id();
    let artifact_dir = artifact_dir_for(&cwd);
    ensure_artifact_dir(&artifact_dir)?;

    let runtime_dir = artifact_dir.join("sessions").join(&id);
    fs::create_dir_all(&runtime_dir).map_err(|error| {
        format!(
            "failed to create Session runtime directory `{}`: {error}",
            runtime_dir.display()
        )
    })?;

    let listen = std::path::PathBuf::from(format!("/tmp/neowright-{id}.sock"));
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

    command
        .spawn()
        .map_err(|error| format!("failed to start Session supervisor: {error}"))?;

    wait_until_ready(&listen, &ready_file, &supervisor_log)?;

    let name = args.name.as_deref().unwrap_or("(unnamed)");
    Ok(format!(
        "Session opened.\n- Session ID: `{id}`\n- Session Name: `{name}`\n- Opened From: `{}`\n- Size: `{size}`\n- Artifact Directory: `{}`",
        cwd.display(),
        artifact_dir.display()
    ))
}

pub fn run_supervisor(args: SessionSupervisorArgs) -> Result<String, String> {
    let _ = fs::remove_file(&args.listen);
    let _ = fs::remove_file(&args.ready_file);

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: args.size.rows,
            cols: args.size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("failed to open PTY: {error}"))?;

    let mut command = CommandBuilder::new("nvim");
    command.cwd(&args.cwd);
    command.arg("--listen");
    command.arg(args.listen.as_os_str());
    for arg in &args.neovim_args {
        command.arg(arg);
    }

    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| format!("failed to start nvim: {error}"))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("failed to read PTY output: {error}"))?;
    thread::spawn(move || {
        let mut buffer = [0; 8192];
        while reader.read(&mut buffer).is_ok() {}
    });

    wait_for_socket(&args.listen, READY_TIMEOUT)?;

    add_record(SessionRecord {
        id: args.session.clone(),
        name: args.name.clone(),
        cwd: args.cwd.clone(),
        artifact_dir: args.artifact_dir.clone(),
        size: SizeRecord::from(args.size),
        supervisor_pid: std::process::id(),
        listen: args.listen.clone(),
    })?;

    fs::write(&args.ready_file, b"ready").map_err(|error| {
        format!(
            "failed to write readiness file `{}`: {error}",
            args.ready_file.display()
        )
    })?;

    let result = child.wait();
    let _ = session::remove_record(&args.session);
    let _ = fs::remove_file(&args.listen);
    result.map_err(|error| format!("failed while waiting for nvim: {error}"))?;
    Ok("Session supervisor exited.".to_string())
}

fn wait_until_ready(
    listen: &std::path::Path,
    ready_file: &std::path::Path,
    supervisor_log: &std::path::Path,
) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < READY_TIMEOUT {
        if ready_file.exists() && socket_accepts_connections(listen) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    let log = fs::read_to_string(supervisor_log).unwrap_or_default();
    if log.trim().is_empty() {
        Err("timed out waiting for Session readiness".to_string())
    } else {
        Err(format!(
            "timed out waiting for Session readiness\n\nSupervisor log:\n```\n{}\n```",
            log.trim()
        ))
    }
}

fn wait_for_socket(listen: &std::path::Path, timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if socket_accepts_connections(listen) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    Err(format!(
        "timed out waiting for Neovim control socket `{}`",
        listen.display()
    ))
}

fn socket_accepts_connections(path: &std::path::Path) -> bool {
    UnixStream::connect(path)
        .map(|stream| {
            let _ = stream.shutdown(Shutdown::Both);
        })
        .is_ok()
}
