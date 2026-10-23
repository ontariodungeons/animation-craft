//! Turnaround registration accuracy and 2D preview mode.

use serde_json::{Value, json};

use crate::session::{LoadedManifest, MeasuredMotionRange, MeasuredTurnaroundRegistration};
use crate::{EngineError, Result, Session};

/// Run `optimize-accuracy` on the turnaround + pose dirs and store measured telemetry on the active asset.
pub fn cmd_turnaround_optimize_accuracy(s: &mut Session, params: &Value) -> Result<Value> {
    let root = super::workspace::workspace_root(s);
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .unwrap_or_else(|| "hopper".to_string());

    let default_turnaround = s
        .assets
        .get(&asset_id)
        .and_then(|a| a.turnaround_manifest.as_ref())
        .map(|m| m.manifest_dir.clone())
        .unwrap_or_else(|| root.join("exports/pose_and_parts/hopper_turnaround"));

    let default_poses = s
        .assets
        .get(&asset_id)
        .and_then(|a| a.pose_manifest.as_ref())
        .map(|m| m.manifest_dir.clone())
        .unwrap_or_else(|| root.join("exports/pose_and_parts/hopper_poses"));

    let turnaround_dir = params
        .get("turnaround_dir")
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p))
        .unwrap_or(default_turnaround);

    let poses_dir = params
        .get("poses_dir")
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p))
        .unwrap_or(default_poses);

    let mut out_dir = params
        .get("out_dir")
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p))
        .unwrap_or_else(|| root.join("exports/pose_and_parts").join(&asset_id));

    if out_dir == turnaround_dir || out_dir == poses_dir {
        out_dir = root
            .join("exports/pose_and_parts")
            .join(format!("{asset_id}_accuracy"));
    }

    let args = vec![
        "optimize-accuracy".to_string(),
        "--turnaround-dir".to_string(),
        turnaround_dir.to_string_lossy().into_owned(),
        "--poses-dir".to_string(),
        poses_dir.to_string_lossy().into_owned(),
        "--out-dir".to_string(),
        out_dir.to_string_lossy().into_owned(),
    ];
    let report = super::workspace::run_pipeline(s, &args)?;

    let motion_file = report
        .get("motionRangeFile")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| out_dir.join("motion_range.json"));
    let reg_file = report
        .get("registrationFile")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| out_dir.join("turnaround_registration.json"));

    let measured_motion = MeasuredMotionRange::load_from_path(&motion_file)?;
    let measured_reg = MeasuredTurnaroundRegistration::load_from_path(&reg_file)?;
    let reg_manifest_path = out_dir.join("registered_turnaround/manifest.json");
    let reg_manifest = LoadedManifest::load_from_path(&reg_manifest_path).ok();
    let pose_manifest = LoadedManifest::load_from_path(&poses_dir).ok();

    let asset = s.assets.entry(asset_id.clone()).or_insert_with(|| crate::session::Asset {
        id: asset_id.clone(),
        ..Default::default()
    });
    if asset.pose_manifest.is_none() {
        asset.pose_manifest = pose_manifest;
    }
    if let Some(rm) = reg_manifest {
        asset.turnaround_manifest = Some(rm);
    }
    let foot_lift_max_px = measured_motion.foot_lift_max_px;
    let width_max_delta_px = measured_motion.width_max_delta_px;
    let max_vertical_drift_px = measured_reg.max_vertical_drift_px;
    let registration_passed = measured_reg.registration_passed;

    asset.motion_range = Some(measured_motion);
    asset.turnaround_registration = Some(measured_reg);
    asset.sync_parts_from_manifests();
    s.active_asset = Some(asset_id.clone());

    Ok(json!({
        "ok": true,
        "command": "turnaround.optimize_accuracy",
        "asset_id": asset_id,
        "registration_passed": registration_passed,
        "max_vertical_drift_px": max_vertical_drift_px,
        "foot_lift_max_px": foot_lift_max_px,
        "width_max_delta_px": width_max_delta_px,
        "report": report,
    }))
}

/// Set the active 2D preview mode (idle|walk|run|jump|wave|static), optional speed and frame range.
pub fn cmd_anim_preview_2d(s: &mut Session, params: &Value) -> Result<Value> {
    let mode = params
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("walk")
        .to_ascii_lowercase();
    if !matches!(
        mode.as_str(),
        "idle" | "walk" | "run" | "jump" | "wave" | "static" | "standing" | "walking" | "running" | "waving"
    ) {
        return Err(EngineError::InvalidParams(format!(
            "invalid preview mode '{mode}'; expected idle|walk|run|jump|wave|static"
        )));
    }
    s.preview_mode = mode.clone();
    if let Some(speed) = params.get("speed").and_then(Value::as_f64) {
        s.preview_speed = (speed as f32).clamp(0.1, 4.0);
    }
    if let (Some(start), Some(end)) = (
        params.get("range_start").and_then(Value::as_u64),
        params.get("range_end").and_then(Value::as_u64),
    ) && end > start
    {
        s.preview_range = (start as u32, end as u32);
    }

    let available_poses: Vec<String> = s
        .current_asset()
        .and_then(|a| a.pose_manifest.as_ref())
        .map(|m| m.frames.iter().map(|f| f.label.clone()).collect())
        .unwrap_or_default();

    // Only a rig moves. A reference sheet or plain image shows one still pose, so an action
    // other than static is reported as not playing rather than as a success.
    let rig_type = s
        .current_asset()
        .and_then(|a| a.cutout_rig.as_ref())
        .map(|r| r.rig_type.clone());
    let still = matches!(mode.as_str(), "static" | "standing");
    let (ok, plays) = match rig_type.as_deref() {
        Some("layered_motion") => (true, "the rig spec's own motion tracks (the action mode does not apply)".to_string()),
        Some(_) => (true, format!("the cutout rig's {mode} motion")),
        None if still => (true, "a still image".to_string()),
        None => (false, "a still image: this asset has no rig, so it cannot move".to_string()),
    };
    let mut out = json!({
        "ok": ok,
        "command": "anim.preview_2d",
        "mode": mode,
        "plays": plays,
        "speed": s.preview_speed,
        "range": [s.preview_range.0, s.preview_range.1],
        "active_asset": s.active_asset,
        "available_poses": available_poses,
    });
    if !ok {
        out["next"] = json!([
            "Build a rig for it: asset.build_cutout_rig, or connect a rig spec with project.add_asset kind=cutout_rig."
        ]);
    }
    Ok(out)
}

use std::path::PathBuf;

