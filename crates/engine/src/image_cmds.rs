//! Photocraft image-breaking commands: break apart into polygonal parts, puppet warp,
//! color-to-alpha, and content-aware fill. Each resolves the active asset, shells out to
//! `photocraft-cli`, and writes a derived PNG.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{EngineError, Result, Session};

use crate::workspace::{build_photocraft_run_args, resolve_craft_cli, resolve_workspace_path, run_tool_in, workspace_root};

/// Register a reviewed pose onto a shared canvas without distorting or clipping its silhouette.
pub fn cmd_image_register_frame(s: &mut Session, p: &Value) -> Result<Value> {
    let invalid = |message: &str| EngineError::InvalidParams(message.into());
    let pair = |value: &Value| -> Result<[f64; 2]> {
        let a = value.as_array().filter(|a| a.len() == 2)
            .ok_or_else(|| invalid("Pivots and canvas must be two-number arrays"))?;
        let number = |v: &Value| v.as_f64().filter(|v| v.is_finite())
            .ok_or_else(|| invalid("Coordinates must be finite numbers"));
        Ok([number(&a[0])?, number(&a[1])?])
    };
    let source = resolve_workspace_path(s, p["source"].as_str().ok_or_else(|| invalid("source image is required"))?);
    let out = resolve_workspace_path(s, p["out"].as_str().ok_or_else(|| invalid("out is required; originals are never overwritten"))?);
    if !out.extension().and_then(|v| v.to_str()).is_some_and(|v| v.eq_ignore_ascii_case("png")) {
        return Err(invalid("out must be PNG to preserve pose transparency"));
    }
    let receipt = out.with_extension("registration.json");
    if out.exists() || receipt.exists() { return Err(invalid("Registration output already exists; choose a new version")); }
    let canvas = pair(&p["canvas"])?;
    if canvas.iter().any(|v| v.fract() != 0.0 || !(16.0..=8192.0).contains(v)) || canvas[0]*canvas[1] > 16_777_216.0 {
        return Err(invalid("canvas must use integer dimensions 16..8192 and at most 16777216 pixels"));
    }
    let from = pair(&p["source_pivot"])?;
    let to = if p.get("target_pivot").is_some() { pair(&p["target_pivot"])? } else { [canvas[0]/2.0, canvas[1]/2.0] };
    let scale = p.get("scale").map(|v| v.as_f64().filter(|v| v.is_finite() && *v > 0.0 && *v <= 16.0)
        .ok_or_else(|| invalid("scale must be in (0,16]"))).transpose()?.unwrap_or(1.0);
    let (w, h) = image::image_dimensions(&source)?;
    if u64::from(w)*u64::from(h) > 16_777_216 { return Err(invalid("Source exceeds 16777216 pixels")); }
    let pixels = image::open(&source)?.to_rgba8();
    let mut bounds = [w, h, 0, 0];
    for (x, y, _) in pixels.enumerate_pixels().filter(|(_,_,p)| p[3] > 0) {
        bounds = [bounds[0].min(x), bounds[1].min(y), bounds[2].max(x+1), bounds[3].max(y+1)];
    }
    if bounds[2] == 0 { return Err(invalid("Source has no visible pixels")); }
    let placed = [to[0]+(bounds[0] as f64-from[0])*scale, to[1]+(bounds[1] as f64-from[1])*scale,
        to[0]+(bounds[2] as f64-from[0])*scale, to[1]+(bounds[3] as f64-from[1])*scale];
    if placed[0] < 0.0 || placed[1] < 0.0 || placed[2] > canvas[0] || placed[3] > canvas[1] {
        return Err(invalid("Registered silhouette would be clipped; enlarge canvas or adjust target_pivot/scale"));
    }
    let center = [to[0]+(w as f64/2.0-from[0])*scale, to[1]+(h as f64/2.0-from[1])*scale];
    if let Some(parent) = out.parent() { std::fs::create_dir_all(parent)?; }
    let root = workspace_root(s);
    run_tool_in(&root, "photocraft-cli", &[
        "run".into(), "--new".into(), json!({"width":canvas[0] as u32,"height":canvas[1] as u32,"background":"transparent"}).to_string(),
        "--cmd".into(), "file.placeEmbedded".into(), "--params".into(), json!({"path":source,"scale":scale*100.0,"fit":false,"center":center}).to_string(),
        "--out".into(), out.to_string_lossy().into_owned(),
    ])?;
    let decoded = image::open(&out)?;
    if [decoded.width(), decoded.height()] != [canvas[0] as u32, canvas[1] as u32] || !decoded.color().has_alpha() {
        return Err(invalid("PhotoCraft output does not match the requested transparent canvas; inspect retained output"));
    }
    let result = json!({"ok":true,"source":source,"output":out,"canvas":canvas,"source_pivot":from,
        "target_pivot":to,"scale":scale,"source_bounds":bounds,"placed_bounds":placed,
        "source_sha256":format!("{:x}",Sha256::digest(std::fs::read(&source)?)),"receipt":receipt});
    std::fs::write(&receipt, serde_json::to_vec_pretty(&result).map_err(|e| invalid(&e.to_string()))?)?;
    Ok(result)
}

