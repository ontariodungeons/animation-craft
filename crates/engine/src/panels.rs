use serde::{Deserialize, Serialize, Deserializer, Serializer};
use serde_json::Value;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Signal {
    #[default]
    Idle = 0,
    InUse = 1,
    Attention = 2,
    Problem = 3,
}

impl Signal {
    pub fn from_u64(v: u64) -> Self {
        match v {
            0 => Signal::Idle,
            1 => Signal::InUse,
            2 => Signal::Attention,
            3 => Signal::Problem,
            _ => Signal::Problem,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Signal::Idle => "idle",
            Signal::InUse => "in use",
            Signal::Attention => "attention",
            Signal::Problem => "problem",
        }
    }
}

impl From<u64> for Signal {
    fn from(v: u64) -> Self { Signal::from_u64(v) }
}

impl Serialize for Signal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for Signal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = u64::deserialize(deserializer)?;
        Ok(Signal::from_u64(v))
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Panel {
    pub id: String,
    pub label: Option<String>,
    #[serde(default)]
    pub look: Option<String>,
    #[serde(default)]
    pub signal: Signal,
    #[serde(default)]
    pub busy: bool,
}

/// One element's entry in `apps/magicstudio-shell/ui/components.json`.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct Entry {
    pub component: String,
    #[serde(default)]
    pub look: Option<String>,
    #[serde(default)]
    pub density: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

/// The UI config file, read from disk: what every element is, with or without a Studio.
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Config {
    #[serde(default)]
    pub elements: std::collections::BTreeMap<String, Entry>,
    /// Component name to the looks it allows.
    #[serde(default)]
    pub looks: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub densities: Vec<String>,
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    Unstyled,
    #[serde(other)]
    Untracked,
}

impl Default for Reason {
    fn default() -> Self { Reason::Untracked }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Unconfigured {
    pub id: String,
    pub tag: String,
    pub label: Option<String>,
    #[serde(default)]
    pub reason: Reason,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct PanelReport {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub items: Vec<Panel>,
    #[serde(default)]
    pub unconfigured: Vec<Unconfigured>,
}

impl PanelReport {
    pub fn from_feedback(feedback: &Value) -> Self {
        let panels = feedback.get("panels").cloned().unwrap_or(Value::Null);
        if panels.is_null() {
            return PanelReport { available: false, ..Default::default() };
        }
        serde_json::from_value(panels).unwrap_or_default()
    }
    pub fn worst(&self) -> Signal {
        self.items.iter().map(|p| p.signal).fold(Signal::Idle, |acc, s| {
            if (s as u8) > (acc as u8) { s } else { acc }
        })
    }
    pub fn count(&self, reason: Reason) -> usize {
        self.unconfigured.iter().filter(|u| u.reason == reason).count()
    }
}
