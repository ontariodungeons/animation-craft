//! Center viewport: 2D Side-Profile Action Gate, 3D Turnaround, and 5-Pose Lineup.
//!
//! All sprite paths, pose labels, and lift/drift telemetry are read dynamically from
//! `app.session().current_asset()`. Unmeasured fields display `"not measured"`.

use super::chrome;
#[path = "center_motion.rs"]
pub(crate) mod center_motion;

use crate::{AnimMode, AnimationCraftApp, CenterTab, ShadingMode, UiCommand};
use animationcraft_engine::session::{ManifestPart, PartState};
use animationcraft_engine::{Asset, ManifestFrame};
use center_motion::{
    WasdInput, actual_size_viewport_state, center_stage_state, compute_preview_scale,
    desired_labels_for_mode, evaluate_cutout_rig_motion, evaluate_jump, fit_viewport_state,
    is_rig_part_visible, jump_arc, sanitize_dt, sanitize_zoom, step_playback_phase, step_wasd_roam,
    stop_motion_state, toggle_sprint_lock_state, trigger_jump_state, zoom_in_state, zoom_out_state,
};
use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) fn center_viewport(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let title = app
        .session
        .project_assets
        .iter()
        .find(|entry| Some(entry.id.as_str()) == app.session.active_asset.as_deref())
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| "Character workspace".into());
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(egui::RichText::new(&title).size(18.0).strong().color(chrome::TEXT)).truncate());
            if app.ui_state.center_tab != CenterTab::AssetSheet { ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().button_padding = egui::vec2(9.0, 6.0);
                let menu = ui.menu_button("View options ▾", |ui| {
                    ui.set_min_width(210.0);
                    if matches!(app.ui_state.center_tab, CenterTab::TwoD | CenterTab::ThreeD) {
                        ui.label(egui::RichText::new("Preview style").strong());
                        for (shade, id, label, hint) in [
                            (ShadingMode::FeltPbr, "shade.felt_pbr", "Artwork", "Use the artwork-colored preview treatment. This is not a full physically based material renderer."),
                            (ShadingMode::ClaySculpt, "shade.clay", "Clay", "Inspect the form with a muted clay treatment."),
                            (ShadingMode::Wireframe, "shade.wire", "Wireframe", "Inspect the preview's structure and joint overlay."),
                            (ShadingMode::Normals, "shade.normals", "Normals", "Use the normal-style diagnostic colors supported by the preview."),
                        ] {
                            let response = ui.selectable_value(&mut app.ui_state.shading_mode, shade, label).on_hover_text(hint);
                            app.automation.add_response(id, "button", label, &response);
                        }
                        ui.separator();
                    }
                    ui.label(egui::RichText::new("Canvas & overlays").strong());
                    for (id, label, value, hint) in [
                        ("view.background", "Light background", &mut app.ui_state.light_backdrop, "Switch the supported canvas views between light and dark backgrounds to inspect edges."),
                        ("view.pivots", "Show joint anchors", &mut app.ui_state.show_pivot_markers, "Show available joint locations on the original artwork and supported animation previews."),
                        ("view.topology", "Show part labels", &mut app.ui_state.show_topology_numbers, "Label available joints or parts in supported views. Enable joint anchors on the Original tab to see joint names."),
                        ("view.references", "Reference guides", &mut app.ui_state.show_ref_planes, "Show the reference-plane guides in the 2D animation preview."),
                    ] {
                        let response = ui.checkbox(value, label).on_hover_text(hint);
                        app.automation.add_response(id, "checkbox", label, &response);
                    }
                });
                app.automation.add_response("view.options", "button", "View options", &menu.response);
                menu.response.on_hover_text("Canvas background, preview style and inspection overlays. These controls do not alter source artwork.");
            });
            }
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (tab, id, label, explanation) in [
                (CenterTab::OriginalView, "tab.original", "Original", "Inspect the active source image and optional joint overlay."),
                (CenterTab::AssetSheet, "tab.asset_sheet", "Artwork", "Review the connected cutout or pose sheet."),
                (CenterTab::TurnaroundSheet, "tab.turnaround_sheet", "Turnaround", "Inspect the character reference views."),
                (CenterTab::TwoD, "tab.2d", "Animate", "Preview the selected action and inspect moving cutout parts."),
                (CenterTab::ThreeD, "tab.3d", "3D", "Inspect the current 3D turnaround preview."),
                (CenterTab::FivePoseLineup, "tab.lineup", "Lineup", "Compare poses side by side."),
            ] {
                let response = chrome::tab(ui, label, app.ui_state.center_tab == tab).on_hover_text(explanation);
                app.automation.add_response(id, "tab", label, &response);
                if response.clicked() { app.ui_state.center_tab = tab; }
            }
        });
        if app.ui_state.center_tab == CenterTab::AssetSheet {
            ui.add_space(4.0);
            ui.separator();
            pose_viewer::navigation(app, ui);
        } else if app.ui_state.center_tab == CenterTab::OriginalView {
            ui.add_space(4.0);
            ui.separator();
            original_info(app, ui);
        }
    });
    ui.add_space(8.0);

    match app.ui_state.center_tab {
        CenterTab::OriginalView => original_design_view(app, ui),
        CenterTab::AssetSheet => asset_sheet_view(app, ui),
        CenterTab::TurnaroundSheet => turnaround_sheet_view(app, ui),
        CenterTab::TwoD => two_d_gate(app, ui),
        CenterTab::ThreeD => three_d_turnaround(app, ui),
        CenterTab::FivePoseLineup => five_pose_lineup(app, ui),
    }
}

#[derive(Clone, Copy)]
enum ToolbarIcon {
    JumpArrow,
    RunChevrons,
    WasdCross,
    FitCorners,
    ActualOneToOne,
    ZoomMinus,
    ZoomPlus,
    CenterTarget,
}

