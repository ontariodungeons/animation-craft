//! Guarded, project-local replacement of one rig layer image.
//!
//! The candidate and prior image are retained under `.part-image-history`; the
//! rig document is never normalized or rewritten by this operation.

use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn safe_id(value: &str) -> String {
    crate::workspace::sanitize_asset_id(value)
}

fn project_root(s: &Session) -> Result<PathBuf> {
    let path = s
        .project_path
        .as_deref()
        .ok_or_else(|| invalid("Open a project before replacing a part image"))?;
    let root = path
        .parent()
        .ok_or_else(|| invalid("Project has no containing directory"))?;
    std::fs::canonicalize(root).map_err(EngineError::from)
}

fn resolve_project_file(root: &Path, value: &str, label: &str) -> Result<PathBuf> {
    let raw = PathBuf::from(value);
    let path = if raw.is_absolute() {
        raw
    } else {
        root.join(raw)
    };
    let canonical = std::fs::canonicalize(&path)
        .map_err(|e| invalid(format!("Cannot resolve {label} {}: {e}", path.display())))?;
    if !canonical.starts_with(root) {
        return Err(invalid(format!("{label} resolves outside the project")));
    }
    let meta = std::fs::symlink_metadata(&canonical)?;
    if !meta.file_type().is_file() {
        return Err(invalid(format!("{label} must be a regular file")));
    }
    Ok(canonical)
}

pub(crate) fn layer_file(plan: &Value, spec: &Path, part: &str) -> Result<PathBuf> {
    let base = spec
        .parent()
        .ok_or_else(|| invalid("Rig has no parent directory"))?;
    if let Some(layers) = plan.get("layers").and_then(Value::as_object) {
        if !layers.contains_key(part) {
            return Err(invalid("Selected part is not in this rig"));
        }
        return Ok(base.join(format!("{part}.png")));
    }
    Err(invalid(
        "Image replacement currently supports keyed rigs with one PNG per stable part ID; other rig formats are read-only",
    ))
}

fn keyed_geometry_matches(plan: &crate::session::LoadedCutoutRigPlan, raw: &Value) -> Result<()> {
    let layers = raw
        .get("layers")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid(
                "Image replacement currently supports keyed rigs with explicit boxes and pivots",
            )
        })?;
    let order = raw["layer_order"]
        .as_array()
        .ok_or_else(|| invalid("Keyed rig has no layer_order"))?;
    if order.len() != plan.parts.len() {
        return Err(invalid(
            "Loaded rig geometry is stale; reload the asset before replacing",
        ));
    }
    let canvas_values = raw["canvas"]
        .as_array()
        .filter(|values| values.len() == 2)
        .ok_or_else(|| invalid("Keyed rig has no explicit canvas"))?;
    let canvas = canvas_values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|n| *n > 0 && *n <= 8192)
                .map(|n| n as u32)
                .ok_or_else(|| invalid("Keyed rig canvas is invalid"))
        })
        .collect::<Result<Vec<_>>>()?;
    let canvas = [canvas[0], canvas[1]];
    if canvas != plan.canvas_size {
        return Err(invalid(
            "Loaded rig canvas is stale; reload the asset before replacing",
        ));
    }
    for (index, id) in order.iter().enumerate() {
        let id = id
            .as_str()
            .ok_or_else(|| invalid("Invalid keyed rig layer ID"))?;
        let layer = layers
            .get(id)
            .ok_or_else(|| invalid("Invalid keyed rig layer binding"))?;
        let part = &plan.parts[index];
        let box_values = layer["box"]
            .as_array()
            .filter(|values| values.len() == 4)
            .ok_or_else(|| invalid("Keyed rig layer has no explicit box"))?;
        let b = box_values
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .filter(|n| *n <= 8192)
                    .map(|n| n as u32)
                    .ok_or_else(|| invalid("Keyed rig layer box is invalid"))
            })
            .collect::<Result<Vec<_>>>()?;
        if b[2] == 0 || b[3] == 0 || b[0] + b[2] > canvas[0] || b[1] + b[3] > canvas[1] {
            return Err(invalid("Keyed rig layer box is outside the canvas"));
        }
        let pivot_values = layer["pivot"]
            .as_array()
            .filter(|values| values.len() == 2)
            .ok_or_else(|| invalid("Keyed rig layer has no explicit pivot"))?;
        let p = pivot_values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .filter(|n| n.is_finite())
                    .map(|n| n as f32)
                    .ok_or_else(|| invalid("Keyed rig layer pivot is invalid"))
            })
            .collect::<Result<Vec<_>>>()?;
        if p[0] < 0.0 || p[1] < 0.0 || p[0] > canvas[0] as f32 || p[1] > canvas[1] as f32 {
            return Err(invalid("Keyed rig layer pivot lies outside the canvas"));
        }
        let expected_box = [b[0], b[1], b[0] + b[2], b[1] + b[3]];
        if part.id != id || part.crop_box != expected_box || part.pivot != [p[0], p[1]] {
            return Err(invalid(
                "Loaded rig geometry is stale; reload the asset before replacing",
            ));
        }
    }
    Ok(())
}

fn checked_dir(root: &Path, path: &Path, create: bool) -> Result<()> {
    if !path.starts_with(root) {
        return Err(invalid("Part-image history resolves outside the project"));
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid("Invalid history path"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid("Part-image history cannot use symlinked paths"));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(invalid("Part-image history path contains a non-directory"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                std::fs::create_dir(&current)?
            }
            Err(error) => return Err(EngineError::from(error)),
        }
    }
    Ok(())
}

fn preflight_cache_path(root: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(root) {
        return Err(invalid("Rig preview cache resolves outside the project"));
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid("Invalid preview cache path"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid("Rig preview cache cannot use symlinked paths"));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(invalid("Rig preview cache path contains a non-directory"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(EngineError::from(error)),
        }
    }
    Ok(())
}

fn preflight_cache_entries(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let kind = entry?.file_type()?;
        if !kind.is_file() || kind.is_symlink() {
            return Err(invalid(
                "Rig preview cache contains a non-regular file; refusing to load",
            ));
        }
    }
    Ok(())
}

