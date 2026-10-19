//! Snapshot exports sample the native evaluator; no second set of animation equations.
use crate::{
    UiState,
    panels::center::center_motion::{
        evaluate_cutout_rig_motion, is_rig_part_visible, step_playback_phase, transform_rig_point,
    },
};
use animationcraft_engine::{
    Session,
    session::{LoadedCutoutRigPlan, PartState},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};

/// An explicit external spec is a document export, not a request to export the active viewport.
pub(crate) fn uses_preview(session: &Session, id: &str, params: &Value) -> bool {
    if id == "export.cutout_mp4" {
        return true;
    }
    if id != "export.layered_motion" {
        return false;
    }
    let Some(spec) = params
        .get("spec")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return true;
    };
    let Some(rig) = session.current_asset().and_then(|a| a.cutout_rig.as_ref()) else {
        return false;
    };
    match (
        std::fs::canonicalize(animationcraft_engine::commands::resolve_workspace_path(
            session, spec,
        )),
        std::fs::canonicalize(&rig.plan_path),
    ) {
        (Ok(requested), Ok(loaded)) => requested == loaded,
        _ => false,
    }
}

pub(crate) fn sampled_spec(
    rig: &LoadedCutoutRigPlan,
    state: &UiState,
    parts: &[PartState],
    duration: f64,
    fps: u32,
) -> Result<Value, String> {
    if !duration.is_finite() || duration <= 0.0 || duration > 600.0 || fps == 0 || fps > 120 {
        return Err(
            "Export needs a finite duration between 0 and 600 seconds and FPS between 1 and 120"
                .into(),
        );
    }
    if state.explode_pct > 0.01 {
        return Err("Reset Parts separation before exporting the animation".into());
    }
    let mut snapshot = state.clone();
    snapshot.playing = true;
    snapshot.playback_phase = 0.0;
    let mut channels: BTreeMap<String, [Vec<Value>; 4]> = BTreeMap::new();
    let count = (duration * fps as f64).ceil() as usize;
    let mut bounds = [
        0.0_f32,
        0.0,
        rig.canvas_size[0] as f32,
        rig.canvas_size[1] as f32,
    ];
    for frame in 0..=count {
        let t = (frame as f64 / fps as f64).min(duration);
        let motion = evaluate_cutout_rig_motion(rig, &snapshot);
        for part in &rig.parts {
            let keys = channels.entry(part.id.clone()).or_default();
            let mut pos = motion
                .pos_offset
                .get(&part.id)
                .copied()
                .unwrap_or([0.0, 0.0]);
            if part.parent.is_none() {
                pos[1] += motion.root_dy_canvas;
            }
            let scale = motion.scale_xy.get(&part.id).copied().unwrap_or([1.0, 1.0]);
            let opacity = if is_rig_part_visible(&part.id, parts) {
                motion.opacity.get(&part.id).copied().unwrap_or(1.0)
            } else {
                0.0
            };
            if opacity > 0.01 {
                let [x0, y0, x1, y1] = part.crop_box;
                for corner in [[x0, y0], [x1, y0], [x1, y1], [x0, y1]] {
                    let p = transform_rig_point(
                        rig,
                        &motion,
                        &part.id,
                        [corner[0] as f32, corner[1] as f32],
                    );
                    if p.iter().any(|n| !n.is_finite()) {
                        return Err("Rig motion produced a non-finite transform".into());
                    }
                    bounds[0] = bounds[0].min(p[0]);
                    bounds[1] = bounds[1].min(p[1]);
                    bounds[2] = bounds[2].max(p[0]);
                    bounds[3] = bounds[3].max(p[1]);
                }
            }
            keys[0].push(json!([t, pos]));
            keys[1].push(json!([
                t,
                motion.angles_deg.get(&part.id).copied().unwrap_or(0.0)
            ]));
            keys[2].push(json!([
                t,
                [(scale[0] - 1.0) * 100.0, (scale[1] - 1.0) * 100.0]
            ]));
            keys[3].push(json!([t, (opacity - 1.0) * 100.0]));
        }
        snapshot.playback_phase = step_playback_phase(
            snapshot.playback_phase,
            1.0 / fps as f32,
            true,
            state.anim_speed,
            state.sprint_lock,
        );
    }
    let motion: serde_json::Map<String, Value> = channels
        .into_iter()
        .map(|(id, keys)| {
            let props: serde_json::Map<String, Value> =
                ["position", "rotation", "scale", "opacity"]
                    .into_iter()
                    .zip(keys)
                    .map(|(name, keys)| (name.into(), json!([{"kind":"keys","keys":keys}])))
                    .collect();
            (id, Value::Object(props))
        })
        .collect();
    let mut ordered = rig.parts.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|p| p.layer);
    let layers: Vec<Value> = ordered
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let mut layer =
                json!({"id":part.id,"file":format!("layer-{index}.png"),"pivot":part.pivot});
            if let Some(parent) = &part.parent {
                layer["parent"] = json!(parent);
            }
            layer
        })
        .collect();
    let mut margin = [
        -bounds[0],
        -bounds[1],
        bounds[2] - rig.canvas_size[0] as f32,
        bounds[3] - rig.canvas_size[1] as f32,
    ];
    for (i, value) in margin.iter_mut().enumerate() {
        *value = value
            .ceil()
            .max(rig.effectcraft["margin"][i].as_f64().unwrap_or(0.0) as f32);
    }
    Ok(
        json!({"canvas":rig.canvas_size,"margin":margin,"fps":fps,"layers":layers,"tracks":[{"id":"preview","duration":duration,"motion":motion}]}),
    )
}