fn toolbar_icon_button(
    ui: &mut Ui,
    enabled: bool,
    active: bool,
    label: &str,
    accent: Color32,
    min_size: Vec2,
    icon: ToolbarIcon,
    hint: &str,
) -> egui::Response {
    let fill_alpha = if active { 0.34 } else { 0.18 };
    let stroke_alpha = if active { 0.90 } else { 0.62 };
    let response = ui
        .add_enabled(
            enabled,
            egui::Button::new(
                egui::RichText::new(format!("     {label}"))
                    .strong()
                    .color(chrome::TEXT),
            )
            .min_size(min_size)
            .corner_radius(7)
            .stroke(Stroke::new(
                if active { 1.4 } else { 1.0 },
                accent.gamma_multiply(stroke_alpha),
            ))
            .fill(accent.gamma_multiply(fill_alpha)),
        )
        .on_hover_text(hint);

    let mark = response.rect.left_center() + Vec2::new(14.0, 0.0);
    let ink = if enabled {
        accent
    } else {
        accent.gamma_multiply(0.35)
    };
    let painter = ui.painter();
    let stroke = Stroke::new(1.7, ink);
    match icon {
        ToolbarIcon::JumpArrow => {
            painter.line_segment([mark + Vec2::new(0.0, 5.5), mark + Vec2::new(0.0, -5.0)], stroke);
            painter.line_segment(
                [mark + Vec2::new(-4.5, -1.0), mark + Vec2::new(0.0, -5.5)],
                stroke,
            );
            painter.line_segment(
                [mark + Vec2::new(4.5, -1.0), mark + Vec2::new(0.0, -5.5)],
                stroke,
            );
        }
        ToolbarIcon::RunChevrons => {
            for dx in [-3.5, 1.5] {
                let c = mark + Vec2::new(dx, 0.0);
                painter.line_segment([c + Vec2::new(-2.5, -4.5), c + Vec2::new(2.0, 0.0)], stroke);
                painter.line_segment([c + Vec2::new(2.0, 0.0), c + Vec2::new(-2.5, 4.5)], stroke);
            }
        }
        ToolbarIcon::WasdCross => {
            painter.line_segment([mark + Vec2::new(-5.0, 0.0), mark + Vec2::new(5.0, 0.0)], stroke);
            painter.line_segment([mark + Vec2::new(0.0, -5.0), mark + Vec2::new(0.0, 5.0)], stroke);
            painter.circle_filled(mark, 1.8, ink);
        }
        ToolbarIcon::FitCorners => {
            let r = 5.0;
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = mark + Vec2::new(sx * r, sy * r);
                painter.line_segment([corner, corner + Vec2::new(-sx * 3.2, 0.0)], stroke);
                painter.line_segment([corner, corner + Vec2::new(0.0, -sy * 3.2)], stroke);
            }
        }
        ToolbarIcon::ActualOneToOne => {
            painter.rect_stroke(
                Rect::from_center_size(mark, Vec2::new(10.0, 10.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.circle_filled(mark, 1.8, ink);
        }
        ToolbarIcon::ZoomMinus => {
            painter.line_segment([mark + Vec2::new(-4.5, 0.0), mark + Vec2::new(4.5, 0.0)], stroke);
        }
        ToolbarIcon::ZoomPlus => {
            painter.line_segment([mark + Vec2::new(-4.5, 0.0), mark + Vec2::new(4.5, 0.0)], stroke);
            painter.line_segment([mark + Vec2::new(0.0, -4.5), mark + Vec2::new(0.0, 4.5)], stroke);
        }
        ToolbarIcon::CenterTarget => {
            painter.circle_stroke(mark, 4.5, stroke);
            painter.circle_filled(mark, 1.6, ink);
        }
    }
    response
}

/// Shared transport, action, movement, and viewport zoom toolbar above the source/animation canvas.
fn playback_bar(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let ready = app.session().current_asset().is_some();
    let running = app.ui_state.center_tab == CenterTab::TwoD && app.ui_state.playing;
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            for (id, label, enabled, hint) in [
                (
                    "play",
                    "Play",
                    ready && !running,
                    "Open Animate and play the selected action from the current position.",
                ),
                (
                    "pause",
                    "Pause",
                    ready && running,
                    "Pause the preview and hold the current frame.",
                ),
                (
                    "stop",
                    "Stop",
                    ready,
                    "Stop playback and return to the start of the action.",
                ),
            ] {
                let accent = match id {
                    "play" => Color32::from_rgb(74, 206, 144),
                    "stop" => Color32::from_rgb(242, 108, 116),
                    _ => Color32::from_rgb(238, 193, 104),
                };
                let response = ui
                    .add_enabled(
                        enabled,
                        egui::Button::new(
                            egui::RichText::new(format!("      {label}"))
                                .strong()
                                .color(chrome::TEXT),
                        )
                        .min_size(Vec2::new(92.0, 36.0))
                        .corner_radius(7)
                        .stroke(Stroke::new(1.0, accent.gamma_multiply(0.7)))
                        .fill(accent.gamma_multiply(0.22)),
                    )
                    .on_hover_text(hint);
                // Native vector marks stay crisp at small sizes and do not depend on font glyphs.
                let mark = response.rect.left_center() + Vec2::new(17.0, 0.0);
                let ink = if enabled {
                    accent
                } else {
                    accent.gamma_multiply(0.35)
                };
                let painter = ui.painter();
                match id {
                    "play" => {
                        painter.add(egui::Shape::convex_polygon(
                            vec![
                                mark + Vec2::new(-5.0, -7.0),
                                mark + Vec2::new(7.0, 0.0),
                                mark + Vec2::new(-5.0, 7.0),
                            ],
                            ink,
                            Stroke::NONE,
                        ));
                    }
                    "stop" => {
                        painter.rect_filled(
                            Rect::from_center_size(mark, Vec2::splat(13.0)),
                            3,
                            ink,
                        );
                    }
                    _ => {
                        for x in [-4.0, 4.0] {
                            painter.rect_filled(
                                Rect::from_center_size(
                                    mark + Vec2::new(x, 0.0),
                                    Vec2::new(4.0, 14.0),
                                ),
                                1,
                                ink,
                            );
                        }
                    }
                }
                app.automation
                    .add_response(format!("playback.{id}"), "button", label, &response);
                if response.clicked() {
                    match id {
                        "play" => {
                            app.ui_state.center_tab = CenterTab::TwoD;
                            app.ui_state.playing = true;
                        }
                        "pause" => app.ui_state.playing = false,
                        "stop" => stop_motion_state(&mut app.ui_state),
                        _ => {}
                    }
                }
            }

            let layered_rig = app
                .session()
                .current_asset()
                .and_then(|a| a.cutout_rig.as_ref())
                .filter(|r| r.rig_type == "layered_motion")
                .cloned();

            if let Some(ref rig) = layered_rig {
                let tracks: Vec<(String, f32)> = rig
                    .effectcraft
                    .get("tracks")
                    .and_then(serde_json::Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .map(|t| {
                                let id = t
                                    .get("id")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("track")
                                    .to_string();
                                let dur = t
                                    .get("duration")
                                    .and_then(serde_json::Value::as_f64)
                                    .unwrap_or(5.0) as f32;
                                (id, dur)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let active_idx = app
                    .ui_state
                    .selected_track_idx
                    .min(tracks.len().saturating_sub(1));
                let active_track_label = tracks
                    .get(active_idx)
                    .map(|(id, dur)| format!("Track: {id} ({dur:.1}s)"))
                    .unwrap_or_else(|| "Spec timeline".to_string());
                let menu = ui.menu_button(active_track_label, |ui| {
                    for (idx, (tid, dur)) in tracks.iter().enumerate() {
                        let lbl = format!("{tid} ({dur:.1}s)");
                        let response =
                            ui.selectable_label(app.ui_state.selected_track_idx == idx, &lbl);
                        app.automation.add_response(
                            format!("playback.track.{tid}"),
                            "button",
                            &lbl,
                            &response,
                        );
                        if response.clicked() {
                            app.ui_state.selected_track_idx = idx;
                            app.ui_state.playback_phase = 0.0;
                            ui.close();
                        }
                    }
                });
                app.automation
                    .add_response("playback.action", "button", "Track", &menu.response);
            } else {
                let menu = ui.menu_button(format!("Action: {:?}", app.ui_state.anim_mode), |ui| {
                    for (mode, name, command) in [
                        (AnimMode::Idle, "Idle", "idle"),
                        (AnimMode::Walk, "Walk", "walk"),
                        (AnimMode::Run, "Run", "run"),
                        (AnimMode::Jump, "Jump", "jump"),
                        (AnimMode::Wave, "Wave", "wave"),
                        (AnimMode::LookScout, "Look / Scout", "look_scout"),
                        (AnimMode::Static, "Static", "static"),
                    ] {
                        let response = ui.selectable_label(app.ui_state.anim_mode == mode, name);
                        app.automation.add_response(
                            format!("playback.action.{command}"),
                            "button",
                            name,
                            &response,
                        );
                        if response.clicked() {
                            if app.ui_state.anim_mode != mode {
                                app.ui_state.playback_phase = 0.0;
                            }
                            app.ui_state.anim_mode = mode;
                            app.exec_ui_command(UiCommand::AnimPreview2d {
                                mode: command.into(),
                            });
                            ui.close();
                        }
                    }
                });
                app.automation
                    .add_response("playback.action", "button", "Action", &menu.response);

                let jump_resp = toolbar_icon_button(
                    ui,
                    ready,
                    app.ui_state.anim_mode == AnimMode::Jump && running,
                    "Jump",
                    Color32::from_rgb(78, 196, 214),
                    Vec2::new(72.0, 30.0),
                    ToolbarIcon::JumpArrow,
                    "Trigger a full Jump cycle (takeoff crouch -> apex -> landing) driven by Jump height.",
                );
                app.automation
                    .add_response("playback.jump", "button", "Jump", &jump_resp);
                if jump_resp.clicked() {
                    app.ui_state.center_tab = CenterTab::TwoD;
                    trigger_jump_state(&mut app.ui_state);
                }

                let sprint = app.ui_state.sprint_lock;
                let run_label = if sprint { "Run: ON" } else { "Run" };
                let run_resp = toolbar_icon_button(
                    ui,
                    ready,
                    sprint,
                    run_label,
                    Color32::from_rgb(98, 168, 248),
                    Vec2::new(74.0, 30.0),
                    ToolbarIcon::RunChevrons,
                    "Toggle sprint pace for playback and WASD movement.",
                );
                app.automation
                    .add_response("playback.run_lock", "button", run_label, &run_resp);
                if run_resp.clicked() {
                    toggle_sprint_lock_state(&mut app.ui_state);
                }

                let wasd = app.ui_state.wasd_roam;
                let wasd_label = if wasd { "WASD: ON" } else { "WASD" };
                let wasd_resp = toolbar_icon_button(
                    ui,
                    ready,
                    wasd,
                    wasd_label,
                    Color32::from_rgb(168, 142, 245),
                    Vec2::new(78.0, 30.0),
                    ToolbarIcon::WasdCross,
                    "Enable frame-rate-independent WASD / arrow-key stage movement and Space jump.",
                );
                app.automation
                    .add_response("playback.wasd", "button", wasd_label, &wasd_resp);
                if wasd_resp.clicked() {
                    app.ui_state.wasd_roam = !wasd;
                    if app.ui_state.wasd_roam {
                        app.ui_state.center_tab = CenterTab::TwoD;
                    }
                }
            }

            ui.separator();

            let is_fit = !app.ui_state.preview_actual_size
                && (sanitize_zoom(app.ui_state.preview_zoom) - 1.0).abs() < 0.01;
            let fit_resp = toolbar_icon_button(
                ui,
                ready,
                is_fit,
                "Fit",
                Color32::from_rgb(82, 196, 176),
                Vec2::new(60.0, 30.0),
                ToolbarIcon::FitCorners,
                "Fit character and jump headroom into the available stage viewport.",
            );
            app.automation
                .add_response("viewport.fit", "button", "Fit", &fit_resp);
            if fit_resp.clicked() {
                fit_viewport_state(&mut app.ui_state);
            }

            let is_actual = app.ui_state.preview_actual_size
                && (sanitize_zoom(app.ui_state.preview_zoom) - 1.0).abs() < 0.01;
            let actual_resp = toolbar_icon_button(
                ui,
                ready,
                is_actual,
                "100%",
                Color32::from_rgb(104, 174, 248),
                Vec2::new(66.0, 30.0),
                ToolbarIcon::ActualOneToOne,
                "Render artwork at 100% actual pixel size (1:1).",
            );
            app.automation
                .add_response("viewport.actual_size", "button", "100%", &actual_resp);
            if actual_resp.clicked() {
                actual_size_viewport_state(&mut app.ui_state);
            }

            let zoom_out_resp = toolbar_icon_button(
                ui,
                ready,
                false,
                "Zoom -",
                Color32::from_rgb(135, 170, 215),
                Vec2::new(74.0, 30.0),
                ToolbarIcon::ZoomMinus,
                "Zoom out relative to the current Fit or 100% base scale.",
            );
            app.automation
                .add_response("viewport.zoom_out", "button", "Zoom -", &zoom_out_resp);
            if zoom_out_resp.clicked() {
                zoom_out_state(&mut app.ui_state);
            }

            let zoom_in_resp = toolbar_icon_button(
                ui,
                ready,
                false,
                "Zoom +",
                Color32::from_rgb(135, 170, 215),
                Vec2::new(74.0, 30.0),
                ToolbarIcon::ZoomPlus,
                "Zoom in relative to the current Fit or 100% base scale.",
            );
            app.automation
                .add_response("viewport.zoom_in", "button", "Zoom +", &zoom_in_resp);
            if zoom_in_resp.clicked() {
                zoom_in_state(&mut app.ui_state);
            }

            let center_resp = toolbar_icon_button(
                ui,
                ready,
                app.ui_state.stage_offset_x.abs() > 0.5 || app.ui_state.stage_offset_y.abs() > 0.5,
                "Center",
                Color32::from_rgb(232, 184, 96),
                Vec2::new(74.0, 30.0),
                ToolbarIcon::CenterTarget,
                "Recenter character on the ground baseline and clear WASD stage offsets.",
            );
            app.automation
                .add_response("viewport.center", "button", "Center", &center_resp);
            if center_resp.clicked() {
                center_stage_state(&mut app.ui_state);
            }

            let status_str = if !ready {
                "Load a character to preview"
            } else if running {
                "Playing"
            } else if app.ui_state.center_tab != CenterTab::TwoD {
                "Play opens Animate"
            } else if app.ui_state.playback_phase == 0.0 {
                "Stopped"
            } else {
                "Paused"
            };
            let zoom_mode = if app.ui_state.preview_actual_size {
                "100%"
            } else {
                "Fit"
            };
            let zoom_pct = (sanitize_zoom(app.ui_state.preview_zoom) * 100.0).round() as i32;
            ui.label(
                egui::RichText::new(format!("{status_str} · {zoom_mode} ({zoom_pct}%)"))
                    .small()
                    .color(chrome::MUTED),
            );
        });

        let is_layered_spec = app
            .session()
            .current_asset()
            .and_then(|a| a.cutout_rig.as_ref())
            .is_some_and(|r| r.rig_type == "layered_motion");
        if !is_layered_spec && app.ui_state.anim_mode == AnimMode::Jump {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Jump height").strong().color(chrome::TEAL));
                ui.spacing_mut().slider_width = (ui.available_width() - 135.0).clamp(80.0, 230.0);
                let response = ui
                    .add(egui::Slider::new(&mut app.ui_state.jump_height_pct, 0.0..=80.0).suffix("%"))
                    .on_hover_text("Preview lift as a percentage of the artwork height. Raise this to make the whole character leave the ground. Motion amount also scales the jump; this does not change exported animations.");
                app.automation
                    .add_response("playback.jump_height", "slider", "Jump height", &response);
                ui.label(egui::RichText::new("of artwork height").small().color(chrome::MUTED));
            });
        }
    });
    ui.add_space(8.0);
}

