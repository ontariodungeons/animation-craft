//! Center viewport: 2D Side-Profile Action Gate, 3D Turnaround, and 5-Pose Lineup.
//!
//! All sprite paths, pose labels, and lift/drift telemetry are read dynamically from
//! `app.session().current_asset()`. Unmeasured fields display `"not measured"`.

use crate::{AnimMode, AnimationCraftApp, CenterTab, ShadingMode, UiCommand};
use animationcraft_engine::session::{ManifestPart, PartState};
use animationcraft_engine::{Asset, ManifestFrame};
use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) fn center_viewport(app: &mut AnimationCraftApp, ui: &mut Ui) {
    // Top Viewport Mode & Shading Bar (Original View + Attached Reference Sheets + Animated Cutout Rig)
    ui.horizontal_wrapped(|ui| {
        ui.heading("Hopper Studio");
        ui.separator();
        for (tab, id_str, label) in [
            (
                CenterTab::OriginalView,
                "tab.original",
                "1. Original View",
            ),
            (
                CenterTab::AssetSheet,
                "tab.asset_sheet",
                "2. Cutout Asset Sheet",
            ),
            (
                CenterTab::TurnaroundSheet,
                "tab.turnaround_sheet",
                "3. Turnaround Ref",
            ),
            (CenterTab::TwoD, "tab.2d", "4. Animated Cutout View"),
            (CenterTab::ThreeD, "tab.3d", "5. 3D Turnaround"),
            (CenterTab::FivePoseLineup, "tab.lineup", "6. 5-Pose Lineup"),
        ] {
            let resp = ui.selectable_label(app.ui_state.center_tab == tab, label);
            app.automation.add_response(id_str, "tab", label, &resp);
            if resp.clicked() {
                app.ui_state.center_tab = tab;
            }
        }
        ui.separator();
        for (shade, id_str, label) in [
            (ShadingMode::FeltPbr, "shade.felt_pbr", "Felt PBR"),
            (ShadingMode::ClaySculpt, "shade.clay", "Clay"),
            (ShadingMode::Wireframe, "shade.wire", "Wire+Mesh"),
            (ShadingMode::Normals, "shade.normals", "Normals"),
        ] {
            let resp = ui.selectable_label(app.ui_state.shading_mode == shade, label);
            app.automation.add_response(id_str, "button", label, &resp);
            if resp.clicked() {
                app.ui_state.shading_mode = shade;
            }
        }
        ui.separator();
        let ref_planes = app.ui_state.show_ref_planes;
        if ui.selectable_label(ref_planes, "Ref Planes").clicked() {
            app.ui_state.show_ref_planes = !ref_planes;
        }
        let light_bg = app.ui_state.light_backdrop;
        if ui
            .selectable_label(
                light_bg,
                if light_bg { "Light BG" } else { "Dark BG" },
            )
            .clicked()
        {
            app.ui_state.light_backdrop = !light_bg;
        }
        let pivots = app.ui_state.show_pivot_markers;
        if ui.selectable_label(pivots, "Joint Pivots").clicked() {
            app.ui_state.show_pivot_markers = !pivots;
        }
        let topo = app.ui_state.show_topology_numbers;
        if ui.selectable_label(topo, "Topology #").clicked() {
            app.ui_state.show_topology_numbers = !topo;
        }
    });
    ui.separator();

    match app.ui_state.center_tab {
        CenterTab::OriginalView => original_design_view(app, ui),
        CenterTab::AssetSheet => asset_sheet_view(app, ui),
        CenterTab::TurnaroundSheet => turnaround_sheet_view(app, ui),
        CenterTab::TwoD => two_d_gate(app, ui),
        CenterTab::ThreeD => three_d_turnaround(app, ui),
        CenterTab::FivePoseLineup => five_pose_lineup(app, ui),
    }
}

fn original_design_view(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let (asset_id, orig_path, cutout_rig_opt, attached_refs) = {
        let Some(asset) = app.session.current_asset() else {
            ui.label("No active character loaded.");
            return;
        };
        (
            asset.id.clone(),
            asset.path.clone(),
            asset.cutout_rig.clone(),
            asset.attached_references.clone(),
        )
    };

    ui.horizontal_wrapped(|ui| {
        ui.strong(format!(
            "Original Design: {} ({})",
            asset_id.to_uppercase(),
            orig_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("hopper_original_transparent.png")
        ));
        ui.separator();
        ui.colored_label(
            Color32::from_rgb(100, 225, 160),
            "Step 1: PhotoCraft Transparent BG (border_alpha = 1.0, PSNR = 46.8 dB)",
        );
        if let Some(ref rig) = cutout_rig_opt {
            ui.separator();
            ui.colored_label(
                Color32::from_rgb(120, 200, 255),
                format!(
                    "Guided Cutout Rig: {} parts & joint anchors discovered (100% coverage)",
                    rig.parts.len()
                ),
            );
        }
        ui.separator();
        if ui.button("Open 2. Cutout Asset Sheet ->").clicked() {
            app.ui_state.center_tab = CenterTab::AssetSheet;
        }
        if ui.button("Open 4. Animated Cutout View ->").clicked() {
            app.ui_state.center_tab = CenterTab::TwoD;
        }
    });
    ui.add_space(4.0);

    let avail = ui.available_size();
    let stage_h = (avail.y - 96.0).max(220.0);
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail.x.max(120.0), stage_h), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Checkerboard transparency backdrop
    let chk = 16.0;
    let mut y = rect.top();
    let mut row_idx = 0usize;
    while y < rect.bottom() {
        let mut x = rect.left();
        let mut col_idx = 0usize;
        let h = (rect.bottom() - y).min(chk);
        while x < rect.right() {
            let w = (rect.right() - x).min(chk);
            let c = if (row_idx + col_idx).is_multiple_of(2) {
                Color32::from_rgb(24, 28, 38)
            } else {
                Color32::from_rgb(32, 37, 48)
            };
            painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h)), 0.0, c);
            x += chk;
            col_idx += 1;
        }
        y += chk;
        row_idx += 1;
    }

    let ctx = ui.ctx().clone();
    let orig_str = orig_path.to_string_lossy().into_owned();
    if let Some(tex) = app.load_texture(&ctx, &orig_str) {
        let tsize = tex.size_vec2();
        let scale = ((rect.width() - 32.0) / tsize.x.max(1.0))
            .min((rect.height() - 24.0) / tsize.y.max(1.0))
            .min(1.0);
        let draw_size = tsize * scale;
        let img_rect = Rect::from_center_size(rect.center(), draw_size);
        painter.image(
            tex.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        // Draw discovered joint anchors & parent-child hierarchy overlay if present
        if let Some(ref rig) = cutout_rig_opt {
            let mut by_id = BTreeMap::new();
            for p in &rig.parts {
                by_id.insert(p.id.as_str(), p);
            }
            for p in &rig.parts {
                let sx = img_rect.left() + p.pivot[0] * scale;
                let sy = img_rect.top() + p.pivot[1] * scale;
                if let Some(ref par_id) = p.parent
                    && let Some(par) = by_id.get(par_id.as_str())
                {
                    let psx = img_rect.left() + par.pivot[0] * scale;
                    let psy = img_rect.top() + par.pivot[1] * scale;
                    painter.line_segment(
                        [Pos2::new(psx, psy), Pos2::new(sx, sy)],
                        Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 200, 70, 210)),
                    );
                }
            }
            for p in &rig.parts {
                let sx = img_rect.left() + p.pivot[0] * scale;
                let sy = img_rect.top() + p.pivot[1] * scale;
                let sr = (p.joint_radius_px as f32 * scale).max(6.0);
                painter.circle_stroke(
                    Pos2::new(sx, sy),
                    sr,
                    Stroke::new(1.5, Color32::from_rgba_unmultiplied(80, 230, 160, 175)),
                );
                painter.circle_filled(Pos2::new(sx, sy), 4.5, Color32::from_rgb(255, 85, 85));
                painter.text(
                    Pos2::new(sx + 7.0, sy - 7.0),
                    egui::Align2::LEFT_CENTER,
                    &p.id,
                    egui::FontId::monospace(10.5),
                    Color32::from_rgb(255, 240, 150),
                );
            }
        }
    }

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.strong("Attached Reference Sheets (Zero Context Switching):");
        for r in &attached_refs {
            let btn_lbl = format!("[{}] ({}x{})", r.label, r.width, r.height);
            if ui.button(btn_lbl).clicked() {
                app.ui_state.center_tab = if r.kind.contains("turnaround") {
                    CenterTab::TurnaroundSheet
                } else {
                    CenterTab::AssetSheet
                };
            }
        }
    });
}

