use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::cli::SessionSupervisorArgs;
use crate::nvim::{NvimClient, NvimValue};
use crate::screen;
use crate::session::{SessionRecord, SessionRegistry, SizeRecord};
use crate::session_io::{self, SessionIo};

const READY_TIMEOUT: Duration = Duration::from_secs(10);
static SUPERVISOR_SHUTDOWN: AtomicBool = AtomicBool::new(false);

pub fn run(args: SessionSupervisorArgs) -> Result<String, String> {
    install_supervisor_signal_handlers();

    let mut runtime = SupervisorRuntime::start(args)?;
    runtime.wait_until_ready()?;
    runtime.register()?;
    runtime.mark_ready()?;
    runtime.run_until_exit()?;

    Ok("Session supervisor exited.".to_string())
}

struct SupervisorRuntime {
    session_id: String,
    name: Option<String>,
    cwd: std::path::PathBuf,
    artifact_dir: std::path::PathBuf,
    listen: std::path::PathBuf,
    ready_file: std::path::PathBuf,
    size: SizeRecord,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    io: SessionIo,
    parser: Arc<Mutex<vt100::Parser>>,
    pty_input_path: std::path::PathBuf,
    registered: bool,
}

impl SupervisorRuntime {
    fn start(args: SessionSupervisorArgs) -> Result<Self, String> {
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

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("failed to start nvim: {error}"))?;
        drop(pair.slave);

        let size = SizeRecord::from(args.size);
        let io = SessionIo::new(args.session.clone(), args.artifact_dir.clone());
        let pty_input_path = io.pty_input_path();
        let _ = fs::remove_file(&pty_input_path);
        let parser = Arc::new(Mutex::new(screen::parser_for(size)));
        persist_current_screen(&parser, &io, size)?;

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
        let reader_io = io.clone();
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
                let _ = reader_io.write_latest_screen(&contents);
            }
        });

        Ok(Self {
            session_id: args.session,
            name: args.name,
            cwd: args.cwd,
            artifact_dir: args.artifact_dir,
            listen: args.listen,
            ready_file: args.ready_file,
            size,
            master: pair.master,
            child,
            io,
            parser,
            pty_input_path,
            registered: false,
        })
    }

    fn wait_until_ready(&self) -> Result<(), String> {
        wait_for_socket(&self.listen, READY_TIMEOUT)
            .and_then(|_| session_io::restrict_socket_permissions(&self.listen))
            .and_then(|_| wait_for_rpc(&self.listen, READY_TIMEOUT))
    }

    fn register(&mut self) -> Result<(), String> {
        SessionRegistry::load_global()?.insert(SessionRecord {
            id: self.session_id.clone(),
            name: self.name.clone(),
            cwd: self.cwd.clone(),
            artifact_dir: self.artifact_dir.clone(),
            size: self.size,
            supervisor_pid: std::process::id(),
            child_pid: self.child.process_id(),
            listen: self.listen.clone(),
        })?;
        self.registered = true;
        Ok(())
    }

    fn mark_ready(&self) -> Result<(), String> {
        fs::write(&self.ready_file, b"ready").map_err(|error| {
            format!(
                "failed to write readiness file `{}`: {error}",
                self.ready_file.display()
            )
        })
    }

    fn run_until_exit(&mut self) -> Result<(), String> {
        loop {
            if SUPERVISOR_SHUTDOWN.load(Ordering::Relaxed) {
                self.kill_child();
                break;
            }

            if self
                .child
                .try_wait()
                .map_err(|error| format!("failed while polling nvim: {error}"))?
                .is_some()
            {
                break;
            }

            self.apply_desired_size()?;
            thread::sleep(Duration::from_millis(50));
        }

        Ok(())
    }

    fn apply_desired_size(&mut self) -> Result<(), String> {
        let Some(desired_size) = self.io.read_desired_size()? else {
            return Ok(());
        };
        if desired_size == current_parser_size(&self.parser)? {
            return Ok(());
        }

        self.master
            .resize(PtySize {
                rows: desired_size.rows,
                cols: desired_size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("failed to resize PTY: {error}"))?;
        let mut parser = self
            .parser
            .lock()
            .map_err(|_| "failed to lock Screen parser".to_string())?;
        parser
            .screen_mut()
            .set_size(desired_size.rows, desired_size.cols);
        Ok(())
    }

    fn kill_child(&mut self) {
        if let Some(child_pid) = self.child.process_id() {
            unsafe {
                libc::kill(child_pid as libc::pid_t, libc::SIGKILL);
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for SupervisorRuntime {
    fn drop(&mut self) {
        if self.registered
            && let Ok(registry) = SessionRegistry::load_global()
        {
            let _ = registry.remove(&self.session_id);
        }
        let _ = fs::remove_file(&self.listen);
        let _ = fs::remove_file(&self.pty_input_path);
        if self.child.try_wait().ok().flatten().is_none() {
            self.kill_child();
        }
    }
}

pub fn wait_until_ready(
    listen: &Path,
    ready_file: &Path,
    supervisor_log: &Path,
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

fn spawn_pty_input_listener(
    path: &Path,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
) -> Result<(), String> {
    let listener = UnixListener::bind(path).map_err(|error| {
        format!(
            "failed to create Session PTY input socket `{}`: {error}",
            path.display()
        )
    })?;
    session_io::restrict_socket_permissions(path)?;

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
    io: &SessionIo,
    size: SizeRecord,
) -> Result<(), String> {
    let parser = parser
        .lock()
        .map_err(|_| "failed to lock Screen parser".to_string())?;
    io.write_latest_screen(&screen::snapshot_text(&parser, size))
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

fn wait_for_socket(listen: &Path, timeout: Duration) -> Result<(), String> {
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

fn wait_for_rpc(listen: &Path, timeout: Duration) -> Result<(), String> {
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

fn socket_accepts_connections(path: &Path) -> bool {
    UnixStream::connect(path)
        .map(|stream| {
            let _ = stream.shutdown(Shutdown::Both);
        })
        .is_ok()
}
