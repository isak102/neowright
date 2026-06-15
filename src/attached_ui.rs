use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};

use crate::cli::{
    TERMINAL_PRESETS, TerminalPreset, TerminalPresetDetection, supported_terminal_preset_names,
    terminal_preset_spec,
};
use crate::session::SessionRecord;

pub(crate) fn remote_ui_command(record: &SessionRecord) -> Vec<OsString> {
    vec![
        OsString::from("nvim"),
        OsString::from("--server"),
        record.listen.as_os_str().to_owned(),
        OsString::from("--remote-ui"),
    ]
}

pub(crate) fn shell_join(argv: &[OsString]) -> String {
    argv.iter()
        .map(|arg| shell_quote(&arg.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn launch_summary(launch: &TerminalLaunch) -> String {
    format!(
        "Headed UI process launched.\n- Terminal Command: `{}`\n- Terminal Source: `{}`",
        launch.command,
        launch_source_label(&launch.source)
    )
}

pub(crate) struct TerminalLaunch {
    pub(crate) command: String,
    source: TerminalLaunchSource,
}

enum TerminalLaunchSource {
    CustomCommand,
    Preset(TerminalPreset),
    Detected(DetectedTerminalPreset),
}

struct DetectedTerminalPreset {
    preset: TerminalPreset,
    reason: String,
}

impl TerminalLaunch {
    pub(crate) fn resolve(
        terminal_cmd: Option<&str>,
        terminal_preset: Option<TerminalPreset>,
    ) -> Result<Self, String> {
        if let Some(command) = terminal_cmd {
            return Ok(Self {
                command: command.to_string(),
                source: TerminalLaunchSource::CustomCommand,
            });
        }

        if let Some(preset) = terminal_preset {
            return Ok(Self {
                command: preset_command(preset).to_string(),
                source: TerminalLaunchSource::Preset(preset),
            });
        }

        let Some(detected) = detect_terminal_preset() else {
            return Err(format!(
                "attach requires --terminal-cmd, --terminal-preset, --print-command, or a known current terminal. Supported presets: {}",
                supported_terminal_preset_names()
            ));
        };

        Ok(Self {
            command: preset_command(detected.preset).to_string(),
            source: TerminalLaunchSource::Detected(detected),
        })
    }

    pub(crate) fn source_label(&self) -> String {
        launch_source_label(&self.source)
    }

    pub(crate) fn launch_for_record(&self, record: &SessionRecord) -> Result<(), String> {
        ensure_listen_socket(record)?;
        launch_terminal(&self.command, &remote_ui_command(record))
    }
}

fn preset_command(preset: TerminalPreset) -> &'static str {
    terminal_preset_spec(preset).command
}

fn detect_terminal_preset() -> Option<DetectedTerminalPreset> {
    detect_terminal_preset_from(|name| std::env::var(name).ok())
}

fn detect_terminal_preset_from(
    getenv: impl Fn(&str) -> Option<String>,
) -> Option<DetectedTerminalPreset> {
    let term_program = getenv("TERM_PROGRAM").unwrap_or_default().to_lowercase();

    for spec in TERMINAL_PRESETS {
        for detection in spec.detection {
            if matches!(
                *detection,
                TerminalPresetDetection::EnvEquals("TERMINAL", expected)
                    if getenv("TERMINAL").is_some_and(|value| value.eq_ignore_ascii_case(expected))
            ) {
                let value = getenv("TERMINAL").expect("TERMINAL matched");
                return Some(DetectedTerminalPreset {
                    preset: spec.preset,
                    reason: format!("TERMINAL={value}"),
                });
            }
        }
    }

    for spec in TERMINAL_PRESETS {
        for detection in spec.detection {
            match *detection {
                TerminalPresetDetection::EnvPresent(name) if getenv(name).is_some() => {
                    return Some(DetectedTerminalPreset {
                        preset: spec.preset,
                        reason: format!("{name} is set"),
                    });
                }
                TerminalPresetDetection::EnvEquals(name, expected)
                    if getenv(name).is_some_and(|value| value.eq_ignore_ascii_case(expected)) =>
                {
                    let value = getenv(name).expect("environment variable matched");
                    return Some(DetectedTerminalPreset {
                        preset: spec.preset,
                        reason: format!("{name}={value}"),
                    });
                }
                TerminalPresetDetection::TermProgramContains(value)
                    if term_program.contains(value) =>
                {
                    let actual = getenv("TERM_PROGRAM").unwrap_or_default();
                    return Some(DetectedTerminalPreset {
                        preset: spec.preset,
                        reason: format!("TERM_PROGRAM={actual}"),
                    });
                }
                _ => {}
            }
        }
    }

    None
}

pub(crate) fn ensure_listen_socket(record: &SessionRecord) -> Result<(), String> {
    if !record.listen.exists() {
        return Err(format!(
            "Session Neovim listen socket is unavailable: `{}`",
            record.listen.display()
        ));
    }

    UnixStream::connect(&record.listen)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "failed to connect to Session Neovim listen socket `{}`: {error}",
                record.listen.display()
            )
        })
}