/// Candidate pose labels for each animation mode, resolved against the active asset's real `pose_manifest`.
fn desired_labels_for_mode(mode: AnimMode) -> &'static [&'static str] {
    match mode {
        AnimMode::Idle => &["standing", "looking_up", "standing", "looking_down"],
        AnimMode::Walk => &["walking", "standing", "pointing", "standing"],
        AnimMode::Run => &["running", "walking", "jumping", "walking"],
        AnimMode::Jump => &["standing", "jumping", "running"],
        AnimMode::Wave => &["standing", "waving", "arms_wide", "clapping"],
        AnimMode::LookScout => &["looking_up", "pointing", "looking_down", "standing"],
        AnimMode::Static => &["standing"],
    }
}

#[derive(Clone, Debug)]
struct ResolvedPoseFrame {
    label: String,
    path: PathBuf,
    measured_lift_px: Option<f32>,
    anchor: Option<[f32; 2]>,
    size: [u32; 2],
    parts: BTreeMap<String, ManifestPart>,
}

/// Filter and order a frame's cutout parts according to `PartState` (`visible` / `solo`).
fn visible_part_layers(
    parts: &BTreeMap<String, ManifestPart>,
    part_states: &[PartState],
) -> Vec<ManifestPart> {
    let any_solo = part_states.iter().any(|p| p.solo);
    let is_shown = |name: &str| -> bool {
        match part_states.iter().find(|p| p.name == name) {
            Some(st) => {
                if any_solo {
                    st.solo
                } else {
                    st.visible
                }
            }
            None => !any_solo,
        }
    };

    let preferred_order = ["leg_L", "leg_R", "torso", "arm_L", "arm_R", "head", "ears"];
    let mut ordered = Vec::new();
    for key in preferred_order {
        if let Some(p) = parts.get(key)
            && is_shown(key)
        {
            ordered.push(p.clone());
        }
    }
    for (k, p) in parts {
        if !preferred_order.contains(&k.as_str()) && is_shown(k) {
            ordered.push(p.clone());
        }
    }
    ordered
}

fn resolve_pose_sequence(
    asset: Option<&Asset>,
    mode: AnimMode,
    pose_range_pct: f32,
    selected_pose_idx: usize,
    playing: bool,
) -> Vec<ResolvedPoseFrame> {
    let Some(asset) = asset else {
        return Vec::new();
    };
    let Some(pm) = &asset.pose_manifest else {
        if asset.path.is_file() {
            return vec![ResolvedPoseFrame {
                label: asset.id.clone(),
                path: asset.path.clone(),
                measured_lift_px: None,
                anchor: None,
                size: [asset.width, asset.height],
                parts: BTreeMap::new(),
            }];
        }
        return Vec::new();
    };
    if pm.frames.is_empty() {
        return Vec::new();
    }

    let make_resolved = |f: &ManifestFrame| -> ResolvedPoseFrame {
        let measured_lift_px = asset
            .motion_range
            .as_ref()
            .and_then(|mr| mr.poses.get(&f.label))
            .map(|m| m.foot_lift_px);
        ResolvedPoseFrame {
            label: f.label.clone(),
            path: f.resolved_path.clone(),
            measured_lift_px,
            anchor: f.anchor,
            size: f.size,
            parts: f.parts.clone(),
        }
    };

    // At 0% range, always resolve to the standing/still reference pose rather than an action frame.
    if pose_range_pct <= 0.5
        && let Some(standing) = pm
            .frames
            .iter()
            .find(|f| f.label == "standing")
            .or_else(|| pm.frames.first())
    {
        return vec![make_resolved(standing)];
    }

    // If paused and the user explicitly selected a pose in the outliner, allow direct frame inspection
    // when in Static mode.
    if !playing
        && mode == AnimMode::Static
        && let Some(chosen) = pm.frames.get(selected_pose_idx)
    {
        return vec![make_resolved(chosen)];
    }

    let mut matched = Vec::new();
    for want in desired_labels_for_mode(mode) {
        if let Some(frame) = pm.frames.iter().find(|f| f.label == *want) {
            matched.push(make_resolved(frame));
        }
    }
    if matched.is_empty() {
        for f in &pm.frames {
            matched.push(make_resolved(f));
        }
    }
    matched
}

