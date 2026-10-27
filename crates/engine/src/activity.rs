//! Three recent root actions per canonical project. Polling never appends activity.
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};
thread_local! {static CONTEXT:RefCell<Value>=const{RefCell::new(Value::Null)};}
pub struct Context(Value);
impl Drop for Context {
    fn drop(&mut self) {
        CONTEXT.with(|v| *v.borrow_mut() = self.0.clone());
    }
}
pub fn enter(id: &str, params: &Value) -> Context {
    let previous = CONTEXT.with(|v| v.borrow().clone());
    let mut c = previous.clone();
    if !c.is_object() {
        c = json!({});
    }
    for key in ["_origin", "_client", "_request_id", "_control"] {
        if let Some(v) = params.get(key).filter(|v| !v.is_null()) {
            c[key] = v.clone();
        }
    }
    c["_command"] = json!(id);
    if c["_request_id"].is_null() {
        c["_request_id"] = json!(format!("{}-{}", std::process::id(), now()));
    }
    CONTEXT.with(|v| *v.borrow_mut() = c);
    Context(previous)
}
fn now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0)
}
fn path() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .map(|p| p.join("magicstudio/activity.jsonl"))
}
/// Saves, generations, keyframe edits, errors and page selection reports, kept apart from the
/// three-root journal so UI clicks cannot evict them.
fn history_path() -> Option<PathBuf> {
    path().map(|p| p.with_file_name("activity-history.jsonl"))
}
const HISTORY_LIMIT: usize = 512;
fn notable(event: &Value) -> bool {
    let stage = event["stage"].as_str().unwrap_or_default();
    matches!(stage, "saved_state" | "engine_error" | "handler_error" | "selection_changed" | "clip_switched" | "theatre_selection" | "theatre_saved" | "theatre_error" | "clip_sheet_started" | "clip_sheet_converted" | "clip_sheet_failed" | "theatre_keyframes")
        || (matches!(event["command"].as_str(), Some("image.generate" | "motion.path" | "motion.undo_path" | "motion.recipe" | "export.layered_motion" | "video.place_track"))
            && matches!(stage, "dispatch_running" | "command_result" | "engine_success" | "engine_needs_work"))
}

/// Page selection reports would each become a root action; they live only in the history.
fn history_only(event: &Value) -> bool {
    matches!(event["stage"].as_str(), Some("selection_changed" | "clip_switched" | "theatre_selection" | "clip_sheet_started" | "clip_sheet_converted" | "clip_sheet_failed" | "theatre_keyframes"))
}
fn read_rows(path: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
/// Called with the journal lock held, so sequences stay ordered across both files.
fn remember(path: &std::path::Path, event: &Value) -> std::io::Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(std::io::Error::other("activity history must not be a symlink"));
    }
    let mut rows = read_rows(path);
    rows.push(event.clone());
    let excess = rows.len().saturating_sub(HISTORY_LIMIT);
    let bytes = rows[excess..].iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n";
    fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
