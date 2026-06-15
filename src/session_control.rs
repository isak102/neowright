use std::time::Duration;

use crate::cli::{Size, TargetSelector};
use crate::nvim::{NvimClient, NvimValue};
use crate::session::{self, SessionRecord, SessionRegistry};
use crate::session_io::SessionIo;

const GRACEFUL_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) trait SessionControl {
    fn record(&self) -> &SessionRecord;
    fn eval_lua(&mut self, lua: &str) -> Result<NvimValue, String>;
    fn exec(&mut self, command: &str) -> Result<String, String>;
    fn send_keys(&mut self, keys: &str) -> Result<(), String>;
    fn send_pty_keys(&mut self, keys: &str) -> Result<(), String>;
    fn resize(&mut self, size: Size) -> Result<(), String>;
    fn close(&mut self, force: bool) -> Result<(), String>;
}

pub(crate) struct LiveSessionControl {
    registry: SessionRegistry,
    record: SessionRecord,
    client: Option<NvimClient>,
}

impl LiveSessionControl {
    pub(crate) fn resolve(selector: &TargetSelector) -> Result<Self, String> {
        let registry = SessionRegistry::load_global()?;
        let record = registry.resolve_target(selector)?;
        Ok(Self {
            registry,
            record,
            client: None,
        })
    }

    pub(crate) fn for_record(record: SessionRecord) -> Result<Self, String> {
        Ok(Self {
            registry: SessionRegistry::load_global()?,
            record,
            client: None,
        })
    }

    fn client(&mut self) -> Result<&mut NvimClient, String> {
        if self.client.is_none() {
            self.client = Some(NvimClient::connect(&self.record)?);
        }

        Ok(self.client.as_mut().expect("client was just initialized"))
    }

    fn io(&self) -> SessionIo {
        SessionIo::for_record(&self.record)
    }
}

impl SessionControl for LiveSessionControl {
    fn record(&self) -> &SessionRecord {
        &self.record
    }

    fn eval_lua(&mut self, lua: &str) -> Result<NvimValue, String> {
        self.client()?.eval_lua(lua)
    }

    fn exec(&mut self, command: &str) -> Result<String, String> {
        self.client()?.exec(command)
    }

    fn send_keys(&mut self, keys: &str) -> Result<(), String> {
        self.client()?.feed_keys(keys)
    }

    fn send_pty_keys(&mut self, keys: &str) -> Result<(), String> {
        let bytes = translate_pty_keys(keys)?;
        self.io().write_pty_input(&bytes)
    }

    fn resize(&mut self, size: Size) -> Result<(), String> {
        self.client()?
            .command(&format!("set columns={} lines={}", size.cols, size.rows))?;

        self.record.size = size.into();
        self.registry.update(self.record.clone())?;
        self.io().write_desired_size(self.record.size)
    }

    fn close(&mut self, force: bool) -> Result<(), String> {
        {
            let client = self.client()?;
            if !force {
                let modified = client.eval_lua(
                    r#"
local modified = {}
for _, buffer in ipairs(vim.api.nvim_list_bufs()) do
  if vim.bo[buffer].modified then
    table.insert(modified, vim.api.nvim_buf_get_name(buffer))
  end
end
return modified
"#,
                )?;
                if let NvimValue::Array(buffers) = modified
                    && !buffers.is_empty()
                {
                    return Err(format!(
                        "Session has unsaved changes; use `--force` to discard them ({})",
                        buffers.len()
                    ));
                }
            }

            let command = if force { "qall!" } else { "qall" };
            client.notify_command(command)?;
        }

        let exited = session::wait_for_record_exit(&self.record, GRACEFUL_CLOSE_TIMEOUT);
        self.registry.remove(&self.record.id)?;
        if !exited {
            session::kill_record_processes(&self.record);
        }
        Ok(())
    }
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
        assert_eq!(
            translate_pty_keys("<M-\u{00e5}>").unwrap(),
            "\u{1b}\u{00e5}".as_bytes()
        );
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
