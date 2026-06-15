use crate::cli::KeysArgs;
use crate::commands::CommandOutput;
use crate::nvim::NvimClient;
use crate::output;
use crate::session;
use crate::session_io::SessionIo;

pub fn run(args: KeysArgs) -> Result<CommandOutput, String> {
    let record = session::SessionRegistry::load_global()?.resolve_target(&args.target)?;
    if args.pty {
        let bytes = translate_pty_keys(&args.keys)?;
        SessionIo::for_record(&record).write_pty_input(&bytes)?;

        return Ok(CommandOutput::Markdown(output::sent_keys(
            "Sent PTY Keys",
            &args.keys,
        )));
    }

    let mut client = NvimClient::connect(&record)?;
    client.feed_keys(&args.keys)?;

    Ok(CommandOutput::Markdown(output::sent_keys(
        "Sent Keys",
        &args.keys,
    )))
}

fn translate_pty_keys(keys: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut rest = keys;

    while let Some(start) = rest.find('<') {
        output.extend_from_slice(&rest.as_bytes()[..start]);
        let token_start = &rest[start..];
        let Some(end) = token_start.find('>') else {
            output.extend_from_slice(token_start.as_bytes());
            return Ok(output);
        };
        let token = &token_start[..=end];
        output.extend_from_slice(&translate_pty_token(token)?);
        rest = &token_start[end + 1..];
    }

    output.extend_from_slice(rest.as_bytes());
    Ok(output)
}

fn translate_pty_token(token: &str) -> Result<Vec<u8>, String> {
    let name = &token[1..token.len() - 1];
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "esc" | "escape" => Ok(vec![0x1b]),
        "cr" | "enter" | "return" => Ok(vec![b'\r']),
        "tab" => Ok(vec![b'\t']),
        "bs" | "backspace" => Ok(vec![0x7f]),
        "lt" => Ok(vec![b'<']),
        _ => translate_modified_token(token, name, &lower),
    }
}

fn translate_modified_token(token: &str, name: &str, lower: &str) -> Result<Vec<u8>, String> {
    if let Some(key) = lower.strip_prefix("c-") {
        return translate_control_key(token, key);
    }

    if lower.starts_with("m-") || lower.starts_with("a-") {
        return translate_alt_key(token, &name[2..]);
    }

    Err(unsupported_pty_token(token))
}

fn translate_control_key(token: &str, key: &str) -> Result<Vec<u8>, String> {
    let mut chars = key.chars();
    let Some(char) = chars.next() else {
        return Err(unsupported_pty_token(token));
    };
    if chars.next().is_some() {
        return Err(unsupported_pty_token(token));
    }

    let byte = match char {
        'a'..='z' => char as u8 - b'a' + 1,
        '[' => 0x1b,
        '\\' => 0x1c,
        ']' => 0x1d,
        '^' => 0x1e,
        '_' => 0x1f,
        '?' => 0x7f,
        _ => return Err(unsupported_pty_token(token)),
    };
    Ok(vec![byte])
}

fn translate_alt_key(token: &str, key: &str) -> Result<Vec<u8>, String> {
    if key.is_empty() || key.contains('<') || key.contains('>') {
        return Err(unsupported_pty_token(token));
    }

    let mut output = Vec::with_capacity(1 + key.len());
    output.push(0x1b);
    output.extend_from_slice(key.as_bytes());
    Ok(output)
}

fn unsupported_pty_token(token: &str) -> String {
    format!(
        "unsupported PTY key notation: {token}; --pty supports only plain text, <Esc>, <CR>, <Tab>, <BS>, <lt>, <C-x>, and <M-x> terminal-level notation"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pty_keys_translate_plain_text_and_special_keys() {
        assert_eq!(translate_pty_keys("hello").unwrap(), b"hello");
        assert_eq!(translate_pty_keys("ihello<Esc>").unwrap(), b"ihello\x1b");
        assert_eq!(translate_pty_keys("<Escape>").unwrap(), b"\x1b");
        assert_eq!(
            translate_pty_keys("<CR><Enter><Return>").unwrap(),
            b"\r\r\r"
        );
        assert_eq!(translate_pty_keys("a<Tab>b").unwrap(), b"a\tb");
        assert_eq!(translate_pty_keys("a<BS>b").unwrap(), b"a\x7fb");
        assert_eq!(translate_pty_keys("<lt>").unwrap(), b"<");
    }

    #[test]
    fn pty_keys_translate_control_notation() {
        assert_eq!(translate_pty_keys("<C-c>").unwrap(), vec![0x03]);
        assert_eq!(translate_pty_keys("<c-m>").unwrap(), b"\r");
        assert_eq!(translate_pty_keys("<C-[>").unwrap(), b"\x1b");
        assert_eq!(translate_pty_keys("<C-?>").unwrap(), vec![0x7f]);
    }

    #[test]
    fn pty_keys_translate_alt_notation() {
        assert_eq!(translate_pty_keys("<M-x>").unwrap(), b"\x1bx");
        assert_eq!(translate_pty_keys("<A-x>").unwrap(), b"\x1bx");
        assert_eq!(translate_pty_keys("<M-X>").unwrap(), b"\x1bX");
        assert_eq!(translate_pty_keys("<M-å>").unwrap(), "\u{1b}å".as_bytes());
    }

    #[test]
    fn pty_keys_reject_unsupported_notation() {
        let error = translate_pty_keys("<leader>").unwrap_err();
        assert!(error.contains("unsupported PTY key notation: <leader>"));
        assert!(error.contains("--pty supports"));

        assert!(translate_pty_keys("<F1>").is_err());
        assert!(translate_pty_keys("<Up>").is_err());
        assert!(translate_pty_keys("<M->").is_err());
    }

    #[test]
    fn pty_keys_treat_unclosed_angle_as_literal_text() {
        assert_eq!(translate_pty_keys("a<broken").unwrap(), b"a<broken");
    }
}
