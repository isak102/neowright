use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    name = "neowright",
    about = "Automate and inspect real Neovim TUI sessions"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Open a supervised Neovim session")]
    Open(OpenArgs),
    #[command(about = "Attach a visible Neovim UI to a session")]
    Attach(AttachArgs),
    #[command(about = "List active Neovim sessions")]
    List,
    #[command(hide = true, name = "__session-supervisor")]
    SessionSupervisor(SessionSupervisorArgs),
    #[command(about = "Close one or more Neovim sessions")]
    Close(CloseArgs),
    #[command(about = "Send keys to a Neovim session")]
    Keys(KeysArgs),
    #[command(about = "Execute a Neovim command in a session")]
    Exec(ExecArgs),
    #[command(about = "Evaluate Lua in a Neovim session")]
    Eval(EvalArgs),
    #[command(about = "Wait until a Lua condition becomes true")]
    Wait(WaitArgs),
    #[command(about = "Capture the visible terminal screen")]
    Snapshot(SnapshotArgs),
    #[command(about = "Resize a Neovim session terminal")]
    Resize(ResizeArgs),
    #[command(about = "Install Neowright agent skills")]
    Skills(SkillsArgs),
}

#[derive(Debug, Args)]
pub struct SessionSupervisorArgs {
    #[arg(long)]
    pub session: String,

    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub cwd: PathBuf,

    #[arg(long, value_parser = parse_size)]
    pub size: Size,

    #[arg(long)]
    pub artifact_dir: PathBuf,

    #[arg(long)]
    pub listen: PathBuf,

    #[arg(long)]
    pub ready_file: PathBuf,

    #[arg(last = true)]
    pub neovim_args: Vec<String>,
}

#[derive(Debug, Args)]
pub struct OpenArgs {
    #[arg(long, help = "Assign a human-readable name to the new session")]
    pub name: Option<String>,

    #[arg(long, value_parser = parse_size, help = "Set the terminal size as COLSxROWS")]
    pub size: Option<Size>,

    #[arg(
        long,
        help = "Launch a visible remote UI after opening the session. Uses --terminal-cmd, --terminal-preset, or a detected known terminal"
    )]
    pub headed: bool,

    #[arg(
        long,
        requires = "headed",
        conflicts_with = "terminal_preset",
        help = "Custom terminal command used to launch the visible remote UI"
    )]
    pub terminal_cmd: Option<String>,

    #[arg(
        long,
        requires = "headed",
        conflicts_with = "terminal_cmd",
        value_enum,
        help = "Known terminal preset to launch the visible remote UI"
    )]
    pub terminal_preset: Option<TerminalPreset>,

    #[arg(help = "Arguments passed through to nvim after --")]
    #[arg(last = true)]
    pub neovim_args: Vec<String>,
}

#[derive(Debug, Args)]
pub struct AttachArgs {
    #[command(flatten)]
    pub target: TargetSelector,

    #[arg(
        long,
        conflicts_with_all = ["print_command", "terminal_preset"],
        help = "Custom terminal command used to launch the visible remote UI"
    )]
    pub terminal_cmd: Option<String>,

    #[arg(
        long,
        conflicts_with_all = ["print_command", "terminal_cmd"],
        value_enum,
        help = "Known terminal preset to launch the visible remote UI"
    )]
    pub terminal_preset: Option<TerminalPreset>,

    #[arg(
        long,
        help = "Print the resolved nvim remote-ui command instead of launching it"
    )]
    pub print_command: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotArgs {
    #[command(flatten)]
    pub target: TargetSelector,
}

#[derive(Debug, Args)]
pub struct CloseArgs {
    #[command(flatten)]
    pub target: TargetSelector,

    #[arg(
        long,
        help = "Terminate the session process if graceful shutdown fails"
    )]
    pub force: bool,

    #[arg(long, conflicts_with_all = ["session", "name"], help = "Close every active session")]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct KeysArgs {
    #[command(flatten)]
    pub target: TargetSelector,

    #[arg(
        long,
        help = "Write terminal input bytes directly to the Session PTY instead of using Neovim RPC",
        long_help = "Write terminal input bytes directly to the Session PTY instead of using Neovim RPC. By default, keys are sent through Neovim RPC with Neovim key notation and mappings. PTY mode is an escape hatch for blocked UI states and supports only plain text plus terminal-level notation such as <Esc>, <CR>, <Tab>, <BS>, <C-c>, and <M-x>."
    )]
    pub pty: bool,

    #[arg(help = "Keys to send, using Neovim key notation unless --pty is set")]
    pub keys: String,
}

#[derive(Debug, Args)]
pub struct ExecArgs {
    #[command(flatten)]
    pub target: TargetSelector,

    #[arg(help = "Ex command to execute, without the leading colon")]
    pub command: String,
}

#[derive(Debug, Args)]
pub struct EvalArgs {
    #[command(flatten)]
    pub target: TargetSelector,

    #[arg(long, help = "Print the raw Lua result instead of JSON formatting")]
    pub raw: bool,

    #[arg(help = "Lua expression or chunk to evaluate")]
    pub lua: String,
}

#[derive(Debug, Args)]
pub struct WaitArgs {
    #[command(flatten)]
    pub target: TargetSelector,

    #[arg(long, value_parser = parse_duration, default_value = "5s", help = "Maximum time to wait, for example 5s or 500ms")]
    pub timeout: Duration,

    #[arg(long, value_parser = parse_duration, default_value = "100ms", help = "Delay between condition checks, for example 100ms")]
    pub interval: Duration,

    #[arg(help = "Lua condition evaluated until it returns true")]
    pub condition: String,
}

#[derive(Debug, Args)]
pub struct ResizeArgs {
    #[command(flatten)]
    pub target: TargetSelector,

