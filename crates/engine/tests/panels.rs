use magicstudio_engine::panels::{Config, PanelReport, Reason, Signal};
use serde_json::json;

fn contract_example() -> serde_json::Value {
    json!({"panels": {
        "revision": 12, "available": true,
        "items": [{"id": "block-song", "label": "Song", "look": "control", "signal": 1, "busy": false}],
        "unconfigured": [
            {"id": "button-17", "tag": "button", "label": "Export", "reason": "untracked"},
            {"id": "blender-placement", "tag": "div", "label": "Place in scene", "reason": "unstyled"}
        ]
    }})
}

#[test]
fn from_feedback_reads_contract_example() {
    let report = PanelReport::from_feedback(&contract_example());
    assert!(report.available);
    assert_eq!(report.revision, 12);
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].signal, Signal::InUse);
    assert_eq!(report.items[0].look.as_deref(), Some("control"));
    assert_eq!(report.count(Reason::Untracked), 1);
    assert_eq!(report.count(Reason::Unstyled), 1);
}

#[test]
fn missing_panels_key_is_unavailable() {
    let report = PanelReport::from_feedback(&json!({"project": "/p/A.magic"}));
    assert!(!report.available, "an older Studio reports unavailable, never zero panels");
    assert!(report.items.is_empty());
}

#[test]
fn signal_out_of_range_is_problem_not_panic() {
    let mut value = contract_example();
    value["panels"]["items"][0]["signal"] = json!(9);
    assert_eq!(PanelReport::from_feedback(&value).items[0].signal, Signal::Problem);
}

#[test]
fn unknown_reason_string_is_untracked_not_panic() {
    let mut value = contract_example();
    value["panels"]["unconfigured"][1]["reason"] = json!("something-new");
    assert_eq!(PanelReport::from_feedback(&value).count(Reason::Untracked), 2);
}

#[test]
fn worst_of_empty_is_idle() {
    assert_eq!(PanelReport::default().worst(), Signal::Idle);
}

#[test]
fn config_loads_the_shell_components_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/magicstudio-shell/ui/components.json");
    let config = Config::load(&path).expect("components.json must load as the TUI reads it");
    assert!(!config.elements.is_empty());
    for (id, entry) in &config.elements {
        if let Some(look) = &entry.look {
            let allowed = config.looks.get(&entry.component).cloned().unwrap_or_default();
            assert!(allowed.contains(look), "{id}: look {look} is not one of {allowed:?}");
        }
    }
}

#[test]
fn invalid_config_is_reported_and_hidden_indicators_remain_configured() {
    let mut config: Config = serde_json::from_value(json!({"elements":{"quiet":{"component":"workspace","indicator":"hidden"}}})).unwrap();
    assert!(config.validate().is_ok());
    config.elements.get_mut("quiet").unwrap().look = Some("missing".into());
    assert!(config.validate().unwrap_err().contains("quiet"));
}

#[test]
fn report_identity_changes_even_when_revision_does_not() {
    let mut input = contract_example();
    input["project"] = json!("/p/A.magic"); input["identity"] = json!({"pid": 1});
    let first = PanelReport::from_feedback(&input);
    input["project"] = json!("/p/B.magic");
    assert_ne!(first, PanelReport::from_feedback(&input));
    input["project"] = json!("/p/A.magic"); input["identity"] = json!({"pid": 2});
    assert_ne!(first, PanelReport::from_feedback(&input));
}

#[test]
fn aliases_share_canonical_state_and_stale_observations_are_unavailable() {
    let config: Config = serde_json::from_value(json!({"elements":{
        "block-song":{"component":"panel"},
        "song-button":{"component":"button","target":"block-song"}
    }})).unwrap();
    let mut input = contract_example();
    let report = PanelReport::from_feedback(&input);
    assert_eq!(report.component(&config, "song-button"), report.component(&config, "block-song"));
    input["frontend_report_age_ms"] = json!(5001);
    let stale = PanelReport::from_feedback(&input);
    assert!(!stale.available);
    assert!(stale.component(&config, "song-button").is_none());
    assert!(stale.error.is_some());
}

#[test]
fn malformed_and_incomplete_observations_cannot_report_healthy() {
    let mut input = contract_example();
    input["panels"]["items"] = json!("broken");
    let broken = PanelReport::from_feedback(&input);
    assert!(!broken.available); assert!(broken.error.unwrap().contains("Invalid"));
    let mut input = contract_example(); input["panels"]["total_items"] = json!(99);
    assert!(PanelReport::from_feedback(&input).truncated);
}

#[test]
fn config_load_validates_instead_of_returning_invalid_configuration() {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("magic-panel-config-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("components.json");
    std::fs::write(&path, r#"{"elements":{"bad":{"component":"panel","indicator":"maybe"}}}"#).unwrap();
    let result = Config::load(&path);
    std::fs::remove_file(&path).unwrap(); std::fs::remove_dir(&dir).unwrap();
    assert!(result.unwrap_err().contains("indicator"));
}