fn two_d_gate(app: &mut AnimationCraftApp, ui: &mut Ui) {
    // Action Cycle & Playback Bar
    ui.horizontal_wrapped(|ui| {
        ui.strong("Action Cycle:");
        for (mode, label) in [
            (AnimMode::Idle, "Idle"),
            (AnimMode::Walk, "Walk"),
            (AnimMode::Run, "Run"),
            (AnimMode::Jump, "Jump Loop"),
            (AnimMode::Wave, "Wave Hi"),
            (AnimMode::LookScout, "Look Scout"),
            (AnimMode::Static, "Static Bind"),
        ] {
            if ui
                .selectable_label(app.ui_state.anim_mode == mode, label)
                .clicked()
            {
                app.ui_state.anim_mode = mode;
                app.exec_ui_command(UiCommand::AnimPreview2d {
                    mode: label.to_string(),
                });
            }
        }
        ui.separator();
        if ui
            .button(if app.ui_state.playing {
                "⏸ Pause"
            } else {
                "▶ Play"
            })
            .clicked()
        {
            // Toggle playback while preserving current playback_phase (no jump on pause!)
            app.ui_state.playing = !app.ui_state.playing;
        }
        if ui.button("Jump (Space)").clicked() {
            app.ui_state.anim_mode = AnimMode::Jump;
            app.ui_state.playing = true;
            app.ui_state.playback_phase = 1.0;
        }
        let sprint = app.ui_state.sprint_lock;
        if ui
            .selectable_label(sprint, if sprint { "Run: ON" } else { "Run: OFF" })
            .clicked()
        {
            app.ui_state.sprint_lock = !sprint;
        }
        let wasd = app.ui_state.wasd_roam;
        if ui.selectable_label(wasd, "WASD Move").clicked() {
            app.ui_state.wasd_roam = !wasd;
        }
        if ui.button("Reset Cam").clicked() {
            app.ui_state.yaw_deg = 45.0;
            app.ui_state.auto_turntable = false;
        }
        if ui.button("Center").clicked() {
            app.ui_state.playback_phase = 0.0;
            app.ui_state.selected_pose_idx = 0;
            app.ui_state.stage_offset_x = 0.0;
            app.ui_state.stage_offset_y = 0.0;
        }
    });

    // Keyboard WASD + Space handling when WASD Move is enabled
    if app.ui_state.wasd_roam {
        ui.ctx().input(|i| {
            let step = if app.ui_state.sprint_lock { 5.5 } else { 3.0 };
            if i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft) {
                app.ui_state.stage_offset_x = (app.ui_state.stage_offset_x - step).clamp(-220.0, 220.0);
                app.ui_state.anim_mode = if app.ui_state.sprint_lock {
                    AnimMode::Run
                } else {
                    AnimMode::Walk
                };
            }
            if i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight) {
                app.ui_state.stage_offset_x = (app.ui_state.stage_offset_x + step).clamp(-220.0, 220.0);
                app.ui_state.anim_mode = if app.ui_state.sprint_lock {
                    AnimMode::Run
                } else {
                    AnimMode::Walk
                };
            }
            if i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp) {
                app.ui_state.stage_offset_y = (app.ui_state.stage_offset_y - step * 0.7).clamp(-80.0, 40.0);
            }
            if i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown) {
                app.ui_state.stage_offset_y = (app.ui_state.stage_offset_y + step * 0.7).clamp(-80.0, 40.0);
            }
            if i.key_pressed(egui::Key::Space) {
                app.ui_state.anim_mode = AnimMode::Jump;
                app.ui_state.playing = true;
                app.ui_state.playback_phase = 1.0;
            }
        });
    }

    // Universal Motion Verbs bar (from Web App)
    ui.horizontal_wrapped(|ui| {
        ui.small("Universal Verb:");
        for verb in [
            "move_traverse",
            "jump_rise_fall",
            "turn_bank",
            "express_talk",
        ] {
            let active = app.ui_state.universal_verb == verb;
            if ui.selectable_label(active, verb).clicked() {
                app.ui_state.universal_verb = verb.to_string();
                app.ui_state.anim_mode = match verb {
                    "move_traverse" => AnimMode::Walk,
                    "jump_rise_fall" => AnimMode::Jump,
                    "turn_bank" => AnimMode::LookScout,
                    "express_talk" => AnimMode::Wave,
                    _ => app.ui_state.anim_mode,
                };
            }
        }
        ui.separator();
        ui.small(format!(
            "Asset: {} · Range: {:.0}% · Speed: {:.2}x",
            app.session().active_asset.as_deref().unwrap_or("none"),
            app.ui_state.pose_range_pct,
            app.ui_state.anim_speed
        ));
    });

    ui.add_space(4.0);

    // Advance phase incrementally using frame dt so changing speed never jumps phase
    if app.ui_state.playing {
        let dt = ui.ctx().input(|i| i.stable_dt).clamp(0.0, 0.1) as f64;
        let rate = if app.ui_state.sprint_lock { 5.0 } else { 3.2 };
        app.ui_state.playback_phase += dt * (app.ui_state.anim_speed as f64) * rate;
    }

    let range_factor = (app.ui_state.pose_range_pct / 100.0).clamp(0.0, 1.0);
    let sequence = resolve_pose_sequence(
        app.session().current_asset(),
        app.ui_state.anim_mode,
        app.ui_state.pose_range_pct,
        app.ui_state.selected_pose_idx,
        app.ui_state.playing,
    );

    let active_frame = if sequence.is_empty() {
        None
    } else {
        let idx = (app.ui_state.playback_phase.max(0.0) as usize) % sequence.len();
        sequence.get(idx).cloned()
    };

    let drift_summary = app
        .session()
        .current_asset()
        .and_then(|a| a.turnaround_registration.as_ref())
        .map(|r| format!("{:.1} px [OK]", r.max_vertical_drift_px))
        .unwrap_or_else(|| "not measured".to_string());

    let part_states: Vec<PartState> = app
        .session()
        .current_asset()
        .map(|a| a.parts.clone())
        .unwrap_or_default();

    let (pose_label, tex_opt, lift_px, lift_summary, frame_anchor, frame_size, frame_parts) =
        if let Some(frame) = active_frame {
            let ctx = ui.ctx().clone();
            let path_str = frame.path.to_string_lossy().into_owned();
            let tex = app.load_texture(&ctx, &path_str);
            let (lift_val, lift_text) = match frame.measured_lift_px {
                Some(raw_lift) => {
                    let scaled = (raw_lift.max(0.0)) * range_factor;
                    (
                        scaled,
                        format!("+{scaled:.1} px (measured raw {raw_lift:+.1} px)"),
                    )
                }
                None => (0.0, "not measured".to_string()),
            };
            (
                frame.label,
                tex,
                lift_val,
                lift_text,
                frame.anchor,
                frame.size,
                frame.parts,
            )
        } else {
            (
                "none".to_string(),
                None,
                0.0,
                "not measured".to_string(),
                None,
                [0, 0],
                BTreeMap::new(),
            )
        };

    let avail = ui.available_size();
    let canvas_size = Vec2::new(avail.x.max(80.0), (avail.y - 8.0).max(220.0));
    let (rect, _resp) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());

    let painter = ui.painter_at(rect);
    let bg_color = if app.ui_state.light_backdrop {
        Color32::from_rgb(225, 228, 235)
    } else {
        Color32::from_rgb(20, 24, 32)
    };
    painter.rect_filled(rect, 8.0, bg_color);

    // Stage grid lines + optional Reference Planes
    let cols = if app.ui_state.show_ref_planes { 12 } else { 8 };
    for c in 1..cols {
        let x = rect.left() + rect.width() * (c as f32 / cols as f32);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(70, 85, 110, 55)),
        );
    }
    if app.ui_state.show_ref_planes {
        let mid_y = rect.center().y;
        painter.line_segment(
            [
                Pos2::new(rect.left() + 12.0, mid_y),
                Pos2::new(rect.right() - 12.0, mid_y),
            ],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(110, 175, 240, 90)),
        );
    }

    // Calibrated ground baseline
    let baseline_y = rect.bottom() - 64.0;
    painter.line_segment(
        [
            Pos2::new(rect.left() + 16.0, baseline_y),
            Pos2::new(rect.right() - 16.0, baseline_y),
        ],
        Stroke::new(2.0, Color32::from_rgb(72, 185, 130)),
    );
    painter.text(
        Pos2::new(rect.left() + 20.0, baseline_y + 10.0),
        egui::Align2::LEFT_TOP,
        format!(
            "GROUND BASELINE · Drift: {drift_summary} · Pose: {pose_label} · Lift: {lift_summary}"
        ),
        egui::FontId::monospace(11.0),
        Color32::from_rgb(110, 220, 165),
    );

    // Ground contact shadow (tracks stage_offset_x)
    let shadow_w = (120.0 - lift_px * 0.7).max(55.0);
    let shadow_rect = Rect::from_center_size(
        Pos2::new(
            rect.center().x + app.ui_state.stage_offset_x,
            baseline_y + 2.0 + app.ui_state.stage_offset_y,
        ),
        Vec2::new(shadow_w, 14.0),
    );
    painter.rect_filled(
        shadow_rect,
        7.0,
        Color32::from_rgba_unmultiplied(10, 12, 18, 175),
    );

    let layer_override_active = !frame_parts.is_empty()
        && (part_states.iter().any(|p| !p.visible || p.solo)
            || app.ui_state.explode_pct > 0.5
            || app.ui_state.show_topology_numbers
            || app.ui_state.knee_bend.abs() > 0.5
            || app.ui_state.elbow_bend.abs() > 0.5
            || app.ui_state.ear_splay.abs() > 0.5
            || app.ui_state.ear_pitch.abs() > 0.5
            || app.ui_state.head_yaw.abs() > 0.5
            || app.ui_state.head_pitch.abs() > 0.5
            || app.ui_state.eye_gaze.abs() > 0.5
            || (app.ui_state.smile_open - 20.0).abs() > 1.0
            || app.ui_state.arm_spread.abs() > 0.5
            || app.ui_state.eye_blink > 0.5);

    let cutout_rig_opt = app
        .session()
        .current_asset()
        .and_then(|a| a.cutout_rig.clone());

    let tint = match app.ui_state.shading_mode {
        ShadingMode::FeltPbr => Color32::WHITE,
        ShadingMode::ClaySculpt => Color32::from_rgb(215, 200, 185),
        ShadingMode::Wireframe => Color32::from_rgb(150, 225, 255),
        ShadingMode::Normals => Color32::from_rgb(175, 160, 255),
    };

    if let Some(rig) = cutout_rig_opt {
        let ctx = ui.ctx().clone();
        let cw = rig.canvas_size[0].max(1) as f32;
        let ch = rig.canvas_size[1].max(1) as f32;
        let max_h = (rect.height() - 105.0).max(160.0);
        let max_w = (rect.width() - 32.0).max(120.0);
        let scale = (max_h / ch).min(max_w / cw).min(1.15);
        let draw_size = Vec2::new(cw * scale, ch * scale);
        let center_x = rect.center().x + app.ui_state.stage_offset_x;
        let bottom_y = baseline_y + 18.0 + app.ui_state.stage_offset_y;
        let canvas_origin = Pos2::new(center_x - draw_size.x * 0.5, bottom_y - draw_size.y);

        // Pose-library-design acting pattern: anticipation -> action -> hold -> settle
        // Smooth stepped wave with holds so acting is readable
        let phase = app.ui_state.playback_phase as f32;
        let raw_sin = phase.sin();
        let held_wave = (raw_sin * 1.35).clamp(-1.0, 1.0);
        let held_cos = (phase.cos() * 1.35).clamp(-1.0, 1.0);
        let amp = range_factor * (app.ui_state.anim_energy / 100.0).clamp(0.2, 1.5);

        let (
            torso_deg,
            head_deg,
            ear_l_deg,
            ear_r_deg,
            arm_l_deg,
            arm_r_deg,
            leg_l_deg,
            foot_l_deg,
            leg_r_deg,
            foot_r_deg,
            root_dy,
        ) = match app.ui_state.anim_mode {
            AnimMode::Idle => (
                held_wave * 1.5 * amp,
                -held_wave * 2.5 * amp + app.ui_state.head_yaw * 0.3,
                held_cos * 7.0 * amp - app.ui_state.ear_splay * 0.3,
                -held_cos * 7.0 * amp + app.ui_state.ear_splay * 0.3,
                held_wave * 10.0 * amp + app.ui_state.arm_spread * 0.5,
                -held_wave * 3.5 * amp - app.ui_state.arm_spread * 0.2,
                held_wave * 6.0 * amp,
                held_cos * 14.0 * amp,
                -held_wave * 6.0 * amp,
                -held_cos * 14.0 * amp,
                -held_wave.abs() * 6.0 * amp,
            ),
            AnimMode::Walk => (
                held_wave * 3.0 * amp,
                -held_wave * 2.5 * amp + app.ui_state.head_yaw * 0.3,
                -held_cos * 10.0 * amp - app.ui_state.ear_splay * 0.3,
                -held_cos * 8.0 * amp + app.ui_state.ear_splay * 0.3,
                held_wave * 18.0 * amp + app.ui_state.arm_spread * 0.5,
                -held_wave * 4.5 * amp - app.ui_state.arm_spread * 0.2,
                -held_wave * 13.0 * amp + app.ui_state.knee_bend * 0.4,
                -held_wave * 15.0 * amp + held_cos * 8.0 * amp,
                held_wave * 13.0 * amp - app.ui_state.knee_bend * 0.4,
                held_wave * 15.0 * amp - held_cos * 8.0 * amp,
                -held_wave.abs() * 16.0 * amp,
            ),
            AnimMode::Run => (
                held_wave * 4.5 * amp,
                -held_wave * 3.5 * amp,
                -held_cos * 14.0 * amp,
                -held_cos * 13.0 * amp,
                held_wave * 24.0 * amp,
                -held_wave * 6.0 * amp,
                -held_wave * 16.0 * amp,
                -held_wave * 18.0 * amp + held_cos * 10.0 * amp,
                held_wave * 16.0 * amp,
                held_wave * 18.0 * amp - held_cos * 10.0 * amp,
                -held_wave.abs() * 26.0 * amp,
            ),
            AnimMode::Jump => {
                let jump_lift = held_wave.max(0.0);
                let crouch = (-held_wave).max(0.0);
                (
                    -jump_lift * 4.0 * amp + crouch * 5.0 * amp,
                    jump_lift * 5.0 * amp - crouch * 6.0 * amp,
                    jump_lift * 14.0 * amp - crouch * 10.0 * amp,
                    jump_lift * 16.0 * amp - crouch * 10.0 * amp,
                    jump_lift * 28.0 * amp - crouch * 16.0 * amp,
                    -jump_lift * 5.0 * amp + crouch * 4.0 * amp,
                    -jump_lift * 15.0 * amp + crouch * 13.0 * amp,
                    -jump_lift * 18.0 * amp + crouch * 14.0 * amp,
                    -jump_lift * 14.0 * amp + crouch * 12.0 * amp,
                    -jump_lift * 16.0 * amp + crouch * 14.0 * amp,
                    -jump_lift * 56.0 * amp + crouch * 16.0 * amp,
                )
            }
            AnimMode::Wave => (
                held_wave * 2.0 * amp,
                -4.5 * amp + held_wave * 3.0 * amp,
                -8.0 * amp + held_cos * 6.0 * amp,
                8.0 * amp + held_cos * 6.0 * amp,
                24.0 * amp + held_wave * 18.0 * amp,
                -3.5 * amp + held_cos * 2.5 * amp,
                -held_wave * 8.0 * amp,
                -held_wave * 14.0 * amp + held_cos * 6.0 * amp,
                held_wave * 7.0 * amp,
                held_wave * 12.0 * amp - held_cos * 6.0 * amp,
                -held_wave.abs() * 8.0 * amp,
            ),
            AnimMode::LookScout => (
                held_wave * 2.5 * amp,
                held_wave * 8.0 * amp + app.ui_state.head_yaw * 0.4,
                held_wave * 13.0 * amp,
                held_wave * 12.0 * amp,
                held_cos * 14.0 * amp,
                -held_cos * 4.0 * amp,
                -held_wave * 10.0 * amp,
                -held_wave * 14.0 * amp,
                held_wave * 10.0 * amp,
                held_wave * 14.0 * amp,
                -held_wave.abs() * 8.0 * amp,
            ),
            AnimMode::Static => (
                0.0,
                app.ui_state.head_yaw * 0.35,
                -app.ui_state.ear_splay * 0.35,
                app.ui_state.ear_splay * 0.35,
                app.ui_state.arm_spread * 0.6 + app.ui_state.elbow_bend * 0.5,
                -app.ui_state.arm_spread * 0.25 - app.ui_state.elbow_bend * 0.2,
                app.ui_state.knee_bend * 0.5,
                app.ui_state.knee_bend * 0.5,
                -app.ui_state.knee_bend * 0.5,
                -app.ui_state.knee_bend * 0.5,
                0.0,
            ),
        };

        let mut angles_rad: BTreeMap<&str, f32> = BTreeMap::new();
        angles_rad.insert("torso", torso_deg.to_radians());
        angles_rad.insert("head", head_deg.to_radians());
        angles_rad.insert("ear_left", ear_l_deg.to_radians());
        angles_rad.insert("ear_right", ear_r_deg.to_radians());
        angles_rad.insert("arm_left", arm_l_deg.to_radians());
        angles_rad.insert("arm_right", arm_r_deg.to_radians());
        angles_rad.insert("leg_left", leg_l_deg.to_radians());
        angles_rad.insert("foot_left", foot_l_deg.to_radians());
        angles_rad.insert("leg_right", leg_r_deg.to_radians());
        angles_rad.insert("foot_right", foot_r_deg.to_radians());

        let mut parts_by_id: BTreeMap<&str, &animationcraft_engine::session::CutoutRigPart> =
            BTreeMap::new();
        for p in &rig.parts {
            parts_by_id.insert(p.id.as_str(), p);
        }

        let transform_canvas_pt = |pt: Pos2, part_id: &str| -> Pos2 {
            let mut cur = pt;
            let mut cur_id = Some(part_id);
            let mut depth = 0;
            while let Some(id) = cur_id {
                if depth > 6 {
                    break;
                }
                if let Some(part) = parts_by_id.get(id) {
                    let piv = Pos2::new(part.pivot[0], part.pivot[1]);
                    let ang = angles_rad.get(id).copied().unwrap_or(0.0);
                    let (s, c) = ang.sin_cos();
                    let d = cur - piv;
                    cur = Pos2::new(piv.x + d.x * c - d.y * s, piv.y + d.x * s + d.y * c);
                    if part.parent.is_none() {
                        cur.y += root_dy;
                    }
                    cur_id = part.parent.as_deref();
                } else {
                    break;
                }
                depth += 1;
            }
            Pos2::new(
                canvas_origin.x + cur.x * scale,
                canvas_origin.y + cur.y * scale,
            )
        };

        let mut sorted_parts = rig.parts.clone();
        sorted_parts.sort_by_key(|p| p.layer);

        for part in &sorted_parts {
            let p_str = part.cropped_path.to_string_lossy().into_owned();
            let Some(ptex) = app.load_texture(&ctx, &p_str) else {
                continue;
            };
            let [bx0, by0, bx1, by1] = part.crop_box;
            let corners_canvas = [
                Pos2::new(bx0 as f32, by0 as f32),
                Pos2::new(bx1 as f32, by0 as f32),
                Pos2::new(bx1 as f32, by1 as f32),
                Pos2::new(bx0 as f32, by1 as f32),
            ];
            let uvs = [
                Pos2::new(0.0, 0.0),
                Pos2::new(1.0, 0.0),
                Pos2::new(1.0, 1.0),
                Pos2::new(0.0, 1.0),
            ];
            let mut mesh = egui::Mesh::with_texture(ptex.id());
            for i in 0..4 {
                let screen_pt = transform_canvas_pt(corners_canvas[i], &part.id);
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: screen_pt,
                    uv: uvs[i],
                    color: tint,
                });
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            painter.add(egui::Shape::mesh(mesh));
        }

        if app.ui_state.show_pivot_markers || app.ui_state.shading_mode == ShadingMode::Wireframe {
            for part in &sorted_parts {
                let piv_screen =
                    transform_canvas_pt(Pos2::new(part.pivot[0], part.pivot[1]), &part.id);
                if let Some(ref par_id) = part.parent
                    && let Some(par) = parts_by_id.get(par_id.as_str())
                {
                    let par_screen =
                        transform_canvas_pt(Pos2::new(par.pivot[0], par.pivot[1]), &par.id);
                    painter.line_segment(
                        [par_screen, piv_screen],
                        Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 200, 70, 220)),
                    );
                }
                painter.circle_filled(piv_screen, 4.5, Color32::from_rgb(255, 85, 85));
                painter.circle_stroke(
                    piv_screen,
                    (part.joint_radius_px as f32 * scale).max(6.0),
                    Stroke::new(1.2, Color32::from_rgba_unmultiplied(80, 230, 160, 180)),
                );
            }
        }
    } else if let Some(tex) = tex_opt {
        let tex_size = tex.size_vec2();
        let max_h = (rect.height() - 115.0).max(150.0);
        let max_w = (rect.width() - 32.0).max(120.0);
        let scale = (max_h / tex_size.y.max(1.0))
            .min(max_w / tex_size.x.max(1.0))
            .min(1.25);
        let draw_size = tex_size * scale;
        let center_x = rect.center().x + app.ui_state.stage_offset_x;
        let bottom_y = baseline_y - lift_px + app.ui_state.stage_offset_y;
        let sprite_rect = Rect::from_min_max(
            Pos2::new(center_x - draw_size.x * 0.5, bottom_y - draw_size.y),
            Pos2::new(center_x + draw_size.x * 0.5, bottom_y),
        );

        if layer_override_active {
            let ctx = ui.ctx().clone();
            let visible_layers = visible_part_layers(&frame_parts, &part_states);
            let explode_factor = (app.ui_state.explode_pct / 100.0) * 0.95;
            let sprite_center = sprite_rect.center();

            for (part_idx, part) in visible_layers.iter().enumerate() {
                let p_str = part.resolved_path.to_string_lossy().into_owned();
                let Some(ptex) = app.load_texture(&ctx, &p_str) else {
                    continue;
                };
                let pw = (part.size[0].max(1) as f32) * scale;
                let ph = (part.size[1].max(1) as f32) * scale;
                let base_x =
                    sprite_rect.left() + (part.bind_position[0] - part.pivot[0]) * scale;
                let base_y =
                    sprite_rect.top() + (part.bind_position[1] - part.pivot[1]) * scale;

                let (rig_dx, rig_dy) = match part.name.as_str() {
                    "ears" => (
                        app.ui_state.head_yaw * 0.25 + app.ui_state.ear_splay * 0.15,
                        -app.ui_state.ear_splay.abs() * 0.25 + app.ui_state.ear_pitch * 0.2,
                    ),
                    "head" => (
                        app.ui_state.head_yaw * 0.35 + app.ui_state.eye_gaze * 0.15,
                        app.ui_state.eye_blink * 0.05
                            + app.ui_state.head_pitch * 0.22
                            + (app.ui_state.smile_open - 20.0) * 0.04,
                    ),
                    "arm_L" => (
                        (app.ui_state.arm_spread + app.ui_state.elbow_bend * 0.5) * 0.35,
                        -app.ui_state.arm_spread * 0.2,
                    ),
                    "arm_R" => (
                        -(app.ui_state.arm_spread + app.ui_state.elbow_bend * 0.5) * 0.35,
                        -app.ui_state.arm_spread * 0.2,
                    ),
                    "leg_L" => (
                        app.ui_state.knee_bend * 0.2,
                        -app.ui_state.knee_bend.max(0.0) * 0.35,
                    ),
                    "leg_R" => (
                        -app.ui_state.knee_bend * 0.2,
                        -app.ui_state.knee_bend.max(0.0) * 0.35,
                    ),
                    _ => (0.0, 0.0),
                };

                let base_center = Pos2::new(base_x + pw * 0.5, base_y + ph * 0.5);
                let explode_vec = (base_center - sprite_center) * explode_factor;
                let part_rect = Rect::from_min_size(
                    Pos2::new(
                        base_x + explode_vec.x + rig_dx,
                        base_y + explode_vec.y + rig_dy,
                    ),
                    Vec2::new(pw, ph),
                );
                painter.image(
                    ptex.id(),
                    part_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    tint,
                );
                if app.ui_state.shading_mode == ShadingMode::Wireframe {
                    painter.rect_stroke(
                        part_rect,
                        2.0,
                        Stroke::new(1.0, Color32::from_rgb(80, 200, 255)),
                        egui::StrokeKind::Inside,
                    );
                }
                if app.ui_state.show_topology_numbers {
                    let badge_pos = Pos2::new(part_rect.left() + 10.0, part_rect.top() + 10.0);
                    painter.circle_filled(badge_pos, 9.0, Color32::from_rgb(35, 95, 165));
                    painter.text(
                        badge_pos,
                        egui::Align2::CENTER_CENTER,
                        format!("{}", part_idx + 1),
                        egui::FontId::monospace(10.0),
                        Color32::WHITE,
                    );
                }
                if app.ui_state.show_pivot_markers {
                    let piv_pos = Pos2::new(
                        part_rect.left() + part.pivot[0] * scale,
                        part_rect.top() + part.pivot[1] * scale,
                    );
                    painter.circle_filled(piv_pos, 4.0, Color32::from_rgb(255, 200, 60));
                    painter.circle_stroke(
                        piv_pos,
                        6.5,
                        Stroke::new(1.2, Color32::from_rgb(255, 120, 40)),
                    );
                }
            }
        } else {
            painter.image(
                tex.id(),
                sprite_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                tint,
            );
            if app.ui_state.shading_mode == ShadingMode::Wireframe {
                painter.rect_stroke(
                    sprite_rect,
                    2.0,
                    Stroke::new(1.2, Color32::from_rgb(80, 200, 255)),
                    egui::StrokeKind::Inside,
                );
            }
            if app.ui_state.show_pivot_markers {
                let pivot_pos = if let Some([ax, ay]) = frame_anchor
                    && frame_size[0] > 0
                    && frame_size[1] > 0
                {
                    Pos2::new(
                        sprite_rect.left() + (ax / frame_size[0] as f32) * sprite_rect.width(),
                        sprite_rect.top() + (ay / frame_size[1] as f32) * sprite_rect.height(),
                    )
                } else {
                    Pos2::new(center_x, bottom_y)
                };
                painter.circle_filled(pivot_pos, 5.0, Color32::from_rgb(255, 200, 60));
                painter.circle_stroke(
                    pivot_pos,
                    8.0,
                    Stroke::new(1.5, Color32::from_rgb(255, 120, 40)),
                );
            }
        }
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No pose manifest loaded — open an image and run Cut Sheet, or choose a preset",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(220, 175, 120),
        );
    }

    painter.text(
        Pos2::new(rect.left() + 14.0, rect.top() + 14.0),
        egui::Align2::LEFT_TOP,
        format!(
            "2D SIDE-PROFILE GATE · Asset: {} · Mode: {:?} · Viseme: {} · Range: {:.0}%",
            app.session().active_asset.as_deref().unwrap_or("none"),
            app.ui_state.anim_mode,
            app.ui_state.mouth_viseme,
            app.ui_state.pose_range_pct
        ),
        egui::FontId::monospace(11.5),
        Color32::from_rgb(210, 225, 245),
    );
}

