//! Pure, frame-rate-independent motion and viewport scale helpers for the 2D Action Gate (`center.rs`).
//!
//! Responsibilities:
//! - C0-continuous jump kinematics (takeoff anticipation -> ballistic flight -> apex -> landing absorption -> rest)
//!   driven by `jump_height_pct` and `pose_range_pct`.
//! - `dt`-scaled playback phase stepping and WASD roaming independent of frame rate.
//! - Deterministic Stop / Pause / Reset / Jump / Fit / 100% / Zoom state transitions.
//! - Unified stage scaling (`compute_preview_scale`) shared by cutout-rig and sprite render paths,
//!   preserving ground baseline anchoring and jump headroom.
//! - Single-reference pose lookup per `AnimMode` so static pose reference sheets never cycle
//!   unrelated reference poses as if they were a temporal flipbook.
//! - Data-driven cutout rig evaluation using `LoadedCutoutRigPlan` (`parts`, `pose_library`,
//!   per-joint `rotation` limits, and `PartState` visibility/solo) without inventing anatomy
//!   for unrigged assets.

use crate::{AnimMode, UiState};
use animationcraft_engine::session::{CutoutRigPart, LoadedCutoutRigPlan, PartState};
use serde_json::Value;
use std::collections::BTreeMap;

/// Maximum per-frame delta time (seconds) to prevent large phase/position jumps after window stalls.
pub(crate) const MAX_DT_SEC: f32 = 0.1;

/// Base phase angular velocity (rad/s) for normal playback (`sprint_lock == false`).
pub(crate) const WALK_PHASE_RATE: f64 = 3.2;

/// Base phase angular velocity (rad/s) when sprint lock is active (`sprint_lock == true`).
pub(crate) const SPRINT_PHASE_RATE: f64 = 5.0;

/// Horizontal WASD roaming speed in stage pixels per second when walking.
pub(crate) const WALK_ROAM_PX_PER_SEC: f32 = 180.0;

/// Horizontal WASD roaming speed in stage pixels per second when sprinting.
pub(crate) const SPRINT_ROAM_PX_PER_SEC: f32 = 330.0;

/// Horizontal stage offset bounds (px).
pub(crate) const STAGE_X_MIN: f32 = -220.0;
pub(crate) const STAGE_X_MAX: f32 = 220.0;

/// Vertical stage offset bounds (px).
pub(crate) const STAGE_Y_MIN: f32 = -80.0;
pub(crate) const STAGE_Y_MAX: f32 = 40.0;

/// Preview zoom multiplier bounds and step ratio.
pub(crate) const MIN_PREVIEW_ZOOM: f32 = 0.25;
pub(crate) const MAX_PREVIEW_ZOOM: f32 = 4.0;
pub(crate) const ZOOM_STEP_FACTOR: f32 = 1.25;

/// Normalized cycle fraction where takeoff crouch ends and airborne flight begins.
pub(crate) const JUMP_TAKEOFF_END: f32 = 0.18;

/// Normalized cycle fraction where airborne flight touches down and landing absorption begins.
pub(crate) const JUMP_LANDING_START: f32 = 0.82;

/// Clamp raw frame delta time to `[0.0, MAX_DT_SEC]`.
#[inline]
pub(crate) fn sanitize_dt(raw_dt: f32) -> f32 {
    if raw_dt.is_finite() {
        raw_dt.clamp(0.0, MAX_DT_SEC)
    } else {
        0.0
    }
}

/// Sanitize `preview_zoom` into `[MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM]`.
#[inline]
pub(crate) fn sanitize_zoom(raw_zoom: f32) -> f32 {
    if raw_zoom.is_finite() && raw_zoom > 0.0 {
        raw_zoom.clamp(MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM)
    } else {
        1.0
    }
}

