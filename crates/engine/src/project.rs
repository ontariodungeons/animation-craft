//! Project lifecycle: `.magic` create/open/save/close and connect/load/remove/update assets.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

/// Identifies a Magic Studio project file (`.magic`, JSON).
const PROJECT_FORMAT: &str = "magicstudio.project";

fn saved_path(path: &Path, project: &Path) -> PathBuf {
    path.strip_prefix(project.parent().unwrap_or(Path::new("."))).unwrap_or(path).to_path_buf()
}

fn opened_path(path: &str, project: &Path) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() { path.to_path_buf() } else { project.parent().unwrap_or(Path::new(".")).join(path) }
}

/// The persisted project state, shared by Save and unsaved-change detection.
pub fn project_document(s: &Session) -> Value {
    let Some(path) = &s.project_path else { return Value::Null };
    let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("Untitled");
    // Loaded assets save their current part states; unloaded ones keep what was saved before.
    let assets: Vec<crate::ProjectAsset> = s
        .project_assets
        .iter()
        .map(|entry| {
            let mut entry = entry.clone();
            if let Some(loaded) = s.assets.get(&entry.id) {
                entry.parts = loaded.parts.clone();
            }
            entry
        })
        .collect();
    let mut doc = json!({"format": PROJECT_FORMAT, "version": 1, "name": name, "assets": assets,
        "workshop": {"audio": s.song_audio, "draft_approved": s.draft_approved, "target": s.video_target, "video_project": s.video_project_path}});
    if let Some(state) = &s.theatre_state { doc["theatre"] = state.clone(); }
    if !s.animation_sources.is_empty() { doc["animation_sources"] = json!(s.animation_sources); }
    normalize_connections(&mut doc, path);
    doc
}

fn normalize_connections(doc: &mut Value, project: &Path) {
    for asset in doc["assets"].as_array_mut().into_iter().flatten() {
        if let Some(path) = asset["path"].as_str() { asset["path"] = json!(saved_path(Path::new(path), project)); }
    }
    for key in ["audio", "video_project"] {
        if let Some(path) = doc["workshop"][key].as_str() { doc["workshop"][key] = json!(saved_path(Path::new(path), project)); }
    }
}

pub fn project_is_dirty(s: &Session, saved: &Value) -> bool {
    let mut saved = saved.clone();
    if let Some(path) = &s.project_path { normalize_connections(&mut saved, path); }
    let current = project_document(s);
    ["assets", "workshop", "theatre", "animation_sources"].iter().any(|key| saved[*key] != current[*key])
}

/// Writes the open project's asset connections to its `.magic` file.
pub fn write_project_file(s: &Session) -> Result<()> {
    let Some(path) = &s.project_path else { return Ok(()) };
    let doc = project_document(s);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&doc)
        .map_err(|e| EngineError::InvalidParams(format!("cannot encode project: {e}")))?;
    write_preserving_lease(path, text.as_bytes())?;
    s.project_saves.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if s.record_user_state{crate::activity::stored(path,text.len());}
    Ok(())
}

// Preserve the inode holding Studio's existing writer lease. Replacing it with a renamed
// temporary file would release exclusivity on the new path. Encoding precedes all disk writes;
// an ordinary write failure restores the old bytes. This is not a crash-atomic transaction.
fn write_preserving_lease(path: &Path, bytes: &[u8]) -> Result<()> {
    write_preserving_lease_using(path, bytes, store_project_bytes)
}
fn store_project_bytes(file: &mut std::fs::File, content: &[u8]) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(0))?;
    file.write_all(content)?;
    file.set_len(content.len() as u64)?;
    file.sync_data()
}
fn write_preserving_lease_using(path: &Path, bytes: &[u8], mut store: impl FnMut(&mut std::fs::File, &[u8]) -> std::io::Result<()>) -> Result<()> {
    use std::io::Read;
    let mut file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)?;
    if !file.metadata()?.is_file() { return Err(EngineError::InvalidParams("Project destination must be a regular file; no write performed".into())); }
    let mut before = Vec::new();
    file.read_to_end(&mut before)?;
    if before == bytes { return Ok(()); }
    if let Err(error) = store(&mut file, bytes) {
        if let Err(rollback) = store(&mut file, &before) {
            return Err(EngineError::Io(std::io::Error::other(format!("Project write failed: {error}; restoring prior bytes also failed: {rollback}. Preserve this file and repair storage before retrying"))));
        }
        return Err(EngineError::Io(std::io::Error::other(format!("Project write failed; prior bytes restored: {error}"))));
    }
    Ok(())
}