fn check_regular_or_missing(root: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(root) {
        return Err(invalid(
            "Part-image history file resolves outside the project",
        ));
    }
    if let Some(parent) = path.parent() {
        checked_dir(root, parent, false)?;
    }
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(invalid("Part-image history cannot use symlinked files"))
        }
        Ok(meta) if !meta.is_file() => {
            Err(invalid("Part-image history entry must be a regular file"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(EngineError::from(error)),
    }
}

type KeyedCacheSnapshot = Vec<(PathBuf, Vec<u8>)>;
fn snapshot_keyed_cache(root: &Path, spec: &Path) -> Result<KeyedCacheSnapshot> {
    let Some(base) = spec.parent() else {
        return Err(invalid("Rig has no parent directory"));
    };
    let dir = base.join(".layered_preview_cache/keyed");
    if !dir.exists() {
        checked_dir(root, &dir, true)?;
    } else {
        checked_dir(root, &dir, false)?;
    }
    let mut snapshot = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let meta = entry.file_type()?;
        if !meta.is_file() {
            return Err(invalid(
                "Keyed preview cache contains a non-file entry; refusing replacement",
            ));
        }
        snapshot.push((entry.path(), std::fs::read(entry.path())?));
    }
    Ok(snapshot)
}
fn restore_keyed_cache(root: &Path, spec: &Path, snapshot: &KeyedCacheSnapshot) -> Result<()> {
    let Some(base) = spec.parent() else {
        return Err(invalid("Rig has no parent directory"));
    };
    let dir = base.join(".layered_preview_cache/keyed");
    if !dir.exists() {
        return Ok(());
    }
    checked_dir(root, &dir, false)?;
    let saved: std::collections::BTreeSet<_> =
        snapshot.iter().map(|(path, _)| path.clone()).collect();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if !saved.contains(&path) {
            std::fs::remove_file(path)?;
        }
    }
    for (path, bytes) in snapshot {
        atomic_write(path, bytes, "cache-restore")?;
    }
    Ok(())
}

fn decoded_png(bytes: &[u8], label: &str) -> Result<(u32, u32)> {
    if bytes.len() > 50_000_000 {
        return Err(invalid(format!("{label} exceeds the 50 MB image limit")));
    }
    if image::guess_format(bytes).ok() != Some(image::ImageFormat::Png) {
        return Err(invalid(format!("{label} must be a PNG")));
    }
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| invalid(format!("{label} failed full PNG decode: {e}")))?;
    if image.width() == 0
        || image.height() == 0
        || u64::from(image.width()) * u64::from(image.height()) > 16_777_216
        || !image.color().has_alpha()
    {
        return Err(invalid(format!(
            "{label} must have nonzero dimensions and an alpha channel"
        )));
    }
    Ok((image.width(), image.height()))
}

fn atomic_write(path: &Path, bytes: &[u8], suffix: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Target file has no parent directory"))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| invalid("Target file name is not UTF-8"))?;
    let stage = parent.join(format!(".{name}.{suffix}.tmp"));
    let staged = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)?;
        use std::io::Write;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = staged {
        let _ = std::fs::remove_file(&stage);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&stage, path) {
        let _ = std::fs::remove_file(&stage);
        return Err(EngineError::from(error));
    }
    Ok(())
}

fn reload(s: &mut Session, asset: &str) -> Result<()> {
    crate::project::cmd_project_load_asset(s, &json!({"id":asset,"reload":true}))?;
    Ok(())
}

fn asset_id(s: &mut Session, params: &Value) -> Result<String> {
    // Loading a keyed rig writes derived PNG caches beside its manifest.
    let root = project_root(s)?;
    let id = params
        .get("asset")
        .or_else(|| params.get("character"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| s.active_asset.clone())
        .or_else(|| s.project_assets.first().map(|asset| asset.id.clone()))
        .ok_or_else(|| invalid("Choose a project asset"))?;
    let entry = s.project_assets.iter().find(|asset| asset.id == id);
    let is_rig = entry.is_some_and(|asset| asset.kind == "cutout_rig");
    let source = entry
        .and_then(|asset| asset.path.as_ref())
        .cloned()
        .or_else(|| s.assets.get(&id).map(|asset| asset.path.clone()))
        .ok_or_else(|| invalid("Selected project asset has no source path"))?;
    let source = if source.is_absolute() {
        source
    } else {
        root.join(source)
    };
    let source = std::fs::canonicalize(source).map_err(EngineError::from)?;
    if !source.starts_with(&root) {
        return Err(invalid(
            "Selected project asset resolves outside the project",
        ));
    }
    let manifest = if source.is_dir() {
        source.join("manifest.json")
    } else {
        source.clone()
    };
    if is_rig
        || manifest
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        let manifest = std::fs::canonicalize(&manifest).map_err(EngineError::from)?;
        if !manifest.starts_with(&root) || !manifest.is_file() {
            return Err(invalid("Rig manifest must be a regular project-local file"));
        }
        let raw: Value = serde_json::from_slice(&std::fs::read(&manifest)?)
            .map_err(|error| invalid(format!("Invalid project asset JSON: {error}")))?;
        let rig_like = is_rig
            || raw.get("rig_type").is_some()
            || (raw.get("layers").is_some() && raw.get("canvas").is_some());
        if rig_like && raw.get("layers").and_then(Value::as_object).is_none() {
            return Err(invalid(
                "Image replacement currently supports keyed rigs with one PNG per stable part ID; array/component rigs remain read-only",
            ));
        }
        let parent = manifest
            .parent()
            .ok_or_else(|| invalid("Rig manifest has no parent"))?;
        let cache = parent.join(".layered_preview_cache/keyed");
        preflight_cache_path(&root, &cache)?;
        preflight_cache_entries(&cache)?;
    }
    crate::project::cmd_project_load_asset(s, &json!({"id":id}))?;
    Ok(id)
}

fn rig_snapshot(s: &Session, asset: &str) -> Result<(PathBuf, Value, String)> {
    let rig = s
        .assets
        .get(asset)
        .and_then(|a| a.cutout_rig.as_ref())
        .ok_or_else(|| invalid("Load a layered rig first"))?;
    let spec = std::fs::canonicalize(&rig.plan_path)?;
    let root = project_root(s)?;
    if !spec.starts_with(&root) {
        return Err(invalid("Rig manifest resolves outside the project"));
    }
    let bytes = std::fs::read(&spec)?;
    let raw: Value =
        serde_json::from_slice(&bytes).map_err(|e| invalid(format!("Invalid rig JSON: {e}")))?;
    Ok((spec, raw, sha(&bytes)))
}

