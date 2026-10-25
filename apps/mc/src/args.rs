//! CLI argument parsing for the mc CLI.

use serde_json::Value;

pub(crate) fn parse_value(v: &str) -> Value {
    if let Ok(n) = v.parse::<i64>() {
        return Value::Number(n.into());
    }
    if let Ok(n) = v.parse::<f64>() {
        return Value::Number(
            serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0)),
        );
    }
    if v == "true" {
        return Value::Bool(true);
    }
    if v == "false" {
        return Value::Bool(false);
    }
    if let Ok(v) = serde_json::from_str::<Value>(v) {
        return v;
    }
    Value::String(v.to_string())
}

pub struct Args {
    pub help: bool,
    pub version: bool,
    pub flag_keep_going: bool,
    pub flag_clean: bool,
    pub flag_health: bool,
    pub flag_report: bool,
    pub flag_github: bool,
    pub flag_stop: bool,
    inner: CliArgs,
}

#[derive(Default)]
struct CliArgs {
    args: Vec<String>,
}

impl CliArgs {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        Self {
            args: args.collect(),
        }
    }

    fn flag(&self, name: &str) -> bool {
        self.args.iter().any(|a| a == name)
    }

    fn pos(&self, n: usize) -> Option<&str> {
        let mut positional = Vec::new();
        let mut skip_next = false;
        for a in &self.args {
            if skip_next {
                skip_next = false;
                continue;
            }
            if a == "--bridge" {
                skip_next = true;
                continue;
            }
            if !a.starts_with('-') {
                positional.push(a.as_str());
            }
        }
        positional.get(n).copied()
    }

    fn opt(&self, name: &str) -> Option<String> {
        for (i, a) in self.args.iter().enumerate() {
            if a == name {
                return self.args.get(i + 1).cloned();
            }
            if let Some(rest) = a.strip_prefix(&format!("{name}=")) {
                return Some(rest.to_string());
            }
        }
        None
    }

    fn wants_version(&self) -> bool {
        self.args.iter().any(|a| a == "--version" || a == "-V")
    }

    fn params_from(&self, _start: usize) -> Result<Value, String> {
        let mut obj = serde_json::Map::new();
        for arg in &self.args {
            if arg.starts_with("--") {
                continue;
            }
            if let Some((k, v)) = arg.split_once('=') {
                obj.insert(k.to_string(), parse_value(v));
            }
        }
        Ok(Value::Object(obj))
    }
}

impl Args {
    pub fn parse(args: impl Iterator<Item = String>) -> Self {
        let inner = CliArgs::parse(args);
        Self {
            help: inner.flag("--help"),
            version: inner.wants_version(),
            flag_keep_going: inner.flag("--keep-going"),
            flag_clean: inner.flag("--clean") || inner.flag("-c"),
            flag_health: inner.flag("--health") || inner.flag("-H"),
            flag_report: inner.flag("--report") || inner.flag("-R"),
            flag_github: inner.flag("--github") || inner.flag("-G"),
            flag_stop: inner.flag("--stop") || inner.flag("-S"),
            inner,
        }
    }

    pub fn bridge_addr(&self) -> Option<String> {
        self.inner.opt("--bridge").map(|v| {
            if v=="auto" || v.contains(':') {
                v
            } else {
                format!("127.0.0.1:{v}")
            }
        })
    }

    pub fn pos(&self, n: usize) -> Option<&str> {
        self.inner.pos(n)
    }

    pub fn params_from(&self, start: usize) -> Result<Value, String> {
        self.inner.params_from(start)
    }
}