fn launch_terminal(terminal: &str, remote: &[OsString]) -> Result<(), String> {
    let argv = terminal_argv(terminal, remote)?;
    let mut argv = argv.into_iter();
    let program = argv
        .next()
        .ok_or_else(|| "--terminal-cmd must not be empty".to_string())?;
    Command::new(&program)
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            format!(
                "failed to launch headed UI with `{}`: {error}",
                program.to_string_lossy()
            )
        })
}

fn terminal_argv(terminal: &str, remote: &[OsString]) -> Result<Vec<OsString>, String> {
    let remote_shell = shell_join(remote);
    if terminal.contains("{}") {
        split_shell_words(terminal).map(|argv| {
            os_strings(
                argv.into_iter()
                    .map(|arg| arg.replace("{}", &remote_shell))
                    .collect(),
            )
        })
    } else {
        let mut argv = split_shell_words(terminal).map(os_strings)?;
        argv.extend(remote.iter().cloned());
        Ok(argv)
    }
}

fn launch_source_label(source: &TerminalLaunchSource) -> String {
    match source {
        TerminalLaunchSource::CustomCommand => "custom command".to_string(),
        TerminalLaunchSource::Preset(preset) => format!("preset: {preset}"),
        TerminalLaunchSource::Detected(detected) => {
            format!("detected: {} via {}", detected.preset, detected.reason)
        }
    }
}

