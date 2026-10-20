//! Project lifecycle: `.acproj` create/open/save/close and connect/load/remove/update assets.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

/// Identifies an AnimationCraft project file (`.acproj`, JSON).
const PROJECT_FORMAT: &str = "animationcraft.project";

/// Writes the open project's asset connections to its `.acproj` file.
pub fn write_project_file(s: &Session) -> Result<()> {
    let Some(path) = &s.project_path else { return Ok(()) };
    let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("Untitled");
    let doc = json!({"format": PROJECT_FORMAT, "version": 1, "name": name, "assets": s.project_assets,
        "workshop": {"audio": s.song_audio, "draft_approved": s.draft_approved, "target": s.video_target}});
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&doc)
        .map_err(|e| EngineError::InvalidParams(format!("cannot encode project: {e}")))?;
    std::fs::write(path, text)?;
    Ok(())
}

/// Resolve and canonicalize a project path, appending `.acproj` when missing.
fn project_file_path(s: &Session, params: &Value) -> Result<PathBuf> {
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing project 'path'".into()))?;
    let mut path = super::workspace::resolve_workspace_path(s, raw);
    if path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("acproj")) {
        path.as_mut_os_string().push(".acproj");
    }
    Ok(path)
}

/// A fresh session for a project: no assets loaded, nothing active.
fn reset_for_project(s: &mut Session, path: PathBuf, assets: Vec<crate::ProjectAsset>) {
    *s = Session {
        cwd: s.cwd.clone(),
        blender_available: s.blender_available,
        project_assets: assets,
        project_path: Some(path),
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

/// Create a new `.acproj` session.
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
    reset_for_project(s, path.clone(), Vec::new());
    s.song_audio = audio;
    write_project_file(s)?;
    Ok(json!({"ok": true, "project": path, "assets": s.project_assets, "workshop": super::workshop::workshop_status(s)?}))
}

/// Open an existing `.acproj` session.
pub fn cmd_project_open(s: &mut Session, params: &Value) -> Result<Value> {
    let path = project_file_path(s, params)?;
    let text = std::fs::read_to_string(&path)?;
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| EngineError::InvalidParams(format!("invalid project file: {e}")))?;
    if doc.get("format").and_then(Value::as_str) != Some(PROJECT_FORMAT) {
        return Err(EngineError::InvalidParams(format!(
            "{} is not an AnimationCraft project",
            path.display()
        )));
    }
    let assets: Vec<crate::ProjectAsset> =
        serde_json::from_value(doc.get("assets").cloned().unwrap_or_else(|| json!([])))
            .map_err(|e| EngineError::InvalidParams(format!("invalid project assets: {e}")))?;
    // Report connections whose source files have moved instead of dropping them.
    let missing: Vec<&str> = assets
        .iter()
        .filter(|a| a.path.as_ref().is_some_and(|p| !p.exists()))
        .map(|a| a.id.as_str())
        .collect::<Vec<_>>();
    let missing = json!(missing);
    reset_for_project(s, path.clone(), assets);
    s.song_audio = doc.pointer("/workshop/audio").and_then(Value::as_str).map(PathBuf::from);
    s.draft_approved = doc.pointer("/workshop/draft_approved").and_then(Value::as_bool).unwrap_or(false);
    if let Some(target) = doc.pointer("/workshop/target").and_then(Value::as_str).filter(|t| super::workshop::VIDEO_TARGETS.contains(t)) {
        s.video_target = target.into();
    }
    Ok(json!({"ok": true, "project": path, "assets": s.project_assets, "missing": missing, "workshop": super::workshop::workshop_status(s)?}))
}

/// Save the open project (or save-as when `path` is given).
pub fn cmd_project_save(s: &mut Session, params: &Value) -> Result<Value> {
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
        blender_available: s.blender_available,
        ..Session::default()
    };
    Ok(json!({"ok": true}))
}

