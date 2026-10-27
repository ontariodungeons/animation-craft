//! Guarded saved-rig edits and sampled PNG poses. No alternative motion evaluator.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
fn bad(s: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(s.into())
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad(format!("{k} is required")))
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn bytes(p: &Path) -> Result<Vec<u8>> {
    read_limited(p, 16 * 1024 * 1024)
}
fn read_limited(p: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut b = vec![];
    fs::File::open(p)?.take(limit + 1).read_to_end(&mut b)?;
    if b.len() as u64 > limit {
        return Err(bad("Authoring metadata exceeds read limit"));
    }
    Ok(b)
}
fn stamp() -> Result<String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| bad(e.to_string()))?
        .as_nanos()
        .to_string())
}
pub(crate) fn project_guard(s: &Session, p: &Value) -> Result<PathBuf> {
    let selected = s
        .project_path
        .as_ref()
        .ok_or_else(|| bad("Select the intended project"))?
        .canonicalize()?;
    let expected =
        crate::project::project_file_path(s, &json!({"path":text(p,"expected_project")?}))?
            .canonicalize()?;
    if selected != expected {
        return Err(bad("expected_project differs from selected project"));
    }
    Ok(selected)
}
// Mounting Theatre can create empty native sheets before any saved or visible edit.
// Only recognized empty containers normalize to null; entries (including zero-valued
// overrides and generated rotation seeds) are real state and must never be dropped.
fn empty_theatre(state: &Value) -> bool {
    if state.is_null() {
        return true;
    }
    if state["definitionVersion"] != "0.4.0" {
        return false;
    }
    let Some(root) = state.as_object() else {
        return false;
    };
    if root
        .keys()
        .any(|k| !["definitionVersion", "revisionHistory", "sheetsById"].contains(&k.as_str()))
    {
        return false;
    }
    let Some(sheets) = state["sheetsById"].as_object() else {
        return false;
    };
    sheets.values().all(|sheet| {
        let Some(fields) = sheet.as_object() else {
            return false;
        };
        if fields
            .keys()
            .any(|k| !["sequence", "staticOverrides"].contains(&k.as_str()))
        {
            return false;
        }
        if let Some(overrides) = sheet.get("staticOverrides") {
            let Some(object) = overrides.as_object() else {
                return false;
            };
            if object.keys().any(|k| k != "byObject")
                || !overrides["byObject"]
                    .as_object()
                    .is_some_and(|o| o.is_empty())
            {
                return false;
            }
        }
        if let Some(sequence) = sheet.get("sequence") {
            let Some(object) = sequence.as_object() else {
                return false;
            };
            if object.keys().any(|k| {
                !["type", "length", "subUnitsPerUnit", "tracksByObject"].contains(&k.as_str())
            }) || !sequence["tracksByObject"]
                .as_object()
                .is_some_and(|o| o.is_empty())
            {
                return false;
            }
        }
        true
    })
}
fn same_theatre(a: &Value, b: &Value) -> bool {
    a == b || (empty_theatre(a) && empty_theatre(b))
}