fn state_path(history: &Path) -> PathBuf {
    history.join("state.json")
}

fn read_state(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({"revision":0,"last_token":null}));
    }
    let state: Value = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|e| invalid(format!("Invalid part-image history state: {e}")))?;
    let revision = state
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid("Invalid part-image history state: revision must be a nonnegative integer")
        })?;
    let token_valid = state
        .get("last_token")
        .is_some_and(|token| token.is_null() || token.as_str().is_some_and(|v| !v.is_empty()));
    if !token_valid {
        return Err(invalid(
            "Invalid part-image history state: last_token must be null or a nonempty token",
        ));
    }
    if revision == 0 && !state["last_token"].is_null() {
        return Err(invalid(
            "Invalid part-image history state: revision and last_token disagree",
        ));
    }
    Ok(state)
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| invalid(e.to_string()))?;
    atomic_write(path, &bytes, "metadata")
}

/// Replace one part image. Required parameters: asset, part, candidate,
/// expected_rig_sha256, expected_part_sha256, expected_candidate_sha256,
/// and expected_revision (0 for an asset without prior replacements).
pub fn cmd_part_replace_image(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = asset_id(s, params)?;
    let part = params["part"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("Choose a stable part ID"))?;
    let root = project_root(s)?;
    let (spec, raw, rig_hash) = rig_snapshot(s, &asset)?;
    if params["expected_rig_sha256"].as_str() != Some(&rig_hash) {
        return Err(invalid(
            "Rig changed since this candidate was reviewed; reload and compare again",
        ));
    }
    let plan = s
        .assets
        .get(&asset)
        .and_then(|a| a.cutout_rig.as_ref())
        .ok_or_else(|| invalid("Load a layered rig first"))?;
    keyed_geometry_matches(plan, &raw)?;
    if !plan.parts.iter().any(|p| p.id == part) {
        return Err(invalid("Selected part is not in the loaded rig"));
    }
    let bound_target = layer_file(&raw, &spec, part)?;
    let binding_meta = std::fs::symlink_metadata(&bound_target)?;
    if binding_meta.file_type().is_symlink() {
        return Err(invalid(
            "Symlink-bound part images are read-only; use a regular project PNG",
        ));
    }
    let target = resolve_project_file(
        &root,
        bound_target
            .to_str()
            .ok_or_else(|| invalid("Layer path is not UTF-8"))?,
        "part image",
    )?;
    let candidate_raw = params["candidate"]
        .as_str()
        .ok_or_else(|| invalid("Choose a generated candidate"))?;
    let candidate = resolve_project_file(&root, candidate_raw, "candidate")?;
    if candidate == target {
        return Err(invalid("Candidate must be a separate project-local image"));
    }
    for layer in raw["layers"]
        .as_object()
        .into_iter()
        .flat_map(|layers| layers.keys())
    {
        let binding = spec.parent().unwrap().join(format!("{layer}.png"));
        let meta = std::fs::symlink_metadata(&binding)?;
        if meta.file_type().is_symlink() {
            return Err(invalid(
                "Symlink-bound part images are read-only; use regular project PNGs",
            ));
        }
        let other = resolve_project_file(&root, &binding.to_string_lossy(), "part image")?;
        if layer != part && other == target {
            return Err(invalid(
                "Selected image is shared with another rig part; replacement is unsafe",
            ));
        }
        if other == candidate {
            return Err(invalid(
                "Candidate cannot be another rig part's bound image",
            ));
        }
    }
    let target_bytes = std::fs::read(&target)?;
    let candidate_bytes = std::fs::read(&candidate)?;
    let old_hash = sha(&target_bytes);
    let candidate_hash = sha(&candidate_bytes);
    if params["expected_part_sha256"].as_str() != Some(&old_hash) {
        return Err(invalid(
            "Part image changed since comparison; reload before replacing",
        ));
    }
    if params["expected_candidate_sha256"].as_str() != Some(&candidate_hash) {
        return Err(invalid(
            "Candidate image changed since comparison; compare it again",
        ));
    }
    let old_size = decoded_png(&target_bytes, "Current part image")?;
    let new_size = decoded_png(&candidate_bytes, "Candidate")?;
    if old_size != new_size {
        return Err(invalid(format!(
            "Candidate is {}×{} but this part requires {}×{}; replacement keeps the existing box and does not resize artwork",
            new_size.0, new_size.1, old_size.0, old_size.1
        )));
    }

    let history = spec
        .parent()
        .ok_or_else(|| invalid("Rig has no parent directory"))?
        .join(".part-image-history")
        .join(safe_id(&asset));
    checked_dir(
        &root,
        history
            .parent()
            .ok_or_else(|| invalid("Invalid history directory"))?,
        true,
    )?;
    checked_dir(&root, &history, true)?;
    let state_file = state_path(&history);
    check_regular_or_missing(&root, &state_file)?;
    let mut state = read_state(&state_file)?;
    let revision = state["revision"].as_u64().unwrap_or(0);
    let next_revision = revision
        .checked_add(1)
        .filter(|_| revision.checked_add(2).is_some())
        .ok_or_else(|| {
            invalid("Part-image history revision is exhausted; replacement cannot be undone safely")
        })?;
    if params["expected_revision"].as_u64() != Some(revision) {
        return Err(invalid(
            "Part-image history changed since comparison; reload the asset",
        ));
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| invalid(e.to_string()))?
        .as_nanos();
    let token = format!("{}-{stamp}-{}", safe_id(&asset), std::process::id());
    let operation = history.join(&token);
    checked_dir(&root, &operation, true)?;
    let backup = operation.join("before.png");
    let staged_candidate = operation.join("candidate.png");
    for file in [&backup, &staged_candidate, &operation.join("receipt.json")] {
        check_regular_or_missing(&root, file)?;
    }
    std::fs::write(&backup, &target_bytes)?;
    std::fs::write(&staged_candidate, &candidate_bytes)?;
    let cache_before = snapshot_keyed_cache(&root, &spec)?;
    atomic_write(&target, &candidate_bytes, &token)?;

    if let Err(error) = reload(s, &asset) {
        let rollback = atomic_write(&target, &target_bytes, &format!("rollback-{token}"));
        let cache_rollback = restore_keyed_cache(&root, &spec, &cache_before);
        return Err(rollback
            .err()
            .or_else(|| cache_rollback.err())
            .unwrap_or(error));
    }
    let record = json!({"asset":asset,"part":part,"token":token,"target":target,"backup":backup,"candidate":staged_candidate,
        "before_sha256":old_hash,"candidate_sha256":candidate_hash,"rig_sha256":rig_hash,"revision_before":revision,
        "revision_after":next_revision,"status":"applied","width":old_size.0,"height":old_size.1});
    if let Err(error) = write_json(&operation.join("receipt.json"), &record) {
        let rollback = atomic_write(&target, &target_bytes, &format!("rollback-{token}"));
        let _ = reload(s, &asset);
        let cache_rollback = restore_keyed_cache(&root, &spec, &cache_before);
        return Err(rollback
            .err()
            .or_else(|| cache_rollback.err())
            .unwrap_or(error));
    }
    state["revision"] = json!(next_revision);
    state["last_token"] = json!(token);
    if let Err(error) = write_json(&state_file, &state) {
        let rollback = atomic_write(&target, &target_bytes, &format!("rollback-{token}"));
        let _ = reload(s, &asset);
        let cache_rollback = restore_keyed_cache(&root, &spec, &cache_before);
        return Err(rollback
            .err()
            .or_else(|| cache_rollback.err())
            .unwrap_or(error));
    }
    Ok(
        json!({"ok":true,"asset":asset,"part":part,"undo":token,"revision":next_revision,"before_sha256":old_hash,"candidate_sha256":candidate_hash,"width":old_size.0,"height":old_size.1}),
    )
}