/// Connect an asset (by path or preset) to the open project.
pub fn cmd_project_add_asset(s: &mut Session, params: &Value) -> Result<Value> {
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
    if !["auto", "image", "poses", "turnaround", "cutout_rig"].contains(&kind) {
        return Err(EngineError::InvalidParams(
            "kind must be auto, image, poses, turnaround, or cutout_rig".into(),
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
                        ["png", "jpg", "jpeg", "webp"].contains(&e.to_ascii_lowercase().as_str())
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
            let (folder, files) = match (dest("character"), dest("background")) {
                (Some(c), None) => {
                    let folder = dir.join("assets/characters").join(&c);
                    (folder.clone(), folder.join("parts"))
                }
                (None, Some(b)) => {
                    let folder = dir.join("assets/backgrounds").join(&b);
                    (folder.clone(), folder)
                }
                _ => {
                    return Err(EngineError::InvalidParams(
                        "an image from outside the project needs one destination: \
                         character=<name> (copied to assets/characters/<name>/parts/) or \
                         background=<name> (copied to assets/backgrounds/<name>/)"
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
    });
    if load {
        cmd_project_load_asset(s, &json!({"id": id}))?;
    }
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
    // Failed decoding/reloading must leave the previous active asset and its rig intact.
    let mut staged = s.clone();
    if let Some(preset) = &entry.preset {
        let mut loader = Session {
            cwd: s.cwd.clone(),
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
            path.join("manifest.json")
        } else {
            path.clone()
        };
        if manifest_path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            let raw: Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)
                .map_err(|e| EngineError::InvalidParams(format!("invalid asset JSON: {e}")))?;
            if entry.kind == "cutout_rig"
                || raw.get("rig_type").is_some()
                || (raw.get("layers").is_some() && raw.get("canvas").is_some())
            {
                let mut plan = crate::session::LoadedCutoutRigPlan::load_from_path(&manifest_path)?;
                let base = manifest_path.parent().unwrap_or(Path::new("."));
                let resolve = |p: &Path| {
                    if p.is_absolute() {
                        p.to_path_buf()
                    } else {
                        base.join(p)
                    }
                };
                plan.source_image = resolve(&plan.source_image);
                plan.cutout_asset_sheet = plan.cutout_asset_sheet.as_deref().map(resolve);
                plan.photocraft_psd = plan.photocraft_psd.as_deref().map(resolve);
                for part in &mut plan.parts {
                    part.cropped_path = resolve(&part.cropped_path);
                    part.full_layer_path = resolve(&part.full_layer_path);
                }
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
    }
    *s = staged;
    Ok(json!({"ok": true, "asset_id": entry.id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "animationcraft_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn project_new_open_save_round_trip_keeps_connections() {
        let dir = temp_dir("project_round_trip");
        let project = dir.join("demo.acproj");
        image::RgbaImage::new(8, 8).save(dir.join("hero.png")).unwrap();
        let mut s = Session::default();
        s.cwd = dir.clone();
        s.execute("project.new", json!({"path": &project})).unwrap();
        s.execute(
            "project.add_asset",
            json!({"id": "hero", "path": dir.join("hero.png"), "name": "Hero"}),
        )
        .unwrap();
        assert_eq!(s.project_assets[0].name, "Hero");
        s.execute("project.save", json!({})).unwrap();
        let mut s2 = Session::default();
        s2.cwd = dir.clone();
        let res = s2.execute("project.open", json!({"path": &project})).unwrap();
        assert_eq!(res["assets"][0]["name"], "Hero");
        assert_eq!(s2.active_asset, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn outside_image_is_copied_to_its_destination_with_provenance() {
        let dir = temp_dir("project_copy_in");
        let outside = temp_dir("project_copy_src");
        image::RgbaImage::new(8, 8).save(outside.join("hero.png")).unwrap();
        let mut s = Session::default();
        s.cwd = dir.clone();
        s.execute("project.new", json!({"path": dir.join("demo.acproj")})).unwrap();
        let err = s.execute("project.add_asset", json!({"path": outside.join("hero.png")}));
        assert!(err.is_err(), "an outside image without a destination is refused");
        s.execute("project.add_asset", json!({"path": outside.join("hero.png"), "character": "hero"}))
            .unwrap();
        let copy = dir.join("assets/characters/hero/parts/hero.png");
        assert_eq!(s.project_assets[0].path.as_deref(), Some(copy.as_path()));
        assert!(copy.is_file() && outside.join("hero.png").is_file());
        let provenance: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("assets/characters/hero/provenance.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(provenance["files"][0]["source"], json!(outside.join("hero.png")));
        assert_eq!(provenance["files"][0]["file"], json!("characters/hero/parts/hero.png"));
        s.execute("project.add_asset", json!({"path": outside.join("hero.png"), "background": "sky"}))
            .unwrap();
        assert!(dir.join("assets/backgrounds/sky/hero.png").is_file());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn project_open_reports_missing_sources_instead_of_dropping() {
        let dir = temp_dir("project_missing");
        let mut s = Session::default();
        s.cwd = dir.clone();
        s.execute("project.new", json!({"path": dir.join("demo.acproj")})).unwrap();
        image::RgbaImage::new(8, 8).save(dir.join("gone.png")).unwrap();
        s.execute(
            "project.add_asset",
            json!({"id": "moved", "path": dir.join("gone.png")}),
        )
        .unwrap();
        s.execute("project.save", json!({})).unwrap();
        std::fs::remove_file(dir.join("gone.png")).unwrap();
        let mut s2 = Session::default();
        s2.cwd = dir.clone();
        let res = s2.execute("project.open", json!({"path": dir.join("demo.acproj")})).unwrap();
        assert!(res["missing"].as_array().unwrap().contains(&json!("moved")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