/// Resolve and canonicalize a project path, appending `.magic` when missing.
pub(crate) fn project_file_path(s: &Session, params: &Value) -> Result<PathBuf> {
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing project 'path'".into()))?;
    let mut path = super::workspace::resolve_workspace_path(s, raw);
    if path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("magic")) {
        path.as_mut_os_string().push(".magic");
    }
    if path.exists(){return std::fs::canonicalize(&path).map_err(EngineError::from);}
    Ok(path)
}

/// A fresh session for a project: no assets loaded, nothing active.
fn reset_for_project(s: &mut Session, path: PathBuf, assets: Vec<crate::ProjectAsset>) {
    let owner=if s.project_path.as_ref()==Some(&path){s.project_owner.clone()}else{None};
    *s = Session {
        cwd: s.cwd.clone(),
        record_user_state:s.record_user_state,
        agent_selection:s.agent_selection.clone(),
        blender_available: s.blender_available,
        project_assets: assets,
        project_path: Some(path),
        project_owner: owner,
        ..Session::default()
    };
}

/// Position of a connected project asset by id.
fn project_index(s: &Session, params: &Value) -> Result<usize> {
    let id = params
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing project asset 'id'".into()))?;
    s.project_assets
        .iter()
        .position(|a| a.id == id)
        .ok_or_else(|| EngineError::InvalidParams(format!("project asset '{id}' is not connected")))
}

/// List connected project assets and the active one.
pub fn cmd_project_list_assets(s: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({"assets": s.project_assets, "active_asset": s.active_asset, "project": s.project_path}))
}

// Navigation history is optional global state; a valid project remains created/open if it is unavailable.
fn recent_warning(s:&Session,path:&Path,requested:bool)->Vec<Value>{
    if !s.record_user_state||!requested{return Vec::new();}
    match crate::project_catalog::record(&path.to_string_lossy()){
        Ok(())=>Vec::new(),
        Err(error)=>vec![json!({"code":"recent_history_unavailable","stage":"update_recent_projects","path":crate::project_catalog::file(),"error":error,"project":path,"effect":"Project is available; Home recent history was not updated","next_action":"Continue using the returned project path; repair only the named navigation-history location if needed"})],
    }
}

/// Migrate one legacy `.acproj` to `.magic`: rename the file and write the current format
/// inside it. Explicit, reversible (the old name is reported) and never touches other files.
pub fn cmd_project_migrate(_s:&mut Session,p:&Value)->Result<Value>{
    let raw=p["path"].as_str().ok_or_else(||EngineError::InvalidParams("Give path=<legacy .acproj file>".into()))?;
    let source=std::path::PathBuf::from(raw);
    if !source.is_file(){return Err(EngineError::InvalidParams(format!("{} is not a file",source.display())));}
    if source.extension().is_some_and(|e|e.eq_ignore_ascii_case("magic")){return Err(EngineError::InvalidParams("already a .magic project".into())); }
    if !source.extension().is_some_and(|e|e.eq_ignore_ascii_case("acproj")){return Err(EngineError::InvalidParams("only a legacy .acproj can be migrated".into()));}
    let target=source.with_extension("magic");
    if target.exists(){return Err(EngineError::InvalidParams(format!("{} already exists; nothing was changed",target.display())));}
    let mut doc:Value=serde_json::from_slice(&std::fs::read(&source)?).map_err(|e|EngineError::InvalidParams(format!("not a project file: {e}")))?;
    let before=doc["format"].as_str().unwrap_or_default().to_string();
    doc["format"]=json!(PROJECT_FORMAT);
    std::fs::rename(&source,&target)?;
    std::fs::write(&target,serde_json::to_vec_pretty(&doc).map_err(|_|EngineError::InvalidParams("cannot encode the migrated project".into()))?)?;
    Ok(json!({"ok":true,"from":source,"to":target,"format_before":before,"format_after":PROJECT_FORMAT,"note":"The legacy file name is gone; rename it back to restore the old one."}))
}

