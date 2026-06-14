use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use rmpv::Value;
use rmpv::decode::read_value;
use rmpv::encode::write_value;

use crate::session::SessionRecord;

pub struct NvimClient {
    stream: UnixStream,
    next_request_id: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NvimValue {
    Nil,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<NvimValue>),
    Map(BTreeMap<String, NvimValue>),
}

impl NvimClient {
    pub fn connect(record: &SessionRecord) -> Result<Self, String> {
        Self::connect_path(&record.listen)
    }

    pub(crate) fn connect_path(path: &Path) -> Result<Self, String> {
        let stream = UnixStream::connect(path).map_err(|error| {
            format!(
                "failed to connect to Neovim control socket `{}`: {error}",
                path.display()
            )
        })?;

        Ok(Self {
            stream,
            next_request_id: 1,
        })
    }

    pub(crate) fn connect_path_with_read_timeout(
        path: &Path,
        timeout: Duration,
    ) -> Result<Self, String> {
        let stream = UnixStream::connect(path).map_err(|error| {
            format!(
                "failed to connect to Neovim control socket `{}`: {error}",
                path.display()
            )
        })?;
        stream.set_read_timeout(Some(timeout)).map_err(|error| {
            format!(
                "failed to set Neovim control socket read timeout `{}`: {error}",
                path.display()
            )
        })?;

        Ok(Self {
            stream,
            next_request_id: 1,
        })
    }

    pub fn eval_lua(&mut self, lua: &str) -> Result<NvimValue, String> {
        let result = self.request(
            "nvim_exec_lua",
            vec![Value::from(lua.to_string()), Value::Array(Vec::new())],
        )?;
        Ok(NvimValue::from_msgpack(result))
    }

    pub fn exec(&mut self, command: &str) -> Result<String, String> {
        let opts = vec![(Value::from("output"), Value::Boolean(true))];
        let result = self.request(
            "nvim_exec2",
            vec![Value::from(command.to_string()), Value::Map(opts)],
        )?;

        let NvimValue::Map(map) = NvimValue::from_msgpack(result) else {
            return Ok(String::new());
        };

        match map.get("output") {
            Some(NvimValue::String(output)) => Ok(output.clone()),
            _ => Ok(String::new()),
        }
    }

    pub fn command(&mut self, command: &str) -> Result<(), String> {
        self.request("nvim_command", vec![Value::from(command.to_string())])?;
        Ok(())
    }

    pub fn notify_command(&mut self, command: &str) -> Result<(), String> {
        self.notify("nvim_command", vec![Value::from(command.to_string())])
    }

    pub fn feed_keys(&mut self, keys: &str) -> Result<(), String> {
        let replaced = self.request(
            "nvim_replace_termcodes",
            vec![
                Value::from(keys.to_string()),
                Value::Boolean(true),
                Value::Boolean(false),
                Value::Boolean(true),
            ],
        )?;

        self.request("nvim_input", vec![replaced])?;
        Ok(())
    }

    fn request(&mut self, method: &str, args: Vec<Value>) -> Result<Value, String> {
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let request = Value::Array(vec![
            Value::from(0),
            Value::from(request_id),
            Value::from(method.to_string()),
            Value::Array(args),
        ]);

        write_value(&mut self.stream, &request)
            .map_err(|error| format!("failed to send Neovim RPC request `{method}`: {error}"))?;
        self.stream
            .flush()
            .map_err(|error| format!("failed to flush Neovim RPC request `{method}`: {error}"))?;

        loop {
            let response = read_value(&mut self.stream).map_err(|error| {
                format!("failed to read Neovim RPC response for `{method}`: {error}")
            })?;
            let Value::Array(items) = response else {
                continue;
            };
            if items.len() < 4 || items[0].as_i64() != Some(1) {
                continue;
            }
            if items[1].as_i64() != Some(request_id) {
                continue;
            }
            if !items[2].is_nil() {
                return Err(format!(
                    "Neovim API error from `{method}`: {}",
                    NvimValue::from_msgpack(items[2].clone()).format_compact()
                ));
            }
            return Ok(items[3].clone());
        }
    }

    fn notify(&mut self, method: &str, args: Vec<Value>) -> Result<(), String> {
        let notification = Value::Array(vec![
            Value::from(2),
            Value::from(method.to_string()),
            Value::Array(args),
        ]);

        write_value(&mut self.stream, &notification).map_err(|error| {
            format!("failed to send Neovim RPC notification `{method}`: {error}")
        })?;
        self.stream
            .flush()
            .map_err(|error| format!("failed to flush Neovim RPC notification `{method}`: {error}"))
    }
}

impl NvimValue {
    fn from_msgpack(value: Value) -> Self {
        match value {
            Value::Nil => Self::Nil,
            Value::Boolean(value) => Self::Bool(value),
            Value::Integer(value) => Self::Integer(value.as_i64().unwrap_or_default()),
            Value::F32(value) => Self::Float(value.into()),
            Value::F64(value) => Self::Float(value),
            Value::String(value) => Self::String(
                value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string()),
            ),
            Value::Binary(value) => Self::String(String::from_utf8_lossy(&value).into_owned()),
            Value::Array(values) => {
                Self::Array(values.into_iter().map(Self::from_msgpack).collect())
            }
            Value::Map(entries) => {
                let mut map = BTreeMap::new();
                for (key, value) in entries {
                    map.insert(Self::map_key(key), Self::from_msgpack(value));
                }
                Self::Map(map)
            }
            Value::Ext(_, value) => Self::String(String::from_utf8_lossy(&value).into_owned()),
        }
    }

    fn map_key(value: Value) -> String {
        if let Value::String(value) = &value
            && let Some(value) = value.as_str()
        {
            return value.to_string();
        }

        match Self::from_msgpack(value) {
            Self::String(value) => value,
            value => value.format_compact(),
        }
    }

    pub fn is_truthy(&self) -> bool {
        !matches!(self, Self::Nil | Self::Bool(false))
    }

    pub fn format_display(&self) -> String {
        match self {
            Self::Nil => "nil".to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::String(value) => value.clone(),
            Self::Array(values) => {
                let values = values
                    .iter()
                    .map(Self::format_display_nested)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {values} }}")
            }
            Self::Map(values) => {
                let values = values
                    .iter()
                    .map(|(key, value)| format!("{key} = {}", value.format_display_nested()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {values} }}")
            }
        }
    }

    fn format_display_nested(&self) -> String {
        match self {
            Self::String(value) => format!("{:?}", value),
            _ => self.format_display(),
        }
    }

    pub fn format_raw(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            _ => self.format_compact(),
        }
    }

    pub fn format_compact(&self) -> String {
        let json = self.to_json();
        serde_json::to_string(&json).unwrap_or_else(|_| "null".to_string())
    }

    fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Nil => serde_json::Value::Null,
            Self::Bool(value) => serde_json::Value::Bool(*value),
            Self::Integer(value) => serde_json::Value::Number((*value).into()),
            Self::Float(value) => serde_json::Number::from_f64(*value)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Self::String(value) => serde_json::Value::String(value.clone()),
            Self::Array(values) => {
                serde_json::Value::Array(values.iter().map(Self::to_json).collect())
            }
            Self::Map(values) => serde_json::Value::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_json()))
                    .collect(),
            ),
        }
    }
}