/// Resolve the target asset ID and validated source image path for an `image.*` command.
fn resolve_asset_source(s: &Session, params: &Value, verb: &str) -> Result<(String, std::path::PathBuf)> {
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| EngineError::InvalidParams(format!("no active asset to {verb}")))?;

    let existing_asset = s.assets.get(&asset_id).cloned().ok_or_else(|| {
        EngineError::InvalidParams(format!("asset `{asset_id}` is not loaded in session"))
    })?;

    let source_path = if let Some(src_override) = params.get("source").and_then(Value::as_str) {
        resolve_workspace_path(s, src_override)
    } else {
        existing_asset.path.clone()
    };

    if !source_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "source image for asset `{asset_id}` does not exist: {}",
            source_path.display()
        )));
    }
    Ok((asset_id, source_path))
}

/// Mask the active asset's source into per-part PNGs from explicit `regions` polygons.
pub fn cmd_image_break_apart(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, source_path) = resolve_asset_source(s, params, "break apart")?;

    let root = workspace_root(s);
    let out_dir = if let Some(dir_str) = params.get("out_dir").and_then(Value::as_str) {
        resolve_workspace_path(s, dir_str)
    } else {
        root.join("exports/pose_and_parts").join(format!("{asset_id}_broken_apart"))
    };
    std::fs::create_dir_all(&out_dir)?;

    // Each part is the source masked by an explicit polygon from `regions`
    // (part id -> polygon points). Without regions every "part" would be a copy of
    // the whole image, so regions are required and any failed step fails that part.
    let regions = params
        .get("regions")
        .and_then(Value::as_object)
        .filter(|r| !r.is_empty())
        .ok_or_else(|| {
            EngineError::InvalidParams(
                "image.break_apart requires `regions`: a map of part id to a polygon, or {polygons, keep}".into(),
            )
        })?;
    let parts: Vec<String> = regions.keys().cloned().collect();

    let bin = resolve_craft_cli("photocraft-cli");

    let mut produced_parts: Vec<Value> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for part_id in &parts {
        let part_out = out_dir.join(format!("{part_id}.png"));

        // One PhotoCraft session per part. A region is a polygon ([[x, y], ...]) or an object
        // {"polygons": [polygon, ...], "keep": "inside" | "outside"}: select the polygon(s), then clear
        // everything else (inside) or the selection itself (outside). The source is already matted at
        // intake, so no background removal happens here.
        let region = &regions[part_id.as_str()];
        let (polygons, keep_outside): (Vec<Value>, bool) = match region {
            Value::Object(obj) => (
                obj.get("polygons")
                    .and_then(Value::as_array)
                    .cloned()
                    .or_else(|| obj.get("points").map(|p| vec![p.clone()]))
                    .unwrap_or_default(),
                obj.get("keep").and_then(Value::as_str) == Some("outside"),
            ),
            other => (vec![other.clone()], false),
        };
        if polygons.is_empty() {
            errors.push(format!("{part_id}: region has no polygon"));
            continue;
        }
        let mut args = vec![
            "run".to_string(),
            source_path.to_string_lossy().into_owned(),
        ];
        for (i, polygon) in polygons.iter().enumerate() {
            args.extend([
                "--cmd".to_string(),
                "select.lasso".to_string(),
                "--params".to_string(),
                json!({"points": polygon, "mode": if i == 0 { "replace" } else { "add" }}).to_string(),
            ]);
        }
        if !keep_outside {
            args.extend(["--cmd".to_string(), "select.inverse".to_string()]);
        }
        args.extend([
            "--cmd".to_string(),
            "edit.clear".to_string(),
            "--out".to_string(),
            part_out.to_string_lossy().into_owned(),
        ]);
        if let Err(e) = run_tool_in(&root, &bin, &args) {
            errors.push(format!("{part_id}: {e}"));
            continue;
        }

        if part_out.is_file() {
            let (w, h) = image::image_dimensions(&part_out).unwrap_or((0, 0));
            produced_parts.push(json!({
                "part": part_id,
                "label": part_id,
                "file": part_out.to_string_lossy(),
                "width": w,
                "height": h,
            }));
        } else {
            errors.push(format!("{part_id}: no output file written"));
        }
    }

    // `rig_order` (part ids, back to front) also writes the parts' layered rig, `rig.json`, for
    // project.add_asset kind=cutout_rig: the largest part is the root, the others ride on it, and each
    // pivot starts at its part's centre (refined later in the Animation view). It carries a gentle
    // "rest" track for motion.path to draw on.
    let rig = match params.get("rig_order").and_then(Value::as_array) {
        Some(order) if errors.is_empty() => Some(write_parts_rig(&out_dir, &asset_id, order, None)?),
        Some(_) => { errors.push("rig.json not written: some parts failed".into()); None }
        None => None,
    };

    Ok(json!({
        "ok": !produced_parts.is_empty(),
        "command": "image.break_apart",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "out_dir": out_dir.to_string_lossy(),
        "parts_produced": produced_parts.len(),
        "parts": produced_parts,
        "rig": rig,
        "errors": errors,
    }))
}