/// Create a new `.magic` session.
pub fn cmd_project_new(s: &mut Session, params: &Value) -> Result<Value> {
    let path = project_file_path(s, params)?;
    let overwrite = params.get("overwrite").and_then(Value::as_bool).unwrap_or(false);
    if path.exists() && !overwrite {
        return Err(EngineError::InvalidParams(format!(
            "project already exists: {} (pass \"overwrite\": true to replace it)",
            path.display()
        )));
    }
    let audio = match params.get("audio").and_then(Value::as_str) {
        Some(raw) => Some(super::workspace::song_audio_path(s, raw)?),
        None => None,
    };
    if let Some(parent)=path.parent(){std::fs::create_dir_all(parent)?;}
    let owner=if path.exists(){claim(s,&path)?}else{
        let file=std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&path)?;
        file.try_lock().map_err(|e|EngineError::InvalidParams(format!("Cannot own new project: {e}")))?;
        std::sync::Arc::new(file)
    };
    reset_for_project(s, path.clone(), Vec::new());
    s.project_owner=Some(owner);
    s.song_audio = audio;
    write_project_file(s)?;
    let warnings=recent_warning(s,&path,true);
    Ok(json!({"ok": true, "project": path, "warnings":warnings,"assets": s.project_assets, "workshop": super::workshop::workshop_status(s)?}))
}

/// Open an existing `.magic` session.
pub fn cmd_project_open(s: &mut Session, params: &Value) -> Result<Value> {
    let path = project_file_path(s, params)?;
    let owner=if params["claim_owner"]==true {Some(claim(s,&path)?)} else {None};
    let text = std::fs::read_to_string(&path)?;
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| EngineError::InvalidParams(format!("invalid project file: {e}")))?;
    if doc.get("format").and_then(Value::as_str) == Some("magicstudio.character") {
        let char_id = doc.get("id").and_then(Value::as_str).unwrap_or("character").to_string();
        for anc in path.ancestors().skip(1).take(5) {
            if let Ok(entries) = std::fs::read_dir(anc) {
                for entry in entries.flatten() {
                    let ep = entry.path();
                    if ep != path && ep.extension().is_some_and(|e| e.eq_ignore_ascii_case("magic"))
                        && let Ok(txt) = std::fs::read_to_string(&ep)
                        && let Ok(pdoc) = serde_json::from_str::<Value>(&txt)
                        && pdoc.get("format").and_then(Value::as_str) == Some(PROJECT_FORMAT)
                    {
                        let mut p2 = params.clone();
                        p2["path"] = json!(ep);
                        let mut res = cmd_project_open(s, &p2)?;
                        let _ = cmd_project_load_asset(s, &json!({"id": char_id}));
                        res["active_asset"] = json!(s.active_asset);
                        return Ok(res);
                    }
                }
            }
        }
    }
    if doc.get("format").and_then(Value::as_str)!=Some(PROJECT_FORMAT) {
        return Err(EngineError::InvalidParams(format!(
            "{} is not a Magic Studio project",
            path.display()
        )));
    }
    let mut assets: Vec<crate::ProjectAsset> =
        serde_json::from_value(doc.get("assets").cloned().unwrap_or_else(|| json!([])))
            .map_err(|e| EngineError::InvalidParams(format!("invalid project assets: {e}")))?;
    for asset in &mut assets {
        asset.path = asset.path.as_ref().map(|source| if source.is_absolute() { source.clone() } else { path.parent().unwrap().join(source) });
    }
    let animation_sources = serde_json::from_value(doc.get("animation_sources").filter(|v|!v.is_null()).cloned().unwrap_or_else(||json!({})))
        .map_err(|e|EngineError::InvalidParams(format!("invalid animation sources: {e}")))?;
    // Report connections whose source files have moved instead of dropping them.
    let missing: Vec<&str> = assets
        .iter()
        .filter(|a| a.path.as_ref().is_some_and(|p| !p.exists()))
        .map(|a| a.id.as_str())
        .collect::<Vec<_>>();
    let missing = json!(missing);
    reset_for_project(s, path.clone(), assets);
    if let Some(owner)=owner{s.project_owner=Some(owner);}
    s.theatre_state = doc.get("theatre").filter(|state| !state.is_null()).cloned();
    s.animation_sources = animation_sources;
    s.song_audio = doc.pointer("/workshop/audio").and_then(Value::as_str).map(|source| opened_path(source, &path));
    s.draft_approved = doc.pointer("/workshop/draft_approved").and_then(Value::as_bool).unwrap_or(false);
    if let Some(target) = doc.pointer("/workshop/target").and_then(Value::as_str).filter(|t| super::workshop::VIDEO_TARGETS.contains(t)) {
        s.video_target = target.into();
    }
    s.video_project_path = doc.pointer("/workshop/video_project").and_then(Value::as_str).map(|source| opened_path(source, &path));
    let warnings=recent_warning(s,&path,params["record_recent"]!=false);
    Ok(json!({"ok": true, "project": path,"warnings":warnings,"assets": s.project_assets, "missing": missing, "workshop": super::workshop::workshop_status(s)?}))
}

