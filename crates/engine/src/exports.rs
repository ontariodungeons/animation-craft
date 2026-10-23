//! Export commands: EffectCraft / Three.js / React Motion / Blender scripts,
//! FilmCraft, Blender 5.3, cutout MP4, layered motion, and motion validation.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

use crate::workspace::{
    build_blender_check_args, build_effectcraft_script_args, build_filmcraft_load_args,
    resolve_craft_cli, resolve_workspace_path, run_tool_in, run_pipeline, workspace_root,
};

/// Resolve the active cutout rig plan path (asset's plan or the default on-disk location).
fn resolve_active_cutout_plan(s: &Session) -> Result<(String, PathBuf)> {
    let asset = s
        .current_asset()
        .ok_or_else(|| EngineError::InvalidParams("no active asset to export".into()))?;
    let asset_id = asset.id.clone();
    let plan_path = if let Some(rig) = &asset.cutout_rig {
        rig.plan_path.clone()
    } else {
        let candidate = resolve_workspace_path(
            s,
            &format!("exports/pose_and_parts/{asset_id}/{asset_id}_cutout_rig_plan.json"),
        );
        if candidate.is_file() {
            candidate
        } else {
            return Err(EngineError::InvalidParams(format!(
                "no cutout rig built for active asset '{asset_id}'; run asset.build_cutout_rig first"
            )));
        }
    };
    Ok((asset_id, plan_path))
}

/// Resolve preview timing (mode, speed, duration) from params or session defaults.
fn resolve_preview_export_timing(s: &Session, params: &Value) -> (String, f64, f64) {
    let mode = params
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or(s.preview_mode.as_str())
        .to_string();
    let speed = params
        .get("speed")
        .and_then(Value::as_f64)
        .unwrap_or(s.preview_speed as f64);
    let default_dur =
        ((s.preview_range.1.saturating_sub(s.preview_range.0)).max(12) as f64) / 24.0;
    let duration = params
        .get("duration")
        .and_then(Value::as_f64)
        .unwrap_or(default_dur);
    (mode, speed, duration)
}

/// Run an EffectCraft `.jsx` script via `effectcraft-cli script <script.jsx>`.
pub fn cmd_effectcraft_run_script(s: &mut Session, params: &Value) -> Result<Value> {
    let script_raw = params
        .get("script")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing 'script' parameter".into()))?;
    let resolved_script = resolve_workspace_path(s, script_raw);
    if !resolved_script.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "EffectCraft script file not found: {}",
            resolved_script.display()
        )));
    }
    let root = workspace_root(s);
    let bin = resolve_craft_cli("effectcraft-cli");
    let args = build_effectcraft_script_args(&resolved_script.to_string_lossy());
    let stdout = run_tool_in(&root, &bin, &args)?;
    Ok(json!({
        "ok": true,
        "command": "effectcraft.run_script",
        "tool": bin,
        "script": resolved_script.to_string_lossy(),
        "stdout": stdout,
    }))
}

/// Export an EffectCraft JSX for the active cutout rig, or run a raw `.jsx` when `script` is given without `output`.
pub fn cmd_export_effectcraft(s: &mut Session, params: &Value) -> Result<Value> {
    if params.get("script").is_some() && params.get("output").is_none() {
        let mut res = cmd_effectcraft_run_script(s, params)?;
        if let Some(obj) = res.as_object_mut() {
            obj.insert("command".to_string(), json!("export.effectcraft"));
        }
        return Ok(res);
    }

    let (asset_id, plan_path) = resolve_active_cutout_plan(s)?;
    let default_jsx = format!("exports/{asset_id}_cutout_rig.jsx");
    let output_raw = params
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or(&default_jsx);
    let output_path = resolve_workspace_path(s, output_raw);
    let (mode, speed, duration) = resolve_preview_export_timing(s, params);

    let root = workspace_root(s);
    let builder_script = root.join("pipeline/guided_cutout_rig_builder.py");
    let args = vec![
        builder_script.to_string_lossy().into_owned(),
        "--jsx-only".to_string(),
        "--rig-plan".to_string(),
        plan_path.to_string_lossy().into_owned(),
        "--mode".to_string(),
        mode.clone(),
        "--speed".to_string(),
        format!("{speed:.3}"),
        "--duration".to_string(),
        format!("{duration:.3}"),
        "--out-jsx".to_string(),
        output_path.to_string_lossy().into_owned(),
    ];
    let stdout = run_tool_in(&root, "python3", &args)?;
    if !output_path.is_file() {
        return Err(EngineError::CommandFailed(
            "export.effectcraft".into(),
            format!("EffectCraft JSX script was not created at {}", output_path.display()),
        ));
    }
    Ok(json!({
        "ok": true,
        "command": "export.effectcraft",
        "asset_id": asset_id,
        "output": output_path.to_string_lossy(),
        "script": output_path.to_string_lossy(),
        "mode": mode,
        "speed": speed,
        "duration": duration,
        "stdout": stdout,
    }))
}