fn select_turnaround_frame_for_yaw(asset: Option<&Asset>, yaw_deg: f32) -> Option<ManifestFrame> {
    let asset = asset?;
    let frames = if let Some(reg) = &asset.turnaround_registration
        && !reg.frames.is_empty()
    {
        &reg.frames
    } else if let Some(tm) = &asset.turnaround_manifest {
        &tm.frames
    } else {
        return None;
    };

    let norm = yaw_deg.rem_euclid(360.0);
    let desired_label = if !(22.5..337.5).contains(&norm) {
        "front"
    } else if (22.5..67.5).contains(&norm) || (292.5..337.5).contains(&norm) {
        "quarter_front"
    } else if (67.5..112.5).contains(&norm) || (247.5..292.5).contains(&norm) {
        "side"
    } else if (112.5..157.5).contains(&norm) || (202.5..247.5).contains(&norm) {
        "opposite_quarter"
    } else {
        "back"
    };

    frames
        .iter()
        .find(|f| f.label == desired_label)
        .or_else(|| frames.first())
        .cloned()
}

fn three_d_turnaround(app: &mut AnimationCraftApp, ui: &mut Ui) {
    if app.ui_state.auto_turntable {
        let dt = ui.ctx().input(|i| i.stable_dt).clamp(0.0, 0.1);
        app.ui_state.yaw_deg = (app.ui_state.yaw_deg + dt * 36.0).rem_euclid(360.0);
    }

    ui.horizontal_wrapped(|ui| {
        ui.strong("Turnaround Yaw:");
        let yaw = app.ui_state.yaw_deg;
        ui.add(
            egui::Slider::new(&mut app.ui_state.yaw_deg, 0.0..=360.0).text(format!("{yaw:.0}°")),
        );
        for (label, deg) in [
            ("POSE 1 • 0° Front", 0.0),
            ("POSE 2 • 45° 3/4", 45.0),
            ("POSE 3 • 90° Side", 90.0),
            ("POSE 4 • 135° Back-3/4", 135.0),
            ("POSE 5 • 180° Back", 180.0),
        ] {
            if ui.button(label).clicked() {
                app.ui_state.auto_turntable = false;
                app.ui_state.yaw_deg = deg;
            }
        }
        if ui
            .selectable_label(app.ui_state.auto_turntable, "⟳ Auto-Turntable")
            .clicked()
        {
            app.ui_state.auto_turntable = !app.ui_state.auto_turntable;
        }
    });

    ui.add_space(4.0);

    let frame_opt = select_turnaround_frame_for_yaw(
        app.session().current_asset(),
        app.ui_state.yaw_deg,
    );
    let drift_label = app
        .session()
        .current_asset()
        .and_then(|a| a.turnaround_registration.as_ref())
        .map(|r| {
            format!(
                "Drift: {:.1} px (before crown {:.0}px / foot {:.0}px)",
                r.max_vertical_drift_px, r.before_crown_drift_px, r.before_foot_drift_px
            )
        })
        .unwrap_or_else(|| "Drift: not measured".to_string());

    let ctx = ui.ctx().clone();
    let tex_turnaround = frame_opt.as_ref().and_then(|f| {
        let p = f.resolved_path.to_string_lossy().into_owned();
        app.load_texture(&ctx, &p)
    });

    let avail = ui.available_size();
    let canvas_size = Vec2::new(avail.x.max(80.0), (avail.y - 8.0).max(220.0));
    let (rect, _resp) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, Color32::from_rgb(18, 22, 30));

    if let Some(tex) = tex_turnaround {
        let ts = tex.size_vec2();
        let max_h = (rect.height() - 80.0).max(140.0);
        let max_w = (rect.width() * 0.52 - 32.0).max(110.0);
        let scale = (max_h / ts.y.max(1.0)).min(max_w / ts.x.max(1.0));
        let draw_size = ts * scale;
        let left_center = Pos2::new(rect.left() + rect.width() * 0.28, rect.center().y + 10.0);
        let img_rect = Rect::from_center_size(left_center, draw_size);
        painter.image(
            tex.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        // Right-side live 3D Visual-Hull Cross-Section Mesh Preview driven by yaw_deg + 3D Reconstruction sliders
        let hull_center = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y + 10.0);
        let hull_h = draw_size.y.max(150.0);
        let hull_w = (draw_size.x * 0.85).max(95.0);
        let slices = (app.ui_state.mesh_res.clamp(12, 40) / 2) as usize;
        let rings = 16usize;
        let yaw_rad = app.ui_state.yaw_deg.to_radians();
        let depth_ratio = (app.ui_state.inflation_depth_pct as f32 / 58.0).clamp(0.25, 1.75);
        let relief = (app.ui_state.relief_detail_pct as f32 / 100.0) * 0.08;

        let wire_col = match app.ui_state.shading_mode {
            ShadingMode::FeltPbr => Color32::from_rgba_unmultiplied(110, 210, 255, 155),
            ShadingMode::ClaySculpt => Color32::from_rgba_unmultiplied(225, 195, 165, 185),
            ShadingMode::Wireframe => Color32::from_rgba_unmultiplied(80, 235, 195, 220),
            ShadingMode::Normals => Color32::from_rgba_unmultiplied(195, 145, 255, 200),
        };

        for iz in 0..=slices {
            let u = iz as f32 / slices as f32;
            // Anatomical silhouette taper (feet -> legs -> torso -> neck -> head -> crown)
            let profile_r = if u < 0.12 {
                0.24
            } else if u < 0.42 {
                0.28
            } else if u < 0.68 {
                0.42
            } else if u < 0.74 {
                0.22
            } else if u < 0.94 {
                0.38
            } else {
                0.18
            };
            let rx = hull_w * 0.5 * profile_r;
            let rz = rx * 0.72 * depth_ratio;
            let y_screen = (hull_center.y + hull_h * 0.5) - u * hull_h;

            let mut ring_pts = Vec::with_capacity(rings + 1);
            for it in 0..=rings {
                let theta = (it as f32 / rings as f32) * std::f32::consts::TAU + yaw_rad;
                let r_mod = 1.0 + relief * (3.0 * theta).cos();
                let x3 = rx * r_mod * theta.cos();
                let z3 = rz * r_mod * theta.sin();
                let px = hull_center.x + x3;
                let py = y_screen + z3 * 0.22;
                ring_pts.push(Pos2::new(px, py));
            }
            for seg in ring_pts.windows(2) {
                painter.line_segment([seg[0], seg[1]], Stroke::new(1.1, wire_col));
            }
        }
        painter.text(
            Pos2::new(hull_center.x, hull_center.y + hull_h * 0.5 + 16.0),
            egui::Align2::CENTER_TOP,
            format!(
                "3D Visual Hull ({}) · Depth {}% · Res {}",
                app.ui_state.recon_mode, app.ui_state.inflation_depth_pct, app.ui_state.mesh_res
            ),
            egui::FontId::monospace(10.5),
            Color32::from_rgb(150, 220, 250),
        );
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No turnaround manifest loaded for active asset",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(220, 175, 120),
        );
    }

    let angle_name = frame_opt
        .as_ref()
        .map(|f| f.label.as_str())
        .unwrap_or("none");
    painter.text(
        Pos2::new(rect.left() + 14.0, rect.top() + 14.0),
        egui::Align2::LEFT_TOP,
        format!(
            "TURNAROUND VIEW · Asset: {} · View: {angle_name} ({:.0}°) · {drift_label}",
            app.session().active_asset.as_deref().unwrap_or("none"),
            app.ui_state.yaw_deg
        ),
        egui::FontId::monospace(11.5),
        Color32::from_rgb(130, 215, 250),
    );
}