fn project(event: &Value) -> String {
    event["project"]
        .as_str()
        .map(|p| {
            fs::canonicalize(p)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| p.into())
        })
        .unwrap_or_else(|| "unassigned".into())
}
fn root(event: &Value) -> String {
    event["root_request_id"]
        .as_str()
        .or(event["request_id"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("legacy-{}", event["sequence"]))
}
/// Keep the three most recently triggered action groups independently for every project.
fn retain(rows: Vec<Value>) -> Vec<Value> {
    let mut groups = HashMap::<String, Vec<String>>::new();
    let mut assigned = Vec::<Value>::new();
    let mut destinations = HashMap::<String, String>::new();
    for event in &rows {
        if let Some(id) = event["request_id"].as_str() {
            let p = project(event);
            if p != "unassigned" {
                destinations.insert(id.into(), p);
            }
        }
    }
    for mut event in rows {
        if event["kind"] == "cursor" {
            continue;
        }
        if event["stage"] == "handler_dispatch" && event["request_id"].is_null() {
            continue;
        }
        if let Some(id) = event["request_id"].as_str() {
            if let Some(p) = destinations.get(id) {
                event["project"] = json!(p);
            }
        }
        if event["stage"] == "frontend_ack" && event["request_id"].is_null() {
            if let Some(prior) = assigned.iter().rev().find(|p| {
                p["command"] == event["command"]
                    && project(p) == project(&event)
                    && p["request_id"].is_string()
            }) {
                event["request_id"] = prior["request_id"].clone();
            }
        }
        let p = project(&event);
        event["project"] = if p == "unassigned" {
            Value::Null
        } else {
            json!(p)
        };
        let key = root(&event);
        event["root_request_id"] = json!(key);
        let roots = groups.entry(p).or_default();
        if !roots.contains(&key) {
            roots.push(key);
        }
        assigned.push(event);
    }
    let keep: HashMap<String, HashSet<String>> = groups
        .into_iter()
        .map(|(p, roots)| (p, roots.into_iter().rev().take(3).collect()))
        .collect();
    let mut counts = HashMap::<(String, String), usize>::new();
    let mut selected = Vec::new();
    for event in assigned.into_iter().rev() {
        let p = project(&event);
        let key = root(&event);
        if keep.get(&p).is_some_and(|v| v.contains(&key)) {
            let count = counts.entry((p, key)).or_default();
            *count += 1;
            if *count <= 64 {
                selected.push(event);
            }
        }
    }
    selected.reverse();
    selected
}
pub fn append(mut event: Value) {
    if event["stage"] == "handler_dispatch" && event["request_id"].is_null() {
        return;
    }
    let Some(path) = path() else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    let result = (|| -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(std::io::Error::other(
                "activity state must not be a symlink",
            ));
        }
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.lock()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        let mut rows: Vec<Value> = text
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let sequence = rows
            .iter()
            .filter_map(|v| v["sequence"].as_u64())
            .max()
            .unwrap_or(0)
            + 1;
        event["sequence"] = json!(sequence);
        event["unix_micros"] = json!(now());
        // Legacy acknowledgements can be associated only when command and project match a retained root.
        if event["request_id"].is_null() && event["stage"] == "frontend_ack" {
            if let Some(previous) = rows.iter().rev().find(|v| {
                v["command"] == event["command"]
                    && project(v) == project(&event)
                    && v["request_id"].is_string()
            }) {
                event["request_id"] = previous["request_id"].clone();
            }
        }
        if let Some(request) = event["request_id"].as_str() {
            let p = project(&event);
            if p != "unassigned" {
                for old in &mut rows {
                    if old["request_id"].as_str() == Some(request) && old["project"].is_null() {
                        old["project"] = event["project"].clone();
                    }
                }
            }
        }
        if let Some(history) = history_path().filter(|_| notable(&event)) {
            remember(&history, &event)?;
        }
        if !history_only(&event) {
            rows.push(event);
        }
        let mut rows = retain(rows);
        rows.push(json!({"kind":"cursor","sequence":sequence}));
        let bytes = rows
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        file.seek(SeekFrom::Start(0))?;
        file.write_all(bytes.as_bytes())?;
        file.set_len(bytes.len() as u64)?;
        file.sync_data()?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("activity receipt unavailable: {error}");
    }
}
pub fn command(
    project: Option<&std::path::Path>,
    id: &str,
    params: &Value,
    stage: &str,
    error: Option<&str>,
) {
    let context = CONTEXT.with(|v| v.borrow().clone());
    let mut bounded = json!({});
    for key in [
        "path",
        "root",
        "name",
        "audio",
        "transcript",
        "captions",
        "target",
        "resume",
        "replace",
        "replace_transcript",
    ] {
        if let Some(v) = params.get(key) {
            bounded[key] = if let Some(s) = v.as_str() {
                json!(s.chars().take(220).collect::<String>())
            } else {
                v.clone()
            };
        }
    }
    append(
        json!({"project":project,"project_id":project,"command_id":context["_request_id"],"command":id,"stage":stage,"origin":context["_origin"].as_str().unwrap_or("unspecified"),"client_label":context["_client"].as_str().unwrap_or("unspecified (unverified)"),"identity_status":"supplied label; not authentication","request_id":context["_request_id"],"control":context["_control"],"inputs":bounded,"error":error.map(|s|s.chars().take(350).collect::<String>()),"ui_ack":"not implied by engine completion"}),
    );
}
/// Measured output evidence supplements the existing started/completed lifecycle, never a UI ack.
pub fn result(project:Option<&std::path::Path>,id:&str,value:&Value,elapsed_ms:u128){
 let context=CONTEXT.with(|v|v.borrow().clone());
 let mut outputs=json!({});
 for key in ["ok","project","path","deck","pdf","output","files","intake_ready","video_ready","captions","observation","prerequisites","metrics","status","tasks","assessed","unassessed","scope_assessed","scope_unassessed","healthy","storyboard","storyboard_revision","assessed_at_unix_ms","diagnostic_only","request_id","session","native_message_id"] {
  if let Some(v)=value.get(key){let bytes=v.to_string();outputs[key]=if bytes.len()<=16000{v.clone()}else{json!({"summary":"output exceeds receipt bound; inspect command result","bytes":bytes.len()})};}
 }
 append(json!({"project":project,"project_id":project,"command":id,"command_id":context["_request_id"],"request_id":context["_request_id"],"stage":"command_result","elapsed_ms":elapsed_ms,"outputs":outputs,"ui_ack":"not implied by engine completion"}));
}
pub fn events(s: &mut crate::Session, p: &Value) -> crate::Result<Value> {
    let (text, history) = if let Some(path) = path() {
        if let Ok(mut file) = OpenOptions::new().read(true).open(path) {
            file.lock_shared()?;
            let mut text = String::new();
            file.read_to_string(&mut text)?;
            (text, history_path().map(|p| read_rows(&p)).unwrap_or_default())
        } else {
            (String::new(), Vec::new())
        }
    } else {
        (String::new(), Vec::new())
    };
    let all: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let global = all
        .iter()
        .filter_map(|v| v["sequence"].as_u64())
        .max()
        .unwrap_or(0);
    let rows = retain(all);
    let cursor = p["after"].as_u64().unwrap_or(0);
    let limit = p["limit"].as_u64().unwrap_or(100).clamp(1, 192) as usize;
    let selected = p["project"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            s.project_path
                .as_ref()
                .map(|v| v.to_string_lossy().into_owned())
        })
        .map(|v| project(&json!({"project":v})));
    let matching: Vec<Value> = rows
        .into_iter()
        .filter(|v| selected.as_ref().is_none_or(|p| project(v) == *p))
        .collect();
    let first = matching.first().and_then(|v| v["sequence"].as_u64());
    let mut actions = Vec::<Value>::new();
    for event in &matching {
        let key = root(event);
        if let Some(action) = actions
            .iter_mut()
            .find(|a| a["root_request_id"] == key && a["project"] == event["project"])
        {
            action["last_sequence"] = event["sequence"].clone();
            action["events"]
                .as_array_mut()
                .expect("action events")
                .push(event.clone());
        } else {
            actions.push(json!({"root_request_id":key,"project":event["project"],"first_sequence":event["sequence"],"last_sequence":event["sequence"],"events":[event]}));
        }
    }
    let history: Vec<Value> = history
        .into_iter()
        .filter(|v| selected.as_ref().is_none_or(|p| project(v) == *p))
        .filter(|v| v["sequence"].as_u64().unwrap_or(0) > cursor)
        .collect();
    let history = history[history.len().saturating_sub(limit)..].to_vec();
    let pending: Vec<Value> = matching
        .into_iter()
        .filter(|v| v["sequence"].as_u64().unwrap_or(0) > cursor)
        .collect();
    let events: Vec<Value> = pending.iter().take(limit).cloned().collect();
    let next = if pending.len() > events.len() {
        events
            .last()
            .and_then(|v| v["sequence"].as_u64())
            .unwrap_or(cursor)
    } else {
        global
    };
    Ok(
        json!({"ok":true,"events":events,"actions":actions,"next_cursor":next,"latest_global_cursor":global,"retained_from":first,"cursor_expired":cursor>global||first.is_some_and(|v|cursor>0&&cursor+1<v),"history":history,"project":selected,"retention":"three root actions per project; up to 64 recent causal stages per action; excess stages omitted; history keeps the last 512 saves, generations, keyframe edits, errors and page selection reports across projects","polling":"read-only snapshot; no actions consumed"}),
    )
}
pub fn stored(project: &std::path::Path, bytes: usize) {
    let c = CONTEXT.with(|v| v.borrow().clone());
    append(
        json!({"stage":"saved_state","project":project,"command":c["_command"],"request_id":c["_request_id"],"origin":c["_origin"],"client_label":c["_client"],"control":c["_control"],"storage":"project file","bytes":bytes}),
    );
}