/// Save the open project (or save-as when `path` is given).
pub fn cmd_project_save(s: &mut Session, params: &Value) -> Result<Value> {
    if params.get("expected_project").is_some(){crate::motion_authoring::project_guard(s,params)?;}
    if params.get("path").is_some() {
        s.project_path = Some(project_file_path(s, params)?);
    }
    if s.project_path.is_none() {
        return Err(EngineError::InvalidParams("no project is open; use project.new".into()));
    }
    write_project_file(s)?;
    Ok(json!({"ok": true, "project": s.project_path}))
}

/// Close the project and clear session state.
pub fn cmd_project_close(s: &mut Session, _: &Value) -> Result<Value> {
    *s = Session {
        cwd: s.cwd.clone(),
        record_user_state:s.record_user_state,
        agent_selection:s.agent_selection.clone(),
        blender_available: s.blender_available,
        ..Session::default()
    };
    Ok(json!({"ok": true}))
}

/// Ensure a default isolated-assets `.magic` project is open when working on an isolated asset.
pub fn ensure_default_asset_project(s: &mut Session) -> Result<PathBuf> {
    if let Some(existing) = &s.project_path {
        return Ok(existing.clone());
    }
    let root = super::workspace::workspace_root(s).join("exports/isolated_assets");
    std::fs::create_dir_all(&root)?;
    let default_proj = root.join("isolated_assets.magic");
    if default_proj.is_file() {
        let _ = cmd_project_open(s, &json!({"path": default_proj, "record_recent": false}));
    } else {
        let _ = cmd_project_new(s, &json!({"path": default_proj, "overwrite": true}));
    }
    s.project_path
        .clone()
        .ok_or_else(|| EngineError::InvalidParams("could not initialize default asset project".into()))
}