pub(crate) fn export(
    session: &mut Session,
    state: &UiState,
    params: &Value,
    textures: &BTreeMap<String, Vec<PathBuf>>,
    mp4: bool,
) -> Result<Value, String> {
    let mut export_state = state.clone();
    if let Some(speed) = params.get("speed") {
        let speed = speed
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0 && *n <= 100.0)
            .ok_or("Export speed must be a finite number between 0 and 100")?;
        export_state.anim_speed = speed as f32;
    }
    if let Some(mode) = params.get("mode") {
        export_state.anim_mode = match mode.as_str().ok_or("Export mode must be a string")? {
            "idle" | "standing" => crate::AnimMode::Idle,
            "walk" | "walking" => crate::AnimMode::Walk,
            "run" | "running" => crate::AnimMode::Run,
            "jump" | "jumping" => crate::AnimMode::Jump,
            "wave" | "waving" => crate::AnimMode::Wave,
            "look" | "look_scout" | "explore" => crate::AnimMode::LookScout,
            "static" => crate::AnimMode::Static,
            _ => return Err("Unknown export motion mode".into()),
        };
    }
    let state = &export_state;
    let asset = session.current_asset().ok_or("No active asset to export")?;
    let rig = asset
        .cutout_rig
        .as_ref()
        .ok_or("Connect a rig before exporting the preview")?;
    if let Some(spec) = params
        .get("spec")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        let requested = std::fs::canonicalize(
            animationcraft_engine::commands::resolve_workspace_path(session, spec),
        )
        .map_err(|e| e.to_string())?;
        let loaded = std::fs::canonicalize(&rig.plan_path).map_err(|e| e.to_string())?;
        if requested != loaded {
            return Err(
                "Load the selected motion spec into the preview before exporting it".into(),
            );
        }
    }
    if params.get("duration").is_some_and(|v| v.as_f64().is_none()) {
        return Err("Export duration must be a number".into());
    }
    let track_duration = evaluate_cutout_rig_motion(rig, state).track_duration;
    let duration =
        params
            .get("duration")
            .and_then(Value::as_f64)
            .unwrap_or(if track_duration > 0.0 {
                track_duration as f64
                    / if state.anim_speed > 0.0 {
                        state.anim_speed as f64
                    } else {
                        1.0
                    }
            } else {
                3.0
            });
    let mut spec = sampled_spec(rig, state, &asset.parts, duration, 30)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let root = animationcraft_engine::commands::workspace_root(session)
        .join("exports/preview_snapshots")
        .join(format!("{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    prepare_textures(&mut spec, rig, state, textures, &root)?;
    let path = root.join("preview.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&spec).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut call = json!({});
    for key in [
        "out_dir",
        "stills",
        "video",
        "gpu",
        "crf",
        "resolution",
        "quality",
    ] {
        if let Some(value) = params.get(key) {
            call[key] = value.clone();
        }
    }
    call["spec"] = json!(path);
    call["tracks"] = json!(["preview"]);
    if call.get("out_dir").is_none() {
        call["out_dir"] = json!(root.join("render"));
    }
    // UI-only snapshot path; headless engine exports continue to use their explicit spec.
    let mut result = session
        .execute("export.layered_motion", call)
        .map_err(|e| e.to_string())?;
    if result.get("ok").and_then(Value::as_bool) == Some(false) {
        return Err(result.to_string());
    }
    if mp4 {
        let input = result["outputs"][0]["file"]
            .as_str()
            .ok_or("Renderer returned no video")?;
        let output = params
            .get("output")
            .and_then(Value::as_str)
            .map(|p| animationcraft_engine::commands::resolve_workspace_path(session, p))
            .unwrap_or_else(|| root.join("preview.mp4"));
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-nostdin", "-v", "error", "-y", "-i", input, "-c:v", "libx264", "-pix_fmt",
                "yuv420p",
            ])
            .arg(&output)
            .output()
            .map_err(|e| e.to_string())?;
        if !status.status.success() {
            return Err(format!(
                "MP4 encode failed: {}",
                String::from_utf8_lossy(&status.stderr)
            ));
        }
        result["output"] = json!(output);
    }
    if let Some(rig) = session
        .current_asset_mut()
        .and_then(|a| a.cutout_rig.as_mut())
    {
        if let Some(file) = result["outputs"][0]["file"].as_str() {
            rig.effectcraft["webm_alpha_path"] = json!(file);
        }
        if let Some(file) = result["output"].as_str() {
            rig.effectcraft["mp4_path"] = json!(file);
        }
    }
    Ok(result)
}

