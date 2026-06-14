use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use crate::cli::{OpenArgs, SessionSupervisorArgs};
use crate::nvim::{NvimClient, NvimValue};
use crate::screen;
use crate::session::{
    SessionRecord, SessionRegistry, SizeRecord, artifact_dir_for, ensure_artifact_dir, generate_id,
};

const READY_TIMEOUT: Duration = Duration::from_secs(10);
static SUPERVISOR_SHUTDOWN: AtomicBool = AtomicBool::new(false);

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

    if let Err(error) = wait_until_ready(&listen, &ready_file, &supervisor_log) {
        let _ = supervisor.kill();
        let _ = supervisor.wait();
        return Err(error);
    }

    let name = args.name.as_deref().unwrap_or("(unnamed)");
    Ok(format!(
        "Session opened.\n- Session ID: `{id}`\n- Session Name: `{name}`\n- Opened From: `{}`\n- Size: `{size}`\n- Artifact Directory: `{}`",
        cwd.display(),
        artifact_dir.display()
    ))
}

pub fn run_supervisor(args: SessionSupervisorArgs) -> Result<String, String> {
    install_supervisor_signal_handlers();
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

    let size = SizeRecord::from(args.size);
    let screen_path = screen::runtime_dir(&args.artifact_dir, &args.session).join("screen.txt");
    let desired_size_path = screen::desired_size_path(&args.artifact_dir, &args.session);
    let pty_input_path = screen::pty_input_path(&args.artifact_dir, &args.session);
    let _ = fs::remove_file(&pty_input_path);
    let parser = Arc::new(Mutex::new(screen::parser_for(size)));
    persist_current_screen(&parser, &screen_path, size)?;

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("failed to read PTY output: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("failed to write PTY input: {error}"))?;
    let writer = Arc::new(Mutex::new(writer));
    spawn_pty_input_listener(&pty_input_path, Arc::clone(&writer))?;
    let reader_parser = Arc::clone(&parser);
    let reader_screen_path = screen_path.clone();
    let reader_writer = Arc::clone(&writer);
    thread::spawn(move || {
        let mut buffer = [0; 8192];
        while let Ok(bytes_read) = reader.read(&mut buffer) {
            if bytes_read == 0 {
                break;
            }
            if buffer[..bytes_read]
                .windows(4)
                .any(|window| window == b"\x1b[5n")
                && let Ok(mut writer) = reader_writer.lock()
            {
                let _ = writer.write_all(b"\x1b[0n");
                let _ = writer.flush();
            }
            let Ok(mut parser) = reader_parser.lock() else {
                break;
            };
            parser.process(&buffer[..bytes_read]);
            let size = parser_size(&parser);
            let contents = screen::snapshot_text(&parser, size);
            drop(parser);
            let _ = screen::write_latest(&reader_screen_path, &contents);
        }
    });

    if let Err(error) = wait_for_socket(&args.listen, READY_TIMEOUT)
        .and_then(|_| screen::restrict_socket_permissions(&args.listen))
        .and_then(|_| wait_for_rpc(&args.listen, READY_TIMEOUT))
    {
        if let Some(child_pid) = child.process_id() {
            unsafe {
                libc::kill(child_pid as libc::pid_t, libc::SIGKILL);
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_file(&args.listen);
        let _ = fs::remove_file(&pty_input_path);
        return Err(error);
    }

    SessionRegistry::load_global()?.insert(SessionRecord {
        id: args.session.clone(),
        name: args.name.clone(),
        cwd: args.cwd.clone(),
        artifact_dir: args.artifact_dir.clone(),
        size,
        supervisor_pid: std::process::id(),
        child_pid: child.process_id(),
        listen: args.listen.clone(),
    })?;

    fs::write(&args.ready_file, b"ready").map_err(|error| {
        format!(
            "failed to write readiness file `{}`: {error}",
            args.ready_file.display()
        )
    })?;

    loop {
        if SUPERVISOR_SHUTDOWN.load(Ordering::Relaxed) {
            if let Some(child_pid) = child.process_id() {
                unsafe {
                    libc::kill(child_pid as libc::pid_t, libc::SIGKILL);
                }
            }
            let _ = child.kill();
            let _ = child.wait();
            break;
        }

        if child
            .try_wait()
            .map_err(|error| format!("failed while polling nvim: {error}"))?
            .is_some()
        {
            break;
        }

        if let Some(desired_size) = read_desired_size(&desired_size_path)?
            && desired_size != current_parser_size(&parser)?
        {
            pair.master
                .resize(PtySize {
                    rows: desired_size.rows,
                    cols: desired_size.cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|error| format!("failed to resize PTY: {error}"))?;
            let mut parser = parser
                .lock()
                .map_err(|_| "failed to lock Screen parser".to_string())?;
            parser
                .screen_mut()
                .set_size(desired_size.rows, desired_size.cols);
        }

        thread::sleep(Duration::from_millis(50));
    }

    if let Ok(registry) = SessionRegistry::load_global() {
        let _ = registry.remove(&args.session);
    }
    let _ = fs::remove_file(&args.listen);
    let _ = fs::remove_file(&pty_input_path);
    Ok("Session supervisor exited.".to_string())
}

fn spawn_pty_input_listener(
    path: &std::path::Path,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
) -> Result<(), String> {
    let listener = UnixListener::bind(path).map_err(|error| {
        format!(
            "failed to create Session PTY input socket `{}`: {error}",
            path.display()
        )
    })?;
    screen::restrict_socket_permissions(path)?;

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                break;
            };
            let mut bytes = Vec::new();
            if stream.read_to_end(&mut bytes).is_err() {
                continue;
            }
            let Ok(mut writer) = writer.lock() else {
                break;
            };
            let _ = writer.write_all(&bytes);
            let _ = writer.flush();
        }
    });

    Ok(())
}