/// Connect an asset (by path or preset) to the open project.
pub fn cmd_project_add_asset(s: &mut Session, params: &Value) -> Result<Value> {
    if s.project_path.is_none()
        && (params.get("isolated").and_then(Value::as_bool) == Some(true)
            || params.get("use_default_project").and_then(Value::as_bool) == Some(true))
    {
        let _ = ensure_default_asset_project(s)?;
    }
    let preset = params
        .get("preset")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let path = params
        .get("path")
        .or_else(|| params.get("manifest"))
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p));
    if preset.is_some() == path.is_some() {
        return Err(EngineError::InvalidParams(
            "provide exactly one of 'path' or 'preset'".into(),
        ));
    }
    if let Some(path) = &path
        && !path.exists()
    {
        return Err(EngineError::InvalidParams(format!(
            "asset path does not exist: {}",
            path.display()
        )));
    }
    let kind = params.get("kind").and_then(Value::as_str).unwrap_or("auto");
    if !["auto", "image", "poses", "turnaround", "cutout_rig", "character"].contains(&kind) {
        return Err(EngineError::InvalidParams(
            "kind must be auto, image, poses, turnaround, cutout_rig, or character".into(),
        ));
    }
    let stem = preset
        .as_deref()
        .or_else(|| path.as_ref()?.file_stem()?.to_str())
        .unwrap_or("asset");
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(stem)
        .trim();
    if name.is_empty() {
        return Err(EngineError::InvalidParams(
            "asset name cannot be empty".into(),
        ));
    }
    let if_not_exists = params
        .get("if_not_exists")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let load = params.get("load").and_then(Value::as_bool).unwrap_or(false);
    if if_not_exists
        && let Some(existing_id) = s
            .project_assets
            .iter()
            .find(|a| (path.is_some() && a.path == path) || (preset.is_some() && a.preset == preset))
            .map(|a| a.id.clone())
    {
        if load {
            cmd_project_load_asset(s, &json!({"id": existing_id}))?;
        }
        return Ok(json!({"ok": true, "asset_id": existing_id, "loaded": load, "assets": s.project_assets}));
    }
    let explicit_id = params
        .get("id")
        .and_then(Value::as_str)
        .and_then(super::workspace::try_sanitize_asset_id);
    let base_id = explicit_id
        .clone()
        .or_else(|| super::workspace::try_sanitize_asset_id(name))
        .unwrap_or_else(|| super::workspace::sanitize_asset_id(stem));
    // With a project open, an image from outside it is copied to its destination in the workshop
    // layout: a character's parts or a background. No destination, no copy: the asset is refused.
    let copy_into = match s.project_path.as_ref().and_then(|p| p.parent()) {
        Some(dir)
            if path.as_ref().is_some_and(|p| {
                !p.starts_with(dir)
                    && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        ["png", "jpg", "jpeg", "webp", "svg", "gif", "bmp", "tif", "tiff", "exr"].contains(&e.to_ascii_lowercase().as_str())
                    })
            }) =>
        {
            let dest = |key: &str| {
                params
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    // A plain folder name: nothing that could climb out of assets/.
                    .filter(|n| !n.is_empty() && !n.contains(['/', '\\']) && *n != "." && *n != "..")
                    .map(str::to_owned)
            };
            let (folder, files) = match (dest("character"), dest("background"), dest("image")) {
                (Some(c), None, None) => {
                    let folder = dir.join("assets/characters").join(&c);
                    (folder.clone(), folder.join("parts"))
                }
                (None, Some(b), None) => {
                    let folder = dir.join("assets/backgrounds").join(&b);
                    (folder.clone(), folder)
                }
                (None, None, Some(name)) => {
                    let folder=dir.join("assets/images").join(name);(folder.clone(),folder)
                }
                _ => {
                    return Err(EngineError::InvalidParams(
                        "an image from outside the project needs one destination: \
                         character=<name> (copied to assets/characters/<name>/parts/) or \
                         background=<name> (copied to assets/backgrounds/<name>/) or image=<name> (copied to assets/images/<name>/)"
                            .into(),
                    ));
                }
            };
            Some((folder, files))
        }
        _ => None,
    };
    let mut id = base_id.clone();
    let mut suffix = 2;
    while s.project_assets.iter().any(|a| a.id == id) || s.assets.contains_key(&id) {
        if explicit_id.is_some() {
            return Err(EngineError::InvalidParams(format!(
                "asset id '{id}' already exists"
            )));
        }
        id = format!("{base_id}_{suffix}");
        suffix += 1;
    }
    let mut stored = path.clone();
    if let (Some((folder, files)), Some(source)) = (&copy_into, &path) {
        let copy = files.join(source.file_name().unwrap_or_default());
        if copy.exists() {
            return Err(EngineError::InvalidParams(format!(
                "{} is already in the project; connect that file instead",
                copy.display()
            )));
        }
        std::fs::create_dir_all(files)?;
        std::fs::copy(source, &copy)?;
        // provenance.json beside the destination lists where every copied file came from.
        let meta = std::fs::metadata(source)?;
        let unix = |t: std::time::SystemTime| {
            t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
        };
        let provenance = folder.join("provenance.json");
        let mut doc: Value = std::fs::read_to_string(&provenance)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| json!({}));
        if !doc["files"].is_array() {
            doc["files"] = json!([]);
        }
        let assets_dir = folder.ancestors().nth(2).unwrap_or(folder);
        doc["files"].as_array_mut().unwrap().push(json!({
            "file": copy.strip_prefix(assets_dir).unwrap_or(&copy), "source": source, "bytes": meta.len(),
            "modified_unix": meta.modified().map(unix).unwrap_or(0),
            "copied_unix": unix(std::time::SystemTime::now())}));
        std::fs::write(&provenance, format!("{doc:#}\n"))?;
        stored = Some(copy);
    }
    s.project_assets.push(crate::ProjectAsset {
        id: id.clone(),
        name: name.into(),
        preset,
        path: stored.clone(),
        kind: kind.into(),
        parts: Vec::new(),
    });
    if load {
        cmd_project_load_asset(s, &json!({"id": id}))?;
    }
    if params["save"].as_bool()==Some(true) { cmd_project_save(s,&json!({}))?; }
    Ok(json!({"ok": true, "asset_id": id, "loaded": load, "assets": s.project_assets,
        "copied_to": copy_into.is_some().then_some(&stored)}))
}