/// `joints` (rig.from_sheet's placed parts: id, parent, pivot) replaces the centre pivots and the
/// largest-part root with the placed joints and parents, and seeds initial timed 2D tracks
/// (`rest`, `walk`, `wave`, `jump`) so the cutout rig immediately has fluid multi-part motion.
fn write_parts_rig(out_dir: &std::path::Path, asset_id: &str, order: &[Value], joints: Option<&[Value]>) -> Result<std::path::PathBuf> {
    let mut layers = Vec::new();
    let mut canvas = (0, 0);
    for id in order {
        let id = id.as_str().ok_or_else(|| EngineError::InvalidParams("rig_order holds part ids".into()))?;
        let file = out_dir.join(format!("{id}.png"));
        let image = image::open(&file).map_err(|e| EngineError::InvalidParams(format!("rig_order: {id}: {e}")))?.into_rgba8();
        canvas = image.dimensions();
        let (mut min, mut max, mut area) = ((u32::MAX, u32::MAX), (0, 0), 0u64);
        for (x, y, _) in image.enumerate_pixels().filter(|(_, _, p)| p[3] > 16) {
            min = (min.0.min(x), min.1.min(y)); max = (max.0.max(x), max.1.max(y)); area += 1;
        }
        if area == 0 { return Err(EngineError::InvalidParams(format!("rig_order: part {id} is empty"))); }
        layers.push((id.to_string(), file, [(min.0 + max.0) as f64 / 2.0, (min.1 + max.1) as f64 / 2.0], area));
    }
    let joint = |id: &str| joints.and_then(|j| j.iter().find(|p| p["id"].as_str() == Some(id)));
    let root = joints
        .and_then(|j| j.iter().find(|p| p["parent"].is_null()).and_then(|p| p["id"].as_str()).map(str::to_string))
        .unwrap_or_else(|| layers.iter().max_by_key(|l| l.3).map(|l| l.0.clone()).unwrap_or_default());

    let mut rest_motion = serde_json::Map::new();
    let mut walk_motion = serde_json::Map::new();
    let mut wave_motion = serde_json::Map::new();
    let mut jump_motion = serde_json::Map::new();

    for (id, _, _, _) in &layers {
        let low = id.to_ascii_lowercase();
        if *id == root {
            rest_motion.insert(id.clone(), json!({
                "position": [{"kind": "oscillate", "axis": 1, "amplitude": 4.0, "hz": 0.5, "phase": 0.0}],
                "rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 1.2, "hz": 0.5, "phase": 0.25}]
            }));
            walk_motion.insert(id.clone(), json!({
                "position": [{"kind": "oscillate", "axis": 1, "amplitude": 6.0, "hz": 2.0, "phase": 0.0}],
                "rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 2.5, "hz": 1.0, "phase": 0.0}]
            }));
            wave_motion.insert(id.clone(), json!({
                "position": [{"kind": "oscillate", "axis": 1, "amplitude": 3.0, "hz": 0.75, "phase": 0.0}],
                "rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 1.8, "hz": 0.75, "phase": 0.15}]
            }));
            jump_motion.insert(id.clone(), json!({
                "position": [{"kind": "oscillate", "axis": 1, "amplitude": 22.0, "hz": 1.0, "phase": 0.0}],
                "rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 3.5, "hz": 1.0, "phase": 0.2}]
            }));
        } else if low.contains("head") {
            rest_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 1.8, "hz": 0.5, "phase": 0.15}]}));
            walk_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 3.0, "hz": 1.0, "phase": 0.2}]}));
            wave_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 4.5, "hz": 1.2, "phase": 0.1}]}));
            jump_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 5.0, "hz": 1.0, "phase": 0.25}]}));
        } else if low.contains("ear") || low.contains("tail") || low.contains("hair") || low.contains("prop") {
            // Secondary follow-through with phase lag relative to parent gesture
            let phase = if low.contains("_r") || low.contains("right") { 0.42 } else { 0.32 };
            rest_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 3.5, "hz": 0.5, "phase": phase}]}));
            walk_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 9.0, "hz": 1.0, "phase": phase}]}));
            wave_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 6.5, "hz": 1.2, "phase": phase}]}));
            jump_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 12.0, "hz": 1.0, "phase": phase}]}));
        } else if low.contains("arm") || low.contains("wing") {
            let is_left = low.contains("_l") || low.contains("left");
            let phase = if is_left { 0.0 } else { 0.5 };
            walk_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 16.0, "hz": 1.0, "phase": phase}]}));
            if is_left {
                wave_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 26.0, "hz": 1.8, "phase": 0.0}]}));
            }
            jump_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 22.0, "hz": 1.0, "phase": 0.1}]}));
        } else if low.contains("leg") || low.contains("foot") {
            let is_left = low.contains("_l") || low.contains("left");
            let phase = if is_left { 0.5 } else { 0.0 };
            walk_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 18.0, "hz": 1.0, "phase": phase}]}));
            jump_motion.insert(id.clone(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 14.0, "hz": 1.0, "phase": 0.0}]}));
        }
    }

    let tracks = vec![
        json!({"id": "rest", "label": "Rest · gentle bob", "duration": 4, "loop": true, "motion": rest_motion}),
        json!({"id": "walk", "label": "Walk · stride cycle", "duration": 3, "loop": true, "motion": walk_motion}),
        json!({"id": "wave", "label": "Wave · greeting", "duration": 3, "loop": true, "motion": wave_motion}),
        json!({"id": "jump", "label": "Jump · hop", "duration": 2, "loop": true, "motion": jump_motion}),
    ];
    let rig = json!({"rig_type": "layered_motion", "character_id": asset_id, "canvas": [canvas.0, canvas.1], "fps": 30, "tracks": tracks,
        "layers": layers.iter().map(|(id, file, pivot, _)| match joint(id) {
            Some(j) => json!({"id": id, "file": file, "pivot": j["pivot"], "parent": j["parent"]}),
            None => json!({"id": id, "file": file, "pivot": pivot, "parent": if *id == root { Value::Null } else { json!(root) }}),
        }).collect::<Vec<_>>()});
    let path = out_dir.join("rig.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&rig).map_err(|e| EngineError::InvalidParams(e.to_string()))?)?;
    Ok(path)
}