pub(crate) fn theatre_guard(s: &Session, _character: &str, _track: Option<&str>) -> Result<()> {
    let disk: Value = serde_json::from_slice(&bytes(
        s.project_path.as_ref().ok_or_else(|| bad("No project"))?,
    )?)
    .map_err(|e| bad(e.to_string()))?;
    let saved = s.theatre_state.as_ref().unwrap_or(&Value::Null);
    if !same_theatre(&disk["theatre"], saved) {
        return Err(bad(
            "Theatre state changed on disk; refresh before authoring",
        ));
    }
    if !saved.is_null() && saved["definitionVersion"] != "0.4.0" {
        return Err(bad("Unsupported saved Theatre state"));
    }
    Ok(())
}
struct Edit {
    project: PathBuf,
    rig: PathBuf,
    before: Vec<u8>,
    raw: Value,
    character: String,
    _lock: fs::File,
}
fn begin(s: &Session, p: &Value) -> Result<Edit> {
    let project = project_guard(s, p)?;
    let character = text(p, "character")?.to_owned();
    let entry = s
        .project_assets
        .iter()
        .find(|a| a.id == character)
        .ok_or_else(|| bad("Character is not connected"))?;
    let rig = entry
        .path
        .as_ref()
        .ok_or_else(|| bad("Saved rig required"))?
        .canonicalize()?;
    if !rig.starts_with(
        project
            .parent()
            .ok_or_else(|| bad("No project directory"))?,
    ) {
        return Err(bad("Authoring requires a project-local saved rig"));
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(rig.with_extension("authoring.lock"))?;
    lock.try_lock()
        .map_err(|_| bad("Another authoring command holds this rig; retry"))?;
    let before = bytes(&rig)?;
    if digest(&before) != text(p, "expected_rig_sha256")? {
        return Err(bad("Rig changed; refresh motion.tracks before editing"));
    }
    let raw: Value = serde_json::from_slice(&before).map_err(|e| bad(e.to_string()))?;
    if !raw["layers"].is_array() || !raw["tracks"].is_array() {
        return Err(bad("Only saved layered-motion rigs support authoring"));
    }
    Ok(Edit {
        project,
        rig,
        before,
        raw,
        character,
        _lock: lock,
    })
}
fn number(v: &Value, min: f64, max: f64, name: &str) -> Result<f64> {
    v.as_f64()
        .filter(|v| v.is_finite() && *v >= min && *v <= max)
        .ok_or_else(|| bad(format!("{name} must be finite within {min}..{max}")))
}
fn pair(v: &Value, min: f64, max: f64, name: &str) -> Result<Value> {
    let a = v
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or_else(|| bad(format!("{name} needs two numbers")))?;
    Ok(json!([
        number(&a[0], min, max, name)?,
        number(&a[1], min, max, name)?
    ]))
}
fn ensure_track_index(s: &Session, edit: &mut Edit, p: &Value) -> Result<(usize, String)> {
    let id = text(p, "track")?;
    if !id.starts_with('@') {
        let idx = edit.raw["tracks"]
            .as_array()
            .and_then(|a| a.iter().position(|v| v["id"] == id))
            .ok_or_else(|| bad("Authored action not found"))?;
        return Ok((idx, id.to_owned()));
    }
    let promoted_id = id.trim_start_matches('@').to_owned();
    if let Some(idx) = edit.raw["tracks"]
        .as_array()
        .and_then(|a| a.iter().position(|v| v["id"] == promoted_id))
    {
        return Ok((idx, promoted_id));
    }
    let asset = s
        .assets
        .get(&edit.character)
        .ok_or_else(|| bad("Load character before promoting a standard action"))?;
    let rig = crate::motion::preview_rig(asset)?;
    let std_tracks = crate::motion::standard_tracks(&rig);
    let mut template = std_tracks
        .into_iter()
        .find(|t| t["id"].as_str() == Some(id))
        .ok_or_else(|| bad(format!("Standard action `{id}` not found")))?;
    template["id"] = json!(promoted_id);
    if let Some(obj) = template.as_object_mut() {
        obj.remove("standard");
    }
    let tracks = edit.raw["tracks"]
        .as_array_mut()
        .ok_or_else(|| bad("Rig has no tracks array"))?;
    tracks.push(template);
    Ok((tracks.len() - 1, promoted_id))
}
fn track_index(raw: &Value, p: &Value) -> Result<usize> {
    let id = text(p, "track")?;
    if id.starts_with('@') {
        return Err(bad(
            "Standard actions are read-only; choose a saved authored action",
        ));
    }
    raw["tracks"]
        .as_array()
        .and_then(|a| a.iter().position(|v| v["id"] == id))
        .ok_or_else(|| bad("Authored action not found"))
}
fn history(edit: &Edit) -> Result<PathBuf> {
    let parent = edit.rig.parent().ok_or_else(|| bad("No rig directory"))?;
    Ok(parent
        .join(".motion-authoring")
        .join(digest(edit.rig.to_string_lossy().as_bytes())))
}
fn invalidate(s: &mut Session, character: &str) {
    if let Some(asset) = s.assets.remove(character)
        && let Some(entry) = s.project_assets.iter_mut().find(|a| a.id == character)
    {
        entry.parts = asset.parts;
    }
}
fn publish(s: &mut Session, edit: &Edit, after: Vec<u8>) -> Result<Value> {
    if bytes(&edit.rig)? != edit.before {
        return Err(bad("Rig changed during edit; nothing replaced"));
    }
    let dir = history(edit)?;
    fs::create_dir_all(&dir)?;
    let token = stamp()?;
    let receipt = dir.join(format!("{token}.json"));
    fs::write(&receipt,json!({"project":edit.project,"rig":edit.rig,"character":edit.character,"before":String::from_utf8(edit.before.clone()).map_err(|e|bad(e.to_string()))?,"after_sha256":digest(&after)}).to_string())?;
    let temporary = dir.join(format!("{token}.tmp"));
    fs::write(&temporary, &after)?;
    fs::rename(&temporary, &edit.rig)?;
    // Invalidate only this character. Readback reloads it using the normal loader.
    invalidate(s, &edit.character);
    Ok(
        json!({"ok":true,"character":edit.character,"spec":edit.rig,"rig_sha256":digest(&after),"undo":token,"undo_receipt":receipt,"rig_saved":true,"project_saved":false}),
    )
}
pub fn part_set(s: &mut Session, p: &Value) -> Result<Value> {
    part_edit(s, p, false)
}
pub fn part_reset(s: &mut Session, p: &Value) -> Result<Value> {
    part_edit(s, p, true)
}
fn part_edit(s: &mut Session, p: &Value, reset: bool) -> Result<Value> {
    let mut edit = begin(s, p)?;
    let (track, track_id) = ensure_track_index(s, &mut edit, p)?;
    let part = text(p, "part")?;
    let layer = edit.raw["layers"]
        .as_array()
        .and_then(|a| a.iter().position(|v| v["id"] == part))
        .ok_or_else(|| bad("Part not found"))?;
    let fields: Vec<String> = if reset {
        p["fields"]
            .as_array()
            .ok_or_else(|| bad("reset requires explicit fields"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| bad("fields must be strings"))
            })
            .collect::<Result<_>>()?
    } else {
        ["position", "scale", "rotation", "pivot"]
            .into_iter()
            .filter(|k| p.get(*k).is_some())
            .map(str::to_owned)
            .collect()
    };
    if fields.is_empty()
        || fields.len() > 4
        || fields
            .iter()
            .any(|k| !["position", "scale", "rotation", "pivot"].contains(&k.as_str()))
    {
        return Err(bad("Choose position, scale, rotation or pivot"));
    }
    theatre_guard(
        s,
        &edit.character,
        if fields.iter().any(|f| f == "pivot") {
            None
        } else {
            Some(text(p, "track")?)
        },
    )?;
    for field in fields {
        if field == "pivot" {
            let l = &mut edit.raw["layers"][layer];
            if reset {
                if let Some(original) = l.get("authoring_original_pivot").cloned() {
                    l["pivot"] = original;
                }
            } else {
                let value = pair(&p["pivot"], -65536., 65536., "pivot")?;
                if l.get("authoring_original_pivot").is_none() {
                    l["authoring_original_pivot"] = l["pivot"].clone();
                }
                l["pivot"] = value;
            }
        } else {
            let t = &mut edit.raw["tracks"][track];
            if !t["motion"].is_object() {
                t["motion"] = json!({});
            }
            if !t["motion"][part].is_object() {
                t["motion"][part] = json!({});
            }
            if reset {
                if let Some(o) = t["motion"][part].as_object_mut() {
                    o.remove(&field);
                }
            } else {
                let value = match field.as_str() {
                    "position" => pair(&p[&field], -65536., 65536., "position")?,
                    "scale" => {
                        let v = pair(&p[&field], 1., 1000., "scale")?;
                        json!([
                            v[0].as_f64().unwrap_or(100.) - 100.,
                            v[1].as_f64().unwrap_or(100.) - 100.
                        ])
                    }
                    _ => json!(number(&p[&field], -3600., 3600., "rotation")?),
                };
                t["motion"][part][&field] =
                    json!([{"kind":"keys","keys":[[0,value]],"ease":"linear"}]);
            }
        }
    }
    let after = serde_json::to_vec_pretty(&edit.raw).map_err(|e| bad(e.to_string()))?;
    let mut res = publish(s, &edit, after)?;
    res["track"] = json!(track_id);
    Ok(res)
}
/// Global planar stacking, independent of transform parenting.
pub fn layers_set(s: &mut Session, p: &Value) -> Result<Value> {
    let mut edit = begin(s, p)?;
    theatre_guard(s, &edit.character, None)?;
    let layers = edit.raw["layers"]
        .as_array()
        .ok_or_else(|| bad("Ordered layered rig required"))?;
    if layers.is_empty() || layers.len() > 1024 {
        return Err(bad("Layer count must be 1..1024"));
    }
    let mut ids: Vec<String> = layers
        .iter()
        .map(|v| text(v, "id").map(str::to_owned))
        .collect::<Result<_>>()?;
    let unique: std::collections::BTreeSet<_> = ids.iter().cloned().collect();
    if unique.len() != ids.len() {
        return Err(bad("Rig has duplicate part IDs"));
    }
    if let Some(order) = p.get("order") {
        if p.get("depth").is_some() || p.get("part").is_some() {
            return Err(bad("Pass order or part+depth, not both"));
        }
        let requested: Vec<String> = order
            .as_array()
            .ok_or_else(|| bad("order must be a full back-to-front ID array"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| bad("order entries must be IDs"))
            })
            .collect::<Result<_>>()?;
        let set: std::collections::BTreeSet<_> = requested.iter().cloned().collect();
        if requested.len() != ids.len() || set != unique {
            return Err(bad("order must contain every part exactly once"));
        }
        ids = requested;
    } else {
        let part = text(p, "part")?;
        let depth = p["depth"]
            .as_u64()
            .filter(|v| (*v as usize) < ids.len())
            .ok_or_else(|| bad("depth must be a zero-based stack index"))?
            as usize;
        let index = ids
            .iter()
            .position(|id| id == part)
            .ok_or_else(|| bad("Part not found"))?;
        let id = ids.remove(index);
        ids.insert(depth, id);
    }
    let reordered: Vec<Value> = ids
        .iter()
        .map(|id| {
            layers
                .iter()
                .find(|v| v["id"] == *id)
                .cloned()
                .ok_or_else(|| bad("Part disappeared"))
        })
        .collect::<Result<_>>()?;
    edit.raw["layers"] = json!(reordered);
    let after = serde_json::to_vec_pretty(&edit.raw).map_err(|e| bad(e.to_string()))?;
    let mut result = publish(s, &edit, after)?;
    result["layer_order"] = json!(ids);
    result["order_semantics"] = json!("back_to_front_zero_based");
    Ok(result)
}

