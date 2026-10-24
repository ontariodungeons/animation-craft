//! Static configuration and live observations share the same public report.
use magicstudio_engine::panels::{Config, PanelReport};
use serde_json::{Value, json};

pub fn config_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../magicstudio-shell/ui/components.json")
}

pub fn run(endpoint: Option<&str>, json_output: bool, check: bool) -> i32 {
    let path = config_path();
    let config = match Config::load(&path) {
        Ok(config) => config,
        Err(error) => { eprintln!("{}: {error}", path.display()); return 1; }
    };
    let observation = super::bridge_call(endpoint.unwrap_or("auto"), "ui.feedback", json!({}));
    let (feedback, error) = match observation { Ok(value) => (value, None), Err(error) => (Value::Null, Some(error)) };
    let report = PanelReport::from_feedback(&feedback);
    let valid = config.validate();
    let healthy = valid.is_ok() && feedback["frontend_report_age_ms"].as_u64().is_some_and(|age| age <= 5000) && report.available && report.unconfigured.is_empty() && feedback["panels"]["truncated"] != true;
    if json_output {
        println!("{}", json!({"config_path":path,"config":config,"panels":report,"feedback":feedback,"error":error,"config_error":valid.err(),"healthy":healthy}));
    } else {
        println!("SIGNAL  ID  COMPONENT  LOOK  LABEL  BUSY");
        for (id, entry) in &config.elements {
            let live = report.items.iter().find(|item| item.id == *entry.target.as_ref().unwrap_or(id)).filter(|_| report.available);
            println!("{}  {}  {}  {}  {}  {}", live.map(|v| v.signal.label()).unwrap_or("unavailable"), id, entry.component, entry.look.as_deref().unwrap_or("-"), entry.label.as_deref().unwrap_or(id), live.map(|v| if v.busy {"busy"} else {"-"}).unwrap_or("unknown"));
        }
        for item in &report.unconfigured { println!("{:?}: {}", item.reason, item.id); }
        if let Some(error) = error { eprintln!("Live telemetry unavailable: {error}"); }
        if let Err(error) = valid { eprintln!("Invalid configuration: {error}"); }
    }
    i32::from(check && !healthy)
}