/// A rig sheet (either from `image.generate rig_parts=...` with adjacent `generation.json`, or an existing
/// user sheet with explicit `parts=[{id, parent, pivot}]`) becomes a layered rig: `pipeline/rig_from_sheet.py`
/// cuts the cells, places each part where it sits on the assembled asset and finds the joints; `rig.json` is then
/// written with initial timed 2D tracks (`rest`, `walk`, `wave`, `jump`). Connect it with `project.add_asset kind=cutout_rig`.
pub fn cmd_rig_from_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    let invalid = |message: String| EngineError::InvalidParams(message);
    let sheet_ref = p["sheet"].as_str().filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("rig.from_sheet needs sheet=<rig sheet asset id or PNG path>".into()))?;
    let sheet = match s.project_assets.iter().find(|a| a.id == sheet_ref).and_then(|a| a.path.clone()) {
        Some(path) => path,
        None => resolve_workspace_path(s, sheet_ref),
    };
    if !sheet.is_file() {
        return Err(invalid(format!("rig sheet not found: {}", sheet.display())));
    }
    let receipt = sheet.with_file_name("generation.json");
    let explicit_parts = p.get("parts").or_else(|| p.get("rig_parts")).filter(|v| !v.is_null());
    if !receipt.is_file() && explicit_parts.is_none() {
        return Err(invalid(format!(
            "{} has no adjacent generation.json; pass `parts=[{{\"id\":...,\"parent\":...,\"pivot\":[u,v]}}]` for an existing user sheet, or generate one with `image.generate rig_parts=...`",
            sheet.display()
        )));
    }
    let name = crate::workspace::sanitize_asset_id(p["name"].as_str().unwrap_or("rig"));
    let out_dir = match p["out_dir"].as_str() {
        Some(dir) => resolve_workspace_path(s, dir),
        None => crate::workshop::project_dir(s)?.join("assets/characters").join(&name).join("rig"),
    };
    let root = workspace_root(s);
    let script = root.join("pipeline/rig_from_sheet.py");
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--sheet".into(),
        sheet.to_string_lossy().into_owned(),
        "--out-dir".into(),
        out_dir.to_string_lossy().into_owned(),
    ];
    if receipt.is_file() {
        args.extend(["--receipt".into(), receipt.to_string_lossy().into_owned()]);
    }
    if let Some(parts) = explicit_parts {
        if !parts.as_array().is_some_and(|a| (2..=15).contains(&a.len()) && a.iter().all(|item| item.get("id").and_then(Value::as_str).is_some_and(|id| !id.is_empty()))) {
            return Err(invalid("parts must be an array of 2 to 15 part objects with non-empty `id`".into()));
        }
        args.extend(["--parts".into(), parts.to_string()]);
    }
    if let Some(ref_raw) = p["reference"].as_str().filter(|v| !v.is_empty()) {
        let ref_path = resolve_workspace_path(s, ref_raw);
        if !ref_path.is_file() {
            return Err(invalid(format!("reference image not found: {}", ref_path.display())));
        }
        args.extend(["--reference".into(), ref_path.to_string_lossy().into_owned()]);
    }
    if let Some(pivots) = p.get("pivots").filter(|v| !v.is_null()) {
        if !pivots.as_object().is_some_and(|m| m.values().all(|v| v.as_array().is_some_and(|a| a.len() == 2 && a.iter().all(|n| n.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)))))) {
            return Err(invalid("pivots maps part ids to [u, v] inside the part, each 0 to 1".into()));
        }
        args.extend(["--pivots".into(), pivots.to_string()]);
    }
    if let Some(places) = p.get("places").filter(|v| !v.is_null()) {
        if !places.as_object().is_some_and(|m| m.values().all(|v| v.as_array().is_some_and(|a| a.len() == 2 && a.iter().all(|n| n.as_f64().is_some_and(f64::is_finite))))) {
            return Err(invalid("places maps part ids to [cx, cy], the part's centre in canvas pixels".into()));
        }
        args.extend(["--places".into(), places.to_string()]);
    }
    if let Some(order) = p.get("order").filter(|v| !v.is_null()) {
        if !order.as_array().is_some_and(|a| a.iter().all(Value::is_string)) {
            return Err(invalid("order lists part ids back to front".into()));
        }
        args.extend(["--order".into(), order.to_string()]);
    }
    let stdout = run_tool_in(&root, "python3", &args)?;
    let mut placement: Value = stdout.lines().rev().find_map(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| EngineError::CommandFailed("rig.from_sheet".into(), "rig_from_sheet.py returned no result".into()))?;
    let order = placement["order"].as_array().cloned().unwrap_or_default();
    let joints = placement["parts"].as_array().cloned().unwrap_or_default();
    let rig = write_parts_rig(&out_dir, &name, &order, Some(&joints))?;
    placement["command"] = json!("rig.from_sheet");
    placement["rig"] = json!(rig);
    placement["next"] = json!(format!("project.add_asset path={} kind=cutout_rig id={name} name={name}", rig.display()));
    Ok(placement)
}

