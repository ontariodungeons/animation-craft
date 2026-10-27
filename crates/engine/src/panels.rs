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
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub request: Option<String>,
    #[serde(flatten)]
    pub telemetry: std::collections::BTreeMap<String, Value>,
}

/// One element's entry in `apps/magicstudio-shell/ui/components.json`.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct Entry {
    pub component: String,
    #[serde(default)]
    pub indicator: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub rail: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
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
    #[serde(default)]
    pub rails: Value,
    #[serde(default)]
    pub slots: Value,
    #[serde(default)]
    pub signals: Value,
    #[serde(default)]
    pub companions: Value,
    #[serde(default)]
    pub sections: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub timeline_tones: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub contract_target: Option<String>,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.elements.is_empty() { return Err("no configured elements".into()); }
        for (id, entry) in &self.elements {
            if let Some(target) = &entry.target {
                if !self.elements.contains_key(target) || self.elements[target].target.is_some() { return Err(format!("{id}: target must name a canonical component")); }
            }
            if let Some(rail) = &entry.rail {
                if !self.rails.get(rail).is_some_and(Value::is_object) || !self.slots.get(rail).is_some_and(Value::is_object) { return Err(format!("{id}: missing configured rail or slot")); }
            }
            if entry.component.trim().is_empty() { return Err(format!("{id}: missing component")); }
            if let Some(look) = &entry.look {
                if !self.looks.get(&entry.component).is_some_and(|looks| looks.contains(look)) { return Err(format!("{id}: unknown look {look}")); }
            }
            if entry.density.as_ref().is_some_and(|d| !self.densities.contains(d)) { return Err(format!("{id}: unknown density")); }
            if entry.indicator.as_ref().is_some_and(|v| !matches!(v.as_str(), "icon" | "hidden")) { return Err(format!("{id}: unknown indicator")); }
        }
        Ok(())
    }

    pub fn from_json_str(text: &str) -> Result<Self, String> {
        let config: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_json_str(&text)
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
    #[serde(flatten)]
    pub telemetry: std::collections::BTreeMap<String, Value>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct PanelReport {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub studio: Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub items: Vec<Panel>,
    #[serde(default)]
    pub unconfigured: Vec<Unconfigured>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub total_items: usize,
    #[serde(default)]
    pub total_unconfigured: usize,
}

impl PanelReport {
    pub fn from_feedback(feedback: &Value) -> Self {
        let panels = feedback.get("panels").cloned().unwrap_or(Value::Null);
        let mut report: Self = match serde_json::from_value(panels.clone()) {
            Ok(report) => report,
            Err(error) => Self { error: Some(if panels.is_null() { "No panel observation".into() } else { format!("Invalid panel observation: {error}") }), ..Default::default() },
        };
        report.project = feedback["project"].as_str().map(str::to_owned);
        report.studio = feedback["identity"].clone();
        if feedback["connection"] == "not_observed_open" {
            report.available = false;
            report.error = None;
        } else if feedback["ui_ready"] == false || feedback["frontend_report_age_ms"].as_u64().is_some_and(|age| age > 5000) {
            report.available = false;
            report.error = Some("Studio observation is stale or not ready".into());
        }
        for (key, actual) in [("total_items", report.items.len()), ("total_unconfigured", report.unconfigured.len())] {
            if panels[key].as_u64().is_some_and(|total| total != actual as u64) { report.truncated = true; }
        }
        report
    }
    /// Aliases in CLI and TUI resolve through the same canonical state record.
    pub fn component<'a>(&'a self, config: &Config, id: &str) -> Option<&'a Panel> {
        if !self.available { return None; }
        let target = config.elements.get(id).and_then(|entry| entry.target.as_deref()).unwrap_or(id);
        self.items.iter().find(|panel| panel.id == target)
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
