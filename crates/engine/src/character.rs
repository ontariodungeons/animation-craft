//! Character folder (`assets/characters/<id>/character.magic`) retrieval, import, and angle/pose configuration.

use crate::{EngineError, Result};
use crate::session::Session;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn invalid(msg: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(msg.into())
}

pub fn find_character_magic_path(
    s: &Session,
    id: &str,
    project_root: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(pa) = s.project_assets.iter().find(|a| a.id == id)
        && let Some(p) = &pa.path
    {
        if p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("magic")) {
            return Some(p.clone());
        }
        let cand = p.parent()?.join("character.magic");
        if cand.is_file() {
            return Some(cand);
        }
    }
    let cand = project_root?.join("assets/characters").join(id).join("character.magic");
    cand.is_file().then_some(cand)
}

fn resolve_rel(base_root: &Path, char_dir: &Path, rel: &str) -> String {
    let p = PathBuf::from(rel);
    if p.is_absolute() {
        return p.to_string_lossy().into_owned();
    }
    let from_root = base_root.join(&p);
    let resolved = if from_root.exists() { from_root } else { char_dir.join(p) };
    resolved.to_string_lossy().into_owned()
}

fn resolve_character_map_paths(doc: &mut Value, base_root: &Path, char_dir: &Path) {
    if let Some(sheets) = doc.get_mut("sheets").and_then(Value::as_object_mut) {
        for val in sheets.values_mut() {
            if let Some(s_str) = val.as_str().map(str::to_owned) {
                *val = json!(resolve_rel(base_root, char_dir, &s_str));
            }
        }
    }
    if let Some(angles) = doc.get_mut("angles").and_then(Value::as_object_mut) {
        for a_val in angles.values_mut() {
            if let Some(img) = a_val.get("image").and_then(Value::as_str).map(str::to_owned) {
                a_val["image_path"] = json!(resolve_rel(base_root, char_dir, &img));
            }
            if let Some(rig_p) = a_val.get("rig").and_then(Value::as_str).map(str::to_owned) {
                a_val["rig_path"] = json!(resolve_rel(base_root, char_dir, &rig_p));
            }
        }
    }
    if let Some(poses) = doc.get_mut("poses").and_then(Value::as_object_mut) {
        for p_val in poses.values_mut() {
            if let Some(img) = p_val.get("image").and_then(Value::as_str).map(str::to_owned) {
                p_val["image_path"] = json!(resolve_rel(base_root, char_dir, &img));
            }
        }
    }
}

pub fn load_character_config(s: &Session, id: &str, project_root: Option<&Path>) -> Value {
    let Some(cfg_path) = find_character_magic_path(s, id, project_root) else {
        return Value::Null;
    };
    let Ok(text) = std::fs::read_to_string(&cfg_path) else {
        return Value::Null;
    };
    let Ok(mut doc) = serde_json::from_str::<Value>(&text) else {
        return Value::Null;
    };
    let char_dir = cfg_path.parent().unwrap_or(Path::new("."));
    let base_root = project_root.unwrap_or(char_dir);
    doc["config_path"] = json!(cfg_path);
    doc["folder_path"] = json!(char_dir);
    if let Some(ref_str) = doc.get("reference").and_then(Value::as_str).map(str::to_owned) {
        doc["reference_path"] = json!(resolve_rel(base_root, char_dir, &ref_str));
    }
    resolve_character_map_paths(&mut doc, base_root, char_dir);
    doc
}