/// Surgical PhotoCraft recut of a single rigged part from its clean source art (`pipeline/recut_part.py`).
pub fn cmd_part_recut(s: &mut Session, p: &Value) -> Result<Value> {
    let invalid = |msg: String| EngineError::InvalidParams(msg);
    let source_raw = p["source"].as_str().filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("part.recut needs source=<clean source PNG>".into()))?;
    let part_raw = p["part"].as_str().filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("part.recut needs part=<existing part PNG>".into()))?;
    let out_raw = p["out"].as_str().filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("part.recut needs out=<output part PNG>".into()))?;
    let at_val = p.get("at").ok_or_else(|| invalid("part.recut needs at=[x,y] or \"x,y\"".into()))?;
    let at_str = if let Some(s_at) = at_val.as_str() {
        s_at.to_string()
    } else if let Some(arr) = at_val.as_array().filter(|a| a.len() == 2) {
        format!("{},{}", arr[0].as_f64().unwrap_or(0.0) as i64, arr[1].as_f64().unwrap_or(0.0) as i64)
    } else {
        return Err(invalid("`at` must be [x, y] or \"x,y\"".into()));
    };
    let source = resolve_workspace_path(s, source_raw);
    let part = resolve_workspace_path(s, part_raw);
    let out = resolve_workspace_path(s, out_raw);
    if !source.is_file() {
        return Err(invalid(format!("source PNG not found: {}", source.display())));
    }
    if !part.is_file() {
        return Err(invalid(format!("part PNG not found: {}", part.display())));
    }
    let root = workspace_root(s);
    let script = root.join("pipeline/recut_part.py");
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--source".into(),
        source.to_string_lossy().into_owned(),
        "--part".into(),
        part.to_string_lossy().into_owned(),
        "--at".into(),
        at_str,
        "--out".into(),
        out.to_string_lossy().into_owned(),
    ];
    if let Some(scale) = p["scale"].as_f64() {
        args.extend(["--scale".into(), scale.to_string()]);
    }
    if let Some(expand) = p["expand"].as_i64() {
        args.extend(["--expand".into(), expand.to_string()]);
    }
    if let Some(smooth) = p["smooth"].as_i64() {
        args.extend(["--smooth".into(), smooth.to_string()]);
    }
    if let Some(behind) = p["behind"].as_array() {
        for item in behind {
            if let Some(spec) = item.as_str().filter(|v| !v.is_empty()) {
                args.extend(["--behind".into(), spec.to_string()]);
            }
        }
    }
    let stdout = run_tool_in(&root, "python3", &args)?;
    let mut res: Value = stdout.lines().rev().find_map(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| EngineError::CommandFailed("part.recut".into(), "recut_part.py returned no JSON".into()))?;
    res["command"] = json!("part.recut");
    Ok(res)
}