/// Remove a connected project asset and unload it.
pub fn cmd_project_remove_asset(s: &mut Session, params: &Value) -> Result<Value> {
    let index = project_index(s, params)?;
    let entry = s.project_assets.remove(index);
    s.assets.remove(&entry.id);
    if s.active_asset.as_deref() == Some(&entry.id) {
        s.active_asset = s
            .project_assets
            .iter()
            .find(|e| s.assets.contains_key(&e.id))
            .map(|e| e.id.clone());
    }
    Ok(json!({"ok": true, "removed": entry.id, "active_asset": s.active_asset}))
}

/// Rename or reorder a connected project asset.
pub fn cmd_project_update_asset(s: &mut Session, params: &Value) -> Result<Value> {
    let index = project_index(s, params)?;
    let name = params
        .get("name")
        .map(|v| {
            v.as_str()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .ok_or_else(|| EngineError::InvalidParams("asset name cannot be empty".into()))
        })
        .transpose()?;
    let target = params
        .get("index")
        .map(|v| {
            v.as_u64()
                .filter(|i| *i < s.project_assets.len() as u64)
                .map(|i| i as usize)
                .ok_or_else(|| EngineError::InvalidParams("asset index out of range".into()))
        })
        .transpose()?;
    if let Some(name) = name {
        s.project_assets[index].name = name.into();
    }
    if let Some(target) = target {
        let entry = s.project_assets.remove(index);
        s.project_assets.insert(target, entry);
    }
    cmd_project_list_assets(s, params)
}