pub(crate) fn animation_panel(cfg: &Value) -> Value {
    json!({"available":cfg.is_object(),"name":cfg["name"],"active_angle":cfg["active_angle"].as_str().unwrap_or("front"),
        "direction":cfg["direction"].as_str().unwrap_or("right"),"sheets":cfg["sheets"],
        "angles":cfg["angles"].as_object().map(|m| m.values().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "poses":cfg["poses"].as_object().map(|m| m.values().cloned().map(|mut p| {
            let id = p["id"].as_str().unwrap_or("").to_owned();
            p["track_ids"] = json!([id, format!("@{id}")]);
            p
        }).collect::<Vec<_>>()).unwrap_or_default()})
}

pub fn cmd_character_assets(s: &mut Session, params: &Value) -> Result<Value> {
    let project_root = s
        .project_path
        .as_deref()
        .and_then(Path::parent)
        .and_then(|path| std::fs::canonicalize(path).ok());
    let id = params["character"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| s.active_asset.clone())
        .or_else(|| s.project_assets.first().map(|a| a.id.clone()))
        .ok_or_else(|| invalid("Specify character=<id>"))?;
    let cfg = load_character_config(s, &id, project_root.as_deref());
    Ok(json!({
        "ok": true,
        "character": id,
        "character_panel": animation_panel(&cfg),
        "character_config": cfg,
    }))
}

fn target_dest(char_dir: &Path, role: &str, fname: &str) -> PathBuf {
    match role {
        "turnaround" => char_dir.join("sheets/turnaround.png"),
        "poses" => char_dir.join("sheets/poses.png"),
        "skeleton" => char_dir.join("sheets/skeleton.png"),
        "reference" => char_dir.join("reference.png"),
        _ => char_dir.join("drafts").join(fname),
    }
}

fn update_character_manifest(
    cfg_path: &Path,
    proj_dir: &Path,
    id: &str,
    role: &str,
    dest: &Path,
    draft_copy: &Path,
    source: &Path,
) -> Result<()> {
    let mut doc: Value = if cfg_path.is_file() {
        serde_json::from_str(&std::fs::read_to_string(cfg_path)?).unwrap_or_else(|_| json!({}))
    } else {
        json!({
            "format": "magicstudio.character",
            "schemaVersion": 1,
            "id": id,
            "name": id,
            "folder": format!("assets/characters/{id}"),
        })
    };
    let rel_dest = dest.strip_prefix(proj_dir).unwrap_or(dest).to_string_lossy().into_owned();
    let rel_draft = draft_copy.strip_prefix(proj_dir).unwrap_or(draft_copy).to_string_lossy().into_owned();
    doc["original_draft"] = json!(rel_draft);
    if !doc["drafts"].is_array() {
        doc["drafts"] = json!([]);
    }
    if let Some(arr) = doc["drafts"].as_array_mut() {
        arr.push(json!({"role": role, "file": rel_dest, "draft": rel_draft, "source": source}));
    }
    if matches!(role, "turnaround" | "poses" | "skeleton") {
        doc["sheets"][role] = json!(rel_dest);
    } else if role == "reference" || doc.get("reference").is_none() {
        doc["reference"] = json!(rel_dest);
    }
    let encoded = serde_json::to_string_pretty(&doc).map_err(|e| invalid(e.to_string()))?;
    std::fs::write(cfg_path, encoded)?;
    Ok(())
}

fn parse_import_params(s: &Session, params: &Value) -> Result<(String, PathBuf, String)> {
    let id = params
        .get("character")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .map(crate::workspace::sanitize_asset_id)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| invalid("Specify character=<id>"))?;
    let raw_path = params
        .get("path")
        .or_else(|| params.get("file"))
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            let proj_dir = crate::workshop::project_dir(s).ok()?;
            let cand = proj_dir.join("assets/characters").join(&id).join("reference.png");
            cand.is_file().then(|| cand.to_string_lossy().into_owned())
        })
        .ok_or_else(|| invalid("Specify path=<image_or_sheet>"))?;
    let role = params
        .get("role")
        .or_else(|| params.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("original_draft")
        .to_owned();
    let source = crate::workspace::resolve_workspace_path(s, &raw_path);
    if !source.is_file() {
        return Err(invalid(format!("Import file does not exist: {}", source.display())));
    }
    Ok((id, source, role))
}