/// Export a FilmCraft project: locate the rendered overlay/animation, then run `filmcraft-load`.
pub fn cmd_export_filmcraft(s: &mut Session, params: &Value) -> Result<Value> {
    let active_id = s
        .active_asset
        .clone()
        .unwrap_or_else(|| "track_b".to_string());

    let media_path = if let Some(media_raw) = params
        .get("media")
        .and_then(Value::as_str)
        .filter(|m| !m.trim().is_empty())
    {
        let p = resolve_workspace_path(s, media_raw);
        if !p.is_file() {
            return Err(EngineError::InvalidParams(format!(
                "media file does not exist: {}",
                p.display()
            )));
        }
        p
    } else {
        let asset = s.current_asset().ok_or_else(|| {
            EngineError::InvalidParams(
                "no active asset; provide 'media' or load an asset with rendered animation".into(),
            )
        })?;
        let id = &asset.id;
        let mut candidates: Vec<PathBuf> = Vec::new();
        // Prefer transparent RGBA WebM overlay tracks first so FilmCraft video tracks preserve alpha
        if let Some(rig) = &asset.cutout_rig
            && let Some(webm) = rig.effectcraft.get("webm_alpha_path").and_then(Value::as_str)
        {
            candidates.push(resolve_workspace_path(s, webm));
        }
        candidates.push(resolve_workspace_path(
            s,
            &format!("exports/pose_and_parts/{id}/{id}_effectcraft_cutout_alpha.webm"),
        ));
        if let Some(rig) = &asset.cutout_rig
            && let Some(mp4) = rig.effectcraft.get("mp4_path").and_then(Value::as_str)
        {
            candidates.push(resolve_workspace_path(s, mp4));
        }
        candidates.push(resolve_workspace_path(s, &format!("exports/{id}_cutout.mp4")));
        candidates.push(resolve_workspace_path(
            s,
            &format!("exports/pose_and_parts/{id}/{id}_effectcraft_cutout.mp4"),
        ));
        candidates.push(resolve_workspace_path(
            s,
            &format!("exports/blender_characters/{id}/{id}_blender53.mp4"),
        ));
        if id == "hopper" {
            candidates.push(resolve_workspace_path(s, "exports/hopper_4dir_blender53.mp4"));
        }
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| {
                EngineError::InvalidParams(format!(
                    "no rendered overlay/animation found for active asset '{id}'; build a cutout rig first or pass 'media'"
                ))
            })?
    };

    let default_proj = format!("exports/{active_id}_filmcraft.fcproj");
    let default_frame = format!("exports/{active_id}_filmcraft_frame.png");
    let project_raw = params
        .get("output")
        .or_else(|| params.get("project"))
        .and_then(Value::as_str)
        .unwrap_or(&default_proj);
    let frame_raw = params
        .get("frame_out")
        .and_then(Value::as_str)
        .unwrap_or(&default_frame);

    let project_path = resolve_workspace_path(s, project_raw);
    let frame_path = resolve_workspace_path(s, frame_raw);

    let args = build_filmcraft_load_args(
        &media_path.to_string_lossy(),
        &project_path.to_string_lossy(),
        &frame_path.to_string_lossy(),
    );
    let pipeline_out = run_pipeline(s, &args)?;
    let imported_ok = pipeline_out
        .get("importedToFilmCraft")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && pipeline_out
            .get("projectExists")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    if !imported_ok {
        return Err(EngineError::CommandFailed(
            "export.filmcraft".into(),
            format!("FilmCraft import verification failed: {pipeline_out}"),
        ));
    }

    Ok(json!({
        "ok": true,
        "command": "export.filmcraft",
        "asset_id": active_id,
        "media": media_path.to_string_lossy(),
        "output": project_path.to_string_lossy(),
        "frame_out": frame_path.to_string_lossy(),
        "pipeline": pipeline_out,
    }))
}