fn prepare_textures(
    spec: &mut Value,
    rig: &LoadedCutoutRigPlan,
    state: &UiState,
    textures: &BTreeMap<String, Vec<PathBuf>>,
    root: &std::path::Path,
) -> Result<(), String> {
    let fps = spec["fps"]
        .as_f64()
        .ok_or("Snapshot is missing frame rate")? as f32;
    let mut ordered = rig.parts.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|p| p.layer);
    let mut layers = Vec::new();
    for (index, part) in ordered.iter().enumerate() {
        let fallback = vec![part.cropped_path.clone()];
        let frames = textures.get(&part.id).unwrap_or(&fallback);
        if frames.is_empty() {
            return Err(format!("Layer {} has no ready deformation frames", part.id));
        }
        let original_layer = spec["layers"][index].clone();
        layers.push(original_layer);
        let original_motion = spec["tracks"][0]["motion"][&part.id].clone();
        for (frame_index, source) in frames.iter().enumerate() {
            let pixels = image::open(source)
                .map_err(|e| format!("Cannot export layer {}: {e}", part.id))?
                .to_rgba8();
            let crop_width = part.crop_box[2]
                .checked_sub(part.crop_box[0])
                .ok_or("Invalid crop width")?;
            let crop_height = part.crop_box[3]
                .checked_sub(part.crop_box[1])
                .ok_or("Invalid crop height")?;
            if pixels.dimensions() != (crop_width, crop_height) {
                return Err(format!(
                    "Layer {} texture does not match its preview crop",
                    part.id
                ));
            }
            let mut canvas = image::RgbaImage::new(rig.canvas_size[0], rig.canvas_size[1]);
            image::imageops::overlay(
                &mut canvas,
                &pixels,
                part.crop_box[0] as i64,
                part.crop_box[1] as i64,
            );
            let filename = if frame_index == 0 {
                format!("layer-{index}.png")
            } else {
                format!("layer-{index}-frame-{frame_index}.png")
            };
            canvas
                .save(root.join(&filename))
                .map_err(|e| e.to_string())?;
            if frames.len() > 1 {
                let id = if frame_index == 0 {
                    part.id.clone()
                } else {
                    format!("__preview_frame_{index}_{frame_index}")
                };
                if frame_index > 0 {
                    if rig.parts.iter().any(|p| p.id == id) {
                        return Err("Rig layer ID collides with export frame ID".into());
                    }
                    let mut layer = json!({"id":id,"file":filename,"pivot":part.pivot});
                    if let Some(parent) = &part.parent {
                        layer["parent"] = json!(parent);
                    }
                    layers.push(layer);
                }
                let opacity = &original_motion["opacity"][0]["keys"];
                let mut phase = 0.0;
                let keys: Vec<Value> = opacity
                    .as_array()
                    .ok_or("Missing sampled opacity")?
                    .iter()
                    .map(|key| {
                        let selected = ((phase * 3.0_f64).floor() as usize) % frames.len();
                        phase = step_playback_phase(
                            phase,
                            1.0 / fps,
                            true,
                            state.anim_speed,
                            state.sprint_lock,
                        );
                        json!([
                            key[0],
                            if selected == frame_index {
                                key[1].clone()
                            } else {
                                json!(-100.0)
                            }
                        ])
                    })
                    .collect();
                let mut motion = original_motion.clone();
                motion["opacity"] = json!([{"kind":"keys","keys":keys}]);
                spec["tracks"][0]["motion"][&id] = motion;
            }
        }
    }
    spec["layers"] = json!(layers);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AnimMode;
    use animationcraft_engine::session::CutoutRigPart;

    fn rig() -> LoadedCutoutRigPlan {
        LoadedCutoutRigPlan {
            rig_type: "layered_motion".into(),
            canvas_size: [64, 64],
            parts: vec![CutoutRigPart {
                id: "shape".into(),
                pivot: [32.0, 32.0],
                rotation: [-180.0, 180.0],
                ..Default::default()
            }],
            effectcraft: json!({"fps":30,"tracks":[
                {"id":"first","duration":2,"motion":{"shape":{"position":[{"kind":"keys","keys":[[0,[0,0]],[2,[20,0]]]}]}}},
                {"id":"second","duration":2,"motion":{"shape":{"position":[{"kind":"keys","keys":[[0,[0,0]],[2,[0,40]]]}],"rotation":[{"kind":"keys","keys":[[0,0],[2,80]]}]}}}
            ]}),
            ..Default::default()
        }
    }

    #[test]
    fn snapshot_samples_selected_track_speed_range_and_visibility() {
        let rig = rig();
        let mut state = UiState::default();
        state.selected_track_idx = 1;
        state.anim_mode = AnimMode::Walk;
        state.anim_speed = 2.0;
        state.pose_range_pct = 50.0;
        let spec = sampled_spec(&rig, &state, &[], 1.0, 30).unwrap();
        let keys = &spec["tracks"][0]["motion"]["shape"];
        let mut preview = state.clone();
        preview.playing = true;
        preview.playback_phase = 0.0;
        for frame in 0..=30 {
            let expected = evaluate_cutout_rig_motion(&rig, &preview);
            assert_eq!(
                keys["position"][0]["keys"][frame][1],
                json!(expected.pos_offset["shape"])
            );
            assert_eq!(
                keys["rotation"][0]["keys"][frame][1],
                json!(expected.angles_deg["shape"])
            );
            preview.playback_phase = step_playback_phase(
                preview.playback_phase,
                1.0 / 30.0,
                true,
                state.anim_speed,
                state.sprint_lock,
            );
        }
        assert!(keys["position"][0]["keys"][15][1][1].as_f64().unwrap() > 0.0);
        assert_eq!(keys["position"][0]["keys"][15][1][0], json!(0.0));
        let parts = vec![PartState {
            name: "shape".into(),
            visible: false,
            solo: false,
            verified: false,
        }];
        let hidden = sampled_spec(&rig, &state, &parts, 1.0, 30).unwrap();
        assert!(
            hidden["tracks"][0]["motion"]["shape"]["opacity"][0]["keys"]
                .as_array()
                .unwrap()
                .iter()
                .all(|k| k[1] == json!(-100.0))
        );
    }

    #[test]
    fn invalid_duration_and_inspection_explode_fail_before_export() {
        let rig = rig();
        let mut state = UiState::default();
        for duration in [f64::NAN, -1.0, 0.0, 601.0] {
            assert!(sampled_spec(&rig, &state, &[], duration, 30).is_err());
        }
        state.explode_pct = 10.0;
        assert!(sampled_spec(&rig, &state, &[], 1.0, 30).is_err());
    }
    #[test]
    fn texture_sequence_uses_native_frame_phase_and_preserves_pixels() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "animationcraft-sequence-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut rig = rig();
        rig.parts[0].crop_box = [8, 12, 12, 16];
        let frames: Vec<_> = [image::Rgba([255, 0, 0, 255]), image::Rgba([0, 255, 0, 255])]
            .into_iter()
            .enumerate()
            .map(|(i, color)| {
                let path = root.join(format!("source-{i}.png"));
                image::RgbaImage::from_pixel(4, 4, color)
                    .save(&path)
                    .unwrap();
                path
            })
            .collect();
        let state = UiState::default();
        let mut spec = sampled_spec(&rig, &state, &[], 1.0, 30).unwrap();
        prepare_textures(
            &mut spec,
            &rig,
            &state,
            &BTreeMap::from([("shape".into(), frames)]),
            &root,
        )
        .unwrap();
        let ids = ["shape", "__preview_frame_0_1"];
        let mut phase = 0.0;
        for frame in 0..=30 {
            let selected = ((phase * 3.0_f64).floor() as usize) % 2;
            for (i, id) in ids.iter().enumerate() {
                let opacity = spec["tracks"][0]["motion"][id]["opacity"][0]["keys"][frame][1]
                    .as_f64()
                    .unwrap();
                assert_eq!(opacity, if i == selected { 0.0 } else { -100.0 });
            }
            phase =
                step_playback_phase(phase, 1.0 / 30.0, true, state.anim_speed, state.sprint_lock);
        }
        let padded = image::open(root.join("layer-0-frame-1.png"))
            .unwrap()
            .to_rgba8();
        assert_eq!(padded.get_pixel(8, 12).0, [0, 255, 0, 255]);
        assert_eq!(padded.get_pixel(7, 12).0, [0, 0, 0, 0]);
        assert_eq!(
            spec["tracks"][0]["motion"][ids[0]]["rotation"],
            spec["tracks"][0]["motion"][ids[1]]["rotation"]
        );
        assert!(
            prepare_textures(
                &mut spec,
                &rig,
                &state,
                &BTreeMap::from([("shape".into(), vec![])]),
                &root
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn hierarchical_bounds_preserve_artwork_outside_canvas() {
        let mut rig = rig();
        rig.parts[0].crop_box = [0, 0, 64, 64];
        rig.effectcraft["margin"] = json!([3, 4, 5, 6]);
        rig.effectcraft["tracks"][0]["motion"]["shape"]["position"] =
            json!([{"kind":"keys","keys":[[0,[-20,-30]],[2,[-20,-30]]]}]);
        let mut state = UiState::default();
        state.anim_mode = AnimMode::Walk;
        state.pose_range_pct = 100.0;
        let spec = sampled_spec(&rig, &state, &[], 1.0, 30).unwrap();
        assert!(spec["margin"][0].as_f64().unwrap() >= 20.0);
        assert!(spec["margin"][1].as_f64().unwrap() >= 30.0);
        assert!(spec["margin"][2].as_f64().unwrap() >= 5.0);
        let mut motion = crate::panels::center::center_motion::EvaluatedRigMotion::default();
        rig.parts = (0..9)
            .map(|i| CutoutRigPart {
                id: format!("node{i}"),
                parent: if i == 0 {
                    None
                } else {
                    Some(format!("node{}", i - 1))
                },
                ..Default::default()
            })
            .collect();
        for part in &rig.parts {
            motion.pos_offset.insert(part.id.clone(), [1.0, 0.0]);
        }
        motion.root_dy_canvas = -7.0;
        assert_eq!(
            transform_rig_point(&rig, &motion, "node8", [0.0, 0.0]),
            [9.0, -7.0]
        );
    }
    #[test]
    fn explicit_external_spec_is_not_replaced_by_active_preview() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "animationcraft-spec-routing-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let active = root.join("active.json");
        let external = root.join("external.json");
        std::fs::write(&active, b"{}").unwrap();
        std::fs::write(&external, b"{}").unwrap();
        let mut session = Session::default();
        session.active_asset = Some("fixture".into());
        session.assets.insert(
            "fixture".into(),
            animationcraft_engine::Asset {
                id: "fixture".into(),
                cutout_rig: Some(LoadedCutoutRigPlan {
                    plan_path: active.clone(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert!(uses_preview(&session, "export.layered_motion", &json!({})));
        assert!(uses_preview(
            &session,
            "export.layered_motion",
            &json!({"spec":active})
        ));
        assert!(!uses_preview(
            &session,
            "export.layered_motion",
            &json!({"spec":external})
        ));
        session.assets.clear();
        assert!(!uses_preview(
            &session,
            "export.layered_motion",
            &json!({"spec":external})
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