pub(crate) fn layer_capabilities(s: &Session, character: &str, spec: &Path, raw: &Value) -> Value {
    let local = spec
        .canonicalize()
        .ok()
        .zip(
            s.project_path
                .as_ref()
                .and_then(|p| p.parent())
                .and_then(|p| p.canonicalize().ok()),
        )
        .is_some_and(|(p, root)| p.starts_with(root));
    let reason = if !local || !raw["layers"].is_array() {
        Some("A project-local ordered layered rig is required".to_string())
    } else {
        theatre_guard(s, character, None)
            .err()
            .map(|e| e.to_string())
    };
    json!({"supported":reason.is_none(),"reason":reason,"semantics":"back_to_front_zero_based","hierarchy_changed":false,"physical_z":false})
}

pub fn action_set(s: &mut Session, p: &Value) -> Result<Value> {
    let mut edit = begin(s, p)?;
    let (i, track_id) = ensure_track_index(s, &mut edit, p)?;
    let track = &mut edit.raw["tracks"][i];
    let old_duration = track["duration"].as_f64().unwrap_or(1.0).max(0.1);
    let mut new_duration_opt = None;
    let mut changed = p["track"].as_str().is_some_and(|t| t.starts_with('@'));
    if let Some(label) = p.get("label") {
        let label = label
            .as_str()
            .filter(|v| !v.trim().is_empty() && v.len() <= 120)
            .ok_or_else(|| bad("label must be 1..120 bytes"))?;
        track["label"] = json!(label);
        changed = true;
    }
    if let Some(duration) = p.get("duration") {
        let duration = number(duration, 0.1, 600., "duration")?;
        let retime = p["retime"].as_bool().unwrap_or(true);
        let ratio = if (old_duration - duration).abs() > 1e-9 && old_duration > 0.0 {
            Some(duration / old_duration)
        } else {
            None
        };
        for channels in track["motion"]
            .as_object_mut()
            .into_iter()
            .flatten()
            .map(|(_, v)| v)
        {
            for terms in channels.as_object_mut().into_iter().flatten().map(|(_, v)| v) {
                for term in terms.as_array_mut().into_iter().flatten() {
                    if let Some(keys) = term["keys"].as_array_mut() {
                        if let Some(r) = ratio && retime {
                            for k in keys.iter_mut() {
                                if let Some(t) = k[0].as_f64() {
                                    k[0] = json!((t * r * 1000.0).round() / 1000.0);
                                }
                            }
                        } else if keys.iter().any(|k| k[0].as_f64().is_some_and(|t| t > duration)) {
                            return Err(bad(
                                "Duration would exclude existing keys; retime them first",
                            ));
                        }
                    }
                    if let Some(r) = ratio && retime && term["kind"] == "ellipse" {
                        if let Some(from) = term["from"].as_f64() {
                            term["from"] = json!((from * r * 1000.0).round() / 1000.0);
                        }
                        if let Some(to) = term["to"].as_f64() {
                            term["to"] = json!((to * r * 1000.0).round() / 1000.0);
                        }
                    }
                }
            }
        }
        track["duration"] = json!(duration);
        new_duration_opt = Some(duration);
        changed = true;
    }
    if let Some(value) = p.get("loop") {
        track["loop"] = json!(value.as_bool().ok_or_else(|| bad("loop must be boolean"))?);
        changed = true;
    }
    if !changed {
        return Err(bad("Supply label, duration or loop"));
    }
    let after = serde_json::to_vec_pretty(&edit.raw).map_err(|e| bad(e.to_string()))?;
    let mut res = publish(s, &edit, after)?;
    res["track"] = json!(track_id);
    if let Some(new_dur) = new_duration_opt {
        let mut theatre_updated = false;
        if let Some(state) = s.theatre_state.as_mut()
            && let Some(sheets) = state["sheetsById"].as_object_mut()
        {
            let sheet_id = crate::theatre::clip_sheet_id(&edit.character, &track_id);
            if let Some(sheet) = sheets.get_mut(&sheet_id) {
                let old_seq_len = sheet["sequence"]["length"].as_f64().unwrap_or(old_duration).max(0.1);
                let ratio = new_dur / old_seq_len;
                sheet["sequence"]["length"] = json!(new_dur);
                if (ratio - 1.0).abs() > 1e-6
                    && let Some(objs) = sheet["sequence"]["tracksByObject"].as_object_mut()
                {
                    for (_, obj) in objs {
                        if let Some(tdata) = obj["trackData"].as_object_mut() {
                            for (_, tr) in tdata {
                                if let Some(kfs) = tr["keyframes"].as_array_mut() {
                                    for kf in kfs {
                                        if let Some(pos) = kf["position"].as_f64() {
                                            kf["position"] = json!((pos * ratio * 1000.0).round() / 1000.0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                theatre_updated = true;
            }
        }
        if theatre_updated {
            let _ = crate::project::write_project_file(s);
            res["theatre_synced"] = json!(true);
        }
        if p["sync_video"].as_bool().unwrap_or(true) {
            if let Ok(synced) = crate::video_target::sync_placed_tracks_duration(
                s,
                &edit.character,
                &track_id,
                new_dur,
            ) {
                res["video_tracks_synced"] = synced;
            }
        }
    }
    Ok(res)
}
pub fn undo(s: &mut Session, p: &Value) -> Result<Value> {
    let edit = begin(s, p)?;
    theatre_guard(s, &edit.character, None)?;
    let token = text(p, "undo")?;
    if !token.bytes().all(|v| v.is_ascii_digit()) {
        return Err(bad("Invalid undo token"));
    }
    let receipt: Value = serde_json::from_slice(&read_limited(
        &history(&edit)?.join(format!("{token}.json")),
        128 * 1024 * 1024,
    )?)
    .map_err(|e| bad(e.to_string()))?;
    if receipt["project"] != json!(edit.project)
        || receipt["rig"] != json!(edit.rig)
        || receipt["character"] != edit.character
        || receipt["after_sha256"] != digest(&edit.before)
    {
        return Err(bad(
            "Undo token does not match this project, character and current rig",
        ));
    }
    publish(s, &edit, text(&receipt, "before")?.as_bytes().to_vec())
}
pub(crate) fn capabilities(s: &Session, _character: &str, _track: &Value, spec: &Path) -> Value {
    let local = spec
        .canonicalize()
        .ok()
        .zip(
            s.project_path
                .as_ref()
                .and_then(|p| p.parent())
                .and_then(|p| p.canonicalize().ok()),
        )
        .is_some_and(|(file, root)| file.starts_with(root));
    json!({"part_set":local,"part_reset":local,"action_set":local,"pose_export":local,
 "reason":if !local{Some("A project-local saved rig is required")}else{None},
 "scale_identity":100,"position_units":"canvas pixels","rotation_units":"degrees","channel_mode":"replace selected action channel with constant; other channels preserved","pose_requires_theatre_state":true})
}

/// Constants are inferred from real channel terms, never stale editor-only metadata.
pub(crate) fn static_overrides(track: &Value, layers: &Value) -> Value {
    let mut result = json!({});
    for layer in layers.as_array().into_iter().flatten() {
        let Some(part) = layer["id"].as_str() else {
            continue;
        };
        let channels = &track["motion"][part];
        let mut values = json!({});
        for key in ["position", "scale", "rotation"] {
            let value = channels[key]
                .as_array()
                .filter(|a| a.len() == 1)
                .and_then(|a| {
                    let term = &a[0];
                    (term["kind"] == "keys").then_some(term)
                })
                .and_then(|t| t["keys"].as_array())
                .filter(|k| k.len() == 1 && k[0][0].as_f64() == Some(0.))
                .map(|k| k[0][1].clone());
            values[key] = match (key, value) {
                ("scale", Some(v)) => json!([
                    v[0].as_f64().unwrap_or(0.) + 100.,
                    v[1].as_f64().unwrap_or(0.) + 100.
                ]),
                (_, Some(v)) => v,
                _ if channels.get(key).is_none() => match key {
                    "position" => json!([0, 0]),
                    "scale" => json!([100, 100]),
                    _ => json!(0),
                },
                _ => Value::Null,
            };
        }
        result[part] = values;
    }
    result
}

pub fn pose_export(s: &mut Session, p: &Value) -> Result<Value> {
    use image::Pixel;
    let edit = begin(s, p)?;
    let ti = track_index(&edit.raw, p)?;
    let track = text(p, "track")?;
    let ui_state = p.get("theatre_state").ok_or_else(|| {
        bad("Capture current UI theatre_state (or null); required to detect unsaved edits")
    })?;
    if !same_theatre(ui_state, s.theatre_state.as_ref().unwrap_or(&Value::Null)) {
        return Err(bad(
            "Unsaved Theatre edits differ from saved state; pose export refused",
        ));
    }
    theatre_guard(s, &edit.character, Some(track))?;
    let time = number(
        &p["time"],
        0.,
        edit.raw["tracks"][ti]["duration"].as_f64().unwrap_or(0.),
        "time",
    )?;
    let rest = p
        .get("rest")
        .map(|v| v.as_bool().ok_or_else(|| bad("rest must be boolean")))
        .transpose()?
        .unwrap_or(false);
    let amount = p
        .get("amount")
        .map(|v| number(v, 0., 100., "amount"))
        .transpose()?
        .unwrap_or(100.);
    let canvas = edit.raw["canvas"]
        .as_array()
        .ok_or_else(|| bad("Saved canvas required"))?;
    let w = canvas
        .first()
        .and_then(Value::as_u64)
        .filter(|n| *n > 0 && *n <= 4096)
        .ok_or_else(|| bad("Canvas width must be 1..4096"))? as u32;
    let h = canvas
        .get(1)
        .and_then(Value::as_u64)
        .filter(|n| *n > 0 && *n <= 4096)
        .ok_or_else(|| bad("Canvas height must be 1..4096"))? as u32;
    let count = edit.raw["layers"].as_array().map_or(0, Vec::len);
    if w as u64 * h as u64 > 4 * 1024 * 1024
        || w as u64 * h as u64 * count as u64 > 64 * 1024 * 1024
    {
        return Err(bad("Pose exceeds bounded pixel budget"));
    }
    let reference = if let Some(id) = p.get("reference_asset") {
        let id = id
            .as_str()
            .ok_or_else(|| bad("reference_asset must be an asset ID"))?;
        let asset = s
            .project_assets
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| bad("Reference asset not connected"))?;
        let path = asset
            .path
            .as_ref()
            .ok_or_else(|| bad("Reference has no file"))?;
        let mut hash = Sha256::new();
        std::io::copy(&mut fs::File::open(path)?, &mut hash)?;
        json!({"asset_id":id,"path":path,"sha256":format!("{:x}",hash.finalize())})
    } else {
        Value::Null
    };
    // Force reloading the hash-verified source before using the single shared sampler.
    invalidate(s, &edit.character);
    let meta = crate::motion::cmd_motion_tracks(s, &json!({"character":edit.character}))?;
    let sample = crate::motion::cmd_motion_sample(
        s,
        &json!({"character":edit.character,"track":track,"time":time,"rest":rest,"amount":amount}),
    )?;
    let mut image = image::RgbaImage::new(w, h);
    for (layer, pose) in meta["layers"].as_array().into_iter().flatten().zip(
        sample["frames"][0]["layers"]
            .as_array()
            .into_iter()
            .flatten(),
    ) {
        let opacity = pose["opacity"].as_f64().unwrap_or(0.);
        if opacity <= 0. {
            continue;
        }
        let m: Vec<f64> = pose["matrix"]
            .as_array()
            .ok_or_else(|| bad("Missing sampled matrix"))?
            .iter()
            .map(|v| v.as_f64().ok_or_else(|| bad("Bad sampled matrix")))
            .collect::<Result<_>>()?;
        if m.len() != 6 {
            return Err(bad("Invalid affine matrix"));
        }
        let det = m[0] * m[3] - m[1] * m[2];
        if det.abs() < 1e-10 {
            continue;
        }
        let source = image::open(text(layer, "file")?)?.into_rgba8();
        let cx = layer["crop"][0].as_f64().unwrap_or(0.);
        let cy = layer["crop"][1].as_f64().unwrap_or(0.);
        for y in 0..h {
            for x in 0..w {
                let dx = x as f64 + 0.5 - m[4];
                let dy = y as f64 + 0.5 - m[5];
                let sx = (m[3] * dx - m[2] * dy) / det - cx;
                let sy = (-m[1] * dx + m[0] * dy) / det - cy;
                if sx >= 0. && sy >= 0. && sx < source.width() as f64 && sy < source.height() as f64
                {
                    let mut pixel = *source.get_pixel(sx.floor() as u32, sy.floor() as u32);
                    pixel.0[3] = (pixel.0[3] as f64 * opacity.clamp(0., 1.)).round() as u8;
                    image.get_pixel_mut(x, y).blend(&pixel);
                }
            }
        }
    }
    if bytes(&edit.rig)? != edit.before {
        return Err(bad("Rig changed during pose sampling"));
    }
    let id = format!("pose-{}", stamp()?);
    let dir = edit
        .project
        .parent()
        .ok_or_else(|| bad("No project directory"))?
        .join("assets/generated-poses")
        .join(&id);
    fs::create_dir_all(&dir)?;
    let path = dir.join("pose.png");
    image.save(&path)?;
    let receipt = dir.join("pose.json");
    let registered = crate::project::cmd_project_add_asset(
        s,
        &json!({"path":path,"id":id,"kind":"image","name":format!("{} · {} at {time}s (pending)",edit.character,track)}),
    )?;
    let value = json!({"ok":true,"asset_id":registered["asset_id"],"path":path,"receipt":receipt,"approval":"pending","project_saved":false,"source_character":edit.character,"track":track,"time":time,"rest":rest,"amount":amount,"reference":reference,"rig_sha256":digest(&edit.before),"image_sha256":digest(&fs::read(&path)?),"sampler":"motion.sample","sampling":"nearest-neighbor affine PNG; saved canvas, clipped to canvas","canvas":[w,h],"theatre_applied":false,"sample":sample,"layers":meta["layers"]});
    fs::write(
        &receipt,
        serde_json::to_vec_pretty(&value).map_err(|e| bad(e.to_string()))?,
    )?;
    Ok(value)
}