/// Compute the unified rendering scale for both `cutout_rig` and sprite paths.
///
/// - When `preview_actual_size == false`, the base scale fits `(content_w, content_h * headroom)`
///   into `(avail_w, avail_h)` so the full character and jump arc fit inside the stage.
/// - When `preview_actual_size == true`, the base scale is `1.0` (100% actual pixel size).
/// - `preview_zoom` multiplies the chosen base scale.
pub(crate) fn compute_preview_scale(
    content_w: f32,
    content_h: f32,
    avail_w: f32,
    avail_h: f32,
    headroom: f32,
    preview_actual_size: bool,
    preview_zoom: f32,
) -> f32 {
    let cw = content_w.max(1.0);
    let ch = content_h.max(1.0);
    let hr = if headroom.is_finite() {
        headroom.max(1.0)
    } else {
        1.0
    };
    let fit_scale = (avail_h.max(64.0) / (ch * hr))
        .min(avail_w.max(64.0) / cw)
        .max(0.05);
    let base_scale = if preview_actual_size { 1.0 } else { fit_scale };
    (base_scale * sanitize_zoom(preview_zoom)).clamp(0.05, 8.0)
}

/// Reset viewport scale to Fit mode (`preview_actual_size = false`, `preview_zoom = 1.0`).
pub(crate) fn fit_viewport_state(ui_state: &mut UiState) {
    ui_state.preview_actual_size = false;
    ui_state.preview_zoom = 1.0;
}

/// Set viewport scale to 100% actual pixel size (`preview_actual_size = true`, `preview_zoom = 1.0`).
pub(crate) fn actual_size_viewport_state(ui_state: &mut UiState) {
    ui_state.preview_actual_size = true;
    ui_state.preview_zoom = 1.0;
}

/// Step viewport zoom in by `ZOOM_STEP_FACTOR`, clamped to `MAX_PREVIEW_ZOOM`.
pub(crate) fn zoom_in_state(ui_state: &mut UiState) {
    ui_state.preview_zoom =
        (sanitize_zoom(ui_state.preview_zoom) * ZOOM_STEP_FACTOR).clamp(MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM);
}

/// Step viewport zoom out by `ZOOM_STEP_FACTOR`, clamped to `MIN_PREVIEW_ZOOM`.
pub(crate) fn zoom_out_state(ui_state: &mut UiState) {
    ui_state.preview_zoom =
        (sanitize_zoom(ui_state.preview_zoom) / ZOOM_STEP_FACTOR).clamp(MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM);
}

/// Advance `playback_phase` proportionally to `dt` when `playing` is true.
#[inline]
pub(crate) fn step_playback_phase(
    phase: f64,
    raw_dt: f32,
    playing: bool,
    anim_speed: f32,
    sprint_lock: bool,
) -> f64 {
    if !playing {
        return phase;
    }
    let dt = sanitize_dt(raw_dt) as f64;
    let speed = if anim_speed.is_finite() {
        anim_speed.max(0.0) as f64
    } else {
        1.0
    };
    let rate = if sprint_lock {
        SPRINT_PHASE_RATE
    } else {
        WALK_PHASE_RATE
    };
    phase + dt * speed * rate
}

/// Snapshot of keyboard directional + jump inputs for `step_wasd_roam`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WasdInput {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub jump_pressed: bool,
}

/// Result of a single `dt`-scaled WASD update step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WasdStepOutcome {
    pub offset_x: f32,
    pub offset_y: f32,
    pub mode_override: Option<AnimMode>,
    pub triggered_jump: bool,
    pub moved: bool,
}

