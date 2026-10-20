//! In-memory pixel analysis: `analyze_rgba_image` and `analyze_viewport_region` run without
//! external screenshot/review steps, and `vision.analyze` dispatches them against a loaded asset.

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

use crate::workspace::resolve_workspace_path;

/// Analyze an RGBA sprite/layer image directly in memory without external screenshot/review steps.
pub fn analyze_rgba_image(img: &image::RgbaImage) -> Value {
    let w = img.width();
    let h = img.height();
    let total_px = (w as u64) * (h as u64);
    if total_px == 0 {
        return json!({
            "width": w,
            "height": h,
            "opaque_px": 0,
            "coverage_pct": 0.0,
            "non_empty": false,
        });
    }

    let mut opaque_px: u64 = 0;
    let mut fringe_px: u64 = 0;
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut sum_x: u64 = 0;
    let mut sum_y: u64 = 0;
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;
    let mut border_opaque_px: u64 = 0;

    for (x, y, px) in img.enumerate_pixels() {
        let [r, g, b, a] = px.0;
        if a > 16 {
            opaque_px += 1;
            if a < 240 {
                fringe_px += 1;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            sum_x += x as u64;
            sum_y += y as u64;
            sum_r += r as u64;
            sum_g += g as u64;
            sum_b += b as u64;
            if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
                border_opaque_px += 1;
            }
        }
    }

    if opaque_px == 0 {
        return json!({
            "width": w,
            "height": h,
            "opaque_px": 0,
            "coverage_pct": 0.0,
            "non_empty": false,
        });
    }

    let bbox_w = max_x.saturating_sub(min_x) + 1;
    let bbox_h = max_y.saturating_sub(min_y) + 1;
    let cx = (sum_x as f64) / (opaque_px as f64);
    let cy = (sum_y as f64) / (opaque_px as f64);
    let coverage_pct = ((opaque_px as f64) * 100.0) / (total_px as f64);
    let fringe_ratio = (fringe_px as f64) / (opaque_px as f64);

    json!({
        "width": w,
        "height": h,
        "non_empty": true,
        "opaque_px": opaque_px,
        "coverage_pct": (coverage_pct * 100.0).round() / 100.0,
        "fringe_ratio": (fringe_ratio * 1000.0).round() / 1000.0,
        "bbox": [min_x, min_y, bbox_w, bbox_h],
        "centroid": [(cx * 10.0).round() / 10.0, (cy * 10.0).round() / 10.0],
        "foot_contact_y": max_y,
        "head_top_y": min_y,
        "border_clipped": border_opaque_px > 12,
        "mean_rgb": [
            sum_r / opaque_px,
            sum_g / opaque_px,
            sum_b / opaque_px,
        ],
    })
}

/// Analyze a rendered viewport region (a pixel counts as foreground only when it differs from every
/// backdrop color in `bg_colors`, e.g. the stage fill and its grid lines) so agents get immediate
/// bounding-box, centroid, coverage, and color telemetry without downloading a screenshot.
pub fn analyze_viewport_region(rgba: &[u8], w: u32, h: u32, bg_colors: &[[u8; 3]]) -> Value {
    let total_px = (w as u64) * (h as u64);
    if total_px == 0 || rgba.len() < (total_px as usize) * 4 {
        return json!({
            "width": w,
            "height": h,
            "foreground_px": 0,
            "coverage_pct": 0.0,
            "visible": false,
        });
    }

    let mut fg_px: u64 = 0;
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut sum_x: u64 = 0;
    let mut sum_y: u64 = 0;
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;

    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let r = rgba[idx];
            let g = rgba[idx + 1];
            let b = rgba[idx + 2];
            // Exclude backdrop and subtle grid lines (delta > 36 from every backdrop color)
            let is_fg = bg_colors.iter().all(|bg| {
                (r as i32 - bg[0] as i32).abs()
                    + (g as i32 - bg[1] as i32).abs()
                    + (b as i32 - bg[2] as i32).abs()
                    > 36
            });
            if is_fg {
                fg_px += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                sum_x += x as u64;
                sum_y += y as u64;
                sum_r += r as u64;
                sum_g += g as u64;
                sum_b += b as u64;
            }
        }
    }

    if fg_px == 0 {
        return json!({
            "width": w,
            "height": h,
            "foreground_px": 0,
            "coverage_pct": 0.0,
            "visible": false,
        });
    }

    let bbox_w = max_x.saturating_sub(min_x) + 1;
    let bbox_h = max_y.saturating_sub(min_y) + 1;
    let cx = (sum_x as f64) / (fg_px as f64);
    let cy = (sum_y as f64) / (fg_px as f64);
    let coverage_pct = ((fg_px as f64) * 100.0) / (total_px as f64);

    json!({
        "width": w,
        "height": h,
        "visible": true,
        "foreground_px": fg_px,
        "coverage_pct": (coverage_pct * 100.0).round() / 100.0,
        "character_bbox_px": [min_x, min_y, bbox_w, bbox_h],
        "character_centroid_px": [(cx * 10.0).round() / 10.0, (cy * 10.0).round() / 10.0],
        "foot_contact_y_px": max_y,
        "head_top_y_px": min_y,
        "mean_rgb": [
            sum_r / fg_px,
            sum_g / fg_px,
            sum_b / fg_px,
        ],
    })
}