/// Undo only the latest part-image replacement for this asset. The token is
/// single-use and guarded by the project-local revision plus current image hash.
pub fn cmd_part_undo_image(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = asset_id(s, params)?;
    let token = params["undo"]
        .as_str()
        .ok_or_else(|| invalid("Missing part-image undo token"))?;
    if token.contains('/') || token.contains('\\') || token.contains("..") {
        return Err(invalid("Invalid part-image undo token"));
    }
    let (spec, raw, rig_hash) = rig_snapshot(s, &asset)?;
    let plan = s
        .assets
        .get(&asset)
        .and_then(|a| a.cutout_rig.as_ref())
        .ok_or_else(|| invalid("Load a layered rig first"))?;
    keyed_geometry_matches(plan, &raw)?;
    let history = spec
        .parent()
        .ok_or_else(|| invalid("Rig has no parent directory"))?
        .join(".part-image-history")
        .join(safe_id(&asset));
    let root = project_root(s)?;
    checked_dir(&root, &history, false)?;
    let state_file = state_path(&history);
    check_regular_or_missing(&root, &state_file)?;
    let mut state = read_state(&state_file)?;
    let operation = history.join(token);
    checked_dir(&root, &operation, false)?;
    let receipt_path = operation.join("receipt.json");
    check_regular_or_missing(&root, &receipt_path)?;
    let mut record: Value = serde_json::from_slice(
        &std::fs::read(&receipt_path)
            .map_err(|e| invalid(format!("Unknown or unavailable undo token: {e}")))?,
    )
    .map_err(|e| invalid(format!("Invalid undo receipt: {e}")))?;
    if record["asset"].as_str() != Some(&asset) || record["token"].as_str() != Some(token) {
        return Err(invalid("Undo token belongs to another asset"));
    }
    if record["status"] != "applied" {
        return Err(invalid("This part-image undo token was already consumed"));
    }
    if state["last_token"].as_str() != Some(token) || state["revision"] != record["revision_after"]
    {
        return Err(invalid(
            "A later part-image operation exists; this undo token is stale",
        ));
    }
    if params
        .get("expected_rig_sha256")
        .and_then(Value::as_str)
        .is_some_and(|expected| expected != rig_hash)
        || record["rig_sha256"].as_str() != Some(&rig_hash)
    {
        return Err(invalid(
            "Rig changed since replacement; reload and review before undo",
        ));
    }
    let part = record["part"]
        .as_str()
        .ok_or_else(|| invalid("Undo receipt has no part ID"))?
        .to_owned();
    let bound_target = layer_file(&raw, &spec, &part)?;
    if std::fs::symlink_metadata(&bound_target)?
        .file_type()
        .is_symlink()
    {
        return Err(invalid(
            "Symlink-bound part images are read-only; undo refused",
        ));
    }
    let target = resolve_project_file(
        &root,
        bound_target
            .to_str()
            .ok_or_else(|| invalid("Layer path is not UTF-8"))?,
        "part image",
    )?;
    if record["target"].as_str() != target.to_str() {
        return Err(invalid(
            "Part image binding changed since replacement; undo refused",
        ));
    }
    for layer in raw["layers"]
        .as_object()
        .into_iter()
        .flat_map(|layers| layers.keys())
    {
        let binding = spec
            .parent()
            .ok_or_else(|| invalid("Rig has no parent directory"))?
            .join(format!("{layer}.png"));
        if std::fs::symlink_metadata(&binding)?
            .file_type()
            .is_symlink()
        {
            return Err(invalid(
                "Symlink-bound part images are read-only; undo refused",
            ));
        }
        let other = resolve_project_file(
            &root,
            binding
                .to_str()
                .ok_or_else(|| invalid("Layer path is not UTF-8"))?,
            "part image",
        )?;
        if layer != &part && other == target {
            return Err(invalid(
                "Part image is shared with another rig part; undo refused",
            ));
        }
    }
    let current = std::fs::read(&target)?;
    if sha(&current) != record["candidate_sha256"].as_str().unwrap_or_default() {
        return Err(invalid(
            "Part image changed after replacement; undo would overwrite newer work",
        ));
    }
    let backup = PathBuf::from(
        record["backup"]
            .as_str()
            .ok_or_else(|| invalid("Undo receipt has no original image"))?,
    );
    let backup = resolve_project_file(
        &root,
        backup
            .to_str()
            .ok_or_else(|| invalid("Backup path is not UTF-8"))?,
        "original part backup",
    )?;
    let original = std::fs::read(&backup)?;
    if sha(&original) != record["before_sha256"].as_str().unwrap_or_default() {
        return Err(invalid(
            "Original part backup failed integrity verification",
        ));
    }
    let revision = state["revision"].as_u64().unwrap_or(0);
    let next_revision = revision.checked_add(1).ok_or_else(|| {
        invalid("Part-image history revision is exhausted; undo cannot safely advance it")
    })?;
    let cache_before = snapshot_keyed_cache(&root, &spec)?;
    atomic_write(&target, &original, &format!("undo-{token}"))?;
    if let Err(error) = reload(s, &asset) {
        let rollback = atomic_write(&target, &current, &format!("rollback-undo-{token}"));
        let cache_rollback = restore_keyed_cache(&root, &spec, &cache_before);
        return Err(rollback
            .err()
            .or_else(|| cache_rollback.err())
            .unwrap_or(error));
    }
    record["status"] = json!("undone");
    record["revision_undone"] = json!(next_revision);
    if let Err(error) = write_json(&receipt_path, &record) {
        let rollback = atomic_write(&target, &current, &format!("rollback-undo-{token}"));
        let _ = reload(s, &asset);
        let cache_rollback = restore_keyed_cache(&root, &spec, &cache_before);
        return Err(rollback
            .err()
            .or_else(|| cache_rollback.err())
            .unwrap_or(error));
    }
    state["revision"] = json!(next_revision);
    state["last_token"] = Value::Null;
    if let Err(error) = write_json(&state_file, &state) {
        record["status"] = json!("applied");
        record
            .as_object_mut()
            .map(|map| map.remove("revision_undone"));
        let _ = write_json(&receipt_path, &record);
        let rollback = atomic_write(&target, &current, &format!("rollback-undo-{token}"));
        let _ = reload(s, &asset);
        let cache_rollback = restore_keyed_cache(&root, &spec, &cache_before);
        return Err(rollback
            .err()
            .or_else(|| cache_rollback.err())
            .unwrap_or(error));
    }
    Ok(
        json!({"ok":true,"asset":asset,"part":part,"revision":next_revision,"restored_sha256":record["before_sha256"]}),
    )
}

