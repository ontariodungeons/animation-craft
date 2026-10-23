//! The existing Home navigation history, shared by UI, terminal and MCP. Not a media scan.
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
pub fn file() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("magicstudio/recent_projects.json"))
}
pub fn list() -> Vec<String> {
    file()
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn update(path: &str, forget: bool) -> Result<(), String> {
    let path = fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.into());
    let target = file().ok_or("Config directory unavailable")?;
    fs::create_dir_all(target.parent().ok_or("Config directory unavailable")?)
        .map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(target)
        .map_err(|e| e.to_string())?;
    file.lock().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let mut items: Vec<String> = serde_json::from_slice(&bytes).unwrap_or_default();
    items.retain(|p| {
        fs::canonicalize(p)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| p.clone())
            != path
    });
    // Never persist test/fixture or temp paths as the user's recent project.
    if path.contains("/.agent/") || path.contains("/tmp/") {
        return Ok(());
    }
    if !forget {
        items.insert(0, path);
        items.truncate(10);
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&items).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.set_len(bytes.len() as u64)
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn record(path: &str) -> Result<(), String> {
    update(path, false)
}
pub fn forget(path: &str) -> Result<(), String> {
    update(path, true)
}
pub fn catalog(s: &mut crate::Session, _: &Value) -> crate::Result<Value> {
    let discovery = crate::studio_sessions::discover();
    let mut sessions = discovery["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if s.project_owner.is_some() {
        if let Some(path) = &s.project_path {
            sessions.push(json!({"canonical_project":path,"identity":{"pid":std::process::id()},"unsaved_state":"unknown; inspect live UI feedback"}));
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut projects:Vec<Value>=list().into_iter().map(|p|fs::canonicalize(&p).map(|v|v.to_string_lossy().into_owned()).unwrap_or(p)).filter(|p|seen.insert(p.clone())).map(|p|{
  let path=Path::new(&p);let dir=path.parent().unwrap_or(path);let thumb=dir.join("storyboard/.preview/page-01.png");let owners:Vec<Value>=sessions.iter().filter(|s|s["canonical_project"].as_str()==Some(&p)).map(|s|json!({"endpoint":s["endpoint"],"instance":s["instance"],"pid":s["identity"]["pid"],"unsaved_state":s["unsaved_state"]})).collect();
  json!({"path":p,"project_id":p,"identity_kind":"canonical path","name":path.file_stem().and_then(|v|v.to_str()).unwrap_or(&p),"folder":dir,"exists":path.is_file(),"thumb":thumb.is_file().then(||thumb.to_string_lossy().into_owned()),"open_state":if owners.len()==1{"open"}else if owners.len()>1{"ambiguous"}else if discovery["errors"].as_array().is_some_and(|v|v.iter().any(|e|!e["error"].as_str().is_some_and(|s|s.starts_with("stale endpoint:")))){"unknown"}else{"not_observed_open"},"sessions":owners,"readiness":readiness(path)})
 }).collect();
    projects.sort_by_key(|v| v["exists"] != true);
    Ok(
        json!({"projects":projects,"source":file(),"scope":"existing Home navigation history; newest ten, not all filesystem projects","discovery_errors":discovery["errors"]}),
    )
}

fn readiness(path: &Path) -> Value {
    let mut inspected = crate::Session::default();
    let opened = match crate::project::cmd_project_open(
        &mut inspected,
        &json!({"path":path,"record_recent":false}),
    ) {
        Ok(v) => v,
        Err(error) => {
            return json!({"state":"unreadable","next_step":error.to_string(),"editor_ready":false});
        }
    };
    let control = crate::command_center::status(&mut inspected, &json!({})).unwrap_or(json!({}));
    let workshop = &opened["workshop"];
    let audio = workshop["stages"]
        .as_array()
        .is_some_and(|v| v.iter().any(|s| s["id"] == "song" && s["done"] == true));
    let transcript = control["transcript"]["valid"] == true;
    let editor_ready = workshop["video_project"]
        .as_str()
        .is_some_and(|p| Path::new(p).is_file())
        && workshop["video_server"]["running"] == true;
    let next = if !audio {
        "Choose existing song audio"
    } else if !transcript {
        "Import or review timestamped transcript"
    } else if !editor_ready {
        "Review project; editor setup or server is still required"
    } else {
        "Open the existing running video editor"
    };
    json!({"audio_available":audio,"transcript_valid":transcript,"editor_ready":editor_ready,"approval":"not inferred","next_step":next,"problems":control["problems"],"quick_actions":[{"label":"Project details","view":"project","open_block":"song","enabled":true},{"label":"Lyrics / transcript","view":"project","open_block":"lyrics","enabled":transcript},{"label":"Running video editor","view":"studio","enabled":editor_ready}]})
}
