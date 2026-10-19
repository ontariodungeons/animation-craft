//! Left panel: Intake, Asset/Sheet Slicer, Turnaround cutouts, Layer/Part Outliner.
//!
//! All data is read from `Session.current_asset()` manifests — never hardcoded to
//! Hopper/Hardcoded paths. Every button dispatches a [`UiCommand`].

use crate::{AnimationCraftApp, CenterTab, UiCommand};
use egui::{Color32, Sense, Stroke, Ui, Vec2};

pub(crate) fn left_panel(app: &mut AnimationCraftApp, ui: &mut Ui) {
    egui::ScrollArea::vertical()
        .id_salt("left_scroll")
        .show(ui, |ui| {
            super::header::project_assets(app, ui);
            ui.heading("Intake & Attached Sheets");
            // Attached Sheets per Character (Zero Context Switching)
            let (active_char, asset_sheet_name, pose_count, turn_sheet_name, turn_count) = {
                let asset = app.session().current_asset();
                let id = app
                    .session()
                    .active_asset
                    .clone()
                    .unwrap_or_else(|| "none".to_string());
                let a_name = asset
                    .and_then(|a| a.attached_asset_sheet.as_ref())
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("none")
                    .to_string();
                let p_cnt = asset
                    .and_then(|a| a.pose_manifest.as_ref())
                    .map(|m| m.frames.len())
                    .unwrap_or(0);
                let t_name = asset
                    .and_then(|a| a.attached_turnaround_sheet.as_ref())
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("none")
                    .to_string();
                let t_cnt = asset
                    .and_then(|a| a.turnaround_manifest.as_ref())
                    .map(|m| m.frames.len())
                    .unwrap_or(0);
                (id, a_name, p_cnt, t_name, t_cnt)
            };

            ui.label(format!(
                "Active Character: {active_char} (Transparent BG Step 1)"
            ));
            ui.horizontal_wrapped(|ui| {
                let r_view_asset = ui.selectable_label(
                    app.ui_state.center_tab == CenterTab::AssetSheet,
                    format!("[Asset Sheet] ({pose_count} poses)"),
                );
                app.automation.add_response(
                    "btn.view_asset_sheet",
                    "button",
                    "View Asset Sheet Tab",
                    &r_view_asset,
                );
                if r_view_asset.clicked() {
                    app.ui_state_mut().center_tab = CenterTab::AssetSheet;
                }
                if ui.button("Import Asset Sheet…").clicked()
                    && let Some(file) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                {
                    let path_str = file.display().to_string();
                    let tolerance = app.ui_state.bg_tolerance;
                    app.ui_state_mut().image_path = Some(path_str.clone());
                    app.ui_state_mut().center_tab = CenterTab::AssetSheet;
                    app.exec_ui_command(UiCommand::IntakeValidate {
                        image_path: path_str,
                        slot: "asset_sheet".to_string(),
                        attach_to_active: true,
                        auto_cut: true,
                        tolerance,
                    });
                }
            });
            ui.small(format!("  -> {asset_sheet_name}"));

            ui.horizontal_wrapped(|ui| {
                let r_view_turn = ui.selectable_label(
                    app.ui_state.center_tab == CenterTab::TurnaroundSheet,
                    format!("[Turnaround Sheet] ({turn_count} views)"),
                );
                app.automation.add_response(
                    "btn.view_turnaround_sheet",
                    "button",
                    "View Turnaround Sheet Tab",
                    &r_view_turn,
                );
                if r_view_turn.clicked() {
                    app.ui_state_mut().center_tab = CenterTab::TurnaroundSheet;
                }
                if ui.button("Import Turnaround…").clicked()
                    && let Some(file) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                {
                    let path_str = file.display().to_string();
                    let tolerance = app.ui_state.bg_tolerance;
                    app.ui_state_mut().image_path = Some(path_str.clone());
                    app.ui_state_mut().center_tab = CenterTab::TurnaroundSheet;
                    app.exec_ui_command(UiCommand::IntakeValidate {
                        image_path: path_str,
                        slot: "turnaround_sheet".to_string(),
                        attach_to_active: true,
                        auto_cut: true,
                        tolerance,
                    });
                }
            });
            ui.small(format!("  -> {turn_sheet_name}"));

            ui.horizontal_wrapped(|ui| {
                if ui.button("+ New Character from Image…").clicked()
                    && let Some(file) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                {
                    let path_str = file.display().to_string();
                    let tolerance = app.ui_state.bg_tolerance;
                    app.ui_state_mut().image_path = Some(path_str.clone());
                    app.exec_ui_command(UiCommand::IntakeValidate {
                        image_path: path_str,
                        slot: "auto".to_string(),
                        attach_to_active: false,
                        auto_cut: true,
                        tolerance,
                    });
                }
                ui.small("or drag & drop PNG/JPG/WebP");
            });

            ui.separator();

            // Universal archetype buttons
            ui.horizontal_wrapped(|ui| {
                ui.label("Archetype:");
                let archetype_buttons = [
                    (
                        "Humanoid Biped",
                        "humanoid_biped",
                        "walk_jump_talk",
                        "head,ears,torso,arms,legs",
                    ),
                    (
                        "Axial Creature",
                        "axial_creature",
                        "swim_wave_fins",
                        "spine,fins,tail",
                    ),
                    (
                        "Radial Prop",
                        "radial_prop",
                        "rise_fall_pulse",
                        "core,rays,face",
                    ),
                    (
                        "Vehicle Hull",
                        "vehicle_hull",
                        "traverse_left_right_turn",
                        "hull,mast,rudder",
                    ),
                ];
                for (label, arch, q1, q2) in archetype_buttons {
                    let active = app.ui_state.archetype == arch;
                    let resp = ui.selectable_label(active, label);
                    app.automation.add_response(
                        format!("archetype.{arch}"),
                        "button",
                        label,
                        &resp,
                    );
                    if resp.clicked() {
                        app.ui_state_mut().archetype = arch.to_string();
                        app.ui_state_mut().q1_motion = q1.to_string();
                        app.ui_state_mut().q2_parts = q2.to_string();
                    }
                }
            });

            ui.checkbox(
                &mut app.ui_state_mut().strip_costume,
                "Strip costume to base mannequin",
            );

            ui.horizontal(|ui| {
                ui.label("BG Tolerance:");
                let tol = app.ui_state.bg_tolerance;
                ui.add(
                    egui::Slider::new(&mut app.ui_state_mut().bg_tolerance, 1..=100)
                        .text(format!("{tol}")),
                );
            });
            ui.checkbox(
                &mut app.ui_state_mut().use_ocr,
                "Local VLM / OCR Pose Auto-Label",
            );

            ui.separator();

            // Primary intake / pipeline buttons
            ui.horizontal_wrapped(|ui| {
                let r_val = ui.button("Validate Intake");
                app.automation.add_response(
                    "btn.validate_intake",
                    "button",
                    "Validate Intake",
                    &r_val,
                );
                if r_val.clicked() {
                    let path = app.ui_state.image_path.clone().unwrap_or_default();
                    let tolerance = app.ui_state.bg_tolerance;
                    app.exec_ui_command(UiCommand::IntakeValidate {
                        image_path: path,
                        slot: "auto".to_string(),
                        attach_to_active: true,
                        auto_cut: true,
                        tolerance,
                    });
                }
                let r_3q = ui.button("Ask Omarchy Agent (3Q)");
                app.automation.add_response(
                    "btn.ask_3q",
                    "button",
                    "Ask Omarchy Agent (3Q)",
                    &r_3q,
                );
                if r_3q.clicked() {
                    let st = app.ui_state.clone();
                    app.exec_ui_command(UiCommand::IntakeThreeQuestion {
                        archetype: st.archetype,
                        q1: st.q1_motion,
                        q2: st.q2_parts,
                        q3: st.q3_target,
                        strip_costume: st.strip_costume,
                    });
                }
                let r_cut = ui.button("Cut Sheet (PhotoCraft)");
                app.automation.add_response(
                    "btn.cut_sheet",
                    "button",
                    "Cut Sheet (PhotoCraft)",
                    &r_cut,
                );
                if r_cut.clicked() {
                    let active_id = app
                        .session()
                        .current_asset()
                        .map(|a| a.id.clone())
                        .unwrap_or_else(|| "current".to_string());
                    let tolerance = app.ui_state.bg_tolerance;
                    let use_ocr = app.ui_state.use_ocr;
                    app.exec_ui_command(UiCommand::AssetCutSheet {
                        asset_id: active_id,
                        tolerance,
                        use_ocr,
                    });
                }
                let r_reg = ui.button("0px Drift Register");
                app.automation.add_response(
                    "btn.drift_register",
                    "button",
                    "0px Drift Register",
                    &r_reg,
                );
                if r_reg.clicked() {
                    app.exec_ui_command(UiCommand::TurnaroundRegister);
                }
                let r_prompt = ui.button("Export Agent Prompt (.md)");
                app.automation.add_response(
                    "btn.export_prompt",
                    "button",
                    "Export Agent Prompt (.md)",
                    &r_prompt,
                );
                if r_prompt.clicked() {
                    let slug = app
                        .session()
                        .active_asset
                        .clone()
                        .unwrap_or_else(|| "hopper".to_string());
                    app.exec_ui_command(UiCommand::ExportAgentPrompt {
                        output: format!("exports/{slug}_agent_prompt.md"),
                    });
                }
            });

            ui.separator();

            // ========================================================================
            // Section 2: 5-View Turnaround Strip (dynamic from manifest)
            // ========================================================================
            ui.heading("2. Registered 5-View Turnaround");
            ui.separator();

            let ctx = ui.ctx().clone();
            let turnaround_frames = app
                .session()
                .current_asset()
                .and_then(|a| a.turnaround_manifest.as_ref())
                .map(|m| m.frames.clone());

            if let Some(frames) = &turnaround_frames {
                if frames.is_empty() {
                    ui.label("No turnaround frames in manifest.");
                } else {
                    ui.horizontal_wrapped(|ui| {
                        for frame in frames {
                            let label = &frame.label;
                            let deg = match label.as_str() {
                                "front" | "00_front" | "0° Front" => 0.0,
                                "quarter_front" | "45°" | "45° 3/4" => 45.0,
                                "side" | "90°" | "90° Side" => 90.0,
                                "opposite_quarter" | "135°" | "135° Back-3/4" => 135.0,
                                "back" | "180°" | "180° Back" => 180.0,
                                _ => 0.0,
                            };
                            let is_selected = (app.ui_state.yaw_deg - deg).abs() < 15.0;

                            let desired = Vec2::new(54.0, 68.0);
                            let (rect, response) = ui.allocate_at_least(desired, Sense::click());
                            if response.clicked() {
                                app.ui_state_mut().center_tab = CenterTab::ThreeD;
                                app.ui_state_mut().auto_turntable = false;
                                app.ui_state_mut().yaw_deg = deg;
                            }

                            let bg = if is_selected {
                                Color32::from_rgb(42, 62, 92)
                            } else {
                                Color32::from_rgb(28, 32, 42)
                            };
                            ui.painter().rect_filled(rect, 4.0, bg);
                            if is_selected {
                                ui.painter().rect_stroke(
                                    rect,
                                    4.0,
                                    Stroke::new(1.5, Color32::from_rgb(90, 190, 250)),
                                    egui::StrokeKind::Inside,
                                );
                            }

                            if let Some(tex) =
                                app.load_texture(&ctx, &frame.resolved_path.to_string_lossy())
                            {
                                let pad = rect.shrink(3.0);
                                ui.painter().image(
                                    tex.id(),
                                    pad,
                                    egui::Rect::from_min_max(
                                        egui::Pos2::ZERO,
                                        egui::Pos2::new(1.0, 1.0),
                                    ),
                                    Color32::WHITE,
                                );
                            }
                            ui.small(label);
                        }
                    });
                }
            } else {
                ui.label("No turnaround manifest loaded.");
                ui.small("Use Intake ▸ Validate Intake, then Cut Sheet, or load a preset.");
            }

            ui.separator();

            // ========================================================================
            // Section 3: Pose Sheet Cutouts (dynamic from manifest)
            // ========================================================================
            ui.heading("3. Pose Sheet Cutouts");
            ui.separator();

            let pose_frames = app
                .session()
                .current_asset()
                .and_then(|a| a.pose_manifest.as_ref())
                .map(|m| m.frames.clone());
            // Snapshot foot_lift values so we don't hold an immutable borrow of app
            // while mutating it inside the loop below.
            let foot_lifts: std::collections::HashMap<String, f32> = app
                .session()
                .current_asset()
                .and_then(|a| a.motion_range.as_ref())
                .map(|m| {
                    m.poses
                        .iter()
                        .map(|(label, pm)| (label.clone(), pm.foot_lift_px))
                        .collect()
                })
                .unwrap_or_default();

            if let Some(frames) = &pose_frames {
                if frames.is_empty() {
                    ui.label("No pose frames in manifest.");
                } else {
                    for (i, frame) in frames.iter().enumerate() {
                        let selected = app.ui_state.selected_pose_idx == i;
                        let foot_lift = foot_lifts.get(&frame.label).copied().unwrap_or(f32::NAN);

                        let label_text = if foot_lift.is_nan() {
                            format!("{} (lift: not measured)", frame.label)
                        } else {
                            format!("{} (lift {:.0}px)", frame.label, foot_lift)
                        };

                        if ui.selectable_label(selected, &label_text).clicked() {
                            app.ui_state_mut().selected_pose_idx = i;
                            app.ui_state_mut().playing = false;
                            app.ui_state_mut().center_tab = CenterTab::TwoD;
                            app.exec_ui_command(UiCommand::AnimPreview2d {
                                mode: frame.label.clone(),
                            });
                        }
                    }
                }
            } else {
                ui.label("No pose manifest loaded.");
                ui.small("Use Intake ▸ Cut Sheet (PhotoCraft) or load a preset.");
            }

            ui.separator();

            // ========================================================================
            // Section 4: Part Outliner (dynamic from asset.parts)
            // ========================================================================
            ui.heading("4. Part Outliner");
            ui.separator();

            // Outliner mode toggle + explode + pivots + topology numbers
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut app.ui_state_mut().outliner_collections_mode,
                    "Collections mode",
                );
                ui.checkbox(&mut app.ui_state_mut().show_topology_numbers, "Topology #");
            });
            let explode_pct = app.ui_state.explode_pct;
            ui.horizontal(|ui| {
                ui.label("Explode:");
                ui.add(
                    egui::Slider::new(&mut app.ui_state_mut().explode_pct, 0.0..=100.0)
                        .text(format!("{:.0}%", explode_pct)),
                );
            });
            ui.checkbox(
                &mut app.ui_state_mut().show_pivot_markers,
                "Joint Pivot Markers",
            );

            ui.separator();

            // Show All / Clear Solo + Verify All
            ui.horizontal_wrapped(|ui| {
                if ui.button("Show All / Clear Solo").clicked() {
                    app.exec_ui_command(UiCommand::PartShowAll);
                }
                let r_ver = ui.button("✓ Verify All Parts");
                app.automation.add_response(
                    "btn.verify_all_parts",
                    "button",
                    "Verify All Parts",
                    &r_ver,
                );
                if r_ver.clicked() {
                    app.exec_ui_command(UiCommand::SetPartState {
                        part: "*".to_string(),
                        visible: None,
                        solo: None,
                        verified: Some(true),
                    });
                }
            });

            ui.separator();

            // Per-part visible/solo/verified checkboxes
            let parts: Vec<animationcraft_engine::session::PartState> = app
                .session()
                .current_asset()
                .map(|a| a.parts.clone())
                .unwrap_or_default();

            if parts.is_empty() {
                ui.label("No parts available. Cut a sheet or load a manifest.");
            } else {
                egui::Grid::new("part_outliner_grid").show(ui, |ui| {
                    for (idx, part) in parts.iter().enumerate() {
                        let mut visible = part.visible;
                        let mut solo = part.solo;
                        let mut verified = part.verified;

                        let part_name = part.name.clone();
                        let response = ui
                            .checkbox(&mut visible, "")
                            .on_hover_text(format!("Toggle {part_name}"));
                        if response.changed() {
                            app.exec_ui_command(UiCommand::SetPartState {
                                part: part_name.clone(),
                                visible: Some(visible),
                                solo: None,
                                verified: None,
                            });
                        }

                        let solo_response = ui
                            .checkbox(&mut solo, "S")
                            .on_hover_text(format!("Solo {part_name}"));
                        if solo_response.changed() {
                            app.exec_ui_command(UiCommand::SetPartState {
                                part: part_name.clone(),
                                visible: None,
                                solo: Some(solo),
                                verified: None,
                            });
                        }

                        let ver_response = ui
                            .checkbox(&mut verified, "✓")
                            .on_hover_text(format!("Verify {part_name}"));
                        if ver_response.changed() {
                            app.exec_ui_command(UiCommand::SetPartState {
                                part: part_name.clone(),
                                visible: None,
                                solo: None,
                                verified: Some(verified),
                            });
                        }

                        ui.label(format!("#{} {}", idx + 1, part.name));
                        ui.end_row();
                    }
                });
            }
        });
}