/// Blender 5.3 rigged render via `studio_pipeline_cli.py blender-check`.
pub fn cmd_export_blender(s: &mut Session, params: &Value) -> Result<Value> {
    let explicit_char = params
        .get("character")
        .or_else(|| params.get("asset"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let character = explicit_char.clone().or_else(|| s.active_asset.clone());

    let asset_ref = if let Some(ref c) = explicit_char {
        s.assets.get(c.as_str())
    } else {
        s.current_asset()
    };

    let explicit_image = params
        .get("image")
        .and_then(Value::as_str)
        .filter(|p| !p.trim().is_empty())
        .map(|p| resolve_workspace_path(s, p));

    let source_image_opt = if let Some(img) = explicit_image {
        if !img.is_file() {
            return Err(EngineError::InvalidParams(format!(
                "source image file not found: {}",
                img.display()
            )));
        }
        Some(img)
    } else if let Some(asset) = asset_ref {
        let candidate = asset
            .raw_original_path
            .clone()
            .filter(|p| p.is_file())
            .unwrap_or_else(|| asset.path.clone());
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    } else {
        None
    };

    let known_cast = [
        "hopper",
        "whiskers",
        "puddles",
        "jenny",
        "rusty",
        "miss_hayley",
        "old_mac",
    ];
    let is_known_cast = character
        .as_deref()
        .is_some_and(|c| known_cast.contains(&c.to_ascii_lowercase().as_str()));

    if source_image_opt.is_none() && !is_known_cast {
        return Err(EngineError::InvalidParams(format!(
            "cannot resolve source image for character {:?}; load an asset with a valid image path or pass 'image'",
            character
        )));
    }

    let rec_arch = asset_ref
        .and_then(|a| a.intake_recommendation.as_ref())
        .and_then(|r| r.get("archetype"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let rec_verb = asset_ref
        .and_then(|a| a.intake_recommendation.as_ref())
        .and_then(|r| r.get("primaryVerb"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let archetype = params
        .get("archetype")
        .and_then(Value::as_str)
        .or(rec_arch.as_deref())
        .unwrap_or("humanoid_biped");
    let verb = params
        .get("verb")
        .and_then(Value::as_str)
        .or(rec_verb.as_deref())
        .unwrap_or("move_traverse");

    let default_mp4 = match character.as_deref() {
        Some(c) if c != "hopper" => format!("exports/blender_characters/{c}/{c}_blender53.mp4"),
        _ => "exports/hopper_4dir_blender53.mp4".to_string(),
    };
    let default_png = match character.as_deref() {
        Some(c) if c != "hopper" => {
            format!("exports/blender_characters/{c}/{c}_blender53_preview.png")
        }
        _ => "exports/hopper_4dir_blender53_preview.png".to_string(),
    };
    let out_mp4_raw = params
        .get("out_mp4")
        .or_else(|| params.get("output"))
        .and_then(Value::as_str)
        .unwrap_or(&default_mp4);
    let out_png_raw = params
        .get("out_png")
        .and_then(Value::as_str)
        .unwrap_or(&default_png);
    let glb_opt = params
        .get("glb")
        .and_then(Value::as_str)
        .map(|g| resolve_workspace_path(s, g));

    let out_mp4 = resolve_workspace_path(s, out_mp4_raw);
    let out_png = resolve_workspace_path(s, out_png_raw);
    let glb_str = glb_opt.as_ref().map(|p| p.to_string_lossy().into_owned());

    let mut args = build_blender_check_args(
        archetype,
        verb,
        &out_mp4.to_string_lossy(),
        &out_png.to_string_lossy(),
        glb_str.as_deref(),
    );
    if let Some(c) = &character {
        args.push("--character".to_string());
        args.push(c.clone());
    }
    if let Some(src_img) = &source_image_opt {
        args.push("--image".to_string());
        args.push(src_img.to_string_lossy().into_owned());
    }
    let pipeline_out = run_pipeline(s, &args)?;
    Ok(json!({
        "ok": true,
        "command": "export.blender",
        "character": character,
        "source_image": source_image_opt.map(|p| p.to_string_lossy().into_owned()),
        "archetype": archetype,
        "verb": verb,
        "out_mp4": out_mp4.to_string_lossy(),
        "out_png": out_png.to_string_lossy(),
        "pipeline": pipeline_out,
    }))
}

/// Farm-school scene lineup via `studio_pipeline_cli.py farm-school-scene`.
pub fn cmd_scene_farm_school_lineup(s: &mut Session, params: &Value) -> Result<Value> {
    let out_dir_raw = params
        .get("out_dir")
        .and_then(Value::as_str)
        .unwrap_or("exports/farm_school_milestone");
    let out_dir = resolve_workspace_path(s, out_dir_raw);
    let mut args = vec![
        "farm-school-scene".to_string(),
        "--out-dir".to_string(),
        out_dir.to_string_lossy().into_owned(),
    ];
    if let Some(only_char) = params.get("character").and_then(Value::as_str) {
        args.push("--character".to_string());
        args.push(only_char.to_string());
    }
    let pipeline_out = run_pipeline(s, &args)?;
    Ok(json!({
        "ok": true,
        "command": "scene.farm_school_lineup",
        "out_dir": out_dir.to_string_lossy(),
        "pipeline": pipeline_out,
    }))
}

/// Character iteration export via `studio_pipeline_cli.py iterate-character`.
pub fn cmd_export_character_iteration(s: &mut Session, params: &Value) -> Result<Value> {
    let character = params
        .get("character")
        .or_else(|| params.get("asset"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .unwrap_or_else(|| "hopper".to_string());
    let out_root = params
        .get("out_root")
        .and_then(Value::as_str)
        .unwrap_or("exports/design_share");
    let resolved_out = resolve_workspace_path(s, out_root);
    let args = vec![
        "iterate-character".to_string(),
        "--character".to_string(),
        character.clone(),
        "--out-root".to_string(),
        resolved_out.to_string_lossy().into_owned(),
    ];
    let pipeline_out = run_pipeline(s, &args)?;
    Ok(json!({
        "ok": true,
        "command": "export.character_iteration",
        "character": character,
        "out_root": resolved_out.to_string_lossy(),
        "pipeline": pipeline_out,
    }))
}

/// Export a cutout MP4: render the rig preview via the layered-motion pipeline, then encode with ffmpeg.
pub fn cmd_export_cutout_mp4(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = s
        .current_asset()
        .ok_or_else(|| EngineError::InvalidParams("no active asset".into()))?;
    let asset_id = asset.id.clone();
    let rig = asset
        .cutout_rig
        .as_ref()
        .ok_or_else(|| EngineError::InvalidParams("build or connect a cutout rig first".into()))?;
    let plan_path = rig.plan_path.clone();
    if !plan_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "cutout rig plan not found: {}",
            plan_path.display()
        )));
    }

    let mode = params
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or(s.preview_mode.as_str())
        .to_string();
    let speed = params
        .get("speed")
        .and_then(Value::as_f64)
        .unwrap_or(s.preview_speed as f64);
    let default_dur =
        ((s.preview_range.1.saturating_sub(s.preview_range.0)).max(12) as f64) / 24.0;
    let duration = params
        .get("duration")
        .and_then(Value::as_f64)
        .unwrap_or(default_dur);

    let default_output = format!("exports/{asset_id}_cutout.mp4");
    let output = resolve_workspace_path(
        s,
        params
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or(&default_output),
    );
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let root = workspace_root(s);
    let script = root.join("pipeline/guided_cutout_rig_builder.py");
    run_tool_in(
        &root,
        "python3",
        &[
            script.to_string_lossy().into_owned(),
            "--render-only".into(),
            "--rig-plan".into(),
            plan_path.to_string_lossy().into_owned(),
            "--mode".into(),
            mode.clone(),
            "--speed".into(),
            format!("{speed:.3}"),
            "--duration".into(),
            format!("{duration:.3}"),
            "--out-mp4".into(),
            output.to_string_lossy().into_owned(),
        ],
    )?;
    if !output.is_file() {
        return Err(EngineError::CommandFailed(
            "export.cutout_mp4".into(),
            format!("Cutout MP4 was not created at {}", output.display()),
        ));
    }
    if let Ok(updated_plan) = crate::session::LoadedCutoutRigPlan::load_from_path(&plan_path)
        && let Some(asset) = s.current_asset_mut()
    {
        asset.cutout_rig = Some(updated_plan);
    }
    Ok(json!({
        "ok": true,
        "asset_id": asset_id,
        "mode": mode,
        "speed": speed,
        "duration": duration,
        "output": output,
    }))
}

/// Export a layered motion spec via `pipeline/layered_motion.py`.
pub fn cmd_export_layered_motion(s: &mut Session, params: &Value) -> Result<Value> {
    let spec_path = if let Some(spec_raw) = params.get("spec").and_then(Value::as_str)
        && !spec_raw.trim().is_empty()
    {
        resolve_workspace_path(s, spec_raw)
    } else if let Some(rig) = s.current_asset().and_then(|a| a.cutout_rig.as_ref())
        && rig.rig_type == "layered_motion"
    {
        rig.plan_path.clone()
    } else {
        return Err(EngineError::InvalidParams(
            "export.layered_motion requires `spec`: path to a layered motion spec (JSON)".into(),
        ));
    };
    if !spec_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "layered motion spec not found: {}",
            spec_path.display()
        )));
    }
    let stem = spec_path.file_stem().and_then(|v| v.to_str()).unwrap_or("layered_motion");
    let root = workspace_root(s);
    let out_dir = match params.get("out_dir").and_then(Value::as_str) {
        Some(dir) if !dir.trim().is_empty() => resolve_workspace_path(s, dir),
        _ => root.join("exports/layered_motion").join(stem),
    };
    let script = root.join("pipeline/layered_motion.py");
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--spec".into(),
        spec_path.to_string_lossy().into_owned(),
        "--out-dir".into(),
        out_dir.to_string_lossy().into_owned(),
    ];
    if let Some(tracks) = params.get("tracks") {
        let list = match tracks {
            Value::Array(items) => items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(","),
            other => other.as_str().unwrap_or_default().to_string(),
        };
        if !list.is_empty() {
            args.extend(["--tracks".into(), list]);
        }
    }
    if let Some(count) = params.get("stills").and_then(Value::as_u64) {
        args.extend(["--stills".into(), count.to_string()]);
    }
    if params.get("video").and_then(Value::as_bool) == Some(false) {
        args.push("--no-video".into());
    }
    if params.get("gpu").and_then(Value::as_bool) == Some(false) {
        args.push("--cpu".into());
    }
    if let Some(crf) = params.get("crf").and_then(Value::as_u64) {
        args.extend(["--crf".into(), crf.to_string()]);
    }
    if let Some(res) = params.get("resolution").and_then(Value::as_str)
        && !res.trim().is_empty()
    {
        args.extend(["--resolution".into(), res.to_string()]);
    }
    if let Some(qual) = params.get("quality").and_then(Value::as_str)
        && !qual.trim().is_empty()
    {
        args.extend(["--quality".into(), qual.to_string()]);
    }
    let stdout = run_tool_in(&root, "python3", &args)?;
    let mut result: Value = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| {
            EngineError::CommandFailed("export.layered_motion".into(), "layered_motion.py returned no result".into())
        })?;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("command".into(), json!("export.layered_motion"));
        obj.insert("out_dir".into(), json!(out_dir.to_string_lossy()));
    }
    Ok(result)
}

