//! Transport, action, movement, and viewport zoom toolbar above the source/animation canvas.

use super::center_motion::{
    actual_size_viewport_state, center_stage_state, fit_viewport_state, sanitize_zoom,
    stop_motion_state, toggle_sprint_lock_state, trigger_jump_state, zoom_in_state, zoom_out_state,
};
use super::chrome;
use crate::{AnimMode, AnimationCraftApp, CenterTab, UiCommand};
use egui::{Color32, Rect, Stroke, Ui, Vec2};

#[derive(Clone, Copy)]
pub(super) enum ToolbarIcon {
    JumpArrow,
    RunChevrons,
    WasdCross,
    FitCorners,
    ActualOneToOne,
    ZoomMinus,
    ZoomPlus,
    CenterTarget,
}

pub(super) fn toolbar_icon_button(
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
pub(super) fn playback_bar(app: &mut AnimationCraftApp, ui: &mut Ui) {
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
                            app.exec_ui_command(UiCommand::Engine {
                                id: "anim.preview_2d",
                                params: serde_json::json!({ "mode": command }),
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