/// Normalize a non-standard turnaround or action pose sheet (`pipeline/step_sheet_normalizer.py`):
/// detects panels, resizes cells via PhotoCraft, labels via Hybrid OCR + Jev Vision fallback,
/// reorders into canonical slots, and expands directional walk_right/walk_left slots.
pub fn cmd_sheet_normalize(s: &mut Session, p: &Value) -> Result<Value> {
    let invalid = |msg: String| EngineError::InvalidParams(msg);
    let source_raw = p["source"].as_str().filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("sheet.normalize needs source=<sheet PNG/JPEG path>".into()))?;
    let source = resolve_workspace_path(s, source_raw);
    if !source.is_file() {
        return Err(invalid(format!("sheet source not found: {}", source.display())));
    }
    let character = crate::workspace::sanitize_asset_id(p["character"].as_str().unwrap_or("character"));
    let kind = p["kind"].as_str().unwrap_or("auto");
    let out_dir = match p["out_dir"].as_str().filter(|v| !v.is_empty()) {
        Some(dir) => resolve_workspace_path(s, dir),
        None => workspace_root(s).join("exports/pose_and_parts").join(format!("{character}_{kind}_normalized")),
    };
    let root = workspace_root(s);
    let script = root.join("pipeline/step_sheet_normalizer.py");
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--source".into(),
        source.to_string_lossy().into_owned(),
        "--out-dir".into(),
        out_dir.to_string_lossy().into_owned(),
        "--kind".into(),
        kind.into(),
        "--character".into(),
        character,
    ];
    if let Some(cw) = p["cell_width"].as_i64() {
        args.extend(["--cell-width".into(), cw.to_string()]);
    }
    if let Some(ch) = p["cell_height"].as_i64() {
        args.extend(["--cell-height".into(), ch.to_string()]);
    }
    if p["expand_directions"].as_bool() == Some(false) {
        args.push("--no-expand-directions".into());
    }
    if let Some(tol) = p["tolerance"].as_i64() {
        args.extend(["--tolerance".into(), tol.to_string()]);
    }
    let stdout = run_tool_in(&root, "python3", &args)?;
    let mut res: Value = serde_json::from_str(&stdout)
        .or_else(|_| stdout.lines().rev().find_map(|line| serde_json::from_str(line).ok()).ok_or(()))
        .map_err(|_| EngineError::CommandFailed("sheet.normalize".into(), "step_sheet_normalizer.py returned invalid JSON".into()))?;
    res["command"] = json!("sheet.normalize");
    Ok(res)
}