/// Resolve an installed Craft binary the way `resolve_craft_cli` names it: an
/// explicit path must be a file, a bare name must be on PATH.
fn installed_craft_binary(name: &str) -> Option<PathBuf> {
    let resolved = PathBuf::from(crate::workspace::resolve_craft_cli(name));
    if resolved.components().count() > 1 {
        return resolved.is_file().then_some(resolved);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(&resolved))
            .find(|path| path.is_file())
    })
}

/// The PhotoCraft app's desktop entry (`gio launch` runs whatever version it points at).
fn photocraft_desktop_entry() -> Option<PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    data_home
        .into_iter()
        .chain(std::env::split_paths(&data_dirs))
        .map(|dir| dir.join("applications/ai.storyteller.photocraft.desktop"))
        .find(|path| path.is_file())
}

/// A project-local copy of `source` for PhotoCraft to edit, so saving there never
/// bypasses `part.replace_image`. An existing draft is kept: it may hold unsaved work.
fn edit_draft(root: &Path, history: &Path, part: &str, source: &Path) -> Result<(PathBuf, bool)> {
    let drafts = history.join("drafts");
    checked_dir(root, &drafts, true)?;
    let draft = drafts.join(format!("{}.png", safe_id(part)));
    check_regular_or_missing(root, &draft)?;
    if draft.exists() {
        return Ok((draft, true));
    }
    std::fs::copy(source, &draft)?;
    Ok((draft, false))
}

/// The earliest retained `before.png` for this part: the image the part had
/// before its first replacement. Verified against its receipt digest.
fn original_backup(root: &Path, history: &Path, part: &str) -> Result<Option<PathBuf>> {
    if !history.exists() {
        return Ok(None);
    }
    checked_dir(root, history, false)?;
    let mut earliest: Option<(u64, PathBuf, String)> = None;
    for entry in std::fs::read_dir(history)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let receipt_path = entry.path().join("receipt.json");
        check_regular_or_missing(root, &receipt_path)?;
        let Ok(bytes) = std::fs::read(&receipt_path) else {
            continue;
        };
        let record: Value = serde_json::from_slice(&bytes)
            .map_err(|e| invalid(format!("Invalid part-image receipt: {e}")))?;
        if record["part"].as_str() != Some(part) {
            continue;
        }
        let revision = record["revision_before"]
            .as_u64()
            .ok_or_else(|| invalid("Part-image receipt has no revision"))?;
        let backup = record["backup"]
            .as_str()
            .ok_or_else(|| invalid("Part-image receipt has no original image"))?;
        let digest = record["before_sha256"].as_str().unwrap_or_default().to_owned();
        if earliest.as_ref().is_none_or(|(r, _, _)| revision < *r) {
            earliest = Some((revision, PathBuf::from(backup), digest));
        }
    }
    let Some((_, backup, digest)) = earliest else {
        return Ok(None);
    };
    let backup = resolve_project_file(
        root,
        backup
            .to_str()
            .ok_or_else(|| invalid("Backup path is not UTF-8"))?,
        "original part backup",
    )?;
    if sha(&std::fs::read(&backup)?) != digest {
        return Err(invalid(
            "Original part backup failed integrity verification",
        ));
    }
    Ok(Some(backup))
}