/// Validate a rendered motion clip via `pipeline/motion_validate.py`.
pub fn cmd_validate_motion(s: &mut Session, params: &Value) -> Result<Value> {
    let video_raw = params.get("video").and_then(Value::as_str).ok_or_else(|| {
        EngineError::InvalidParams("validate.motion requires `video`: a rendered transparent clip".into())
    })?;
    let video = resolve_workspace_path(s, video_raw);
    if !video.is_file() {
        return Err(EngineError::InvalidParams(format!("clip not found: {}", video.display())));
    }
    let root = workspace_root(s);
    let script = root.join("pipeline/motion_validate.py");
    let mut args = vec![script.to_string_lossy().into_owned(), "--video".into(), video.to_string_lossy().into_owned()];
    if let Some(frames) = params.get("frames").and_then(Value::as_u64) {
        args.extend(["--frames".into(), frames.to_string()]);
    }
    if let Some(fps) = params.get("fps").and_then(Value::as_f64) {
        args.extend(["--fps".into(), fps.to_string()]);
    }
    if params.get("loop").and_then(Value::as_bool) == Some(true) {
        args.push("--loop".into());
    }
    for key in ["reference", "spec"] {
        if let Some(path) = params.get(key).and_then(Value::as_str)
            && !path.trim().is_empty()
        {
            args.extend([format!("--{key}"), resolve_workspace_path(s, path).to_string_lossy().into_owned()]);
        }
    }
    let stdout = run_tool_in(&root, "python3", &args)?;
    let mut report: Value = stdout.lines().rev().find_map(|line| serde_json::from_str(line).ok()).ok_or_else(|| {
        EngineError::CommandFailed("validate.motion".into(), "motion_validate.py returned no result".into())
    })?;
    if let Some(obj) = report.as_object_mut() {
        obj.insert("command".into(), json!("validate.motion"));
    }
    Ok(report)
}

