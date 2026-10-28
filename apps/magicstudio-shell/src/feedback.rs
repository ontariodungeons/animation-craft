//! Presentation acknowledgement and initialization health; never a claim of visual quality.
use crate::Engine;
use serde_json::{Value, json};
use std::sync::Mutex;
use tauri::Manager;
#[derive(Default)]
pub struct Feedback(pub Mutex<Value>);
#[tauri::command]
pub fn ui_report(app: tauri::AppHandle, report: Value) -> Result<(), String> {
    if let Some(event) = report.get("event").filter(|v| v.is_object()) {
        let mut event = event.clone();
        event["project"] = report["project"].clone();
        event["origin"] = json!("UI interaction");
        event["client_label"] = json!("Studio user (unverified label)");
        magicstudio_engine::activity::append(event);
    }
    let state = app.state::<Feedback>();
    let mut v = state.0.lock().map_err(|e| e.to_string())?;
    let revision = v["ui_revision"].as_u64().unwrap_or(0) + 1;
    let engine_revision = v["engine_revision"].clone();
    let mut last = v["last_command"].clone();
    let history = v["command_revisions"].clone();
    let session_project = v["session_project"].clone();
    let requested = report["ack_engine_revision"].as_u64().unwrap_or(0);
    let expected = history
        .as_array()
        .and_then(|items| items.iter().find(|item| item["revision"] == requested));
    let valid = report["phase"] == "ready"
        && report["rendered"] == true
        && report["errors"].as_array().is_some_and(|a| a.is_empty())
        && expected.is_some_and(|e| e["project"] == report["project"]);
    let previous = v["ack_engine_revision"].as_u64().unwrap_or(0);
    let acknowledged = if valid {
        requested.max(previous)
    } else {
        previous
    };
    if last["revision"] == acknowledged {
        last["ui_ack"] = json!("DOM_applied; visual review separate");
    }
    if valid && requested > previous {
        magicstudio_engine::activity::append(
            json!({"stage":"frontend_ack","project":report["project"],"command":expected.map(|e|e["id"].clone()),"engine_revision":requested,"request_id":expected.map(|e|e["request_id"].clone()),"origin":"UI render report","ui_ack":"DOM applied; visual review separate"}),
        );
    }
    *v = report;
    v["ui_revision"] = json!(revision);
    v["engine_revision"] = engine_revision;
    v["last_command"] = last;
    v["command_revisions"] = history;
    v["session_project"] = session_project;
    v["ack_engine_revision"] = json!(acknowledged);
    v["updated_unix_ms"] = json!(now());
    Ok(())
}
fn now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
pub fn command_finished(
    app: &tauri::AppHandle,
    id: &str,
    result: &Value,
    request_id: Option<&str>,
) -> u64 {
    let project = app
        .state::<Engine>()
        .with(None, |s| Ok(s.project_path.clone()))
        .ok()
        .flatten();
    let state = app.state::<Feedback>();
    let Ok(mut v) = state.0.lock() else {
        return 0;
    };
    let revision = v["engine_revision"].as_u64().unwrap_or(0) + 1;
    v["session_project"] = json!(project);
    v["engine_revision"] = json!(revision);
    v["last_command"] = json!({"request_id":request_id,"revision":revision,"id":id,"ok":result["ok"]!=false,"engine_finished_unix_ms":now(),"ui_ack":"pending"});
    let mut history = v["command_revisions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    history.push(json!({"request_id":request_id,"revision":revision,"id":id,"project":project}));
    if history.len() > 16 {
        history.remove(0);
    }
    v["command_revisions"] = json!(history);
    revision
}
pub fn snapshot(app: &tauri::AppHandle) -> Result<Value, String> {
    let mut v = app
        .state::<Feedback>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    if !v.is_object() {
        v = json!({});
    }
    let engine = app.state::<Engine>();
    let (project, dirty, chat_selection, edits, writer_owner) = if let Ok(s) = engine.0.try_lock() {
        let dirty = if let Some(path) = &s.project_path {
            std::fs::read(path)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .map(|doc| {
                    magicstudio_engine::project::project_is_dirty(&s, &doc)
                })
        } else {
            Some(false)
        };
        (s.project_path.clone(), dirty, s.agent_selection.clone(), Some(s.edits_since_save),Some(s.project_owner.is_some()))
    } else {
        (
            v["session_project"].as_str().map(std::path::PathBuf::from),
            None,
            None,
            None,
            None,
        )
    };
    v["project_guard_version"] = json!(1);
    v["ok"] = json!(true);
    v["connection"] = json!("connected");
    v["identity"] = crate::studio_identity(app);
    #[cfg(unix)]
    if let Some(socket)=app.try_state::<crate::bridge::LocalSocket>() {
        v["instance"]=json!(socket.0.file_name().and_then(|p|p.to_str()));
        v["endpoint"]=json!(format!("unix:{}",socket.0.display()));
    }

    v["writer_owner"] = json!(writer_owner);
    v["active_project"] = json!(project);
    v["project_id"] = json!(project);
    v["project_identity_kind"] = json!("canonical project path");
    v["observed_unix_ms"] = json!(now());
    v["frontend_report_age_ms"] = v["updated_unix_ms"].as_u64().map(|updated|json!(now().saturating_sub(updated as u128))).unwrap_or(Value::Null);
    v["chat_selection"] = match chat_selection {
        Some(selected)=>json!({"agent":selected["agent"],"provider":selected["provider"],"model":selected["model"],"source":"active engine selection","runtime_model_verified":false}),
        None=>json!({"status":"unknown","reason":"active engine selection unavailable","runtime_model_verified":false}),
    };
    let mut reasons=Vec::new();
    if dirty==Some(true){reasons.push(json!("engine state differs from saved project"));}
    if dirty.is_none(){reasons.push(json!("engine state unavailable; busy or unreadable saved project"));}
    for field in v["draft_fields"].as_array().into_iter().flatten(){reasons.push(json!({"draft_field":field}));}
    if v["pending_invokes"].as_u64().unwrap_or(0)>0 {reasons.push(json!({"pending_invokes":v["pending_invokes"]}));}
    v["dirty_reasons"]=json!(reasons);
    // Engine edits since the last project write, plus each changed form field. A clean diff
    // means 0 even after commands (e.g. exports) that never touch project state.
    let drafts=v["draft_fields"].as_array().map_or(0,Vec::len) as u64;
    v["unsaved_edits"]=match (dirty,edits) {(Some(false),_)=>json!(drafts),(Some(true),Some(n))=>json!(n.max(1)+drafts),_=>Value::Null};
    v["engine_unsaved"] = json!(dirty);
    v["unsaved_state"] = json!(if dirty == Some(true)
        || v["draft_fields"].as_array().is_some_and(|a| !a.is_empty())
        || v["pending_invokes"].as_u64().unwrap_or(0) > 0
    {
        "unsaved_or_pending"
    } else if dirty == Some(false) && v["phase"] == "ready" {
        "no_detected_unsaved_changes"
    } else {
        "unknown"
    });
    let ack = v["ack_engine_revision"].as_u64().unwrap_or(0);
    let rev = v["engine_revision"].as_u64().unwrap_or(0);
    v["ui_acknowledgement"] = json!(if v["phase"] == "error" {
        "frontend_error"
    } else if ack >= rev && v["phase"] == "ready" {
        "DOM_applied; visual review separate"
    } else {
        "pending"
    });
    v["ui_ready"] = json!(
        v["phase"] == "ready"
            && v["rendered"] == true
            && v["errors"].as_array().is_some_and(|e| e.is_empty())
            && v["project"] == v["active_project"]
            && ack >= rev
    );
    Ok(v)
}

/// One boundary for replacing/closing the live workspace. Unknown state never permits silent loss.
pub fn guard(app: &tauri::AppHandle, id: &str, ui_origin: bool) -> Result<(), String> {
    if !matches!(
        id,
        "project.open" | "project.new" | "project.close" | "project.intake" | "app.quit"
    ) {
        return Ok(());
    }
    let v = snapshot(app)?;
    if v["active_project"].is_null()
        && id != "app.quit"
        && v["pending_invokes"].as_u64().unwrap_or(0) <= u64::from(ui_origin)
    {
        return Ok(());
    }
    let fields = v["draft_fields"].as_array().is_some_and(|a| !a.is_empty());
    let pending = v["pending_invokes"].as_u64().unwrap_or(u64::MAX) > u64::from(ui_origin);
    if v["engine_unsaved"] != false
        || fields
        || pending
        || v["phase"] != "ready"
        || v["errors"].as_array().is_none_or(|a| !a.is_empty())
    {
        return Err(format!(
            "{id} blocked: unsaved, pending or unknown Studio state. Review edits in Studio and save them; query `magic-cli exec ui.feedback` before retrying. No project was replaced or app closed."
        ));
    }
    Ok(())
}

pub fn new_request() -> String {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "studio-{}-{}-{}",
        std::process::id(),
        now(),
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
/// Reopening an already owned project focuses its existing window; it never reloads unsaved work.
pub fn reuse_open(app: &tauri::AppHandle, params: &Value) -> Result<Option<Value>, String> {
    let Some(raw) = params["path"].as_str() else {
        return Ok(None);
    };
    let target = magicstudio_engine::studio_sessions::canonical(std::path::Path::new(raw))?;
    let active = app
        .state::<Engine>()
        .with(None, |s| Ok(s.project_path.clone()))?;
    if active.as_deref() == Some(target.as_path())
        && app
            .state::<Feedback>()
            .0
            .lock()
            .map_err(|e| e.to_string())?["project"]
            .as_str()
            == target.to_str()
    {
        app.get_window("main")
            .ok_or("Studio window unavailable")?
            .set_focus()
            .map_err(|e| e.to_string())?;
        magicstudio_engine::activity::append(
            json!({"stage":"session_focused","command":"project.open","project":target,"request_id":params["_request_id"],"origin":params["_origin"]}),
        );
        return Ok(Some(
            json!({"ok":true,"project":target,"reused_session":true,"focused":true}),
        ));
    }
    if let Some(live) = magicstudio_engine::studio_sessions::for_project(&target)? {
        // This engine has opened the file but its frontend has not applied it yet. Returning
        // "focused existing session" here used to skip the actual document/name render.
        if live["identity"]["pid"].as_u64()==Some(std::process::id() as u64){return Ok(None);}
        #[cfg(unix)]
        {
            let endpoint = live["endpoint"]
                .as_str()
                .ok_or("Existing Studio endpoint unavailable")?
                .trim_start_matches("unix:");
            magicstudio_engine::studio_sessions::request(
                std::path::Path::new(endpoint),
                "ui.focus",
                json!({"_expected_project":target,"_expected_instance":live["instance"]}),
            )?;
            return Ok(Some(
                json!({"ok":true,"project":target,"reused_session":true,"focused":true,"instance":live["instance"]}),
            ));
        }
    }
    if active.is_some() {
        let exe=std::env::current_exe().map_err(|e|e.to_string())?;
        let mut opened=magicstudio_engine::studio_sessions::open(&exe,target.to_str())?;
        opened["reused_session"]=json!(true);
        opened["opened_new_instance"]=json!(true);
        return Ok(Some(opened));
    }

    Ok(None)
}