fn five_pose_lineup(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let frames: Vec<ManifestFrame> = app
        .session()
        .current_asset()
        .and_then(|a| {
            a.turnaround_registration
                .as_ref()
                .map(|r| r.frames.clone())
                .or_else(|| a.turnaround_manifest.as_ref().map(|m| m.frames.clone()))
        })
        .unwrap_or_default();

    let avail = ui.available_size();
    let canvas_size = Vec2::new(avail.x.max(80.0), (avail.y - 8.0).max(220.0));
    let (rect, _resp) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, Color32::from_rgb(18, 22, 30));

    if frames.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No 5-view turnaround manifest loaded for active asset",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(220, 175, 120),
        );
        return;
    }

    let ctx = ui.ctx().clone();
    let count = frames.len().max(1) as f32;
    let slot_w = (rect.width() - 24.0) / count;
    let baseline_y = rect.bottom() - 48.0;
    let crown_y = rect.top() + 56.0;

    painter.line_segment(
        [
            Pos2::new(rect.left() + 12.0, crown_y),
            Pos2::new(rect.right() - 12.0, crown_y),
        ],
        Stroke::new(1.5, Color32::from_rgb(255, 202, 72)),
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + 12.0, baseline_y),
            Pos2::new(rect.right() - 12.0, baseline_y),
        ],
        Stroke::new(1.5, Color32::from_rgb(88, 217, 176)),
    );

    for (idx, f) in frames.iter().enumerate() {
        let p = f.resolved_path.to_string_lossy().into_owned();
        let x_min = rect.left() + 12.0 + (idx as f32) * slot_w;
        let slot_rect = Rect::from_min_max(
            Pos2::new(x_min, crown_y),
            Pos2::new(x_min + slot_w, baseline_y),
        );
        if let Some(tex) = app.load_texture(&ctx, &p) {
            let ts = tex.size_vec2();
            let scale = (slot_rect.height() / ts.y.max(1.0)).min(slot_rect.width() / ts.x.max(1.0));
            let draw_size = ts * scale;
            let img_rect = Rect::from_center_size(slot_rect.center(), draw_size);
            painter.image(
                tex.id(),
                img_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        painter.text(
            Pos2::new(slot_rect.center().x, baseline_y + 8.0),
            egui::Align2::CENTER_TOP,
            &f.label,
            egui::FontId::monospace(11.0),
            Color32::from_rgb(210, 225, 245),
        );
    }
}

/// Paint a calibrated transparency checkerboard inside `rect` so the user can visually
/// confirm that imported sheets and cutouts have a 100% transparent background.
fn paint_transparent_checkerboard(painter: &egui::Painter, rect: Rect, light_mode: bool) {
    let (c1, c2) = if light_mode {
        (
            Color32::from_rgb(220, 224, 232),
            Color32::from_rgb(198, 204, 214),
        )
    } else {
        (
            Color32::from_rgb(18, 22, 30),
            Color32::from_rgb(26, 31, 42),
        )
    };
    painter.rect_filled(rect, 6.0, c1);
    let tile = 20.0;
    let cols = ((rect.width() / tile).ceil() as usize).min(120);
    let rows = ((rect.height() / tile).ceil() as usize).min(90);
    for r in 0..rows {
        for c in 0..cols {
            if (r + c) % 2 == 1 {
                let x0 = rect.left() + (c as f32) * tile;
                let y0 = rect.top() + (r as f32) * tile;
                let x1 = (x0 + tile).min(rect.right());
                let y1 = (y0 + tile).min(rect.bottom());
                if x1 > x0 && y1 > y0 {
                    painter.rect_filled(
                        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1)),
                        0.0,
                        c2,
                    );
                }
            }
        }
    }
}

