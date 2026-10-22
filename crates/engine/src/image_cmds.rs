//! Photocraft image-breaking commands: break apart into polygonal parts, puppet warp,
//! color-to-alpha, and content-aware fill. Each resolves the active asset, shells out to
//! `photocraft-cli`, and writes a derived PNG.

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

use crate::workspace::{build_photocraft_run_args, resolve_craft_cli, resolve_workspace_path, run_tool_in, workspace_root};

/// Mask the active asset's source into per-part PNGs from explicit `regions` polygons.
pub fn cmd_image_break_apart(s: &mut Session, params: &Value) -> Result<Value> {
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| EngineError::InvalidParams("no active asset to break apart".into()))?;

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

    Ok(json!({
        "ok": !produced_parts.is_empty(),
        "command": "image.break_apart",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "out_dir": out_dir.to_string_lossy(),
        "parts_produced": produced_parts.len(),
        "parts": produced_parts,
        "errors": errors,
    }))
}

/// Warp the active asset's source with explicit src/dst pins via PhotoCraft puppet warp.
pub fn cmd_image_puppet_warp(s: &mut Session, params: &Value) -> Result<Value> {
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| EngineError::InvalidParams("no active asset to warp".into()))?;

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
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| EngineError::InvalidParams("no active asset to convert".into()))?;

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
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| EngineError::InvalidParams("no active asset to fill".into()))?;

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