/// Warp the active asset's source with explicit src/dst pins via PhotoCraft puppet warp.
pub fn cmd_image_puppet_warp(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, source_path) = resolve_asset_source(s, params, "warp")?;

    let root = workspace_root(s);
    let out_path = if let Some(out_raw) = params.get("out").and_then(Value::as_str) {
        resolve_workspace_path(s, out_raw)
    } else {
        root.join("exports/pose_and_parts").join(format!("{asset_id}_warped.png"))
    };
    std::fs::create_dir_all(out_path.parent().unwrap_or(&root))?;

    // Pins come from the caller (e.g. rig joints); there is no sensible default.
    let pins: Vec<Value> = params
        .get("pins")
        .and_then(Value::as_array)
        .filter(|p| !p.is_empty())
        .cloned()
        .ok_or_else(|| {
            EngineError::InvalidParams(
                "image.puppet_warp requires `pins`: [{\"src\":[x,y],\"dst\":[x,y]}, ...]".into(),
            )
        })?;

    let warp_params = json!({
        "pins": pins,
        "mode": params.get("mode").and_then(Value::as_str).unwrap_or("normal"),
        "density": params.get("density").and_then(Value::as_str).unwrap_or("normal"),
        "expansion": params.get("expansion").and_then(Value::as_f64).unwrap_or(2.0),
    });
    let args = build_photocraft_run_args(
        &source_path.to_string_lossy(),
        "edit.puppetWarp",
        Some(&warp_params.to_string()),
        &out_path.to_string_lossy(),
    );

    let bin = resolve_craft_cli("photocraft-cli");
    run_tool_in(&root, &bin, &args)?;

    let (w, h) = if out_path.is_file() {
        image::image_dimensions(&out_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    Ok(json!({
        "ok": out_path.is_file(),
        "command": "image.puppet_warp",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "output": out_path.to_string_lossy(),
        "width": w,
        "height": h,
        "pins": pins,
    }))
}

/// Convert a color to alpha on the active asset's source via PhotoCraft.
pub fn cmd_image_color_to_alpha(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, source_path) = resolve_asset_source(s, params, "convert")?;

    let root = workspace_root(s);
    let out_path = if let Some(out_raw) = params.get("out").and_then(Value::as_str) {
        resolve_workspace_path(s, out_raw)
    } else {
        root.join("exports/pose_and_parts").join(format!("{asset_id}_alpha.png"))
    };
    std::fs::create_dir_all(out_path.parent().unwrap_or(&root))?;

    let color_raw = params
        .get("color")
        .and_then(Value::as_str)
        .unwrap_or("#FFFFFF");

    let transparency_threshold = params
        .get("transparencyThreshold")
        .and_then(Value::as_f64)
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let opacity_threshold = params
        .get("opacityThreshold")
        .and_then(Value::as_f64)
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(1.0);

    let color_params = json!({
        "color": color_raw,
        "transparencyThreshold": transparency_threshold,
        "opacityThreshold": opacity_threshold,
    });
    let args = build_photocraft_run_args(
        &source_path.to_string_lossy(),
        "filter.other.colorToAlpha",
        Some(&color_params.to_string()),
        &out_path.to_string_lossy(),
    );

    let bin = resolve_craft_cli("photocraft-cli");
    run_tool_in(&root, &bin, &args)?;

    let (w, h) = if out_path.is_file() {
        image::image_dimensions(&out_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    Ok(json!({
        "ok": out_path.is_file(),
        "command": "image.color_to_alpha",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "output": out_path.to_string_lossy(),
        "width": w,
        "height": h,
        "color": color_raw,
        "transparencyThreshold": transparency_threshold,
        "opacityThreshold": opacity_threshold,
    }))
}

/// Content-aware fill on a named selection of the active asset's source via PhotoCraft.
pub fn cmd_image_content_aware_fill(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, source_path) = resolve_asset_source(s, params, "fill")?;

    let root = workspace_root(s);
    let out_path = if let Some(out_raw) = params.get("out").and_then(Value::as_str) {
        resolve_workspace_path(s, out_raw)
    } else {
        root.join("exports/pose_and_parts").join(format!("{asset_id}_filled.png"))
    };
    std::fs::create_dir_all(out_path.parent().unwrap_or(&root))?;

    // edit.fill acts on the current selection, so the caller names the selection command
    // (e.g. select.rect, select.colorRange) that runs first in the same document session.
    let select = params
        .get("select")
        .filter(|v| v.get("id").and_then(Value::as_str).is_some())
        .ok_or_else(|| {
            EngineError::InvalidParams(
                "image.content_aware_fill requires `select`: {\"id\":\"select.*\",\"params\":{...}}".into(),
            )
        })?;
    let select_id = select["id"].as_str().unwrap_or_default();
    let fill_contents = params.get("contents").and_then(Value::as_str).unwrap_or("contentAware");
    let color = params.get("color").and_then(Value::as_str);

    let mut fill_params = serde_json::Map::new();
    fill_params.insert("contents".to_string(), json!(fill_contents));
    if let Some(c) = color {
        fill_params.insert("color".to_string(), json!(c));
    }

    let fill_params_json = serde_json::to_string(&fill_params)
        .map_err(|e| EngineError::InvalidParams(format!("cannot serialize fill params: {e}")))?;
    let mut args = vec![
        "run".to_string(),
        source_path.to_string_lossy().into_owned(),
        "--cmd".to_string(),
        select_id.to_string(),
    ];
    if let Some(p) = select.get("params") {
        args.push("--params".to_string());
        args.push(p.to_string());
    }
    args.extend([
        "--cmd".to_string(),
        "edit.fill".to_string(),
        "--params".to_string(),
        fill_params_json,
        "--out".to_string(),
        out_path.to_string_lossy().into_owned(),
    ]);

    let bin = resolve_craft_cli("photocraft-cli");
    run_tool_in(&root, &bin, &args)?;

    let (w, h) = if out_path.is_file() {
        image::image_dimensions(&out_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    Ok(json!({
        "ok": out_path.is_file(),
        "command": "image.content_aware_fill",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "output": out_path.to_string_lossy(),
        "width": w,
        "height": h,
        "select": select_id,
        "contents": fill_contents,
        "color": color,
    }))
}

/// Normalise a sheet to the contract canvas via PhotoCraft `image.imageSize`, then report it.
/// Generated artwork arrives at whatever size the image tool chose; the sheet contract is fixed.
pub fn cmd_image_image_size(s: &mut Session, params: &Value) -> Result<Value> {
    let source = params
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("image.image_size requires `source`".into()))?;
    let source_path = resolve_workspace_path(s, source);
    if !source_path.is_file() {
        return Err(EngineError::InvalidParams(format!("image not found: {}", source_path.display())));
    }
    let width = params.get("width").and_then(Value::as_u64).unwrap_or(2048).clamp(16, 8192);
    let height = params.get("height").and_then(Value::as_u64).unwrap_or(2048).clamp(16, 8192);
    let anchor = params.get("anchor").and_then(Value::as_str).unwrap_or("center");
    let extension = params.get("extensionColor").and_then(Value::as_str).unwrap_or("#ffffff");
    let out_path = match params.get("out").and_then(Value::as_str).filter(|o| !o.is_empty()) {
        Some(out) => resolve_workspace_path(s, out),
        None => source_path.with_file_name(format!(
            "{}-{width}x{height}.png",
            source_path.file_stem().and_then(|v| v.to_str()).unwrap_or("image")
        )),
    };
    let root = workspace_root(s);
    if let Some(dir) = out_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let body = json!({"width": width, "height": height, "anchor": anchor, "extensionColor": extension}).to_string();
    let args = build_photocraft_run_args(
        &source_path.to_string_lossy(),
        "image.imageSize",
        Some(&body),
        &out_path.to_string_lossy(),
    );
    let bin = resolve_craft_cli("photocraft-cli");
    run_tool_in(&root, &bin, &args)?;
    let (w, h) = if out_path.is_file() {
        image::image_dimensions(&out_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    Ok(json!({
        "ok": out_path.is_file() && w == width as u32 && h == height as u32,
        "command": "image.image_size",
        "source": source_path.to_string_lossy(),
        "output": out_path.to_string_lossy(),
        "width": w,
        "height": h,
    }))
}