    #[arg(value_parser = parse_size, help = "New terminal size as COLSxROWS")]
    pub size: Size,
}

#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    #[command(about = "Install bundled Neowright agent skills")]
    Install(SkillsInstallArgs),
}

#[derive(Debug, Args)]
pub struct SkillsInstallArgs {
    #[arg(
        long,
        conflicts_with = "local",
        help = "Install skills into the global agent configuration"
    )]
    pub global: bool,

    #[arg(long, help = "Install skills into this repository")]
    pub local: bool,
}

#[derive(Debug, Args)]
pub struct TargetSelector {
    #[arg(long, conflicts_with = "name", help = "Target a session by id")]
    pub session: Option<String>,

    #[arg(long, conflicts_with = "session", help = "Target a session by name")]
    pub name: Option<String>,
}

pub struct TerminalPresetSpec {
    pub preset: TerminalPreset,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub command: &'static str,
    pub detection: &'static [TerminalPresetDetection],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPresetDetection {
    EnvPresent(&'static str),
    EnvEquals(&'static str, &'static str),
    TermProgramContains(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPreset {
    Alacritty,
    Ghostty,
    Iterm,
}

pub const TERMINAL_PRESETS: &[TerminalPresetSpec] = &[
    TerminalPresetSpec {
        preset: TerminalPreset::Alacritty,
        name: "alacritty",
        aliases: &[],
        command: "alacritty -e",
        detection: &[TerminalPresetDetection::EnvPresent("ALACRITTY_WINDOW_ID")],
    },
    TerminalPresetSpec {
        preset: TerminalPreset::Ghostty,
        name: "ghostty",
        aliases: &[],
        command: "ghostty -e",
        detection: &[
            TerminalPresetDetection::TermProgramContains("ghostty"),
            TerminalPresetDetection::EnvEquals("TERMINAL", "ghostty"),
            TerminalPresetDetection::EnvPresent("GHOSTTY_BIN_DIR"),
            TerminalPresetDetection::EnvPresent("GHOSTTY_RESOURCES_DIR"),
            TerminalPresetDetection::EnvEquals("__CFBundleIdentifier", "com.mitchellh.ghostty"),
        ],
    },
    TerminalPresetSpec {
        preset: TerminalPreset::Iterm,
        name: "iterm",
        aliases: &["iterm2"],
        command: "osascript -e 'tell application \"iTerm2\" to create window with default profile command \"{}\"'",
        detection: &[TerminalPresetDetection::TermProgramContains("iterm")],
    },
];

pub fn terminal_preset_spec(preset: TerminalPreset) -> &'static TerminalPresetSpec {
    TERMINAL_PRESETS
        .iter()
        .find(|spec| spec.preset == preset)
        .expect("terminal preset exists")
}

impl std::fmt::Display for TerminalPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", terminal_preset_spec(*self).name)
    }
}

impl ValueEnum for TerminalPreset {
    fn value_variants<'a>() -> &'a [Self] {
        const VARIANTS: &[TerminalPreset] = &[
            TerminalPreset::Alacritty,
            TerminalPreset::Ghostty,
            TerminalPreset::Iterm,
        ];
        VARIANTS
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        let spec = terminal_preset_spec(*self);
        Some(clap::builder::PossibleValue::new(spec.name).aliases(spec.aliases))
    }
}

pub fn supported_terminal_preset_names() -> String {
    TERMINAL_PRESETS
        .iter()
        .map(|spec| spec.name)
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Default for Size {
    fn default() -> Self {
        crate::session::DEFAULT_SIZE
    }
}

impl FromStr for Size {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (cols, rows) = value
            .split_once('x')
            .ok_or_else(|| "size must use COLSxROWS, for example 240x70".to_string())?;

        let cols = cols
            .parse::<u16>()
            .map_err(|_| "size columns must be a positive integer".to_string())?;
        let rows = rows
            .parse::<u16>()
            .map_err(|_| "size rows must be a positive integer".to_string())?;

        if cols == 0 || rows == 0 {
            return Err("size columns and rows must be greater than zero".to_string());
        }

        Ok(Self { cols, rows })
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.cols, self.rows)
    }
}

pub fn parse_size(value: &str) -> Result<Size, String> {
    value.parse()
}

pub fn parse_duration(value: &str) -> Result<Duration, String> {
    if let Some(ms) = value.strip_suffix("ms") {
        let ms = ms
            .parse::<u64>()
            .map_err(|_| "duration milliseconds must be a positive integer".to_string())?;
        return Ok(Duration::from_millis(ms));
    }

    if let Some(seconds) = value.strip_suffix('s') {
        let seconds = seconds
            .parse::<u64>()
            .map_err(|_| "duration seconds must be a positive integer".to_string())?;
        return Ok(Duration::from_secs(seconds));
    }

    Err("duration must use ms or s, for example 500ms or 5s".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_size() {
        assert_eq!(
            parse_size("240x70"),
            Ok(Size {
                cols: 240,
                rows: 70
            })
        );
    }

    #[test]
    fn rejects_invalid_size() {
        assert!(parse_size("240").is_err());
        assert!(parse_size("0x70").is_err());
    }

    #[test]
    fn parses_duration() {
        assert_eq!(parse_duration("500ms"), Ok(Duration::from_millis(500)));
        assert_eq!(parse_duration("5s"), Ok(Duration::from_secs(5)));
    }

    #[test]
    fn rejects_invalid_duration() {
        assert!(parse_duration("500").is_err());
        assert!(parse_duration("xs").is_err());
    }

    #[test]
    fn rejects_conflicting_targets() {
        let result = Cli::try_parse_from([
            "neowright",
            "snapshot",
            "--session",
            "abc",
            "--name",
            "main",
        ]);

        assert!(result.is_err());
    }
}