/// Load a connected project asset into the session (staged so failures preserve the previous asset).
pub fn cmd_project_load_asset(s: &mut Session, params: &Value) -> Result<Value> {
    let entry = s.project_assets[project_index(s, params)?].clone();
    let reload = params
        .get("reload")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if s.assets.contains_key(&entry.id) && !reload {
        s.active_asset = Some(entry.id.clone());
        return Ok(json!({"ok": true, "asset_id": entry.id}));
    }
    let first_load = !s.assets.contains_key(&entry.id);
    // Failed decoding/reloading must leave the previous active asset and its rig intact.
    let theatre_state = s.theatre_state.take();
    let last_result = s.last_result.take();
    let mut staged = s.clone();
    s.theatre_state = theatre_state;
    s.last_result = last_result;
    if let Some(preset) = &entry.preset {
        let mut loader = Session {
            cwd: s.cwd.clone(),
        record_user_state:s.record_user_state,
        agent_selection:s.agent_selection.clone(),
            ..Session::default()
        };
        crate::photocraft::cmd_asset_load_manifest(&mut loader, &json!({"preset": preset}))?;
        let loaded_id = loader
            .active_asset
            .clone()
            .ok_or_else(|| EngineError::InvalidParams("preset did not load an asset".into()))?;
        let mut asset = loader
            .assets
            .remove(&loaded_id)
            .ok_or_else(|| EngineError::InvalidParams("preset asset missing".into()))?;
        if let Some(previous) = staged.assets.get(&entry.id)
            && asset.cutout_rig.is_none()
        {
            asset.cutout_rig = previous.cutout_rig.clone();
        }
        asset.id = entry.id.clone();
        staged.assets.insert(entry.id.clone(), asset);
    } else if let Some(path) = &entry.path {
        let manifest_path = if path.is_dir() {
            let char_magic = path.join("character.magic");
            if char_magic.is_file() {
                char_magic
            } else {
                path.join("manifest.json")
            }
        } else {
            path.clone()
        };
        if manifest_path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json") || e.eq_ignore_ascii_case("magic"))
        {
            let raw: Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)
                .map_err(|e| EngineError::InvalidParams(format!("invalid asset JSON: {e}")))?;
            if raw.get("format").and_then(Value::as_str) == Some("magicstudio.character") {
                let char_dir = manifest_path.parent().unwrap_or(Path::new("."));
                let proj_root = staged
                    .project_path
                    .as_deref()
                    .and_then(Path::parent)
                    .unwrap_or(char_dir);
                let resolve_rel = |rel_str: &str| -> PathBuf {
                    let p = PathBuf::from(rel_str);
                    if p.is_absolute() {
                        p
                    } else {
                        let from_proj = proj_root.join(&p);
                        if from_proj.exists() {
                            from_proj
                        } else {
                            char_dir.join(p)
                        }
                    }
                };
                let active_angle = raw
                    .get("active_angle")
                    .and_then(Value::as_str)
                    .unwrap_or("three_quarter_right");
                let rig_path = raw
                    .pointer(&format!("/angles/{active_angle}/rig"))
                    .and_then(Value::as_str)
                    .or_else(|| raw.pointer(&format!("/rigs/{active_angle}")).and_then(Value::as_str))
                    .or_else(|| raw.pointer("/rigs/active").and_then(Value::as_str))
                    .map(resolve_rel)
                    .unwrap_or_else(|| char_dir.join("rig.json"));
                let mut plan = crate::session::LoadedCutoutRigPlan::load_from_path(&rig_path)?;
                let rig_base = rig_path.parent().unwrap_or(char_dir);
                if plan.source_image.as_os_str().is_empty() {
                    plan.source_image = rig_base.to_path_buf();
                } else if !plan.source_image.is_absolute() {
                    let from_proj = proj_root.join(&plan.source_image);
                    if from_proj.is_file() {
                        plan.source_image = from_proj;
                    }
                }
                plan.resolve_paths(rig_base);
                let image = image::open(&plan.source_image)?;
                let mut asset = staged.assets.remove(&entry.id).unwrap_or_default();
                asset.id = entry.id.clone();
                asset.path = plan.source_image.clone();
                asset.width = image.width();
                asset.height = image.height();
                asset.has_alpha = image.color().has_alpha();
                asset.attached_asset_sheet = raw
                    .pointer("/sheets/poses")
                    .and_then(Value::as_str)
                    .map(resolve_rel)
                    .filter(|p| p.is_file())
                    .or_else(|| plan.cutout_asset_sheet.clone());
                asset.attached_turnaround_sheet = raw
                    .pointer("/sheets/turnaround")
                    .and_then(Value::as_str)
                    .map(resolve_rel)
                    .filter(|p| p.is_file());
                asset.cutout_rig = Some(plan);
                staged.assets.insert(entry.id.clone(), asset);
            } else if entry.kind == "cutout_rig"
                || raw.get("rig_type").is_some()
                || (raw.get("layers").is_some() && raw.get("canvas").is_some())
            {
                let mut plan = crate::session::LoadedCutoutRigPlan::load_from_path(&manifest_path)?;
                let base = manifest_path.parent().unwrap_or(Path::new("."));
                if plan.source_image.as_os_str().is_empty() {
                    plan.source_image = base.to_path_buf();
                }
                plan.resolve_paths(base);
                let image = image::open(&plan.source_image)?;
                let mut asset = staged.assets.remove(&entry.id).unwrap_or_default();
                asset.id = entry.id.clone();
                asset.path = plan.source_image.clone();
                asset.width = image.width();
                asset.height = image.height();
                asset.has_alpha = image.color().has_alpha();
                asset.attached_asset_sheet = plan.cutout_asset_sheet.clone();
                asset.cutout_rig = Some(plan);
                staged.assets.insert(entry.id.clone(), asset);
            } else {
                if raw.get("frames").and_then(Value::as_array).is_none() {
                    return Err(EngineError::InvalidParams(
                        "expected a pose/turnaround manifest or cutout rig plan".into(),
                    ));
                }
                crate::photocraft::cmd_asset_load_manifest(
                    &mut staged,
                    &json!({"asset": entry.id, "manifest": manifest_path}),
                )?;
            }
        } else {
            let image = image::open(path)?;
            let mut asset = staged.assets.remove(&entry.id).unwrap_or_default();
            asset.id = entry.id.clone();
            asset.path = path.clone();
            asset.width = image.width();
            asset.height = image.height();
            asset.has_alpha = image.color().has_alpha();
            match entry.kind.as_str() {
                "poses" => asset.attached_asset_sheet = Some(path.clone()),
                "turnaround" => asset.attached_turnaround_sheet = Some(path.clone()),
                _ => {}
            }
            staged.assets.insert(entry.id.clone(), asset);
        }
    }
    staged.active_asset = Some(entry.id.clone());
    if let Some(asset) = staged.current_asset_mut() {
        asset.sync_parts_from_manifests();
        // A reload keeps the session's part states; a first load restores the saved ones.
        if first_load {
            for saved in &entry.parts {
                if let Some(part) = asset.parts.iter_mut().find(|p| p.name == saved.name) {
                    (part.visible, part.solo, part.verified) = (saved.visible, saved.solo, saved.verified);
                }
            }
        }
    }
    staged.theatre_state = s.theatre_state.take();
    staged.last_result = s.last_result.take();
    *s = staged;
    Ok(json!({"ok": true, "asset_id": entry.id}))
}