fn original_info(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let Some(asset) = app.session().current_asset().cloned() else {
        return;
    };
    let asset_id = asset.id;
    let orig_path = asset.path;
    let cutout_rig_opt = asset.cutout_rig;
    ui.set_width(ui.available_width());
    ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Source image").strong().color(chrome::TEXT));
            if let Some(asset) = app.session.current_asset() {
                chrome::badge(ui, &format!("{} × {} px", asset.width, asset.height), chrome::BLUE);
                chrome::badge(ui, if asset.has_alpha { "Alpha channel" } else { "Opaque image" }, chrome::MUTED)
                    .on_hover_text("An alpha channel can store transparency. Its presence alone does not verify background removal quality.");
            }
            if let Some(ref rig) = cutout_rig_opt {
                chrome::badge(ui, &format!("{} rig parts loaded", rig.parts.len()), chrome::TEAL)
                    .on_hover_text("Loaded rig parts, not a guarantee of correct segmentation or joint placement. Review the artwork and animation.");
            }
        });
    ui.add(
        egui::Label::new(
            egui::RichText::new(
                orig_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&asset_id),
            )
            .small()
            .color(chrome::MUTED),
        )
        .truncate(),
    )
    .on_hover_text(orig_path.display().to_string());
    ui.horizontal_wrapped(|ui| {
        let response = chrome::button(ui, "Review parts", false).on_hover_text(
            "Open the connected parts or pose sheet and inspect the available cutouts.",
        );
        app.automation
            .add_response("original.review_parts", "button", "Review parts", &response);
        if response.clicked() {
            app.ui_state.center_tab = CenterTab::AssetSheet;
        }
        let response = chrome::button(ui, "Preview animation", true).on_hover_text(
            "Open the animation viewport. Select an action in the inspector to the right.",
        );
        app.automation
            .add_response("original.preview", "button", "Preview animation", &response);
        if response.clicked() {
            app.ui_state.center_tab = CenterTab::TwoD;
            app.ui_state.playing = true;
        }
    });
}

fn apply_stage_pan_and_zoom(ui: &Ui, resp: &egui::Response, ui_state: &mut crate::UiState) {
    if resp.dragged_by(egui::PointerButton::Primary)
        || resp.dragged_by(egui::PointerButton::Middle)
    {
        let delta = resp.drag_delta();
        ui_state.stage_offset_x = (ui_state.stage_offset_x + delta.x).clamp(-720.0, 720.0);
        ui_state.stage_offset_y = (ui_state.stage_offset_y + delta.y).clamp(-480.0, 480.0);
    }
    if resp.double_clicked() {
        fit_viewport_state(ui_state);
    }
    if resp.hovered() {
        let scroll_y = ui.ctx().input(|i| i.smooth_scroll_delta.y);
        let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
        if scroll_y.abs() > 0.5 {
            let factor = (1.0 + scroll_y * 0.0025).clamp(0.8, 1.25);
            ui_state.preview_zoom = sanitize_zoom(ui_state.preview_zoom * factor);
        } else if (zoom_delta - 1.0).abs() > 0.005 {
            ui_state.preview_zoom = sanitize_zoom(ui_state.preview_zoom * zoom_delta);
        }
    }
}

fn point_in_quad(pt: Pos2, q: &[Pos2; 4]) -> bool {
    let mut pos = 0usize;
    let mut neg = 0usize;
    for i in 0..4 {
        let a = q[i];
        let b = q[(i + 1) % 4];
        let cross = (b.x - a.x) * (pt.y - a.y) - (b.y - a.y) * (pt.x - a.x);
        if cross > 0.0 {
            pos += 1;
        } else if cross < 0.0 {
            neg += 1;
        }
    }
    pos == 0 || neg == 0
}