/// Analyze a single image or all frames/parts of a loaded character asset.
pub fn cmd_vision_analyze(s: &mut Session, params: &Value) -> Result<Value> {
    if let Some(img_raw) = params.get("image").and_then(Value::as_str) {
        let resolved = resolve_workspace_path(s, img_raw);
        let img = image::open(&resolved)
            .map_err(|e| {
                EngineError::InvalidParams(format!("cannot open {}: {e}", resolved.display()))
            })?
            .into_rgba8();
        let metrics = analyze_rgba_image(&img);
        return Ok(json!({
            "ok": true,
            "command": "vision.analyze",
            "mode": "single_image",
            "path": resolved,
            "vision": metrics,
        }));
    }

    let character = params
        .get("character")
        .or_else(|| params.get("preset"))
        .or_else(|| params.get("asset"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .unwrap_or_else(|| "hopper".to_string());

    if !s.assets.contains_key(&character) {
        let _ = crate::photocraft::cmd_asset_load_manifest(s, &json!({ "preset": character }));
    }

    let asset = s.assets.get(&character).cloned().ok_or_else(|| {
        EngineError::InvalidParams(format!("character `{character}` could not be loaded"))
    })?;

    let mut turnaround_views = Vec::new();
    let mut foot_ys = Vec::new();
    if let Some(tm) = &asset.turnaround_manifest {
        for frame in &tm.frames {
            if let Ok(dyn_img) = image::open(&frame.resolved_path) {
                let rgba = dyn_img.into_rgba8();
                let m = analyze_rgba_image(&rgba);
                if let Some(fy) = m.get("foot_contact_y").and_then(Value::as_u64) {
                    foot_ys.push(fy as i64);
                }
                turnaround_views.push(json!({
                    "index": frame.index,
                    "label": frame.label,
                    "path": frame.resolved_path,
                    "metrics": m,
                }));
            }
        }
    }
    let measured_pixel_drift_y_px = if foot_ys.len() >= 2 {
        let min_fy = foot_ys.iter().copied().min().unwrap_or(0);
        let max_fy = foot_ys.iter().copied().max().unwrap_or(0);
        Some(max_fy - min_fy)
    } else {
        None
    };

    let mut pose_frames = Vec::new();
    let mut standing_foot_y: Option<i64> = None;
    if let Some(pm) = &asset.pose_manifest {
        for frame in &pm.frames {
            if let Ok(dyn_img) = image::open(&frame.resolved_path) {
                let rgba = dyn_img.into_rgba8();
                let m = analyze_rgba_image(&rgba);
                let fy = m
                    .get("foot_contact_y")
                    .and_then(Value::as_u64)
                    .map(|v| v as i64);
                if frame.index == 0 || frame.label.contains("standing") {
                    standing_foot_y = standing_foot_y.or(fy);
                }
                let lift_px = match (standing_foot_y, fy) {
                    (Some(base), Some(cur)) => Some((base - cur).max(0)),
                    _ => None,
                };
                pose_frames.push(json!({
                    "index": frame.index,
                    "label": frame.label,
                    "path": frame.resolved_path,
                    "foot_lift_from_standing_px": lift_px,
                    "metrics": m,
                }));
            }
        }
    }

    let mut part_layers = Vec::new();
    let parts_source = asset
        .turnaround_manifest
        .as_ref()
        .and_then(|m| m.frames.first().filter(|f| !f.parts.is_empty()))
        .or_else(|| asset.pose_manifest.as_ref().and_then(|m| m.frames.first()));
    if let Some(f0) = parts_source {
        for (part_name, part_meta) in &f0.parts {
            if let Ok(dyn_img) = image::open(&part_meta.resolved_path) {
                let rgba = dyn_img.into_rgba8();
                let m = analyze_rgba_image(&rgba);
                part_layers.push(json!({
                    "part": part_name,
                    "path": part_meta.resolved_path,
                    "pivot": part_meta.pivot,
                    "bind_position": part_meta.bind_position,
                    "metrics": m,
                }));
            }
        }
    }

    Ok(json!({
        "ok": true,
        "command": "vision.analyze",
        "character": character,
        "turnaround_view_count": turnaround_views.len(),
        "measured_pixel_drift_y_px": measured_pixel_drift_y_px,
        "pose_frame_count": pose_frames.len(),
        "part_layer_count": part_layers.len(),
        "all_parts_non_empty": !part_layers.is_empty() && part_layers.iter().all(|p| p["metrics"]["non_empty"] == true),
        "turnaround_views": turnaround_views,
        "pose_frames": pose_frames,
        "part_layers": part_layers,
    }))
}

