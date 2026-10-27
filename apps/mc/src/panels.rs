//! Static configuration and live observations share the same public report.
use magicstudio_engine::panels::{Config, PanelReport};
use serde_json::{Value, json};

const EMBEDDED_COMPONENTS_JSON: &str = include_str!("../../magicstudio-shell/ui/components.json");

pub fn config_path() -> std::path::PathBuf {
    let cwd_candidate = std::path::Path::new("apps/magicstudio-shell/ui/components.json");
    if cwd_candidate.is_file() {
        return cwd_candidate.to_path_buf();
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../magicstudio-shell/ui/components.json")
}

pub fn load_config() -> Result<(std::path::PathBuf, Config), String> {
    let path = config_path();
    if path.is_file() {
        Config::load(&path)
            .map(|c| (path.clone(), c))
            .map_err(|e| format!("{}: {e}", path.display()))
    } else {
        Config::from_json_str(EMBEDDED_COMPONENTS_JSON)
            .map(|c| (std::path::PathBuf::from("embedded://components.json"), c))
    }
}

pub fn run(endpoint: Option<&str>, json_output: bool, check: bool) -> i32 {
    let (path, config) = match load_config() {
        Ok(pair) => pair,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let observation = super::bridge_call(endpoint.unwrap_or("auto"), "ui.feedback", json!({}));
    let (feedback, error) = match observation {
        Ok(value) => (value, None),
        Err(error) => (Value::Null, Some(error)),
    };
    let report = PanelReport::from_feedback(&feedback);
    let valid = config.validate();
    let headless = feedback["connection"] == "not_observed_open";
    let healthy = valid.is_ok()
        && (headless
            || (feedback["ui_ready"] == true
                && feedback["frontend_report_age_ms"]
                    .as_u64()
                    .is_some_and(|age| age <= 5000)
                && report.available
                && report.unconfigured.is_empty()
                && !report.truncated));
    if json_output {
        println!(
            "{}",
            json!({"config_path":path,"config":config,"panels":report,"feedback":feedback,"error":error,"config_error":valid.err(),"healthy":healthy})
        );
    } else {
        println!("SIGNAL  ID  COMPONENT  LOOK  LABEL  BUSY");
        for (id, entry) in &config.elements {
            let live = report.component(&config, id);
            println!(
                "{}  {}  {}  {}  {}  {}",
                live.map(|v| v.signal.label()).unwrap_or("configured"),
                id,
                entry.component,
                entry.look.as_deref().unwrap_or("-"),
                entry.label.as_deref().unwrap_or(id),
                live.map(|v| if v.busy { "busy" } else { "-" })
                    .unwrap_or("-")
            );
        }
        for item in &report.unconfigured {
            println!("{:?}: {}", item.reason, item.id);
        }
        if let Some(error) = error {
            eprintln!("Live telemetry unavailable: {error}");
        }
        if let Err(error) = valid {
            eprintln!("Invalid configuration: {error}");
        }
    }
    i32::from(check && !healthy)
}
