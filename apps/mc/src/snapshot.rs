//! One Studio observation shared by `mc exec center.snapshot`, `mc exec center.tail` and both
//! Command Center views. Reads existing sources only (ui.feedback, ui.state, motion.tracks, the
//! saved project file and the center.events journal); anything unobserved stays null.
use serde_json::{Value, json};

pub const SNAPSHOT_SCHEMA: &str = "mc.center.snapshot/1";
pub const TAIL_SCHEMA: &str = "mc.center.tail/1";

/// Page state and rig fps from the matched Studio window; empty when no window is connected.
/// `previous` is the last snapshot, so fps is re-read only when the subject changes.
pub fn live(ui: &Value, previous: &Value) -> Value {
    let Some(endpoint) = ui["endpoint"].as_str().filter(|_| ui["connection"] == "connected") else {
        return json!({});
    };
    let page = crate::local_ui::call(endpoint, "ui.state", json!({})).unwrap_or_else(|e| json!({"error": e}));
    let subject = page["animation"]["character"].as_str();
    let fps = if subject.is_some() && subject == previous["subject"].as_str() && !previous["fps"].is_null() {
        previous["fps"].clone()
    } else if let Some(character) = subject.filter(|_| ui["engine_unsaved"].is_boolean()) {
        // motion.tracks waits for the engine lock; ask only while ui.feedback found it free.
        // A rig without its own fps is sampled at motion.sample's default of 30.
        crate::local_ui::call(endpoint, "motion.tracks", json!({"character": character}))
            .map(|t| if t["fps"].is_number() { t["fps"].clone() } else { json!(30) })
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    json!({"page": page, "fps": fps})
}

/// The snapshot object agents read. Keys are stable; values are null when unknown.
pub fn build(project: Option<&str>, ui: &Value, live: &Value, activity: &Value) -> Value {
    let connected = ui["connection"] == "connected";
    let project = ui["active_project"].as_str().filter(|_| connected).or(project);
    // Page state counts only when the page shows the same project ui.feedback reported.
    let animation = if connected && live["page"]["project"] == ui["active_project"] {
        &live["page"]["animation"]
    } else {
        &Value::Null
    };
    let unsaved = if connected && ui["engine_unsaved"].is_boolean() {
        json!(ui["dirty_reasons"].as_array().map_or(0, Vec::len))
    } else {
        Value::Null
    };
    // workshop.set_target saves immediately, so the project file holds the live target.
    let hyperframes = project
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .map(|doc| doc["workshop"]["target"].as_str().unwrap_or("remotion") == "hyperframes");
    let error = |source: &str, command: &Value, message: &Value, at: &Value| {
        json!({"source": source, "command": command, "message": message, "unix_micros": at})
    };
    let last_error = project
        .map(|p| tail(activity, Some(p)))
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|e| e["type"] == "error")
        .map(|e| error("engine", &e["command"], &e["error"], &e["unix_micros"]))
        .or_else(|| {
            ui["errors"].as_array().and_then(|v| v.last()).map(|e| error("studio page", &Value::Null, &e["message"], &Value::Null))
        })
        .or_else(|| {
            ui["error"].as_str().or(live["page"]["error"].as_str()).map(|m| error("connection", &Value::Null, &json!(m), &Value::Null))
        });
    json!({
        "schema": SNAPSHOT_SCHEMA,
        "observed_unix_ms": now_ms(),
        "connection": ui["connection"].as_str().unwrap_or("unknown"),
        "project": project,
        "subject": animation["character"],
        "clip": animation["track"],
        "playhead_sec": animation["time"],
        "selected_part": animation["selected_layer"],
        "unsaved_edits": unsaved,
        "hyperframes": hyperframes,
        "fps": if animation.is_null() { Value::Null } else { live["fps"].clone() },
        "last_error": last_error,
    })
}

/// Typed events from the activity journal, oldest first, for `project` when given. Selection
/// changes and clip switches are not journaled by the app, so they never appear here.
pub fn tail(activity: &Value, project: Option<&str>) -> Vec<Value> {
    let mut out = Vec::new();
    for e in activity["events"].as_array().into_iter().flatten() {
        if project.is_some_and(|p| e["project"].as_str() != Some(p)) {
            continue;
        }
        let command = e["command"].as_str().unwrap_or_default();
        let stage = e["stage"].as_str().unwrap_or_default();
        let failed = matches!(stage, "engine_error" | "handler_error");
        let kinds: &[&str] = match (command, stage) {
            (_, "saved_state") => &["saved"],
            ("image.generate", "dispatch_running") => &["generation_started"],
            ("image.generate", "engine_success" | "engine_needs_work") => &["generation_finished"],
            ("image.generate", _) if failed => &["generation_finished", "error"],
            ("motion.path", "engine_success") => &["keyframe_added"],
            ("motion.undo_path", "engine_success") => &["keyframe_removed"],
            _ if failed => &["error"],
            _ => &[],
        };
        for kind in kinds {
            out.push(json!({
                "sequence": e["sequence"],
                "unix_micros": e["unix_micros"],
                "type": kind,
                "command": e["command"],
                "ok": !failed,
                "error": e["error"],
                "origin": e["origin"],
                "client": e["client_label"],
                "request_id": e["request_id"],
                "project": e["project"],
            }));
        }
    }
    out
}

