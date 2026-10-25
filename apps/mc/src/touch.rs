//! Each agent's last touch: the newest command (and UI control) its pane sent through the CLI.
//! The journal keeps only three root actions per project and the app's own writes fill that, so
//! the CLI also leaves one small file per client under `$XDG_RUNTIME_DIR/magicstudio-touch/`.
//! MOPS reads them while it is open; nothing polls and nothing is kept after a reboot.
use serde_json::{Value, json};
use std::{fs, path::{Path, PathBuf}, time::SystemTime};

/// Observation and the app's own bookkeeping say nothing about what an agent is working on.
const QUIET: &[&str] = &["ui.feedback", "ui.state", "ui.elements", "ui.components", "command.list", "workshop.status"];

fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("magicstudio-touch"))
}

fn file_for(dir: &Path, client: &str) -> PathBuf {
    let safe: String = client.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    dir.join(format!("{safe}.json"))
}

/// The UI element a command acted on, when it acted on one.
fn control(id: &str, params: &Value) -> Option<String> {
    if !matches!(id, "ui.click" | "ui.set" | "ui.key" | "ui.look") {
        return None;
    }
    ["id", "target", "click"].iter().find_map(|k| params[*k].as_str()).map(str::to_owned)
}

pub fn record(id: &str, params: &Value) {
    if let Some(dir) = dir() {
        record_in(&dir, id, params);
    }
}

pub fn record_in(dir: &Path, id: &str, params: &Value) {
    let Some(client) = params["_client"].as_str() else { return };
    if QUIET.contains(&id) || id.starts_with("center.") {
        return;
    }
    let at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let record = json!({
        "client": client, "command": id, "control": control(id, params),
        "project": params["project"].as_str().or(params["_expected_project"].as_str()), "at": at,
    });
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    let target = file_for(dir, client);
    let temp = target.with_extension("tmp");
    if fs::write(&temp, record.to_string()).is_ok() {
        let _ = fs::rename(&temp, &target);
    }
}

pub fn read_all() -> Vec<Value> {
    dir().map(|d| read_in(&d)).unwrap_or_default()
}

pub fn read_in(dir: &Path) -> Vec<Value> {
    fs::read_dir(dir).into_iter().flatten().flatten()
        .filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_command_per_client_is_kept_with_its_control_and_noise_is_ignored() {
        let tmp = std::env::temp_dir().join(format!("touch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        record_in(&tmp, "ui.click", &json!({"_client": "herdr:wG:pY", "id": "sb-next"}));
        record_in(&tmp, "ui.feedback", &json!({"_client": "herdr:wG:pY"}));
        record_in(&tmp, "center.events", &json!({"_client": "herdr:wG:pY"}));
        record_in(&tmp, "storyboard.edit", &json!({"_client": "herdr:wG:pS", "project": "/p/a.magic"}));
        record_in(&tmp, "project.save", &json!({}));
        let all = read_in(&tmp);
        assert_eq!(all.len(), 2, "one file per client; no client, no record: {all:?}");
        let mine = all.iter().find(|r| r["client"] == "herdr:wG:pY").unwrap();
        assert_eq!((mine["command"].as_str(), mine["control"].as_str()), (Some("ui.click"), Some("sb-next")), "observation did not replace the click");
        record_in(&tmp, "project.save", &json!({"_client": "herdr:wG:pY"}));
        let again = read_in(&tmp);
        let mine = again.iter().find(|r| r["client"] == "herdr:wG:pY").unwrap();
        assert_eq!((mine["command"].as_str(), mine["control"].is_null()), (Some("project.save"), true), "the newest touch replaces the old one");
        assert_eq!(again.iter().find(|r| r["client"] == "herdr:wG:pS").unwrap()["project"], "/p/a.magic");
        let _ = fs::remove_dir_all(&tmp);
    }
}
