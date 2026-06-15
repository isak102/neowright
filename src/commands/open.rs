use std::fs::{self, File};
use std::process::{Command, Stdio};

use crate::attached_ui;
use crate::cli::{OpenArgs, SessionSupervisorArgs};
use crate::commands::CommandFailure;
use crate::output;
use crate::session::{
    SessionRecord, SessionRegistry, SizeRecord, artifact_dir_for, ensure_artifact_dir, generate_id,
};
use crate::session_io::SessionIo;
use crate::session_supervisor;

pub fn run(args: OpenArgs) -> Result<String, CommandFailure> {
    if args.headed {
        attached_ui::validate_launch_options(args.terminal_cmd.as_deref(), args.terminal_preset)?;
    }

    let cwd = std::env::current_dir().map_err(|error| format!("failed to resolve cwd: {error}"))?;
    let size = args.size.unwrap_or_default();
    SessionRegistry::load_global()?.ensure_name_available(args.name.as_deref())?;

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
        return Err(error.into());
    }

    let record = SessionRecord {
        id,
        name: args.name,
        cwd,
        artifact_dir,
        size: SizeRecord::from(size),
        supervisor_pid: supervisor.id(),
        child_pid: None,
        listen,
    };

    let opened = output::opened_session(&record);
    if args.headed {
        let launch = match attached_ui::launch_for_record(
            &record,
            args.terminal_cmd.as_deref(),
            args.terminal_preset,
        ) {
            Ok(launch) => launch,
            Err(error) => {
                return Err(CommandFailure {
                    message: format!("Headed UI launch failed.\n\n{error}"),
                    stdout: Some(crate::output::status_document(&opened)),
                });
            }
        };

        return Ok(format!(
            "{opened}\n\n{}",
            attached_ui::launch_summary(&launch)
        ));
    }

    Ok(opened)
}

pub fn run_supervisor(args: SessionSupervisorArgs) -> Result<String, String> {
    session_supervisor::run(args)
}