/// Canonical paths identify existing projects without changing their file format.
pub fn cmd_project_identity(s:&mut Session,p:&Value)->Result<Value>{
 let path=project_file_path(s,p)?;let path=std::fs::canonicalize(path)?;
 Ok(json!({"ok":true,"project":path,"project_id":path,"identity_kind":"canonical path"}))
}
pub(crate) fn claim(s:&Session,path:&Path)->Result<std::sync::Arc<std::fs::File>>{
 if s.project_path.as_deref()==Some(path){if let Some(owner)=&s.project_owner{return Ok(owner.clone());}}
 let file=std::fs::OpenOptions::new().read(true).write(true).open(path)?;
 file.try_lock().map_err(|_|EngineError::InvalidParams(format!("Project is owned by another live writer: {}. Inspect project.catalog and attach to its existing Studio; no competing write performed.",path.display())))?;
 Ok(std::sync::Arc::new(file))
}

/// Attach disk metadata without a UI switch, writer lease, action receipt or navigation-history change.
pub fn cmd_project_attach(s:&mut Session,p:&Value)->Result<Value>{
 let mut p=p.clone();p["record_recent"]=json!(false);p["claim_owner"]=json!(false);
 let mut value=cmd_project_open(s,&p)?;value["attachment"]=json!("read-only disk metadata; Studio unchanged");Ok(value)
}