/// One status line for both Command Center views.
pub fn status_line(s: &Value) -> String {
    let text = |key: &str| s[key].as_str().map(str::to_owned).unwrap_or_else(|| "?".into());
    let project = s["project"]
        .as_str()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .and_then(|v| v.to_str())
        .unwrap_or("no project");
    let playhead = s["playhead_sec"].as_f64().map(|t| format!("{t:.3}s")).unwrap_or_else(|| "?".into());
    let unsaved = s["unsaved_edits"].as_u64().map(|n| n.to_string()).unwrap_or_else(|| "?".into());
    let hyperframes = match s["hyperframes"].as_bool() { Some(true) => "on", Some(false) => "off", None => "?" };
    let fps = s["fps"].as_f64().map(|f| format!("{f}")).unwrap_or_else(|| "?".into());
    let error = s["last_error"]["message"].as_str().map(|m| format!(" · err {m}")).unwrap_or_default();
    format!(
        "{project} · subject {} · clip {} · ▶ {playhead} · part {} · unsaved {unsaved} · HF {hyperframes} · {fps}fps{error}",
        text("subject"),
        text("clip"),
        text("selected_part")
    )
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// `mc exec center.snapshot` / `center.tail`: observe the Studio for `project` (or the only
/// open Studio) and read its journal. Never launches, switches or writes anything.
pub fn collect(project: Option<&str>) -> (Value, Value) {
    let ui = crate::local_ui::call("auto", "ui.feedback", json!({"project": project}))
        .unwrap_or_else(|e| json!({"connection": "error", "error": e}));
    let project = ui["active_project"].as_str().or(project).map(str::to_owned);
    let mut session = animationcraft_engine::Session::default();
    let activity = session
        .execute("center.events", json!({"project": project, "limit": 192}))
        .unwrap_or_else(|e| json!({"error": e.to_string()}));
    let live = live(&ui, &Value::Null);
    (build(project.as_deref(), &ui, &live, &activity), activity)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity() -> Value {
        json!({"events": [
            {"sequence": 1, "stage": "dispatch_running", "command": "image.generate", "request_id": "g"},
            {"sequence": 2, "stage": "engine_error", "command": "image.generate", "request_id": "g", "error": "provider down"},
            {"sequence": 3, "stage": "engine_success", "command": "motion.path"},
            {"sequence": 4, "stage": "engine_success", "command": "motion.undo_path"},
            {"sequence": 5, "stage": "saved_state", "command": "project.save"},
            {"sequence": 6, "stage": "engine_success", "command": "center.status"},
            {"sequence": 7, "stage": "ui_click", "control": "motion-play"}
        ]})
    }

    #[test]
    fn tail_types_journal_rows_and_skips_untyped_ones() {
        let kinds: Vec<_> = tail(&activity(), None).iter().map(|e| e["type"].as_str().unwrap().to_owned()).collect();
        assert_eq!(kinds, ["generation_started", "generation_finished", "error", "keyframe_added", "keyframe_removed", "saved"]);
        let failed = &tail(&activity(), None)[1];
        assert_eq!(failed["ok"], false);
        assert_eq!(failed["error"], "provider down");
    }

    #[test]
    fn snapshot_keeps_stable_keys_and_null_when_unobserved() {
        let s = build(None, &json!({"connection": "not_observed_open"}), &json!({}), &json!({}));
        for key in ["schema", "observed_unix_ms", "connection", "project", "subject", "clip", "playhead_sec", "selected_part", "unsaved_edits", "hyperframes", "fps", "last_error"] {
            assert!(s.get(key).is_some(), "missing {key}");
        }
        assert!(s["subject"].is_null() && s["unsaved_edits"].is_null() && s["fps"].is_null() && s["hyperframes"].is_null());
        assert!(status_line(&s).contains("unsaved ?"));
    }

    #[test]
    fn snapshot_reads_matched_page_state_and_newest_engine_error() {
        let dir = std::env::temp_dir().join(format!("mc-snapshot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("song.acproj");
        std::fs::write(&project, r#"{"workshop":{"target":"hyperframes"}}"#).unwrap();
        let path = project.to_str().unwrap();
        let ui = json!({"connection": "connected", "active_project": path, "engine_unsaved": true, "dirty_reasons": ["engine state differs", {"draft_field": "x"}]});
        let page = json!({"page": {"project": path, "animation": {"character": "bee", "track": "fly", "time": 1.25, "selected_layer": "wing"}}, "fps": 24});
        let mut journal = activity();
        for e in journal["events"].as_array_mut().unwrap() { e["project"] = json!(path); }
        let s = build(None, &ui, &page, &journal);
        assert_eq!((s["subject"].as_str(), s["clip"].as_str(), s["selected_part"].as_str()), (Some("bee"), Some("fly"), Some("wing")));
        assert_eq!((s["playhead_sec"].as_f64(), s["fps"].as_u64(), s["unsaved_edits"].as_u64()), (Some(1.25), Some(24), Some(2)));
        assert_eq!(s["hyperframes"], true);
        assert_eq!(s["last_error"]["message"], "provider down");
        assert!(status_line(&s).starts_with("song · subject bee · clip fly · ▶ 1.250s · part wing · unsaved 2 · HF on · 24fps"));
        // Another project's journal error is not this project's last error.
        assert!(build(None, &ui, &page, &activity())["last_error"].is_null());
        // A page showing another project is not reported as this project's state.
        let stale = json!({"page": {"project": "/other.acproj", "animation": {"character": "bee"}}, "fps": 24});
        assert!(build(None, &ui, &stale, &json!({}))["subject"].is_null());
        std::fs::remove_file(&project).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