fn persist_current_screen(
    parser: &Arc<Mutex<vt100::Parser>>,
    path: &std::path::Path,
    size: SizeRecord,
) -> Result<(), String> {
    let parser = parser
        .lock()
        .map_err(|_| "failed to lock Screen parser".to_string())?;
    screen::write_latest(path, &screen::snapshot_text(&parser, size))
}

fn parser_size(parser: &vt100::Parser) -> SizeRecord {
    let (rows, cols) = parser.screen().size();
    SizeRecord { cols, rows }
}

fn current_parser_size(parser: &Arc<Mutex<vt100::Parser>>) -> Result<SizeRecord, String> {
    let parser = parser
        .lock()
        .map_err(|_| "failed to lock Screen parser".to_string())?;
    Ok(parser_size(&parser))
}

fn read_desired_size(path: &std::path::Path) -> Result<Option<SizeRecord>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read desired Session size `{}`: {error}",
            path.display()
        )
    })?;
    serde_json::from_str(&contents).map(Some).map_err(|error| {
        format!(
            "failed to parse desired Session size `{}`: {error}",
            path.display()
        )
    })
}

fn install_supervisor_signal_handlers() {
    SUPERVISOR_SHUTDOWN.store(false, Ordering::Relaxed);
    unsafe {
        libc::signal(
            libc::SIGTERM,
            handle_supervisor_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGINT,
            handle_supervisor_signal as *const () as libc::sighandler_t,
        );
    }
}

extern "C" fn handle_supervisor_signal(_: libc::c_int) {
    SUPERVISOR_SHUTDOWN.store(true, Ordering::Relaxed);
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

fn wait_for_rpc(listen: &std::path::Path, timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    let mut last_error = String::new();
    while start.elapsed() < timeout {
        match NvimClient::connect_path_with_read_timeout(listen, Duration::from_millis(250))
            .and_then(|mut client| client.eval_lua("return vim.v.vim_did_enter == 1"))
        {
            Ok(NvimValue::Bool(true)) => return Ok(()),
            Ok(_) => {}
            Err(error) => last_error = error,
        }
        thread::sleep(Duration::from_millis(50));
    }

    if last_error.is_empty() {
        Err("timed out waiting for Neovim RPC readiness".to_string())
    } else {
        Err(format!(
            "timed out waiting for Neovim RPC readiness: {last_error}"
        ))
    }
}

fn socket_accepts_connections(path: &std::path::Path) -> bool {
    UnixStream::connect(path)
        .map(|stream| {
            let _ = stream.shutdown(Shutdown::Both);
        })
        .is_ok()
}