pub fn cmd_character_import(s: &mut Session, params: &Value) -> Result<Value> {
    let (id, source, role) = parse_import_params(s, params)?;
    let proj_dir = crate::workshop::project_dir(s)?;
    let char_dir = proj_dir.join("assets/characters").join(&id);
    std::fs::create_dir_all(char_dir.join("drafts"))?;
    std::fs::create_dir_all(char_dir.join("sheets"))?;

    let fname = source.file_name().and_then(|n| n.to_str()).unwrap_or("imported.png");
    let dest = target_dest(&char_dir, &role, fname);
    if source != dest {
        std::fs::copy(&source, &dest)?;
    }
    let draft_copy = char_dir.join("drafts").join(fname);
    if source != draft_copy && dest != draft_copy {
        let _ = std::fs::copy(&source, &draft_copy);
    }
    let cfg_path = char_dir.join("character.magic");
    update_character_manifest(&cfg_path, &proj_dir, &id, &role, &dest, &draft_copy, &source)?;

    if !s.project_assets.iter().any(|a| a.id == id) {
        let _ = crate::project::cmd_project_add_asset(
            s,
            &json!({"id": id, "name": id, "path": cfg_path, "kind": "character", "load": true, "save": true}),
        );
    } else {
        let _ = crate::project::cmd_project_load_asset(s, &json!({"id": id, "reload": true}));
    }
    Ok(json!({
        "ok": true,
        "character": id,
        "role": role,
        "stored": dest,
        "original_draft": draft_copy,
        "character_magic": cfg_path,
    }))
}

fn apply_pose_and_angle_params(doc: &mut Value, params: &Value) {
    if let Some(pose) = params.get("pose").and_then(Value::as_str).filter(|p| !p.is_empty()) {
        doc["active_pose"] = json!(pose);
        if params.get("angle").is_none()
            && let Some(pa) = doc.pointer(&format!("/poses/{pose}/angle")).and_then(Value::as_str)
        {
            doc["active_angle"] = json!(pa.to_owned());
        }
        if params.get("direction").is_none()
            && let Some(pd) = doc.pointer(&format!("/poses/{pose}/direction")).and_then(Value::as_str)
        {
            doc["direction"] = json!(pd.to_owned());
        }
    }
    if let Some(angle) = params.get("angle").and_then(Value::as_str).filter(|a| !a.is_empty()) {
        doc["active_angle"] = json!(angle);
        if params.get("direction").is_none() {
            let dir = if angle.contains("left") { "left" } else { "right" };
            doc["direction"] = json!(dir);
        }
    }
    if let Some(dir) = params.get("direction").and_then(Value::as_str).filter(|d| !d.is_empty()) {
        doc["direction"] = json!(dir);
        if params.get("angle").is_none() && params.get("pose").is_none() {
            let next = if dir == "left" { "three_quarter_left" } else if dir == "right" { "three_quarter_right" } else { "front" };
            doc["active_angle"] = json!(next);
        }
    }
}

fn sync_angle_rig_files(char_dir: &Path, active_angle: &str) {
    let rig_angle_dir = char_dir.join("rigs").join(active_angle);
    let rig_src = rig_angle_dir.join("rig.json");
    if !rig_src.is_file() {
        return;
    }
    let _ = std::fs::copy(&rig_src, char_dir.join("rig.json"));
    let src_layers = rig_angle_dir.join("canvas_layers");
    let dst_layers = char_dir.join("canvas_layers");
    if let Ok(entries) = std::fs::read_dir(&src_layers) {
        let _ = std::fs::create_dir_all(&dst_layers);
        for entry in entries.flatten().filter(|e| e.path().is_file()) {
            let _ = std::fs::copy(entry.path(), dst_layers.join(entry.file_name()));
        }
    }
    let _ = std::fs::remove_dir_all(char_dir.join(".layered_preview_cache"));
    let _ = std::fs::remove_dir_all(rig_angle_dir.join(".layered_preview_cache"));
}