/// Update stage offsets using `dt`-scaled velocity so movement speed is independent of frame rate.
pub(crate) fn step_wasd_roam(
    offset_x: f32,
    offset_y: f32,
    raw_dt: f32,
    input: WasdInput,
    sprint_lock: bool,
    anim_speed: f32,
) -> WasdStepOutcome {
    let dt = sanitize_dt(raw_dt);
    let speed_mult = if anim_speed.is_finite() {
        anim_speed.clamp(0.1, 3.0)
    } else {
        1.0
    };
    let px_per_sec = if sprint_lock {
        SPRINT_ROAM_PX_PER_SEC
    } else {
        WALK_ROAM_PX_PER_SEC
    };
    let step = px_per_sec * speed_mult * dt;

    let dx = (if input.right { 1.0 } else { 0.0 }) - (if input.left { 1.0 } else { 0.0 });
    let dy = (if input.down { 1.0 } else { 0.0 }) - (if input.up { 1.0 } else { 0.0 });

    let next_x = (offset_x + dx * step).clamp(STAGE_X_MIN, STAGE_X_MAX);
    let next_y = (offset_y + dy * step * 0.7).clamp(STAGE_Y_MIN, STAGE_Y_MAX);

    let (mode_override, triggered_jump) = if input.jump_pressed {
        (Some(AnimMode::Jump), true)
    } else if dx != 0.0 {
        (
            Some(if sprint_lock {
                AnimMode::Run
            } else {
                AnimMode::Walk
            }),
            false,
        )
    } else {
        (None, false)
    };

    WasdStepOutcome {
        offset_x: next_x,
        offset_y: next_y,
        mode_override,
        triggered_jump,
        moved: dx != 0.0 || dy != 0.0,
    }
}

/// Stop playback and return phase and stage roam offsets to the origin rest state.
pub(crate) fn stop_motion_state(ui_state: &mut UiState) {
    ui_state.playing = false;
    ui_state.playback_phase = 0.0;
    ui_state.stage_offset_x = 0.0;
    ui_state.stage_offset_y = 0.0;
}

/// Center the character on the stage without altering zoom mode.
pub(crate) fn center_stage_state(ui_state: &mut UiState) {
    ui_state.stage_offset_x = 0.0;
    ui_state.stage_offset_y = 0.0;
    if !ui_state.playing {
        ui_state.playback_phase = 0.0;
    }
}

/// Start a Jump cycle cleanly from `phase = 0.0` (takeoff anticipation -> flight -> apex -> landing).
pub(crate) fn trigger_jump_state(ui_state: &mut UiState) {
    ui_state.anim_mode = AnimMode::Jump;
    ui_state.playing = true;
    ui_state.playback_phase = 0.0;
}

/// Toggle `sprint_lock` and keep Walk/Run mode synchronized so the Run toggle has immediate effect.
pub(crate) fn toggle_sprint_lock_state(ui_state: &mut UiState) {
    ui_state.sprint_lock = !ui_state.sprint_lock;
    if ui_state.sprint_lock && ui_state.anim_mode == AnimMode::Walk {
        ui_state.anim_mode = AnimMode::Run;
    } else if !ui_state.sprint_lock && ui_state.anim_mode == AnimMode::Run {
        ui_state.anim_mode = AnimMode::Walk;
    }
}

/// Distinct biomechanical stages within a single `TAU`-radian Jump cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JumpStage {
    Grounded,
    Takeoff,
    Ascending,
    Apex,
    Descending,
    Landing,
}

impl JumpStage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Grounded => "grounded",
            Self::Takeoff => "takeoff",
            Self::Ascending => "ascending",
            Self::Apex => "apex",
            Self::Descending => "descending",
            Self::Landing => "landing",
        }
    }
}

/// Continuous jump kinematics evaluated at a given `playback_phase`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct JumpKinematics {
    pub stage: JumpStage,
    /// Normalized cycle progress in `[0.0, 1.0)`.
    pub cycle_u: f32,
    /// Unscaled ballistic flight parabola in `[0.0, 1.0]` (`0.0` outside `[0.18, 0.82]`, `1.0` at `0.50`).
    pub lift_unit: f32,
    /// Unscaled grounded crouch compression in `[0.0, 1.0]` (`0.0` at `0.0`, `0.18`, `0.82`, and `1.0`).
    pub crouch_unit: f32,
    /// Height-scaled lift fraction (`lift_unit * (jump_height_pct / 100.0) * range_factor`).
    pub lift_fraction: f32,
    /// Height-scaled crouch compression (`crouch_unit * (jump_height_pct / 80.0) * range_factor`).
    pub crouch_fraction: f32,
}