/// Resolve one rig part's PNG (current, or its retained original) and report
/// what PhotoCraft would open. `spawn: true` launches the PhotoCraft app on an
/// edit draft of it; the saved draft is the candidate for `part.replace_image`.
pub fn cmd_part_open_photocraft(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = asset_id(s, params)?;
    let part = params["part"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("Choose a stable part ID"))?;
    let which = params["which"].as_str().unwrap_or("current");
    if which != "current" && which != "original" {
        return Err(invalid("`which` must be \"current\" or \"original\""));
    }
    let spawn = params["spawn"].as_bool().unwrap_or(false);
    let root = project_root(s)?;
    let (spec, raw, _) = rig_snapshot(s, &asset)?;
    let bound = layer_file(&raw, &spec, part)?;
    let current = resolve_project_file(
        &root,
        bound
            .to_str()
            .ok_or_else(|| invalid("Layer path is not UTF-8"))?,
        "part image",
    )?;
    let history = spec
        .parent()
        .ok_or_else(|| invalid("Rig has no parent directory"))?
        .join(".part-image-history")
        .join(safe_id(&asset));
    let original = original_backup(&root, &history, part)?;
    let file = match (which, &original) {
        ("original", Some(backup)) => backup.clone(),
        _ => current,
    };
    let bytes = std::fs::read(&file)?;
    let (width, height) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()?
        .into_dimensions()
        .map_err(|e| invalid(format!("Cannot read part image dimensions: {e}")))?;
    let photocraft_cli = installed_craft_binary("photocraft-cli");
    let mut draft = Value::Null;
    if spawn {
        // photocraft-cli is headless; only the PhotoCraft app opens a window.
        // An explicit `photocraft` binary wins, else the installed desktop entry.
        let mut launch = match installed_craft_binary("photocraft") {
            Some(app) => std::process::Command::new(app),
            None => {
                let entry = photocraft_desktop_entry().ok_or_else(|| {
                    invalid("PhotoCraft app is not installed (no `photocraft` binary or ai.storyteller.photocraft.desktop)")
                })?;
                let mut gio = std::process::Command::new("gio");
                gio.arg("launch").arg(entry);
                gio
            }
        };
        let (path, reused) = edit_draft(&root, &history, part, &file)?;
        launch.arg(&path).spawn()?;
        draft = json!({"file":path,"reused":reused});
    }
    Ok(json!({"ok":true,"command":"part.open_photocraft","part":part,"which":which,"file":file,
        "original_file":original,"width":width,"height":height,"sha256":sha(&bytes),
        "photocraft_cli":photocraft_cli,"spawned":spawn,"draft":draft}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use image::{ImageBuffer, Rgba};
    use serde_json::json;

    fn fixture() -> (Session, PathBuf, PathBuf, PathBuf, Value) {
        let root = std::env::temp_dir().join(format!(
            "ac-part-replace-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let original = root.join("fin.png");
        let candidate = root.join("candidate.png");
        let other = root.join("other.png");
        ImageBuffer::from_pixel(8, 6, Rgba::<u8>([40, 50, 60, 255]))
            .save(root.join("source.png"))
            .unwrap();
        ImageBuffer::from_pixel(3, 2, Rgba::<u8>([10, 20, 30, 255]))
            .save(&original)
            .unwrap();
        ImageBuffer::from_pixel(3, 2, Rgba::<u8>([80, 100, 120, 190]))
            .save(&candidate)
            .unwrap();
        ImageBuffer::from_pixel(2, 2, Rgba::<u8>([1, 2, 3, 255]))
            .save(&other)
            .unwrap();
        let rig = root.join("rig.json");
        let raw = json!({"fish":"test_fish","source":"source.png","canvas":[8,6],"layer_order":["fin","body"],"layers":{
            "fin":{"box":[1,1,3,2],"pivot":[2.0,1.5]},"body":{"box":[4,1,2,2],"pivot":[5.0,2.0]}},
            "effectcraft":{"tracks":[{"id":"sway","duration":2.0,"motion":{"fin":{"rotation":[{"kind":"keys","keys":[[0,0],[2,35]]}]}}}]}});
        std::fs::write(&rig, serde_json::to_vec_pretty(&raw).unwrap()).unwrap();
        // Keyed rig loading expects one image named after each stable layer ID.
        ImageBuffer::from_pixel(2, 2, Rgba::<u8>([45, 55, 65, 255]))
            .save(root.join("body.png"))
            .unwrap();
        let project = root.join("project.acproj");
        std::fs::write(&project, serde_json::to_vec_pretty(&json!({"format":"animationcraft.project","version":1,"name":"Fixture","assets":[
            {"id":"fish","name":"Fish","preset":null,"path":rig,"kind":"cutout_rig","parts":[]}],"workshop":{}})).unwrap()).unwrap();
        let mut session = Session::default();
        session.cwd = root.clone();
        session
            .execute("project.open", json!({"path":project}))
            .unwrap();
        session
            .execute("project.load_asset", json!({"id":"fish"}))
            .unwrap();
        // `original` is the layer's physical source; candidate is an independent local PNG.
        (session, root, rig, candidate, raw)
    }

    fn hashes(rig: &Path, source: &Path, candidate: &Path) -> (String, String, String) {
        (
            sha(&std::fs::read(rig).unwrap()),
            sha(&std::fs::read(source).unwrap()),
            sha(&std::fs::read(candidate).unwrap()),
        )
    }

    fn replace_args(rig: &Path, source: &Path, candidate: &Path, revision: u64) -> Value {
        let (rig_hash, part_hash, candidate_hash) = hashes(rig, source, candidate);
        json!({"asset":"fish","part":"fin","candidate":candidate,"expected_rig_sha256":rig_hash,"expected_part_sha256":part_hash,"expected_candidate_sha256":candidate_hash,"expected_revision":revision})
    }

    #[test]
    fn replacement_keeps_rig_motion_and_pivot_and_undo_survives_reopen() {
        let (mut session, root, rig, candidate, before_raw) = fixture();
        let source = root.join("fin.png");
        let track_meta = session
            .execute("motion.tracks", json!({"character":"fish"}))
            .unwrap();
        assert_eq!(
            track_meta["original_reference"],
            root.join("source.png").to_string_lossy().as_ref()
        );
        let original_hash = sha(&std::fs::read(&source).unwrap());
        let candidate_hash = sha(&std::fs::read(&candidate).unwrap());
        let before_manifest = std::fs::read(&rig).unwrap();
        let result = session
            .execute(
                "part.replace_image",
                replace_args(&rig, &source, &candidate, 0),
            )
            .unwrap();
        assert_eq!(sha(&std::fs::read(&source).unwrap()), candidate_hash);
        assert_eq!(
            image::open(&source).unwrap().to_rgba8().get_pixel(0, 0).0,
            [80, 100, 120, 190]
        );
        assert_eq!(
            image::open(root.join("body.png"))
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [45, 55, 65, 255]
        );
        assert_eq!(
            std::fs::read(&rig).unwrap(),
            before_manifest,
            "keyed rig bytes remain unchanged"
        );
        let after_rig: Value = serde_json::from_slice(&std::fs::read(&rig).unwrap()).unwrap();
        assert_eq!(
            after_rig["layers"]["fin"]["box"],
            before_raw["layers"]["fin"]["box"]
        );
        assert_eq!(
            after_rig["layers"]["fin"]["pivot"],
            before_raw["layers"]["fin"]["pivot"]
        );
        assert_eq!(after_rig["effectcraft"], before_raw["effectcraft"]);

        session.execute("project.save", json!({})).unwrap();
        let mut reopened = Session::default();
        reopened.cwd = root.clone();
        reopened
            .execute("project.open", json!({"path":session.project_path}))
            .unwrap();
        reopened
            .execute("project.load_asset", json!({"id":"fish"}))
            .unwrap();
        assert_eq!(
            reopened.assets["fish"]
                .cutout_rig
                .as_ref()
                .unwrap()
                .parts
                .iter()
                .find(|p| p.id == "fin")
                .unwrap()
                .pivot,
            [2.0, 1.5]
        );
        let undone = reopened
            .execute(
                "part.undo_image",
                json!({"asset":"fish","undo":result["undo"]}),
            )
            .unwrap();
        assert_eq!(undone["ok"], true);
        assert_eq!(sha(&std::fs::read(&source).unwrap()), original_hash);
        assert!(
            reopened
                .execute(
                    "part.undo_image",
                    json!({"asset":"fish","undo":result["undo"]})
                )
                .is_err(),
            "undo is consumed"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn replacement_rejects_bad_dimensions_and_stale_image_and_stale_undo() {
        let (mut session, root, rig, candidate, _) = fixture();
        let source = root.join("fin.png");
        let mut args = replace_args(&rig, &source, &candidate, 0);
        args["candidate"] = json!(root.join("other.png"));
        let (_, _, hash) = hashes(&rig, &source, &root.join("other.png"));
        args["expected_candidate_sha256"] = json!(hash);
        assert!(
            session
                .execute("part.replace_image", args)
                .unwrap_err()
                .to_string()
                .contains("requires 3×2")
        );
        let args = replace_args(&rig, &source, &candidate, 0);
        let mut missing = args.clone();
        missing["candidate"] = json!(root.join("missing.png"));
        assert!(
            session
                .execute("part.replace_image", missing)
                .unwrap_err()
                .to_string()
                .contains("Cannot resolve candidate")
        );
        std::fs::write(&source, b"externally changed").unwrap();
        assert!(
            session.execute("part.replace_image", args).is_err(),
            "stale source digest blocks replacement"
        );
        // Restore fixture source and make two operations: first token cannot undo after second.
        ImageBuffer::from_pixel(3, 2, Rgba::<u8>([10, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let first = session
            .execute(
                "part.replace_image",
                replace_args(&rig, &source, &candidate, 0),
            )
            .unwrap();
        let candidate2 = root.join("candidate2.png");
        ImageBuffer::from_pixel(3, 2, Rgba::<u8>([120, 110, 100, 220]))
            .save(&candidate2)
            .unwrap();
        let candidate2_body = root.join("candidate2-body.png");
        ImageBuffer::from_pixel(2, 2, Rgba::<u8>([120, 110, 100, 220]))
            .save(&candidate2_body)
            .unwrap();
        let (rig_hash, part_hash, candidate_hash) =
            hashes(&rig, &root.join("body.png"), &candidate2_body);
        let second_args = json!({"asset":"fish","part":"body","candidate":candidate2_body,"expected_rig_sha256":rig_hash,"expected_part_sha256":part_hash,"expected_candidate_sha256":candidate_hash,"expected_revision":1});
        let second = session.execute("part.replace_image", second_args).unwrap();
        assert!(
            session
                .execute(
                    "part.undo_image",
                    json!({"asset":"fish","undo":first["undo"]})
                )
                .unwrap_err()
                .to_string()
                .contains("stale")
        );
        session
            .execute(
                "part.undo_image",
                json!({"asset":"fish","undo":second["undo"]}),
            )
            .unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn photocraft_edit_draft_is_a_kept_copy_that_replaces_as_candidate() {
        let (mut session, root, rig, _, _) = fixture();
        let source = root.join("fin.png");
        let before = std::fs::read(&source).unwrap();
        let history = rig.parent().unwrap().join(".part-image-history/fish");
        let (draft, reused) = edit_draft(&root, &history, "fin", &source).unwrap();
        assert!(!reused);
        assert_eq!(std::fs::read(&draft).unwrap(), before);
        // PhotoCraft saves an edit into the draft; reopening keeps it.
        ImageBuffer::from_pixel(3, 2, Rgba::<u8>([200, 10, 10, 255]))
            .save(&draft)
            .unwrap();
        let edited = std::fs::read(&draft).unwrap();
        let (again, reused) = edit_draft(&root, &history, "fin", &source).unwrap();
        assert!(reused);
        assert_eq!((again.clone(), std::fs::read(&again).unwrap()), (draft.clone(), edited));
        assert_eq!(std::fs::read(&source).unwrap(), before, "live layer untouched");
        session
            .execute("part.replace_image", replace_args(&rig, &source, &draft, 0))
            .unwrap();
        assert_eq!(sha(&std::fs::read(&source).unwrap()), sha(&std::fs::read(&draft).unwrap()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_existing_history_state_is_rejected_before_replacement() {
        let (mut session, root, rig, candidate, _) = fixture();
        let source = root.join("fin.png");
        let before = std::fs::read(&source).unwrap();
        let history = rig.parent().unwrap().join(".part-image-history/fish");
        std::fs::create_dir_all(&history).unwrap();
        std::fs::write(history.join("state.json"), "42").unwrap();
        let error = session
            .execute(
                "part.replace_image",
                replace_args(&rig, &source, &candidate, 0),
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("revision must be a nonnegative integer"));
        assert_eq!(std::fs::read(&source).unwrap(), before);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn replacement_rejects_bound_candidate_and_symlinked_history() {
        let (mut session, root, rig, candidate, _) = fixture();
        let source = root.join("fin.png");
        let body_candidate = root.join("body.png");
        let (_, _, hash) = hashes(&rig, &source, &body_candidate);
        let mut args = replace_args(&rig, &source, &body_candidate, 0);
        args["expected_candidate_sha256"] = json!(hash);
        assert!(
            session
                .execute("part.replace_image", args)
                .unwrap_err()
                .to_string()
                .contains("another rig part")
        );

        #[cfg(unix)]
        {
            let original = root.join("fin-original.png");
            std::fs::rename(&source, &original).unwrap();
            std::os::unix::fs::symlink(root.join("body.png"), &source).unwrap();
            let args = replace_args(&rig, &source, &candidate, 0);
            assert!(
                session
                    .execute("part.replace_image", args)
                    .unwrap_err()
                    .to_string()
                    .contains("Symlink-bound")
            );
            std::fs::remove_file(&source).unwrap();
            std::fs::rename(original, &source).unwrap();

            let outside = root.join("outside-history");
            std::fs::create_dir(&outside).unwrap();
            std::os::unix::fs::symlink(&outside, root.join(".part-image-history")).unwrap();
            let args = replace_args(&rig, &source, &candidate, 0);
            assert!(
                session
                    .execute("part.replace_image", args)
                    .unwrap_err()
                    .to_string()
                    .contains("symlink")
            );
            assert_ne!(
                sha(&std::fs::read(&source).unwrap()),
                sha(&std::fs::read(&candidate).unwrap())
            );

            let cache = root.join(".layered_preview_cache");
            let saved_cache = root.join("saved-preview-cache");
            let outside_cache = root.join("outside-preview-cache");
            std::fs::rename(&cache, &saved_cache).unwrap();
            std::fs::create_dir(&outside_cache).unwrap();
            std::os::unix::fs::symlink(&outside_cache, &cache).unwrap();
            session.assets.clear();
            session.active_asset = None;
            let args = replace_args(&rig, &source, &candidate, 0);
            assert!(
                session
                    .execute("part.replace_image", args)
                    .unwrap_err()
                    .to_string()
                    .contains("symlink")
            );
            assert_eq!(
                std::fs::read_dir(&outside_cache).unwrap().count(),
                0,
                "first asset load must not write through cache symlink"
            );
            std::fs::remove_file(&cache).unwrap();
            std::fs::rename(saved_cache, cache).unwrap();
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn undo_rejects_part_binding_changed_to_symlink_alias() {
        let (mut session, root, rig, candidate, _) = fixture();
        let source = root.join("fin.png");
        let applied = session
            .execute(
                "part.replace_image",
                replace_args(&rig, &source, &candidate, 0),
            )
            .unwrap();
        let replaced = std::fs::read(&source).unwrap();
        let detached = root.join("detached-fin.png");
        std::fs::rename(&source, &detached).unwrap();
        std::os::unix::fs::symlink(&candidate, &source).unwrap();
        assert!(
            session
                .execute(
                    "part.undo_image",
                    json!({"asset":"fish","undo":applied["undo"]})
                )
                .unwrap_err()
                .to_string()
                .contains("Symlink-bound")
        );
        assert_eq!(
            std::fs::read(&candidate).unwrap(),
            replaced,
            "undo must never write through a changed alias"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn part_commands_load_project_asset_and_plain_image_motion_still_inspects() {
        let (mut session, root, rig, candidate, _) = fixture();
        let source = root.join("fin.png");
        session.assets.clear();
        session.active_asset = None;
        let result = session
            .execute(
                "part.replace_image",
                replace_args(&rig, &source, &candidate, 0),
            )
            .unwrap();
        assert!(
            result["undo"].as_str().is_some(),
            "replacement command can load the selected asset on first use"
        );

        let image_path = root.join("plain.png");
        ImageBuffer::from_pixel(2, 2, Rgba::<u8>([3, 4, 5, 255]))
            .save(&image_path)
            .unwrap();
        let project = root.join("plain.acproj");
        std::fs::write(&project, serde_json::to_vec_pretty(&json!({"format":"animationcraft.project","version":1,"name":"Plain","assets":[
            {"id":"plain","name":"Plain","preset":null,"path":image_path,"kind":"image","parts":[]}],"workshop":{}})).unwrap()).unwrap();
        let mut plain = Session::default();
        plain.cwd = root.clone();
        plain
            .execute("project.open", json!({"path":project}))
            .unwrap();
        let tracks = plain
            .execute("motion.tracks", json!({"character":"plain"}))
            .unwrap();
        assert_eq!(tracks["layers"][0]["id"], "artwork");
        assert!(
            tracks["rig_sha256"].is_null(),
            "non-rig preview has no replacement precondition metadata"
        );
        assert!(
            tracks["original_reference"]
                .as_str()
                .unwrap()
                .ends_with("plain.png")
        );
        assert!(tracks["layers"][0]["source_size"].is_null());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_malformed_geometry_fails_without_panicking_or_touching_pixels() {
        let (mut session, root, rig, candidate, _) = fixture();
        let source = root.join("fin.png");
        let before = std::fs::read(&source).unwrap();
        let mut raw: Value = serde_json::from_slice(&std::fs::read(&rig).unwrap()).unwrap();
        raw["layers"]["fin"]["box"] = json!([]);
        std::fs::write(&rig, serde_json::to_vec(&raw).unwrap()).unwrap();
        let args = replace_args(&rig, &source, &candidate, 0);
        let error = session
            .execute("part.replace_image", args)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("explicit box"),
            "malformed edited rig must return an actionable error: {error}"
        );
        assert_eq!(std::fs::read(&source).unwrap(), before);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn first_asset_load_guards_nested_preview_cache_before_loader_writes() {
        let (mut session, root, rig, candidate, _) = fixture();
        let nested = root.join("assets/characters/fish");
        std::fs::create_dir_all(&nested).unwrap();
        for name in ["rig.json", "fin.png", "body.png"] {
            std::fs::rename(root.join(name), nested.join(name)).unwrap();
        }
        let nested_rig = nested.join("rig.json");
        session.project_assets[0].path = Some(nested_rig.clone());
        session.assets.clear();
        session.active_asset = None;
        let outside = nested.join("outside-cache");
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, nested.join(".layered_preview_cache")).unwrap();
        let args = replace_args(&nested_rig, &nested.join("fin.png"), &candidate, 0);
        assert!(
            session
                .execute("part.replace_image", args)
                .unwrap_err()
                .to_string()
                .contains("symlink")
        );
        assert_eq!(
            std::fs::read_dir(&outside).unwrap().count(),
            0,
            "preflight must run before the native loader writes cache PNGs"
        );
        std::fs::remove_file(nested.join(".layered_preview_cache")).unwrap();
        let cache = nested.join(".layered_preview_cache/keyed");
        std::fs::create_dir_all(&cache).unwrap();
        let protected = outside.join("fin.png");
        std::fs::write(&protected, b"unchanged").unwrap();
        std::os::unix::fs::symlink(&protected, cache.join("fin.png")).unwrap();
        let args = replace_args(&nested_rig, &nested.join("fin.png"), &candidate, 0);
        assert!(
            session
                .execute("part.replace_image", args)
                .unwrap_err()
                .to_string()
                .contains("non-regular file")
        );
        assert_eq!(std::fs::read(&protected).unwrap(), b"unchanged");
        let _ = std::fs::remove_dir_all(root);
        let _ = rig;
    }
}
