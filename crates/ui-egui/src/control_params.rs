//! Live CLI control updates use the same values and ranges as the native sliders.
use crate::UiState;
use serde_json::{Value, json};

pub(crate) fn updated(state: &UiState, params: &Value) -> Result<UiState, String> {
    if !params.is_object() { return Err("ui.set_view parameters must be an object".into()); }
    let mut next = state.clone();
    for (key, value, low, high) in [
        ("anim_speed", &mut next.anim_speed, 0.25, 2.25),
        ("pose_range_pct", &mut next.pose_range_pct, 0.0, 100.0),
        ("jump_height_pct", &mut next.jump_height_pct, 0.0, 80.0),
        ("mouth_opening", &mut next.smile_open, 0.0, 100.0),
        ("eye_blink", &mut next.eye_blink, 0.0, 100.0),
        ("eye_gaze", &mut next.eye_gaze, -45.0, 45.0),
        ("head_yaw", &mut next.head_yaw, -45.0, 45.0),
        ("head_pitch", &mut next.head_pitch, -45.0, 45.0),
        ("arm_spread", &mut next.arm_spread, -45.0, 90.0),
        ("elbow_bend", &mut next.elbow_bend, -45.0, 90.0),
        ("knee_bend", &mut next.knee_bend, -45.0, 90.0),
        ("ear_splay", &mut next.ear_splay, -45.0, 45.0),
        ("ear_pitch", &mut next.ear_pitch, -45.0, 45.0),
        ("proj_blend_pct", &mut next.proj_blend_pct, 0.0, 100.0),
        ("bump_strength", &mut next.bump_strength, 0.0, 100.0),
        ("explode_pct", &mut next.explode_pct, 0.0, 100.0),
        ("yaw", &mut next.yaw_deg, 0.0, 360.0),
    ] {
        if let Some(raw) = params.get(key) {
            let number = raw.as_f64().filter(|n| n.is_finite()).ok_or_else(|| format!("{key} must be a finite number"))?;
            *value = number.clamp(low, high) as f32;
        }
    }
    for (key, value, low, high) in [
        ("inflation_depth_pct", &mut next.inflation_depth_pct, 10, 100),
        ("relief_detail_pct", &mut next.relief_detail_pct, 0, 100),
        ("mesh_res", &mut next.mesh_res, 12, 64),
        ("bg_tolerance", &mut next.bg_tolerance, 1, 100),
    ] {
        if let Some(raw) = params.get(key) {
            *value = raw.as_u64().ok_or_else(|| format!("{key} must be a nonnegative integer"))?.clamp(low, high) as u32;
        }
    }
    if let Some(raw) = params.get("mouth_expression") {
        let expression = raw.as_str().filter(|e| ["smile", "laugh", "ooh", "mbp", "talk"].contains(e)).ok_or("Unknown mouth expression")?;
        next.mouth_viseme = expression.into();
    }
    if let Some(raw) = params.get("playing") {
        next.playing = raw.as_bool().ok_or("playing must be a boolean")?;
    }
    Ok(next)
}

pub(crate) fn snapshot(s: &UiState) -> Value {
    json!({"anim_speed":s.anim_speed,"pose_range_pct":s.pose_range_pct,"jump_height_pct":s.jump_height_pct,
        "mouth_opening":s.smile_open,"eye_blink":s.eye_blink,"eye_gaze":s.eye_gaze,"head_yaw":s.head_yaw,"head_pitch":s.head_pitch,
        "arm_spread":s.arm_spread,"elbow_bend":s.elbow_bend,"knee_bend":s.knee_bend,"ear_splay":s.ear_splay,"ear_pitch":s.ear_pitch,
        "inflation_depth_pct":s.inflation_depth_pct,"relief_detail_pct":s.relief_detail_pct,"mesh_res":s.mesh_res,
        "proj_blend_pct":s.proj_blend_pct,"bump_strength":s.bump_strength,"explode_pct":s.explode_pct,"yaw":s.yaw_deg,"bg_tolerance":s.bg_tolerance})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_controls_change_state_and_invalid_batch_preserves_original() {
        let state = UiState::default();
        let changed = updated(&state, &json!({"head_pitch":12.5,"eye_gaze":20,"mesh_res":64,"bump_strength":70})).unwrap();
        assert_eq!((changed.head_pitch, changed.eye_gaze, changed.mesh_res, changed.bump_strength), (12.5,20.0,64,70.0));
        assert!(updated(&state, &json!({"head_pitch":12.5,"mesh_res":"64"})).is_err());
        assert_eq!(state.head_pitch, 0.0);
        assert!(updated(&state, &json!({"playing":"false"})).is_err());
        assert_eq!(updated(&state, &json!({"eye_gaze":900})).unwrap().eye_gaze,45.0);
    }
}