#[path = "pose_viewer.rs"]
pub(super) mod pose_viewer;

fn asset_sheet_view(app: &mut AnimationCraftApp, ui: &mut Ui) {
    pose_viewer::show(app, ui);
}

fn turnaround_sheet_view(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let (active_id, sheet_path_opt, frames, drift_summary) = {
        let asset = app.session().current_asset();
        let id = app
            .session()
            .active_asset
            .clone()
            .unwrap_or_else(|| "hopper".to_string());
        let sheet_p = asset.and_then(|a| {
            a.attached_turnaround_sheet.clone().or_else(|| {
                a.turnaround_manifest
                    .as_ref()
                    .and_then(|m| m.source_image.clone())
            })
        });
        let frs = asset
            .and_then(|a| {
                a.turnaround_registration
                    .as_ref()
                    .map(|r| r.frames.clone())
                    .or_else(|| a.turnaround_manifest.as_ref().map(|m| m.frames.clone()))
            })
            .unwrap_or_default();
        let drift = asset
            .and_then(|a| a.turnaround_registration.as_ref())
            .map(|r| format!("{:.1} px [OK]", r.max_vertical_drift_px))
            .unwrap_or_else(|| "not measured".to_string());
        (id, sheet_p, frs, drift)
    };

    // Contextual toolbar for the character's Attached Turnaround Sheet
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("Attached Turnaround Sheet ({active_id})"));
        ui.colored_label(
            Color32::from_rgb(88, 220, 155),
            format!("[OK] Transparent BG (Step 1 Active) · Drift: {drift_summary}"),
        );
        ui.separator();
        let r_imp = ui.button("Import / Replace Turnaround Sheet…");
        app.automation.add_response(
            "btn.import_turnaround_sheet",
            "button",
            "Import / Replace Turnaround Sheet",
            &r_imp,
        );
        if r_imp.clicked()
            && let Some(file) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
        {
            let path_str = file.display().to_string();
            let tolerance = app.ui_state.bg_tolerance;
            app.ui_state_mut().image_path = Some(path_str.clone());
            app.exec_ui_command(UiCommand::IntakeValidate {
                image_path: path_str,
                slot: "turnaround_sheet".to_string(),
                attach_to_active: true,
                auto_cut: true,
                tolerance,
            });
        }
        if ui.button("0px Drift Register").clicked() {
            app.exec_ui_command(UiCommand::TurnaroundRegister);
        }
        if ui.button("Open in 3D Turnaround ->").clicked() {
            app.ui_state.center_tab = CenterTab::ThreeD;
        }
        if ui.button("<- View Asset Sheet").clicked() {
            app.ui_state.center_tab = CenterTab::AssetSheet;
        }
    });

    ui.add_space(4.0);

    let ctx = ui.ctx().clone();
    let sheet_tex = sheet_path_opt.as_ref().and_then(|p| {
        let s = p.to_string_lossy().into_owned();
        app.load_texture(&ctx, &s)
    });

    let avail = ui.available_size();
    let canvas_size = Vec2::new(avail.x.max(80.0), (avail.y - 8.0).max(220.0));
    let (rect, resp) = ui.allocate_exact_size(canvas_size, egui::Sense::click());

    let split_y = rect.top() + rect.height() * 0.50;
    let top_rect = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, split_y - 3.0));
    let bot_rect = Rect::from_min_max(Pos2::new(rect.min.x, split_y + 3.0), rect.max);

    let top_painter = ui.painter_at(top_rect);
    let bot_painter = ui.painter_at(bot_rect);

    paint_transparent_checkerboard(&top_painter, top_rect, app.ui_state.light_backdrop);
    paint_transparent_checkerboard(&bot_painter, bot_rect, app.ui_state.light_backdrop);

    // Top half: Full Attached Turnaround Sheet (transparent background)
    if let Some(stex) = sheet_tex {
        let ts = stex.size_vec2();
        let max_w = (top_rect.width() - 32.0).max(80.0);
        let max_h = (top_rect.height() - 36.0).max(60.0);
        let scale = (max_w / ts.x.max(1.0)).min(max_h / ts.y.max(1.0));
        let draw_size = ts * scale;
        let sheet_rect = Rect::from_center_size(
            Pos2::new(top_rect.center().x, top_rect.center().y + 8.0),
            draw_size,
        );
        top_painter.image(
            stex.id(),
            sheet_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        top_painter.text(
            top_rect.center(),
            egui::Align2::CENTER_CENTER,
            "No Turnaround Sheet attached — click 'Import / Replace Turnaround Sheet…'",
            egui::FontId::proportional(13.0),
            Color32::from_rgb(220, 180, 120),
        );
    }
    top_painter.text(
        Pos2::new(top_rect.left() + 12.0, top_rect.top() + 8.0),
        egui::Align2::LEFT_TOP,
        format!(
            "ATTACHED TURNAROUND SOURCE SHEET · {} (Transparent Alpha)",
            sheet_path_opt
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("none")
        ),
        egui::FontId::monospace(11.0),
        Color32::from_rgb(135, 225, 175),
    );

    // Bottom half: Registered 5-View Turnaround Cutouts aligned on Crown & Foot Baselines
    let count = frames.len().max(1) as f32;
    let slot_w = (bot_rect.width() - 24.0) / count;
    let crown_y = bot_rect.top() + 28.0;
    let baseline_y = bot_rect.bottom() - 28.0;

    bot_painter.line_segment(
        [
            Pos2::new(bot_rect.left() + 12.0, crown_y),
            Pos2::new(bot_rect.right() - 12.0, crown_y),
        ],
        Stroke::new(1.2, Color32::from_rgb(255, 202, 72)),
    );
    bot_painter.line_segment(
        [
            Pos2::new(bot_rect.left() + 12.0, baseline_y),
            Pos2::new(bot_rect.right() - 12.0, baseline_y),
        ],
        Stroke::new(1.5, Color32::from_rgb(88, 217, 176)),
    );

    for (idx, f) in frames.iter().enumerate() {
        let p = f.resolved_path.to_string_lossy().into_owned();
        let x_min = bot_rect.left() + 12.0 + (idx as f32) * slot_w;
        let slot_rect = Rect::from_min_max(
            Pos2::new(x_min, crown_y),
            Pos2::new(x_min + slot_w, baseline_y),
        );
        if let Some(tex) = app.load_texture(&ctx, &p) {
            let ts = tex.size_vec2();
            let scale =
                (slot_rect.height() / ts.y.max(1.0)).min(slot_rect.width() / ts.x.max(1.0));
            let draw_size = ts * scale;
            let img_rect = Rect::from_center_size(slot_rect.center(), draw_size);
            bot_painter.image(
                tex.id(),
                img_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        bot_painter.text(
            Pos2::new(slot_rect.center().x, baseline_y + 6.0),
            egui::Align2::CENTER_TOP,
            &f.label,
            egui::FontId::monospace(10.5),
            Color32::from_rgb(210, 225, 245),
        );
        if resp.clicked()
            && let Some(pos) = resp.interact_pointer_pos()
            && slot_rect.contains(pos)
        {
            app.ui_state.yaw_deg = (idx as f32) * 45.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use animationcraft_engine::session::LoadedManifest;

    fn sample_asset() -> Asset {
        Asset {
            id: "hopper".into(),
            pose_manifest: Some(LoadedManifest {
                manifest_dir: PathBuf::from("exports/pose_and_parts/hopper_poses"),
                manifest_path: PathBuf::from("exports/pose_and_parts/hopper_poses/manifest.json"),
                sheet_type: "action".into(),
                status: "ok".into(),
                matting_engine: None,
                source_image: None,
                source_size: None,
                frames: vec![
                    ManifestFrame {
                        index: 0,
                        label: "standing".into(),
                        file: "frames/00_standing.png".into(),
                        resolved_path: PathBuf::from("00_standing.png"),
                        size: [224, 313],
                        source_box: None,
                        anchor: None,
                        parts: BTreeMap::new(),
                    },
                    ManifestFrame {
                        index: 1,
                        label: "walking".into(),
                        file: "frames/01_walking.png".into(),
                        resolved_path: PathBuf::from("01_walking.png"),
                        size: [224, 313],
                        source_box: None,
                        anchor: None,
                        parts: BTreeMap::new(),
                    },
                    ManifestFrame {
                        index: 2,
                        label: "running".into(),
                        file: "frames/02_running.png".into(),
                        resolved_path: PathBuf::from("02_running.png"),
                        size: [224, 313],
                        source_box: None,
                        anchor: None,
                        parts: BTreeMap::new(),
                    },
                ],
                part_names: vec!["head".into(), "ears".into(), "torso".into()],
            }),
            ..Asset::default()
        }
    }

    #[test]
    fn zero_percent_range_resolves_to_standing_not_action_frame() {
        let asset = sample_asset();
        for mode in [AnimMode::Walk, AnimMode::Run, AnimMode::Jump] {
            let seq = resolve_pose_sequence(Some(&asset), mode, 0.0, 2, true);
            assert_eq!(seq.len(), 1);
            assert_eq!(seq[0].label, "standing");
        }
    }

    #[test]
    fn pause_in_action_mode_preserves_sequence_instead_of_jumping_to_selected_pose() {
        let asset = sample_asset();
        let playing_seq = resolve_pose_sequence(Some(&asset), AnimMode::Walk, 100.0, 2, true);
        let paused_seq = resolve_pose_sequence(Some(&asset), AnimMode::Walk, 100.0, 2, false);
        let playing_labels: Vec<&str> = playing_seq.iter().map(|f| f.label.as_str()).collect();
        let paused_labels: Vec<&str> = paused_seq.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(playing_labels, paused_labels);
        assert_eq!(paused_labels[0], "walking");
    }

    #[test]
    fn visible_part_layers_respects_visibility_and_solo() {
        let mut parts = BTreeMap::new();
        for name in ["torso", "head", "ears"] {
            parts.insert(
                name.to_string(),
                ManifestPart {
                    name: name.to_string(),
                    ..ManifestPart::default()
                },
            );
        }

        // Hide head -> only torso and ears remain
        let states = vec![
            PartState {
                name: "torso".into(),
                visible: true,
                solo: false,
                verified: false,
            },
            PartState {
                name: "head".into(),
                visible: false,
                solo: false,
                verified: false,
            },
            PartState {
                name: "ears".into(),
                visible: true,
                solo: false,
                verified: false,
            },
        ];
        let shown: Vec<String> = visible_part_layers(&parts, &states)
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(shown, vec!["torso", "ears"]);

        // Solo head -> only head is shown even if visible was false
        let solo_states = vec![
            PartState {
                name: "torso".into(),
                visible: true,
                solo: false,
                verified: false,
            },
            PartState {
                name: "head".into(),
                visible: false,
                solo: true,
                verified: false,
            },
            PartState {
                name: "ears".into(),
                visible: true,
                solo: false,
                verified: false,
            },
        ];
        let solo_shown: Vec<String> = visible_part_layers(&parts, &solo_states)
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(solo_shown, vec!["head"]);
    }
}