fn os_strings(values: Vec<String>) -> Vec<OsString> {
    values.into_iter().map(OsString::from).collect()
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'-' | b'_' | b':' | b'=')
        })
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn split_shell_words(input: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None;
    let mut has_current = false;

    while let Some(char) = chars.next() {
        match quote {
            Some('\'') => {
                if char == '\'' {
                    quote = None;
                } else {
                    current.push(char);
                    has_current = true;
                }
            }
            Some('"') => match char {
                '"' => quote = None,
                '\\' => {
                    let Some(next) = chars.next() else {
                        current.push('\\');
                        has_current = true;
                        continue;
                    };
                    current.push(next);
                    has_current = true;
                }
                _ => {
                    current.push(char);
                    has_current = true;
                }
            },
            Some(_) => unreachable!(),
            None => match char {
                '\'' | '"' => {
                    quote = Some(char);
                    has_current = true;
                }
                '\\' => {
                    let Some(next) = chars.next() else {
                        return Err("--terminal-cmd ends with an unfinished escape".to_string());
                    };
                    current.push(next);
                    has_current = true;
                }
                char if char.is_whitespace() => {
                    if has_current {
                        words.push(std::mem::take(&mut current));
                        has_current = false;
                    }
                }
                _ => {
                    current.push(char);
                    has_current = true;
                }
            },
        }
    }

    if let Some(quote) = quote {
        return Err(format!("--terminal-cmd has an unterminated {quote} quote"));
    }
    if has_current {
        words.push(current);
    }
    if words.is_empty() {
        return Err("--terminal-cmd must not be empty".to_string());
    }

    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote() -> Vec<OsString> {
        vec![
            OsString::from("nvim"),
            OsString::from("--server"),
            OsString::from("/tmp/socket path"),
            OsString::from("--remote-ui"),
        ]
    }

    fn detected_preset(detected: Option<DetectedTerminalPreset>) -> Option<TerminalPreset> {
        detected.map(|detected| detected.preset)
    }

    #[test]
    fn terminal_without_placeholder_appends_remote_command_as_tokens() {
        assert_eq!(
            terminal_argv("terminal -e", &remote()).unwrap(),
            vec![
                OsString::from("terminal"),
                OsString::from("-e"),
                OsString::from("nvim"),
                OsString::from("--server"),
                OsString::from("/tmp/socket path"),
                OsString::from("--remote-ui"),
            ]
        );
    }

    #[test]
    fn terminal_placeholder_expands_to_one_shell_quoted_command() {
        assert_eq!(
            terminal_argv("tmux split-window {}", &remote()).unwrap(),
            vec![
                OsString::from("tmux"),
                OsString::from("split-window"),
                OsString::from("nvim --server '/tmp/socket path' --remote-ui"),
            ]
        );
    }

    #[test]
    fn terminal_presets_define_known_commands() {
        for spec in TERMINAL_PRESETS {
            assert_eq!(preset_command(spec.preset), spec.command);
            assert!(!spec.name.is_empty());
            assert!(!spec.command.is_empty());
        }
    }

    #[test]
    fn terminal_detection_recognizes_known_terminal_environments() {
        for spec in TERMINAL_PRESETS {
            let detection = spec.detection[0];
            let detected = detect_terminal_preset_from(|name| match detection {
                TerminalPresetDetection::EnvPresent(env_name) if name == env_name => {
                    Some("1".to_string())
                }
                TerminalPresetDetection::EnvEquals(env_name, value) if name == env_name => {
                    Some(value.to_string())
                }
                TerminalPresetDetection::TermProgramContains(value) if name == "TERM_PROGRAM" => {
                    Some(value.to_string())
                }
                _ => None,
            });

            assert_eq!(detected_preset(detected), Some(spec.preset));
        }
    }

    #[test]
    fn terminal_detection_recognizes_ghostty_inside_tmux() {
        let detected = detect_terminal_preset_from(|name| match name {
            "TERM_PROGRAM" => Some("tmux".to_string()),
            "TERMINAL" => Some("ghostty".to_string()),
            _ => None,
        });

        let detected = detected.expect("ghostty detected");
        assert_eq!(detected.preset, TerminalPreset::Ghostty);
        assert_eq!(detected.reason, "TERMINAL=ghostty");
    }

    #[test]
    fn terminal_env_takes_precedence_over_other_detection() {
        let detected = detect_terminal_preset_from(|name| match name {
            "TERM_PROGRAM" => Some("iterm".to_string()),
            "TERMINAL" => Some("ghostty".to_string()),
            _ => None,
        });

        assert_eq!(detected_preset(detected), Some(TerminalPreset::Ghostty));
    }

    #[test]
    fn detected_source_label_explains_where_detection_came_from() {
        let source = TerminalLaunchSource::Detected(DetectedTerminalPreset {
            preset: TerminalPreset::Ghostty,
            reason: "TERMINAL=ghostty".to_string(),
        });

        assert_eq!(
            launch_source_label(&source),
            "detected: ghostty via TERMINAL=ghostty"
        );
    }

    #[test]
    fn shell_split_handles_quotes_and_escapes() {
        assert_eq!(
            split_shell_words("sh -lc 'echo hi' \"two words\" a\\ b").unwrap(),
            vec!["sh", "-lc", "echo hi", "two words", "a b"]
        );
    }

    #[test]
    fn shell_split_rejects_unterminated_quotes() {
        assert!(split_shell_words("terminal 'bad").is_err());
    }
}