pub fn cmd_character_configure(s: &mut Session, params: &Value) -> Result<Value> {
    let project_root = s
        .project_path
        .as_deref()
        .and_then(Path::parent)
        .and_then(|path| std::fs::canonicalize(path).ok());
    let id = params["character"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| s.active_asset.clone())
        .or_else(|| s.project_assets.first().map(|a| a.id.clone()))
        .ok_or_else(|| invalid("Specify character=<id>"))?;
    let cfg_path = find_character_magic_path(s, &id, project_root.as_deref())
        .ok_or_else(|| invalid(format!("Character '{id}' does not have a character.magic file")))?;
    if !s.project_assets.iter().any(|a| a.id == id) {
        let _ = crate::project::cmd_project_add_asset(
            s,
            &json!({"id": id, "name": id, "path": cfg_path, "kind": "cutout_rig", "load": false, "save": true}),
        );
    }
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&cfg_path)?)
        .map_err(|e| invalid(format!("Invalid character.magic JSON: {e}")))?;

    apply_pose_and_angle_params(&mut doc, params);

    let char_dir = cfg_path.parent().unwrap_or(Path::new("."));
    let encoded = serde_json::to_string_pretty(&doc).map_err(|e| invalid(e.to_string()))?;
    std::fs::write(&cfg_path, &encoded)?;
    let named_cfg = char_dir.join(format!("{id}.character.magic"));
    if named_cfg.is_file() {
        let _ = std::fs::write(&named_cfg, &encoded);
    }
    let active_angle = doc.get("active_angle").and_then(Value::as_str).unwrap_or("three_quarter_right");
    sync_angle_rig_files(char_dir, active_angle);

    crate::project::cmd_project_load_asset(s, &json!({"id": id, "reload": true}))?;
    crate::motion::cmd_motion_tracks(s, &json!({"character": id}))
}

pub fn preflight_rig_summary(rig: &crate::session::LoadedCutoutRigPlan) -> Value {
    let parts_ok = rig.parts.len() >= 6;
    let checks = vec![
        json!({"id": "multi_part_rig", "label": "Segmented into >= 6 articulated parts", "ok": parts_ok, "detail": rig.parts.len()}),
    ];
    json!({
        "ready": parts_ok,
        "passed_count": usize::from(parts_ok),
        "total_checks": 1,
        "failures": if parts_ok { Vec::<String>::new() } else { vec!["Segmented into >= 6 articulated parts".to_owned()] },
        "checks": checks,
    })
}

pub fn theatre_part_key_times(sheet: &Value, part_id: &str) -> Vec<f64> {
    let Some(track_data) = sheet
        .pointer("/sequence/tracksByObject")
        .and_then(|o| o.get(part_id))
        .and_then(|p| p.get("trackData"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    track_data
        .values()
        .filter_map(|tr| tr.get("keyframes").and_then(Value::as_array))
        .flatten()
        .filter_map(|k| k.get("position").and_then(Value::as_f64))
        .collect()
}

fn find_rotation_track<'a>(sheet: &'a Value, part_id: &str) -> Option<&'a Vec<Value>> {
    let part_obj = sheet.pointer("/sequence/tracksByObject")?.get(part_id)?;
    let track_data = part_obj.get("trackData")?.as_object()?;
    let tid = part_obj
        .get("trackIdByPropPath")
        .and_then(|m| m.get("[\"rotation\"]").or_else(|| m.get("rotation")))
        .and_then(Value::as_str);
    let tr = tid
        .and_then(|id| track_data.get(id))
        .or_else(|| track_data.get("rotation"))
        .or_else(|| track_data.values().next())?;
    tr.get("keyframes")?.as_array()
}

pub fn eval_theatre_rotation_at(sheet: &Value, part_id: &str, t: f64) -> f64 {
    if let Some(keyframes) = find_rotation_track(sheet, part_id) {
        let mut pts: Vec<(f64, f64)> = keyframes
            .iter()
            .filter_map(|k| Some((k["position"].as_f64()?, k["value"].as_f64()?)))
            .collect();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some(&(t_first, v_first)) = pts.first() {
            if t <= t_first {
                return v_first;
            }
            for w in pts.windows(2) {
                let (t0, v0) = w[0];
                let (t1, v1) = w[1];
                if t <= t1 {
                    let u = if t1 > t0 { ((t - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { 1.0 };
                    let s = u * u * (3.0 - 2.0 * u);
                    return v0 + (v1 - v0) * s;
                }
            }
            return pts.last().map(|p| p.1).unwrap_or(0.0);
        }
    }
    sheet
        .pointer("/staticOverrides/byObject")
        .and_then(|o| o.get(part_id))
        .and_then(|p| p.get("rotation"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}