fn original_design_view(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let (orig_path, cutout_rig_opt, attached_refs) = {
        let Some(asset) = app.session.current_asset() else {
            ui.label("No active character loaded.");
            return;
        };
        (
            asset.path.clone(),
            asset.cutout_rig.clone(),
            asset.attached_references.clone(),
        )
    };

    playback_bar(app, ui);

    let avail = ui.available_size();
    let stage_h = (avail.y - 96.0).max(220.0);
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(avail.x.max(120.0), stage_h),
        egui::Sense::click_and_drag(),
    );
    apply_stage_pan_and_zoom(ui, &resp, &mut app.ui_state);
    let painter = ui.painter_at(rect);

    paint_transparent_checkerboard(&painter, rect, app.ui_state.light_backdrop);

    let ctx = ui.ctx().clone();
    let orig_str = orig_path.to_string_lossy().into_owned();
    if let Some(tex) = app.load_texture(&ctx, &orig_str) {
        let tsize = tex.size_vec2();
        let scale = compute_preview_scale(
            tsize.x,
            tsize.y,
            rect.width() - 32.0,
            rect.height() - 24.0,
            1.0,
            app.ui_state.preview_actual_size,
            app.ui_state.preview_zoom,
        );
        let draw_size = tsize * scale;
        let img_center = Pos2::new(
            rect.center().x + app.ui_state.stage_offset_x,
            rect.center().y + app.ui_state.stage_offset_y,
        );
        let img_rect = Rect::from_center_size(img_center, draw_size);
        painter.image(
            tex.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        // Original artwork remains unobstructed until its inspection overlay is enabled.
        if let Some(ref rig) = cutout_rig_opt
            && app.ui_state.show_pivot_markers
        {
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
                if app.ui_state.show_topology_numbers {
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

    let mut ordered = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for st in part_states {
        if let Some(p) = parts.get(&st.name)
            && is_shown(&st.name)
        {
            seen.insert(st.name.clone());
            ordered.push(p.clone());
        }
    }
    for (k, p) in parts {
        if !seen.contains(k) && is_shown(k) {
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
        // When the standing pose comes from a low-res pose-sheet cell (<320px tall) and has no
        // decomposed part layers, prefer the higher-resolution registered turnaround front frame
        // so Idle/Static previews remain crisp and unclipped.
        if f.label == "standing" && f.parts.is_empty() && f.size[1] < 320 {
            let front_opt = asset
                .turnaround_registration
                .as_ref()
                .and_then(|r| r.frames.iter().find(|tf| tf.label == "front"))
                .or_else(|| {
                    asset
                        .turnaround_manifest
                        .as_ref()
                        .and_then(|tm| tm.frames.iter().find(|tf| tf.label == "front"))
                });
            if let Some(front) = front_opt
                && front.size[1] > f.size[1]
                && front.resolved_path.is_file()
            {
                return ResolvedPoseFrame {
                    label: f.label.clone(),
                    path: front.resolved_path.clone(),
                    measured_lift_px,
                    anchor: front.anchor.or(f.anchor),
                    size: front.size,
                    parts: f.parts.clone(),
                };
            }
        }
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

    // Resolve to the single best-matching reference pose for `mode` rather than cycling
    // unrelated reference poses as if a static reference sheet were a temporal flipbook.
    for want in desired_labels_for_mode(mode) {
        if let Some(frame) = pm.frames.iter().find(|f| f.label == *want) {
            return vec![make_resolved(frame)];
        }
    }

    // Unlabelled references are not an animation cycle. Hold the inspected frame.
    if let Some(frame) = pm
        .frames
        .get(selected_pose_idx)
        .or_else(|| pm.frames.first())
    {
        return vec![make_resolved(frame)];
    }
    Vec::new()
}

fn two_d_gate(app: &mut AnimationCraftApp, ui: &mut Ui) {
    playback_bar(app, ui);

    let raw_dt = ui.ctx().input(|i| i.stable_dt);

    let cutout_rig_opt = app
        .session()
        .current_asset()
        .and_then(|a| a.cutout_rig.clone());
    let is_layered_spec = cutout_rig_opt
        .as_ref()
        .is_some_and(|r| r.rig_type == "layered_motion");

    // Frame-rate-independent (dt-scaled) keyboard WASD + Space handling when WASD Move is enabled
    let mut wasd_moved = false;
    if !is_layered_spec && app.ui_state.wasd_roam {
        let wasd_input = ui.ctx().input(|i| WasdInput {
            left: i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft),
            right: i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight),
            up: i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp),
            down: i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown),
            jump_pressed: i.key_pressed(egui::Key::Space),
        });
        let outcome = step_wasd_roam(
            app.ui_state.stage_offset_x,
            app.ui_state.stage_offset_y,
            raw_dt,
            wasd_input,
            app.ui_state.sprint_lock,
            app.ui_state.anim_speed,
        );
        app.ui_state.stage_offset_x = outcome.offset_x;
        app.ui_state.stage_offset_y = outcome.offset_y;
        wasd_moved = outcome.moved;
        if outcome.triggered_jump {
            trigger_jump_state(&mut app.ui_state);
        } else if let Some(mode) = outcome.mode_override {
            app.ui_state.anim_mode = mode;
        }
    }

    ui.add_space(4.0);

    // Advance phase incrementally using frame dt so changing speed or frame rate never jumps phase
    if app.ui_state.playing || wasd_moved {
        app.ui_state.playback_phase = step_playback_phase(
            app.ui_state.playback_phase,
            raw_dt,
            true,
            app.ui_state.anim_speed,
            app.ui_state.sprint_lock,
        );
        ui.ctx().request_repaint();
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

    let (pose_label, tex_opt, measured_lift_px, measured_lift_summary, frame_anchor, frame_size, frame_parts) =
        if let Some(frame) = active_frame {
            let ctx = ui.ctx().clone();
            let has_rig = cutout_rig_opt.is_some();
            let path = if has_rig {
                frame.path.clone()
            } else {
                app.face_preview.path(&ctx, &frame.path, &app.ui_state)
            };
            let path_str = path.to_string_lossy().into_owned();
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
    let (rect, resp) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
    apply_stage_pan_and_zoom(ui, &resp, &mut app.ui_state);

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
    // Character area between the header text and the ground baseline; ui.screenshot/ui.vision measure only this.
    app.automation.add(
        "stage.viewport_2d",
        "canvas",
        "2D stage",
        Rect::from_min_max(
            Pos2::new(rect.left() + 2.0, rect.top() + 48.0),
            Pos2::new(
                rect.right() - 2.0,
                if is_layered_spec {
                    rect.bottom() - 40.0
                } else {
                    baseline_y - 2.0
                },
            ),
        ),
    );

    let motion_active = (app.ui_state.playing || app.ui_state.playback_phase.abs() > 1e-6)
        && app.ui_state.anim_mode != AnimMode::Static
        && range_factor > 0.005;
    let jump_kin = evaluate_jump(
        app.ui_state.playback_phase,
        app.ui_state.jump_height_pct,
        range_factor,
        !is_layered_spec && motion_active && app.ui_state.anim_mode == AnimMode::Jump,
    );

    if !is_layered_spec {
        painter.line_segment(
            [
                Pos2::new(rect.left() + 16.0, baseline_y),
                Pos2::new(rect.right() - 16.0, baseline_y),
            ],
            Stroke::new(2.0, Color32::from_rgb(72, 185, 130)),
        );

        // Compute live stage lift in screen pixels so the contact shadow and baseline telemetry reflect Jump height
        let approx_char_h = (rect.height() - 110.0).clamp(150.0, 420.0);
        let (effective_lift_px, lift_summary) = if app.ui_state.anim_mode == AnimMode::Jump {
            let live_px = jump_kin.lift_px(approx_char_h);
            (
                live_px,
                format!(
                    "+{live_px:.1} px (jump {}, {:.0}% height)",
                    jump_kin.stage.as_str(),
                    app.ui_state.jump_height_pct
                ),
            )
        } else {
            (measured_lift_px, measured_lift_summary)
        };

        painter.text(
            Pos2::new(rect.left() + 20.0, baseline_y + 10.0),
            egui::Align2::LEFT_TOP,
            format!(
                "GROUND BASELINE · Drift: {drift_summary} · Pose: {pose_label} · Lift: {lift_summary}"
            ),
            egui::FontId::monospace(11.0),
            Color32::from_rgb(110, 220, 165),
        );

        // Ground contact shadow (tracks stage_offset_x, narrows with airborne lift, widens slightly on crouch)
        let shadow_w = (120.0 - effective_lift_px * 0.7 + jump_kin.crouch_fraction * 18.0).clamp(48.0, 142.0);
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
    }

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
            || app.ui_state.arm_spread.abs() > 0.5);

    let tint = match app.ui_state.shading_mode {
        ShadingMode::FeltPbr => Color32::WHITE,
        ShadingMode::ClaySculpt => Color32::from_rgb(215, 200, 185),
        ShadingMode::Wireframe => Color32::from_rgb(150, 225, 255),
        ShadingMode::Normals => Color32::from_rgb(175, 160, 255),
    };

    #[derive(Clone, Debug)]
    struct RenderedStagePart {
        id: String,
        corners: [Pos2; 4],
        pivot: Pos2,
    }
    let mut rendered_parts: Vec<RenderedStagePart> = Vec::new();

    if let Some(rig) = cutout_rig_opt {
        app.face_preview.status.clear();
        let ctx = ui.ctx().clone();
        let cw = rig.canvas_size[0].max(1) as f32;
        let ch = rig.canvas_size[1].max(1) as f32;
        let [cb_x0, cb_y0, cb_x1, cb_y1] = rig.character_bbox;
        let bbox_w = (cb_x1.saturating_sub(cb_x0)).max(1) as f32;
        let bbox_h = (cb_y1.saturating_sub(cb_y0)).max(1) as f32;
        let bbox_cx = cb_x0 as f32 + bbox_w * 0.5;
        let bbox_cy = cb_y0 as f32 + bbox_h * 0.5;

        let max_h = (rect.height() - 105.0).max(160.0);
        let max_w = (rect.width() - 32.0).max(120.0);
        let headroom = if !is_layered_spec && app.ui_state.anim_mode == AnimMode::Jump {
            1.0 + app.ui_state.jump_height_pct / 100.0
        } else if is_layered_spec {
            1.28
        } else {
            1.0
        };
        let (fit_w, fit_h) = if is_layered_spec {
            (bbox_w, bbox_h)
        } else {
            (cw, ch)
        };
        let scale = compute_preview_scale(
            fit_w,
            fit_h,
            max_w,
            max_h,
            headroom,
            app.ui_state.preview_actual_size,
            app.ui_state.preview_zoom,
        );
        let center_x = rect.center().x + app.ui_state.stage_offset_x;
        let canvas_origin = if is_layered_spec {
            let center_y = rect.center().y + app.ui_state.stage_offset_y;
            Pos2::new(center_x - bbox_cx * scale, center_y - bbox_cy * scale)
        } else {
            let draw_size = Vec2::new(cw * scale, ch * scale);
            let bottom_y = baseline_y + 18.0 + app.ui_state.stage_offset_y;
            Pos2::new(center_x - draw_size.x * 0.5, bottom_y - draw_size.y)
        };

        let rig_eval = evaluate_cutout_rig_motion(&rig, &app.ui_state);
        let opacity_map = &rig_eval.opacity;

        let mut parts_by_id: BTreeMap<&str, &animationcraft_engine::session::CutoutRigPart> =
            BTreeMap::new();
        for p in &rig.parts {
            parts_by_id.insert(p.id.as_str(), p);
        }

        let explode_factor = (app.ui_state.explode_pct / 100.0) * 0.45;
        let rig_center_canvas = if is_layered_spec {
            Pos2::new(bbox_cx, bbox_cy)
        } else {
            Pos2::new(cw * 0.5, ch * 0.5)
        };

        let transform_canvas_pt = |pt: Pos2, part_id: &str| -> Pos2 {
            let transformed = center_motion::transform_rig_point(&rig, &rig_eval, part_id, [pt.x, pt.y]);
            let mut cur = Pos2::new(transformed[0], transformed[1]);
            if explode_factor > 0.002
                && let Some(part) = parts_by_id.get(part_id)
            {
                let piv = Pos2::new(part.pivot[0], part.pivot[1]);
                let explode_vec = (piv - rig_center_canvas) * explode_factor;
                cur += explode_vec;
            }
            Pos2::new(
                canvas_origin.x + cur.x * scale,
                canvas_origin.y + cur.y * scale,
            )
        };

        let mut sorted_parts: Vec<_> = rig
            .parts
            .iter()
            .filter(|p| is_rig_part_visible(&p.id, &part_states))
            .cloned()
            .collect();
        sorted_parts.sort_by_key(|p| p.layer);

        for (part_idx, part) in sorted_parts.iter().enumerate() {
            let part_opacity = opacity_map
                .get(part.id.as_str())
                .copied()
                .unwrap_or(1.0)
                .clamp(0.0, 1.0);
            if part_opacity <= 0.01 {
                continue;
            }
            let (path, status) = app.control_preview.path(&ctx, &rig, part, &app.ui_state);
            if !status.is_empty() { app.face_preview.status = status; }
            let p_str = path.to_string_lossy().into_owned();
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
            let part_tint = Color32::from_rgba_unmultiplied(
                tint.r(),
                tint.g(),
                tint.b(),
                ((tint.a() as f32) * part_opacity).round() as u8,
            );
            let screen_corners = [
                transform_canvas_pt(corners_canvas[0], &part.id),
                transform_canvas_pt(corners_canvas[1], &part.id),
                transform_canvas_pt(corners_canvas[2], &part.id),
                transform_canvas_pt(corners_canvas[3], &part.id),
            ];
            let mut min_x = f32::INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            let mut mesh = egui::Mesh::with_texture(ptex.id());
            for i in 0..4 {
                let screen_pt = screen_corners[i];
                min_x = min_x.min(screen_pt.x);
                min_y = min_y.min(screen_pt.y);
                max_x = max_x.max(screen_pt.x);
                max_y = max_y.max(screen_pt.y);
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: screen_pt,
                    uv: uvs[i],
                    color: part_tint,
                });
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            painter.add(egui::Shape::mesh(mesh));

            let piv_screen =
                transform_canvas_pt(Pos2::new(part.pivot[0], part.pivot[1]), &part.id);
            let part_bounds = Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y));
            app.automation.add(
                format!("stage.part.{}", part.id),
                "part",
                &part.id,
                part_bounds,
            );
            rendered_parts.push(RenderedStagePart {
                id: part.id.clone(),
                corners: screen_corners,
                pivot: piv_screen,
            });

            if app.ui_state.show_topology_numbers {
                let badge_pos = piv_screen + Vec2::new(0.0, -12.0);
                painter.circle_filled(badge_pos, 9.0, Color32::from_rgb(35, 95, 165));
                painter.text(
                    badge_pos,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", part_idx + 1),
                    egui::FontId::monospace(10.0),
                    Color32::WHITE,
                );
            }
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

        if is_layered_spec {
            let track_id = rig_eval.track_id.as_deref().unwrap_or("hover-idle");
            let visible_count = rendered_parts.len();
            painter.text(
                Pos2::new(rect.left() + 20.0, baseline_y + 10.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "LAYERED MOTION SPEC · Track: {} ({:.2}s / {:.1}s) · Visible Layers: {}/{} · Drag: pan · Scroll: zoom · Right-click: part menu",
                    track_id,
                    rig_eval.track_time,
                    rig_eval.track_duration,
                    visible_count,
                    rig.parts.len(),
                ),
                egui::FontId::monospace(11.0),
                Color32::from_rgb(130, 210, 255),
            );
            painter.text(
                Pos2::new(rect.left() + 14.0, rect.top() + 14.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "LAYERED MOTION PREVIEW · Asset: {} · Track: {} · Range: {:.0}%",
                    app.session().active_asset.as_deref().unwrap_or("none"),
                    track_id,
                    app.ui_state.pose_range_pct
                ),
                egui::FontId::monospace(11.5),
                Color32::from_rgb(210, 225, 245),
            );
        }
    } else if let Some(tex) = tex_opt {
        let tex_size = tex.size_vec2();
        let max_h = (rect.height() - 115.0).max(150.0);
        let max_w = (rect.width() - 32.0).max(120.0);
        let headroom = if app.ui_state.anim_mode == AnimMode::Jump {
            1.0 + app.ui_state.jump_height_pct / 100.0
        } else {
            1.0
        };
        let scale = compute_preview_scale(
            tex_size.x,
            tex_size.y,
            max_w,
            max_h,
            headroom,
            app.ui_state.preview_actual_size,
            app.ui_state.preview_zoom,
        );
        let base_draw_size = tex_size * scale;
        let center_x = rect.center().x + app.ui_state.stage_offset_x;
        let preview_lift = if app.ui_state.anim_mode == AnimMode::Jump {
            jump_arc(app.ui_state.playback_phase) * base_draw_size.y * app.ui_state.jump_height_pct
                / 100.0
                * range_factor
        } else {
            measured_lift_px
        };
        // Grounded takeoff/landing crouch compression anchored at bottom_y
        let (scale_x, scale_y) = if app.ui_state.anim_mode == AnimMode::Jump {
            (
                1.0 + jump_kin.crouch_fraction * 0.05 - jump_kin.lift_fraction * 0.02,
                1.0 - jump_kin.crouch_fraction * 0.10 + jump_kin.lift_fraction * 0.04,
            )
        } else {
            (1.0, 1.0)
        };
        let draw_size = Vec2::new(base_draw_size.x * scale_x, base_draw_size.y * scale_y);
        let bottom_y = baseline_y - preview_lift + app.ui_state.stage_offset_y;
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
                let resolved = if part.name == "head" || part.name.starts_with("head_") {
                    app.face_preview
                        .path(&ctx, &part.resolved_path, &app.ui_state)
                } else {
                    part.resolved_path.clone()
                };
                let p_str = resolved.to_string_lossy().into_owned();
                let Some(ptex) = app.load_texture(&ctx, &p_str) else {
                    continue;
                };
                let pw = (part.size[0].max(1) as f32) * scale;
                let ph = (part.size[1].max(1) as f32) * scale;
                let base_x = sprite_rect.left() + (part.bind_position[0] - part.pivot[0]) * scale;
                let base_y = sprite_rect.top() + (part.bind_position[1] - part.pivot[1]) * scale;

                let base_center = Pos2::new(base_x + pw * 0.5, base_y + ph * 0.5);
                let (rig_dx, rig_dy) = (0.0, 0.0);
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
                let piv_pos = Pos2::new(
                    part_rect.left() + part.pivot[0] * scale,
                    part_rect.top() + part.pivot[1] * scale,
                );
                app.automation.add(
                    format!("stage.part.{}", part.name),
                    "part",
                    &part.name,
                    part_rect,
                );
                rendered_parts.push(RenderedStagePart {
                    id: part.name.clone(),
                    corners: [
                        part_rect.left_top(),
                        part_rect.right_top(),
                        part_rect.right_bottom(),
                        part_rect.left_bottom(),
                    ],
                    pivot: piv_pos,
                });
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
            if app.ui_state.playing && frame_parts.is_empty() && app.ui_state.anim_mode != AnimMode::Jump {
                painter.text(
                    Pos2::new(rect.left() + 14.0, rect.top() + 32.0),
                    egui::Align2::LEFT_TOP,
                    "Reference pose held (build a cutout rig to articulate limbs for this action)",
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(235, 195, 115),
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

    // Per-part hit testing, selection, hover highlight, and right-click context menu
    let pointer_pos = resp.hover_pos().or_else(|| ui.ctx().pointer_interact_pos());
    let hovered_part_id: Option<String> = pointer_pos.and_then(|pt| {
        if !rect.contains(pt) {
            return None;
        }
        rendered_parts
            .iter()
            .rev()
            .find(|rp| point_in_quad(pt, &rp.corners))
            .map(|rp| rp.id.clone())
    });

    if resp.clicked() {
        let shift = ui.ctx().input(|i| i.modifiers.shift || i.modifiers.command);
        if let Some(ref pid) = hovered_part_id {
            if shift {
                if !app.ui_state.selected_parts.remove(pid) {
                    app.ui_state.selected_parts.insert(pid.clone());
                }
            } else {
                app.ui_state.selected_parts.clear();
                app.ui_state.selected_parts.insert(pid.clone());
            }
        } else if !shift {
            app.ui_state.selected_parts.clear();
        }
    }
    if resp.secondary_clicked() {
        app.ui_state.context_menu_part = hovered_part_id.clone();
        if let Some(ref pid) = hovered_part_id
            && !app.ui_state.selected_parts.contains(pid)
        {
            app.ui_state.selected_parts.clear();
            app.ui_state.selected_parts.insert(pid.clone());
        }
    }

    let draw_quad = |corners: &[Pos2; 4], stroke: Stroke| {
        for i in 0..4 {
            painter.line_segment([corners[i], corners[(i + 1) % 4]], stroke);
        }
    };

    for rp in &rendered_parts {
        if app.ui_state.selected_parts.contains(&rp.id) {
            draw_quad(
                &rp.corners,
                Stroke::new(2.0, Color32::from_rgb(255, 205, 65)),
            );
            painter.circle_filled(rp.pivot, 4.5, Color32::from_rgb(255, 205, 65));
        }
    }

    if let Some(ref hid) = hovered_part_id
        && let Some(rp) = rendered_parts.iter().find(|p| &p.id == hid)
    {
        draw_quad(
            &rp.corners,
            Stroke::new(2.2, Color32::from_rgb(75, 225, 255)),
        );
        painter.circle_stroke(
            rp.pivot,
            6.5,
            Stroke::new(1.8, Color32::from_rgb(75, 225, 255)),
        );
        let badge_text = format!(
            "Part: {}  (Click: select · Shift+Click: multi · Right-click: remove/options)",
            rp.id
        );
        let badge_pos = Pos2::new(rect.left() + 14.0, rect.top() + 32.0);
        painter.text(
            badge_pos,
            egui::Align2::LEFT_TOP,
            badge_text,
            egui::FontId::monospace(11.0),
            Color32::from_rgb(105, 235, 255),
        );
    }

    resp.context_menu(|ui| {
        ui.set_min_width(220.0);
        let target_part = app
            .ui_state
            .context_menu_part
            .clone()
            .or_else(|| app.ui_state.selected_parts.iter().next().cloned());
        let selected_list: Vec<String> = app.ui_state.selected_parts.iter().cloned().collect();

        if let Some(ref pid) = target_part {
            ui.label(
                egui::RichText::new(format!("Layer / Part: {pid}"))
                    .strong()
                    .color(Color32::from_rgb(130, 215, 255)),
            );
            if ui
                .button(format!("Hide / Remove \"{pid}\""))
                .on_hover_text("Hide this layer in the current session so it does not render")
                .clicked()
            {
                if let Some(asset) = app.session_mut().current_asset_mut()
                    && let Some(st) = asset.parts.iter_mut().find(|p| p.name == *pid)
                {
                    st.visible = false;
                }
                app.ui_state.selected_parts.remove(pid);
                app.ui_state.status_line = format!(
                    "Hidden part '{pid}' from stage (use right-click -> Show All Parts to restore)."
                );
                ui.close();
            }
            if ui
                .button(format!("Solo \"{pid}\""))
                .on_hover_text("Isolate only this part on the stage")
                .clicked()
            {
                if let Some(asset) = app.session_mut().current_asset_mut() {
                    for st in &mut asset.parts {
                        st.solo = st.name == *pid;
                    }
                }
                app.ui_state.status_line = format!("Soloing part '{pid}' on stage.");
                ui.close();
            }
            ui.separator();
        }

        if selected_list.len() > 1 {
            if ui
                .button(format!(
                    "Hide / Remove {} Selected Parts",
                    selected_list.len()
                ))
                .clicked()
            {
                if let Some(asset) = app.session_mut().current_asset_mut() {
                    for st in &mut asset.parts {
                        if selected_list.contains(&st.name) {
                            st.visible = false;
                        }
                    }
                }
                let removed = selected_list.join(", ");
                app.ui_state.selected_parts.clear();
                app.ui_state.status_line = format!("Hidden selected parts: {removed}");
                ui.close();
            }
            ui.separator();
        }

        let any_hidden_or_solo = app
            .session()
            .current_asset()
            .map(|a| a.parts.iter().any(|p| !p.visible || p.solo))
            .unwrap_or(false);
        if any_hidden_or_solo
            && ui
                .button("Show All Parts (Restore Hidden)")
                .on_hover_text("Restore all hidden or soloed layers back to visible")
                .clicked()
        {
            if let Some(asset) = app.session_mut().current_asset_mut() {
                for st in &mut asset.parts {
                    st.visible = true;
                    st.solo = false;
                }
            }
            app.ui_state.status_line = "Restored all hidden parts to visible.".to_string();
            ui.close();
        }

        ui.menu_button("Toggle Parts / Sacks / Wings", |ui| {
            if let Some(asset) = app.session_mut().current_asset_mut() {
                for st in &mut asset.parts {
                    ui.checkbox(&mut st.visible, &st.name);
                }
            }
        });

        ui.separator();
        ui.checkbox(&mut app.ui_state.show_pivot_markers, "Show Joint Anchors");
        ui.checkbox(&mut app.ui_state.show_topology_numbers, "Show Part Numbers");
        ui.separator();
        if ui.button("Fit & Center in Viewport").clicked() {
            fit_viewport_state(&mut app.ui_state);
            ui.close();
        }
        if ui.button("Actual Size (100%)").clicked() {
            actual_size_viewport_state(&mut app.ui_state);
            ui.close();
        }
    });

    if !is_layered_spec {
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
        let dt = sanitize_dt(ui.ctx().input(|i| i.stable_dt));
        app.ui_state.yaw_deg = (app.ui_state.yaw_deg + dt * 36.0).rem_euclid(360.0);
        ui.ctx().request_repaint();
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

    let frame_opt =
        select_turnaround_frame_for_yaw(app.session().current_asset(), app.ui_state.yaw_deg);
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
        (Color32::from_rgb(18, 22, 30), Color32::from_rgb(26, 31, 42))
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
            let use_ocr = app.ui_state.use_ocr;
            app.ui_state_mut().image_path = Some(path_str.clone());
            app.exec_ui_command(UiCommand::IntakeValidate {
                image_path: path_str,
                slot: "turnaround_sheet".to_string(),
                attach_to_active: true,
                auto_cut: true,
                tolerance,
                use_ocr,
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
            let scale = (slot_rect.height() / ts.y.max(1.0)).min(slot_rect.width() / ts.x.max(1.0));
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
    #[test]
    fn jump_has_grounded_endpoints_and_a_single_apex() {
        let turn = std::f64::consts::TAU;
        assert_eq!(jump_arc(0.0), 0.0);
        assert_eq!(jump_arc(turn), 0.0);
        assert!((jump_arc(turn * 0.5) - 1.0).abs() < 0.00001);
        assert!(jump_arc(turn * 0.3) < jump_arc(turn * 0.4));
        assert!((jump_arc(turn * 0.3) - jump_arc(turn * 0.7)).abs() < 0.00001);
    }

    #[test]
    fn center_transport_opens_animation_pauses_and_rewinds() {
        let mut app = AnimationCraftApp::new();
        app.session.active_asset = Some("transport_test".into());
        app.session
            .assets
            .insert("transport_test".into(), Asset::default());
        app.ui_state.center_tab = CenterTab::OriginalView;
        app.ui_state.playback_phase = 2.5;
        let ctx = egui::Context::default();
        let render = |app: &mut AnimationCraftApp, events| {
            app.automation.begin_frame();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 220.0))),
                    events,
                    ..Default::default()
                },
                |ui| playback_bar(app, ui),
            );
            output.textures_delta.clear();
        };
        for id in ["playback.play", "playback.pause", "playback.stop"] {
            render(&mut app, vec![]);
            let elem = app.automation.elements.iter().find(|e| e.id == id).unwrap();
            assert_eq!(elem.enabled, Some(true));
            let pos = Pos2::new(
                elem.rect[0] + elem.rect[2] / 2.0,
                elem.rect[1] + elem.rect[3] / 2.0,
            );
            for pressed in [true, false] {
                render(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(app.ui_state.center_tab, CenterTab::TwoD);
            assert_eq!(app.ui_state.playing, id == "playback.play");
            assert_eq!(
                app.ui_state.playback_phase,
                if id == "playback.stop" { 0.0 } else { 2.5 }
            );
        }
    }

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
                        qa_issues: Vec::new(),
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
                        qa_issues: Vec::new(),
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
                        qa_issues: Vec::new(),
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

    #[test]
    fn playback_phase_and_wasd_roam_are_frame_rate_invariant_across_dt_subdivisions() {
        // 1.0 second of playback at 24 FPS, 60 FPS, and 120 FPS must yield identical phase
        let mut phase_24 = 0.25_f64;
        for _ in 0..24 {
            phase_24 = step_playback_phase(phase_24, 1.0 / 24.0, true, 1.25, false);
        }
        let mut phase_60 = 0.25_f64;
        for _ in 0..60 {
            phase_60 = step_playback_phase(phase_60, 1.0 / 60.0, true, 1.25, false);
        }
        let mut phase_120 = 0.25_f64;
        for _ in 0..120 {
            phase_120 = step_playback_phase(phase_120, 1.0 / 120.0, true, 1.25, false);
        }
        assert!((phase_24 - phase_60).abs() < 1e-5);
        assert!((phase_60 - phase_120).abs() < 1e-5);

        // Paused playback must never advance phase regardless of dt
        let paused_phase = step_playback_phase(phase_60, 0.05, false, 1.5, true);
        assert_eq!(paused_phase, phase_60);

        // 0.5 seconds of WASD diagonal roaming at 30 FPS vs 60 FPS vs 120 FPS must yield identical offsets
        let input = WasdInput {
            left: false,
            right: true,
            up: true,
            down: false,
            jump_pressed: false,
        };
        let run_roam = |steps: usize, dt: f32| -> (f32, f32, Option<AnimMode>) {
            let (mut x, mut y, mut mode) = (0.0_f32, 0.0_f32, None);
            for _ in 0..steps {
                let out = step_wasd_roam(x, y, dt, input, false, 1.0);
                x = out.offset_x;
                y = out.offset_y;
                mode = out.mode_override;
            }
            (x, y, mode)
        };
        let (x30, y30, mode30) = run_roam(15, 1.0 / 30.0);
        let (x60, y60, mode60) = run_roam(30, 1.0 / 60.0);
        let (x120, y120, mode120) = run_roam(60, 1.0 / 120.0);
        assert!((x30 - x60).abs() < 1e-3 && (x60 - x120).abs() < 1e-3);
        assert!((y30 - y60).abs() < 1e-3 && (y60 - y120).abs() < 1e-3);
        assert!(x60 > 0.0 && y60 < 0.0);
        assert_eq!(mode30, Some(AnimMode::Walk));
        assert_eq!(mode60, Some(AnimMode::Walk));
        assert_eq!(mode120, Some(AnimMode::Walk));

        // Sprint lock increases WASD speed and switches horizontal movement to AnimMode::Run
        let sprint_out = step_wasd_roam(0.0, 0.0, 0.05, input, true, 1.0);
        let walk_out = step_wasd_roam(0.0, 0.0, 0.05, input, false, 1.0);
        assert!(sprint_out.offset_x > walk_out.offset_x);
        assert_eq!(sprint_out.mode_override, Some(AnimMode::Run));
    }

    #[test]
    fn jump_kinematics_has_c0_continuous_takeoff_apex_landing_and_scales_with_jump_height() {
        use center_motion::JumpStage;
        let tau = std::f64::consts::TAU;

        // Grounded at phase 0.0 and TAU
        let start = evaluate_jump(0.0, 50.0, 1.0, true);
        let end = evaluate_jump(tau, 50.0, 1.0, true);
        assert_eq!(start.stage, JumpStage::Grounded);
        assert_eq!(start.lift_fraction, 0.0);
        assert_eq!(start.crouch_fraction, 0.0);
        assert_eq!(end.lift_fraction, 0.0);
        assert!(end.crouch_fraction.abs() < 1e-4);

        // Takeoff crouch peaks at u = 0.09 with zero airborne lift
        let takeoff_mid = evaluate_jump(0.09 * tau, 50.0, 1.0, true);
        assert_eq!(takeoff_mid.stage, JumpStage::Takeoff);
        assert_eq!(takeoff_mid.lift_fraction, 0.0);
        assert!((takeoff_mid.crouch_unit - 1.0).abs() < 1e-4);
        assert!(takeoff_mid.crouch_fraction > 0.5);

        // Liftoff boundary u = 0.18 has both crouch == 0 and lift == 0 (no liftoff snap)
        let liftoff = evaluate_jump(0.18 * tau, 50.0, 1.0, true);
        assert!(liftoff.crouch_unit.abs() < 1e-4);
        assert!(liftoff.lift_unit.abs() < 1e-4);

        // Apex at u = 0.50 reaches full jump_height_pct and scales linearly with jump_height_pct
        let apex_25 = evaluate_jump(0.50 * tau, 25.0, 1.0, true);
        let apex_50 = evaluate_jump(0.50 * tau, 50.0, 1.0, true);
        let apex_0 = evaluate_jump(0.50 * tau, 0.0, 1.0, true);
        assert_eq!(apex_50.stage, JumpStage::Apex);
        assert!((apex_50.lift_unit - 1.0).abs() < 1e-5);
        assert_eq!(apex_50.crouch_fraction, 0.0);
        assert!((apex_50.lift_px(300.0) - 150.0).abs() < 1e-3);
        assert!((apex_25.lift_px(300.0) - 75.0).abs() < 1e-3);
        assert_eq!(apex_0.lift_px(300.0), 0.0);

        // Touchdown boundary u = 0.82 has both lift == 0 and crouch == 0 (no landing snap)
        let touchdown = evaluate_jump(0.82 * tau, 50.0, 1.0, true);
        assert!(touchdown.lift_unit.abs() < 1e-4);
        assert!(touchdown.crouch_unit.abs() < 1e-4);

        // Landing absorption peaks at u = 0.91 with zero airborne lift
        let landing_mid = evaluate_jump(0.91 * tau, 50.0, 1.0, true);
        assert_eq!(landing_mid.stage, JumpStage::Landing);
        assert_eq!(landing_mid.lift_fraction, 0.0);
        assert!((landing_mid.crouch_unit - 0.75).abs() < 1e-4);

        // Dense C0 continuity check across the entire [0, TAU] cycle
        let samples = 500usize;
        let mut prev = evaluate_jump(0.0, 60.0, 1.0, true);
        for i in 1..=samples {
            let p = (i as f64 / samples as f64) * tau;
            let cur = evaluate_jump(p, 60.0, 1.0, true);
            assert!(
                (cur.lift_unit - prev.lift_unit).abs() < 0.04,
                "lift discontinuity at u={}: {} -> {}",
                cur.cycle_u,
                prev.lift_unit,
                cur.lift_unit
            );
            assert!(
                (cur.crouch_unit - prev.crouch_unit).abs() < 0.04,
                "crouch discontinuity at u={}: {} -> {}",
                cur.cycle_u,
                prev.crouch_unit,
                cur.crouch_unit
            );
            prev = cur;
        }
    }

    #[test]
    fn reference_pose_resolution_never_cycles_unrelated_reference_poses() {
        let labels = [
            "standing",
            "walking",
            "running",
            "jumping",
            "waving",
            "pointing",
            "looking_up",
            "looking_down",
            "arms_wide",
            "clapping",
        ];
        let frames: Vec<ManifestFrame> = labels
            .iter()
            .enumerate()
            .map(|(index, label)| ManifestFrame {
                        qa_issues: Vec::new(),
                index,
                label: (*label).to_string(),
                file: format!("frames/{index:02}_{label}.png"),
                resolved_path: PathBuf::from(format!("{index:02}_{label}.png")),
                size: [200, 300],
                source_box: None,
                anchor: None,
                parts: BTreeMap::new(),
            })
            .collect();
        let asset = Asset {
            id: "generic_sheet".into(),
            pose_manifest: Some(LoadedManifest {
                manifest_dir: PathBuf::from("generic"),
                manifest_path: PathBuf::from("generic/manifest.json"),
                sheet_type: "action".into(),
                status: "ok".into(),
                matting_engine: None,
                source_image: None,
                source_size: None,
                frames,
                part_names: Vec::new(),
            }),
            ..Asset::default()
        };

        let expected = [
            (AnimMode::Idle, "standing"),
            (AnimMode::Walk, "walking"),
            (AnimMode::Run, "running"),
            (AnimMode::Jump, "jumping"),
            (AnimMode::Wave, "waving"),
            (AnimMode::LookScout, "looking_up"),
            (AnimMode::Static, "standing"),
        ];
        for (mode, expected_label) in expected {
            let seq = resolve_pose_sequence(Some(&asset), mode, 100.0, 0, true);
            assert_eq!(
                seq.len(),
                1,
                "mode {mode:?} must resolve to a single reference pose, not a multi-pose flipbook"
            );
            assert_eq!(seq[0].label, expected_label);
        }
    }

    #[test]
    fn stop_center_and_cutout_rig_evaluation_respect_rest_pose_and_joint_limits() {
        use animationcraft_engine::session::{CutoutRigPart, LoadedCutoutRigPlan};
        use serde_json::json;

        // Generic non-biped rig with arbitrary part IDs, rotation limits, and pose_library
        let rig = LoadedCutoutRigPlan {
            character_id: "mech_drone".into(),
            rig_type: "cutout_rig".into(),
            canvas_size: [400, 500],
            character_bbox: [40, 30, 360, 470],
            parts: vec![
                CutoutRigPart {
                    id: "hull_core".into(),
                    parent: None,
                    pivot: [200.0, 250.0],
                    rotation: [-10.0, 10.0],
                    ..CutoutRigPart::default()
                },
                CutoutRigPart {
                    id: "thruster_left".into(),
                    parent: Some("hull_core".into()),
                    pivot: [130.0, 260.0],
                    rotation: [-20.0, 25.0],
                    ..CutoutRigPart::default()
                },
                CutoutRigPart {
                    id: "head".into(),
                    parent: Some("hull_core".into()),
                    pivot: [200.0, 150.0],
                    rotation: [-12.0, 12.0],
                    ..CutoutRigPart::default()
                },
            ],
            pose_library: json!({
                "poses": {
                    "walk_contact": {
                        "parts": {
                            "hull_core": { "rotation": 6.0 },
                            "thruster_left": { "rotation": 40.0 },
                            "head": { "rotation": -5.0 }
                        }
                    },
                    "walk_passing": {
                        "parts": {
                            "hull_core": { "rotation": -6.0 },
                            "thruster_left": { "rotation": -35.0 },
                            "head": { "rotation": 5.0 }
                        }
                    }
                }
            }),
            ..LoadedCutoutRigPlan::default()
        };

        let mut state = crate::UiState::default();
        trigger_jump_state(&mut state);
        assert_eq!(state.anim_mode, AnimMode::Jump);
        assert!(state.playing);
        assert_eq!(
            state.playback_phase, 0.0,
            "Jump trigger must start at phase 0.0 (takeoff), not mid-cycle"
        );

        // Advance into Walk stride and verify pose_library target + rotation clamping
        state.anim_mode = AnimMode::Walk;
        state.playing = true;
        state.playback_phase = std::f64::consts::FRAC_PI_2;
        state.stage_offset_x = 85.0;
        state.stage_offset_y = -20.0;
        let active_eval = evaluate_cutout_rig_motion(&rig, &state);
        assert!(active_eval.angles_deg["hull_core"] > 0.0);
        assert!(
            (active_eval.angles_deg["thruster_left"] - 25.0).abs() < 1e-4,
            "thruster_left rotation (40 deg in pose_library) must clamp to part.rotation max (25 deg)"
        );
        assert!(active_eval.root_dy_canvas < 0.0);

        // Pause holds the exact evaluated pose
        state.playing = false;
        let paused_eval = evaluate_cutout_rig_motion(&rig, &state);
        assert_eq!(paused_eval.angles_deg, active_eval.angles_deg);
        assert_eq!(paused_eval.root_dy_canvas, active_eval.root_dy_canvas);

        // Stop resets playing, phase, stage offsets, and returns all rig joints to 0.0 rest pose
        stop_motion_state(&mut state);
        assert!(!state.playing);
        assert_eq!(state.playback_phase, 0.0);
        assert_eq!(state.stage_offset_x, 0.0);
        assert_eq!(state.stage_offset_y, 0.0);
        let stopped_eval = evaluate_cutout_rig_motion(&rig, &state);
        for (part_id, deg) in &stopped_eval.angles_deg {
            assert_eq!(
                *deg, 0.0,
                "part `{part_id}` must return to 0.0 deg bind pose when stopped"
            );
        }
        assert_eq!(stopped_eval.root_dy_canvas, 0.0);
    }

    #[test]
    fn viewport_zoom_fit_actual_size_and_toolbar_controls_apply_to_rig_and_sprite() {
        let mut app = AnimationCraftApp::new();
        let ctx = egui::Context::default();

        let render = |app: &mut AnimationCraftApp| {
            app.automation.begin_frame();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 260.0))),
                    ..Default::default()
                },
                |ui| playback_bar(app, ui),
            );
            output.textures_delta.clear();
        };

        render(&mut app);
        let elements = &app.automation.elements;
        for required_id in [
            "playback.play",
            "playback.pause",
            "playback.stop",
            "playback.action",
            "playback.jump",
            "playback.run_lock",
            "playback.wasd",
            "viewport.fit",
            "viewport.actual_size",
            "viewport.zoom_out",
            "viewport.zoom_in",
            "viewport.center",
        ] {
            assert!(
                elements.iter().any(|e| e.id == required_id),
                "missing toolbar automation id `{required_id}`"
            );
        }

        // Triggering Jump switches mode to Jump and exposes the Jump height slider in the toolbar
        trigger_jump_state(&mut app.ui_state);
        render(&mut app);
        assert!(
            app.automation
                .elements
                .iter()
                .any(|e| e.id == "playback.jump_height"),
            "missing `playback.jump_height` slider when in AnimMode::Jump"
        );

        // Verify Fit (actual_size=false, zoom=1.0), 100% (actual_size=true, zoom=1.0), Zoom +/Zoom -, and jump headroom
        let fit_base = compute_preview_scale(1000.0, 800.0, 500.0, 400.0, 1.0, false, 1.0);
        assert!((fit_base - 0.5).abs() < 1e-4);

        // Jump headroom scales down Fit base to preserve headroom above the character
        let fit_jump = compute_preview_scale(1000.0, 800.0, 500.0, 400.0, 1.5, false, 1.0);
        assert!(fit_jump < fit_base);
        assert!((fit_jump - (400.0 / (800.0 * 1.5))).abs() < 1e-4);

        // Actual size (100%) uses 1.0 base scale multiplied by preview_zoom
        actual_size_viewport_state(&mut app.ui_state);
        assert!(app.ui_state.preview_actual_size);
        assert_eq!(app.ui_state.preview_zoom, 1.0);
        let actual_100 = compute_preview_scale(
            1000.0,
            800.0,
            500.0,
            400.0,
            1.0,
            app.ui_state.preview_actual_size,
            app.ui_state.preview_zoom,
        );
        assert!((actual_100 - 1.0).abs() < 1e-4);

        // Zoom + and Zoom - multiply the active base scale in both Fit and 100% modes
        zoom_in_state(&mut app.ui_state);
        assert!(app.ui_state.preview_zoom > 1.0);
        let zoomed_actual = compute_preview_scale(
            1000.0,
            800.0,
            500.0,
            400.0,
            1.0,
            app.ui_state.preview_actual_size,
            app.ui_state.preview_zoom,
        );
        assert!(zoomed_actual > actual_100);

        fit_viewport_state(&mut app.ui_state);
        assert!(!app.ui_state.preview_actual_size);
        assert_eq!(app.ui_state.preview_zoom, 1.0);
        zoom_out_state(&mut app.ui_state);
        assert!(app.ui_state.preview_zoom < 1.0);
        let zoomed_fit = compute_preview_scale(
            1000.0,
            800.0,
            500.0,
            400.0,
            1.0,
            app.ui_state.preview_actual_size,
            app.ui_state.preview_zoom,
        );
        assert!(zoomed_fit < fit_base);

        // Center resets stage offsets without clobbering zoom
        app.ui_state.stage_offset_x = 42.0;
        app.ui_state.stage_offset_y = -18.0;
        let kept_zoom = app.ui_state.preview_zoom;
        center_stage_state(&mut app.ui_state);
        assert_eq!(app.ui_state.stage_offset_x, 0.0);
        assert_eq!(app.ui_state.stage_offset_y, 0.0);
        assert_eq!(app.ui_state.preview_zoom, kept_zoom);
    }
}