impl JumpKinematics {
    /// Compute vertical pixel lift for a character of rendered height `char_height_px`.
    #[inline]
    pub(crate) fn lift_px(self, char_height_px: f32) -> f32 {
        self.lift_fraction * char_height_px.max(0.0)
    }
}

/// Grounded anticipation/landing interval surrounding a smooth ballistic flight parabola.
#[inline]
pub(crate) fn jump_arc(phase: f64) -> f32 {
    let cycle = (phase / std::f64::consts::TAU).rem_euclid(1.0) as f32;
    if !(JUMP_TAKEOFF_END..=JUMP_LANDING_START).contains(&cycle) {
        return 0.0;
    }
    let flight = (cycle - JUMP_TAKEOFF_END) / (JUMP_LANDING_START - JUMP_TAKEOFF_END);
    4.0 * flight * (1.0 - flight)
}

/// Evaluate C0-continuous takeoff crouch, ballistic flight, apex, and landing absorption.
pub(crate) fn evaluate_jump(
    phase: f64,
    jump_height_pct: f32,
    range_factor: f32,
    active: bool,
) -> JumpKinematics {
    let height_norm = if jump_height_pct.is_finite() {
        (jump_height_pct / 100.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let crouch_scale = if jump_height_pct.is_finite() {
        (jump_height_pct / 80.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let range = if range_factor.is_finite() {
        range_factor.clamp(0.0, 1.0)
    } else {
        0.0
    };

    if !active || height_norm <= 1e-5 || range <= 1e-5 {
        return JumpKinematics {
            stage: JumpStage::Grounded,
            cycle_u: 0.0,
            lift_unit: 0.0,
            crouch_unit: 0.0,
            lift_fraction: 0.0,
            crouch_fraction: 0.0,
        };
    }

    let u = (phase / std::f64::consts::TAU).rem_euclid(1.0) as f32;
    let (stage, lift_unit, crouch_unit) = if u <= 1e-5 || (1.0 - u) <= 1e-5 {
        (JumpStage::Grounded, 0.0, 0.0)
    } else if u < JUMP_TAKEOFF_END {
        // Smooth half-sine anticipation crouch: 0 at u=0, peak 1.0 at u=0.09, 0 at liftoff u=0.18
        let t = (u / JUMP_TAKEOFF_END).clamp(0.0, 1.0);
        let crouch = (t * std::f32::consts::PI).sin().max(0.0);
        (JumpStage::Takeoff, 0.0, crouch)
    } else if u <= JUMP_LANDING_START {
        let flight = ((u - JUMP_TAKEOFF_END) / (JUMP_LANDING_START - JUMP_TAKEOFF_END)).clamp(0.0, 1.0);
        let lift = (4.0 * flight * (1.0 - flight)).max(0.0);
        let stage = if (u - 0.50).abs() <= 0.06 {
            JumpStage::Apex
        } else if u < 0.50 {
            JumpStage::Ascending
        } else {
            JumpStage::Descending
        };
        (stage, lift, 0.0)
    } else {
        // Smooth half-sine landing absorption: 0 at touchdown u=0.82, peak 0.75 at u=0.91, 0 at u=1.0
        let t = ((u - JUMP_LANDING_START) / (1.0 - JUMP_LANDING_START)).clamp(0.0, 1.0);
        let crouch = (t * std::f32::consts::PI).sin().max(0.0) * 0.75;
        (JumpStage::Landing, 0.0, crouch)
    };

    JumpKinematics {
        stage,
        cycle_u: u,
        lift_unit,
        crouch_unit,
        lift_fraction: lift_unit * height_norm * range,
        crouch_fraction: crouch_unit * crouch_scale * range,
    }
}

/// Priority-ordered candidate reference pose labels for each animation mode.
///
/// The first matching label in an asset's `pose_manifest` is used as the single reference
/// pose for that action mode; unrelated reference poses are never cycled as a flipbook.
pub(crate) fn desired_labels_for_mode(mode: AnimMode) -> &'static [&'static str] {
    match mode {
        AnimMode::Idle => &["standing", "idle"],
        AnimMode::Walk => &["walking", "walk", "standing"],
        AnimMode::Run => &["running", "run", "walking", "standing"],
        AnimMode::Jump => &["jumping", "jump", "standing"],
        AnimMode::Wave => &["waving", "wave", "standing"],
        AnimMode::LookScout => &["looking_up", "pointing", "standing"],
        AnimMode::Static => &["standing"],
    }
}

/// Check whether a cutout rig part should be rendered according to `part_states` (`visible` / `solo`).
///
/// Supports both canonical rig IDs (`arm_left`, `ear_left`, etc.) and manifest part aliases
/// (`arm_L`, `ears`, etc.).
pub(crate) fn is_rig_part_visible(part_id: &str, part_states: &[PartState]) -> bool {
    if part_states.is_empty() {
        return true;
    }
    let any_solo = part_states.iter().any(|p| p.solo);
    let single = [part_id];
    let aliases: &[&str] = match part_id {
        "arm_left" => &["arm_left", "arm_L"],
        "arm_right" => &["arm_right", "arm_R"],
        "leg_left" => &["leg_left", "leg_L"],
        "leg_right" => &["leg_right", "leg_R"],
        "foot_left" => &["foot_left", "leg_left", "leg_L"],
        "foot_right" => &["foot_right", "leg_right", "leg_R"],
        "ear_left" => &["ear_left", "ears"],
        "ear_right" => &["ear_right", "ears"],
        _ => &single,
    };

    for candidate in aliases {
        if let Some(st) = part_states.iter().find(|p| p.name == *candidate) {
            return if any_solo { st.solo } else { st.visible };
        }
    }
    !any_solo
}

/// Evaluated joint rotations and root offset for a `LoadedCutoutRigPlan`.
#[derive(Debug, Clone, Default)]
pub(crate) struct EvaluatedRigMotion {
    /// Clamped joint angle in radians for each `part.id`.
    pub angles_rad: BTreeMap<String, f32>,
    /// Clamped joint angle in degrees for each `part.id`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub angles_deg: BTreeMap<String, f32>,
    /// Vertical root translation in rig canvas pixels (negative = upward lift, positive = downward crouch).
    pub root_dy_canvas: f32,
}

fn pose_library_part_rotation(pose_library: &Value, pose_name: &str, part_id: &str) -> Option<f32> {
    pose_library
        .get("poses")?
        .get(pose_name)?
        .get("parts")?
        .get(part_id)?
        .get("rotation")?
        .as_f64()
        .map(|v| v as f32)
}

fn slider_offset_deg(part_id: &str, ui_state: &UiState) -> f32 {
    match part_id {
        "head" => ui_state.head_yaw * 0.35 + ui_state.head_pitch * 0.25,
        "ear_left" => -ui_state.ear_splay * 0.35 + ui_state.ear_pitch * 0.25,
        "ear_right" => ui_state.ear_splay * 0.35 + ui_state.ear_pitch * 0.25,
        "arm_left" => ui_state.arm_spread * 0.55 + ui_state.elbow_bend * 0.45,
        "arm_right" => -ui_state.arm_spread * 0.25 - ui_state.elbow_bend * 0.20,
        "leg_left" => ui_state.knee_bend * 0.45,
        "foot_left" => ui_state.knee_bend * 0.45,
        "leg_right" => -ui_state.knee_bend * 0.45,
        "foot_right" => -ui_state.knee_bend * 0.45,
        _ => 0.0,
    }
}

fn clamp_part_rotation(part: &CutoutRigPart, deg: f32) -> f32 {
    let [r_min, r_max] = part.rotation;
    if r_min < r_max {
        deg.clamp(r_min, r_max)
    } else {
        deg
    }
}

fn fallback_part_dynamic_deg(
    part_id: &str,
    mode: AnimMode,
    held_wave: f32,
    follow_wave: f32,
    envelope: f32,
    amp: f32,
) -> f32 {
    match mode {
        AnimMode::Idle => match part_id {
            "torso" => held_wave * 1.5 * amp,
            "head" => -held_wave * 2.5 * amp,
            "ear_left" => follow_wave * 6.0 * amp,
            "ear_right" => -follow_wave * 6.0 * amp,
            "arm_left" => held_wave * 8.0 * amp,
            "arm_right" => -held_wave * 3.0 * amp,
            "leg_left" => held_wave * 4.0 * amp,
            "foot_left" => follow_wave * 10.0 * amp,
            "leg_right" => -held_wave * 4.0 * amp,
            "foot_right" => -follow_wave * 10.0 * amp,
            _ => 0.0,
        },
        AnimMode::Walk => match part_id {
            "torso" => held_wave * 3.0 * amp,
            "head" => -held_wave * 2.5 * amp,
            "ear_left" => -follow_wave * 10.0 * amp,
            "ear_right" => -follow_wave * 8.0 * amp,
            "arm_left" => held_wave * 18.0 * amp,
            "arm_right" => -held_wave * 4.5 * amp,
            "leg_left" => -held_wave * 13.0 * amp,
            "foot_left" => -held_wave * 15.0 * amp + follow_wave * 6.0 * amp,
            "leg_right" => held_wave * 13.0 * amp,
            "foot_right" => held_wave * 15.0 * amp - follow_wave * 6.0 * amp,
            _ => 0.0,
        },
        AnimMode::Run => match part_id {
            "torso" => held_wave * 4.5 * amp,
            "head" => -held_wave * 3.5 * amp,
            "ear_left" => -follow_wave * 14.0 * amp,
            "ear_right" => -follow_wave * 13.0 * amp,
            "arm_left" => held_wave * 24.0 * amp,
            "arm_right" => -held_wave * 6.0 * amp,
            "leg_left" => -held_wave * 16.0 * amp,
            "foot_left" => -held_wave * 18.0 * amp + follow_wave * 8.0 * amp,
            "leg_right" => held_wave * 16.0 * amp,
            "foot_right" => held_wave * 18.0 * amp - follow_wave * 8.0 * amp,
            _ => 0.0,
        },
        AnimMode::Wave => match part_id {
            "torso" => held_wave * 2.0 * amp,
            "head" => (-4.5 * envelope + held_wave * 3.0) * amp,
            "ear_left" => (-8.0 * envelope + follow_wave * 6.0) * amp,
            "ear_right" => (8.0 * envelope + follow_wave * 6.0) * amp,
            "arm_left" => (24.0 * envelope + held_wave * 18.0) * amp,
            "arm_right" => (-3.5 * envelope + follow_wave * 2.5) * amp,
            "leg_left" => -held_wave * 6.0 * amp,
            "foot_left" => (-held_wave * 10.0 + follow_wave * 5.0) * amp,
            "leg_right" => held_wave * 6.0 * amp,
            "foot_right" => (held_wave * 10.0 - follow_wave * 5.0) * amp,
            _ => 0.0,
        },
        AnimMode::LookScout => match part_id {
            "torso" => held_wave * 2.5 * amp,
            "head" => held_wave * 8.0 * amp,
            "ear_left" => held_wave * 13.0 * amp,
            "ear_right" => held_wave * 12.0 * amp,
            "arm_left" => follow_wave * 14.0 * amp,
            "arm_right" => -follow_wave * 4.0 * amp,
            "leg_left" => -held_wave * 10.0 * amp,
            "foot_left" => -held_wave * 14.0 * amp,
            "leg_right" => held_wave * 10.0 * amp,
            "foot_right" => held_wave * 14.0 * amp,
            _ => 0.0,
        },
        AnimMode::Jump | AnimMode::Static => 0.0,
    }
}

fn fallback_jump_poses_deg(part_id: &str) -> (f32, f32) {
    match part_id {
        "torso" => (5.0, -4.0),
        "head" => (-6.0, 6.0),
        "ear_left" => (-12.0, 14.0),
        "ear_right" => (-10.0, 16.0),
        "arm_left" => (-18.0, 28.0),
        "arm_right" => (5.0, -6.0),
        "leg_left" => (14.0, -16.0),
        "foot_left" => (14.0, -18.0),
        "leg_right" => (12.0, -14.0),
        "foot_right" => (14.0, -16.0),
        _ => (0.0, 0.0),
    }
}

/// Evaluate hierarchical cutout rig joint angles and root displacement for the current `UiState`.
///
/// Guarantees:
/// - When stopped at `playback_phase == 0.0` (or in `AnimMode::Static` or at `0%` range),
///   all dynamic oscillations and `root_dy_canvas` evaluate to `0.0` (pure rest/slider pose).
/// - When paused (`!playing && playback_phase > 0.0`), holds the exact pose for `playback_phase`.
/// - Reuses `rig.pose_library` pose targets when available for each `part.id` in `rig.parts` and
///   clamps every joint angle to `part.rotation` `[min_deg, max_deg]`.
pub(crate) fn evaluate_cutout_rig_motion(
    rig: &LoadedCutoutRigPlan,
    ui_state: &UiState,
) -> EvaluatedRigMotion {
    let ch = rig.canvas_size[1].max(1) as f32;
    let range_factor = (ui_state.pose_range_pct / 100.0).clamp(0.0, 1.0);
    let energy_factor = (ui_state.anim_energy / 100.0).clamp(0.2, 1.5);
    let amp = range_factor * energy_factor;

    let motion_active = (ui_state.playing || ui_state.playback_phase.abs() > 1e-6)
        && ui_state.anim_mode != AnimMode::Static
        && range_factor > 0.005;

    let phase = ui_state.playback_phase as f32;
    // Smooth onset envelope: 0.0 at phase == 0.0, rapidly approaches 1.0 as phase > 0.
    // Prevents any non-zero cosine snap at phase == 0.0.
    let envelope = if motion_active {
        1.0 - (-phase.abs() * 3.5).exp()
    } else {
        0.0
    };
    let held_wave = if motion_active {
        (phase.sin() * 1.35).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let follow_wave = if motion_active {
        ((phase - 0.65).sin() * 1.35).clamp(-1.0, 1.0) * envelope
    } else {
        0.0
    };

    let jump_kin = if ui_state.anim_mode == AnimMode::Jump {
        Some(evaluate_jump(
            ui_state.playback_phase,
            ui_state.jump_height_pct,
            range_factor,
            motion_active,
        ))
    } else {
        None
    };

    let root_dy_canvas = if !motion_active {
        0.0
    } else {
        match ui_state.anim_mode {
            AnimMode::Idle => -held_wave.abs() * 6.0 * amp,
            AnimMode::Walk => -held_wave.abs() * 16.0 * amp,
            AnimMode::Run => -held_wave.abs() * 26.0 * amp,
            AnimMode::Jump => {
                let jk = jump_kin.unwrap_or_else(|| {
                    evaluate_jump(
                        ui_state.playback_phase,
                        ui_state.jump_height_pct,
                        range_factor,
                        true,
                    )
                });
                -jk.lift_px(ch) + jk.crouch_fraction * (ch * 0.022).max(16.0) * energy_factor
            }
            AnimMode::Wave | AnimMode::LookScout => -held_wave.abs() * 8.0 * amp,
            AnimMode::Static => 0.0,
        }
    };

    let mut angles_rad = BTreeMap::new();
    let mut angles_deg = BTreeMap::new();

    for part in &rig.parts {
        let pid = part.id.as_str();
        let slider_deg = slider_offset_deg(pid, ui_state);

        let dynamic_deg = if !motion_active {
            0.0
        } else {
            match ui_state.anim_mode {
                AnimMode::Jump => {
                    let jk = jump_kin.unwrap();
                    let (fb_antic, fb_apex) = fallback_jump_poses_deg(pid);
                    let antic = pose_library_part_rotation(&rig.pose_library, "jump_anticipation", pid)
                        .unwrap_or(fb_antic);
                    let apex = pose_library_part_rotation(&rig.pose_library, "jump_apex", pid)
                        .unwrap_or(fb_apex);
                    let height_gain = (ui_state.jump_height_pct / 45.0).clamp(0.0, 1.75);
                    (jk.crouch_unit * antic + jk.lift_unit * apex) * height_gain * amp
                }
                AnimMode::Walk | AnimMode::Run => {
                    let (contact_name, passing_name) = if ui_state.anim_mode == AnimMode::Run {
                        ("run_contact", "run_passing")
                    } else {
                        ("walk_contact", "walk_passing")
                    };
                    if let (Some(c_rot), Some(p_rot)) = (
                        pose_library_part_rotation(&rig.pose_library, contact_name, pid),
                        pose_library_part_rotation(&rig.pose_library, passing_name, pid),
                    ) {
                        let primary = if held_wave >= 0.0 {
                            held_wave * c_rot
                        } else {
                            (-held_wave) * p_rot
                        };
                        let secondary = if pid.starts_with("foot_") || pid.starts_with("ear_") {
                            follow_wave * 0.35 * (c_rot.abs().max(p_rot.abs()))
                        } else {
                            0.0
                        };
                        (primary + secondary) * amp
                    } else {
                        fallback_part_dynamic_deg(
                            pid,
                            ui_state.anim_mode,
                            held_wave,
                            follow_wave,
                            envelope,
                            amp,
                        )
                    }
                }
                AnimMode::Wave => {
                    if let (Some(g_rot), Some(alt_rot)) = (
                        pose_library_part_rotation(&rig.pose_library, "wave_greeting", pid),
                        pose_library_part_rotation(&rig.pose_library, "wave_greeting_alt", pid),
                    ) {
                        let mid = 0.5 * (g_rot + alt_rot);
                        let swing = 0.5 * (g_rot - alt_rot);
                        (mid * envelope + swing * held_wave) * amp
                    } else {
                        fallback_part_dynamic_deg(
                            pid,
                            ui_state.anim_mode,
                            held_wave,
                            follow_wave,
                            envelope,
                            amp,
                        )
                    }
                }
                AnimMode::Idle => {
                    if let (Some(i_rot), Some(b_rot)) = (
                        pose_library_part_rotation(&rig.pose_library, "idle", pid),
                        pose_library_part_rotation(&rig.pose_library, "idle_breathe", pid),
                    ) {
                        let primary = if held_wave >= 0.0 {
                            held_wave * b_rot
                        } else {
                            (-held_wave) * i_rot
                        };
                        primary * amp
                    } else {
                        fallback_part_dynamic_deg(
                            pid,
                            ui_state.anim_mode,
                            held_wave,
                            follow_wave,
                            envelope,
                            amp,
                        )
                    }
                }
                AnimMode::LookScout | AnimMode::Static => fallback_part_dynamic_deg(
                    pid,
                    ui_state.anim_mode,
                    held_wave,
                    follow_wave,
                    envelope,
                    amp,
                ),
            }
        };

        let clamped_deg = clamp_part_rotation(part, dynamic_deg + slider_deg);
        angles_deg.insert(part.id.clone(), clamped_deg);
        angles_rad.insert(part.id.clone(), clamped_deg.to_radians());
    }

    EvaluatedRigMotion {
        angles_rad,
        angles_deg,
        root_dy_canvas,
    }
}
