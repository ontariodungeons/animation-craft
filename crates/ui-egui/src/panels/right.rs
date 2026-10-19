//! Right panel: Motion Range Controls, Verified Telemetry, Visemes/Sliders, and 1-Click Export.
//!
//! All silhouette motion range and registration drift values are read strictly from
//! `app.session().current_asset()`. Unmeasured fields display `"not measured"`.

use super::chrome::{self, help};
use crate::{AnimMode, AnimationCraftApp, UiCommand};
use egui::{CollapsingHeader, Color32, RichText, Ui};

#[path = "inspector_widgets.rs"]
mod widgets;

// Inspector accents: labels carry meaning even when colors cannot be distinguished.
const MOTION: Color32 = Color32::from_rgb(112, 196, 255);
const POSE: Color32 = Color32::from_rgb(195, 164, 255);
const MESH: Color32 = Color32::from_rgb(240, 190, 112);
const EXPORT: Color32 = Color32::from_rgb(100, 215, 194);
const READY: Color32 = Color32::from_rgb(128, 218, 154);

fn slider<T: egui::emath::Numeric>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    suffix: &str,
    explanation: (&str, &str, &str),
) {
    let (what, why, how) = explanation;
    help(ui.label(label), what, why, how);
    // Keep labels above controls: long names must not disappear off a narrow inspector.
    ui.spacing_mut().slider_width = (ui.available_width() - 66.0).max(32.0);
    help(
        ui.add(egui::Slider::new(value, range).suffix(suffix)),
        what,
        why,
        how,
    );
    ui.add_space(4.0);
}

pub(crate) fn right_panel(app: &mut AnimationCraftApp, ui: &mut Ui) {
    egui::ScrollArea::vertical()
        .id_salt("right_scroll")
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.heading("Animation inspector");
            ui.add(egui::Label::new(RichText::new("Hover any control to learn what, why & how.").small().weak()).wrap());
            ui.separator();

            // --- 1. Action Mode selector ---
            CollapsingHeader::new(RichText::new("1 · Choose an action").color(MOTION).strong())
                .id_salt("1. Action Mode (2D & 3D)")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (mode, label) in [
                            (AnimMode::Idle, "Idle"),
                            (AnimMode::Walk, "Walk"),
                            (AnimMode::Run, "Run"),
                            (AnimMode::Jump, "Jump"),
                            (AnimMode::Wave, "Wave"),
                            (AnimMode::LookScout, "Scout"),
                            (AnimMode::Static, "Static"),
                        ] {
                            let resp = ui.selectable_value(
                                &mut app.ui_state_mut().anim_mode,
                                mode,
                                label,
                            );
                            let resp = help(resp,
                                &format!("Select the {label} action for the viewport preview."),
                                "Compare motion and silhouette before choosing an animation to export.",
                                "Click an action, then adjust Speed and Motion amount below. Available artwork and rig determine the visible result; export may use a separate animation.");
                            app.automation.add_response(
                                format!("mode.{}", label.to_ascii_lowercase()),
                                "button",
                                label,
                                &resp,
                            );
                            if resp.clicked() {
                                app.exec_ui_command(UiCommand::AnimPreview2d {
                                    mode: label.to_string(),
                                });
                            }
                        }
                    });
                });

            ui.separator();

            // --- 2. Pose Motion Range & Verified Telemetry ---
            CollapsingHeader::new(RichText::new("2 · Motion & timing").color(MOTION).strong())
                .id_salt("2. Pose Motion Range (0–100%)")
                .default_open(true)
                .show(ui, |ui| {
                    slider(ui, "Playback speed", &mut app.ui_state.anim_speed, 0.25..=2.25, "×",
                        ("Changes how quickly the preview plays.", "Slow down to inspect joints; speed up to judge the rhythm.", "Drag the slider or edit its number. 1× is normal speed; 0.5× is half speed. This is a preview setting."));
                    slider(ui, "Motion amount", &mut app.ui_state.pose_range_pct, 0.0..=100.0, "%",
                        ("Scales the preview motion and displayed motion estimates.", "Use a smaller amount to inspect subtle movement and reduce exaggerated offsets.", "Start at 35%, then increase while watching the artwork. 100% is the configured range, not a guarantee that the rig is safe."));

                    ui.horizontal_wrapped(|ui| {
                        for (label, pct) in [
                            ("0% Still", 0.0),
                            ("35% Subtle", 35.0),
                            ("70% Broad", 70.0),
                            ("100% Full", 100.0),
                        ] {
                            if help(ui.small_button(label),
                                &format!("Set motion amount to {pct:.0}%."),
                                "Quickly compare the same action at different strengths.",
                                "Click a preset, then fine-tune Motion amount above.").clicked() {
                                app.ui_state_mut().pose_range_pct = pct;
                            }
                        }
                    });

                    let scale = (app.ui_state.pose_range_pct / 100.0).clamp(0.0, 1.0);
                    let asset = app.session().current_asset();
                    let motion_opt = asset.and_then(|a| a.motion_range.as_ref());
                    let reg_opt = asset.and_then(|a| a.turnaround_registration.as_ref());

                    ui.add_space(4.0);
                    help(ui.colored_label(MOTION, "Motion measurements"),
                        "Values loaded from the asset's motion and registration reports.",
                        "Check how far the silhouette moves and whether views stay aligned.",
                        "Lift, width change and side reach below are scaled estimates at the current motion amount, not live measurements of each rendered frame.");
                    if let Some(mr) = motion_opt {
                        ui.label(RichText::new(format!("Estimates at {:.0}% motion", scale * 100.0)).small().weak());
                        let foot_samples: Vec<_> = mr.poses.keys().filter_map(|name| mr.raw["poses"][name.as_str()]["metrics"]["footLiftPx"].as_f64().map(|value| (name.as_str(), value as f32))).collect();
                        let width_samples: Vec<_> = mr.poses.keys().filter_map(|name| mr.raw["poses"][name.as_str()]["deltaFromStill"]["widthPx"].as_f64().map(|value| (name.as_str(), value as f32))).collect();
                        let lift = mr.raw["limits"]["footLiftPx"]["max"].as_f64();
                        let width = mr.raw["limits"]["widthPx"]["maxDeltaFromStill"].as_f64();
                        let lift_value = lift.map(|v| format!("{:.1}", v * scale as f64)).unwrap_or_else(|| "—".into());
                        let width_value = width.map(|v| format!("{:.1}", v * scale as f64)).unwrap_or_else(|| "—".into());
                        let lift_caption = lift.map(|v| format!("Peak {v:.1} px · {}", mr.foot_lift_max_pose)).unwrap_or_else(|| "Peak lift not measured".into());
                        let width_caption = width.map(|v| format!("Peak {v:.1} px · {}", mr.width_max_pose)).unwrap_or_else(|| "Peak width change not measured".into());
                        widgets::metric_card(ui, "Foot lift", &lift_value, "px",
                            &lift_caption, &foot_samples, MOTION,
                            ("Estimated foot lift at the selected motion amount. Bars show original measurements for each pose.", "Judge foot clearance and compare the pose references.", "Change Motion amount to scale the estimate. Hover a bar for its pose and measured value; the chart is not a playback timeline."));
                        widgets::metric_card(ui, "Width change", &width_value, "px",
                            &width_caption, &width_samples, POSE,
                            ("Estimated change from the still silhouette width. Bars show recorded changes by pose.", "Allow enough space around the character for wider poses.", "Negative bars are narrower than the reference; positive bars are wider. Hover to read a pose measurement."));
                        help(ui.label(format!("Side reach: {:.1} / {:+.1} px", mr.left_min_delta_px * scale, mr.right_max_delta_px * scale)),
                            "Scaled horizontal reach relative to the reference silhouette.", "Keep movement inside the stage framing.", "Left values extend left; right values extend right. Adjust Motion amount to compare.");
                        help(ui.label(format!("Reference height: {:.0} px", mr.still_height_px)),
                            &format!("Height of the still reference: {}.", mr.still_label), "Compare the scale of poses.", "This is a recorded reference value; it does not change with Motion amount.");
                    } else {
                        widgets::metric_card(ui, "Motion measurements", "—", "",
                            "No motion report loaded for this asset.", &[], MOTION,
                            ("No measurement data is available.", "Unknown motion must not be mistaken for zero movement.", "Load or generate a motion report before comparing pose measurements."));
                    }
                    let drift = reg_opt.map(|r| format!("Alignment drift: {:.1} px", r.max_vertical_drift_px))
                        .unwrap_or_else(|| "Alignment drift: Not measured".into());
                    help(ui.label(drift), "Largest vertical alignment difference between registered views.",
                        "Large differences can make a character jump vertically when changing views.", "Run turnaround registration, then review its measured drift. Lower is better.");
                });

            ui.separator();

            // --- 3. Mouth Visemes & Pose Sliders (from Web App) ---
            CollapsingHeader::new(RichText::new("3 · Expression & pose").color(POSE).strong())
                .id_salt("3. Visemes & Pose Sliders")
                .default_open(false)
                .show(ui, |ui| { expression_controls(app, ui);
                });

            ui.separator();

            // --- 3b. 3D Volumetric Reconstruction Controls (from Web App) ---
            CollapsingHeader::new(RichText::new("4 · 3D shape").color(MESH).strong())
                .id_salt("3b. 3D Reconstruction & Multi-View")
                .default_open(false)
                .show(ui, |ui| { shape_controls(app, ui);
                });

            ui.separator();

            // --- 4. Verified Asset & QA Status ---
            CollapsingHeader::new(RichText::new("5 · Quality checks").color(READY).strong())
                .id_salt("4. Asset & Registration QA")
                .default_open(true)
                .show(ui, |ui| { quality_cards(app, ui);
                });

            ui.separator();

            // --- 5. 1-Click Pipeline Export ---
            CollapsingHeader::new(RichText::new("6 · Export & integrations").color(EXPORT).strong())
                .id_salt("5. 1-Click Pipeline Export")
                .default_open(true)
                .show(ui, |ui| {
                    let slug = app
                        .session()
                        .active_asset
                        .clone()
                        .unwrap_or_else(|| "hopper".to_string());
                    let r_iter = widgets::action_card(ui, "Action reel", "Create a review package of layers and six actions.", "Preset workflow · MP4 + layers", "Create action reel", app.ui_state.busy_command.is_none(), ("Runs the character-iteration pipeline to create layers and an action reel.", "Share a review package of supported character actions.", "Uses the selected character ID. This preset-oriented pipeline may not support a newly imported character."));
                    app.automation.add_response(
                        "export.iteration",
                        "button",
                        "Export 2D Layers + 6-Action Track",
                        &r_iter,
                    );
                    if r_iter.clicked() {
                        let out = format!("exports/design_share/{slug}/{slug}_6actions_track.mp4");
                        app.ui_state.last_export = Some(out);
                        app.exec_ui_command(UiCommand::ExportCharacterIteration {
                            character: slug.clone(),
                        });
                    }
                    let r_ec = widgets::action_card(ui, "EffectCraft", "Build a composition from an existing script.", "Requires a prepared JSX script", "Run script", app.ui_state.busy_command.is_none(), ("Executes the existing EffectCraft JSX script.", "Build the composition described by that script in EffectCraft.", "Requires exports/effectcraft_demo/build_scene.jsx. It does not generate a new script from the current preview."));
                    app.automation.add_response(
                        "export.effectcraft",
                        "button",
                        "Export EffectCraft (.jsx)",
                        &r_ec,
                    );
                    if r_ec.clicked() {
                        let out = "exports/effectcraft_demo/build_scene.jsx".to_string();
                        app.ui_state.last_export = Some(out.clone());
                        app.exec_ui_command(UiCommand::ExportEffectcraft { output: out });
                    }
                    let active_layered_spec = app
                        .session()
                        .current_asset()
                        .and_then(|a| a.cutout_rig.as_ref())
                        .filter(|r| r.rig_type == "layered_motion")
                        .map(|r| r.plan_path.to_string_lossy().into_owned());
                    let r_lm = widgets::action_card(
                        ui,
                        "Layered motion",
                        "Render transparent WebM tracks from a layered motion spec.",
                        if active_layered_spec.is_some() {
                            "Active layered spec · WebM RGBA per track"
                        } else {
                            "Choose spec JSON · WebM RGBA per track"
                        },
                        "Render layered motion",
                        app.ui_state.busy_command.is_none(),
                        (
                            "Builds an EffectCraft project from a generic layered motion spec and renders transparent WebM tracks.",
                            "Animate any multi-layer rig (oscillate, ellipse, keys, follow) without fixed biped part assumptions.",
                            "Uses the active asset's layered motion spec if loaded, or prompts for a spec JSON file.",
                        ),
                    );
                    app.automation.add_response(
                        "export.layered_motion",
                        "button",
                        "Export Layered Motion (.webm)",
                        &r_lm,
                    );
                    if r_lm.clicked() {
                        let chosen_spec = active_layered_spec.clone().or_else(|| {
                            rfd::FileDialog::new()
                                .add_filter("Layered Motion Spec", &["json"])
                                .pick_file()
                                .map(|p| p.to_string_lossy().into_owned())
                        });
                        if let Some(spec_path) = chosen_spec {
                            app.ui_state.last_export = Some(spec_path.clone());
                            app.exec_ui_command(UiCommand::ExportLayeredMotion {
                                spec: spec_path,
                                out_dir: None,
                            });
                        }
                    }
                    let recent_clip = app
                        .ui_state
                        .last_motion_validation
                        .as_ref()
                        .and_then(|v| {
                            v.get("video")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned)
                                .or_else(|| {
                                    v.get("tracks")
                                        .and_then(serde_json::Value::as_array)
                                        .and_then(|arr| arr.first())
                                        .and_then(|t| t.get("video"))
                                        .and_then(serde_json::Value::as_str)
                                        .map(str::to_owned)
                                })
                        });
                    let r_vm = widgets::action_card(
                        ui,
                        "Validate motion",
                        "Check alpha, clipping, teleports, flicker, loop closure and reference fidelity.",
                        if recent_clip.is_some() {
                            "Re-validate latest rendered WebM clip"
                        } else {
                            "Choose rendered WebM clip · programmatic checks"
                        },
                        "Validate clip",
                        app.ui_state.busy_command.is_none(),
                        (
                            "Runs programmatic motion validation on a rendered transparent WebM clip.",
                            "Verify alpha channel, empty/clipped frames, teleports, flicker, loop closure, colour/size fidelity vs reference, and Nyquist aliasing.",
                            "Uses the most recent rendered clip or prompts for a .webm file.",
                        ),
                    );
                    app.automation.add_response(
                        "validate.motion",
                        "button",
                        "Validate Motion Clip (.webm)",
                        &r_vm,
                    );
                    if r_vm.clicked() {
                        let chosen_video = recent_clip.or_else(|| {
                            rfd::FileDialog::new()
                                .add_filter("Rendered Clip", &["webm", "mp4"])
                                .pick_file()
                                .map(|p| p.to_string_lossy().into_owned())
                        });
                        if let Some(video_path) = chosen_video {
                            let reference = app
                                .session()
                                .current_asset()
                                .filter(|a| a.path.is_file())
                                .map(|a| a.path.to_string_lossy().into_owned());
                            app.exec_ui_command(UiCommand::ValidateMotion {
                                video: video_path,
                                reference,
                                spec: active_layered_spec.clone(),
                                loop_clip: false,
                            });
                        }
                    }
                    let r_fc = widgets::action_card(ui, "FilmCraft", "Open rendered video in an editing project.", "Currently uses the default Hopper video", "Import video", app.ui_state.busy_command.is_none(), ("Imports media into a FilmCraft project.", "Continue editing a rendered animation on a timeline.", "This button currently uses the pipeline default Hopper video. For other media, use export.filmcraft with an explicit media path."));
                    app.automation.add_response(
                        "export.filmcraft",
                        "button",
                        "Export FilmCraft (.fcproj)",
                        &r_fc,
                    );
                    if r_fc.clicked() {
                        let out = format!("exports/{slug}_filmcraft.fcproj");
                        app.ui_state.last_export = Some(out.clone());
                        app.exec_ui_command(UiCommand::ExportFilmcraft { output: out });
                    }
                    let r_glb = widgets::action_card(ui, "3D mesh", "Build a volume from the available image views.", "GLB + OBJ · volume only", "Build mesh", app.ui_state.busy_command.is_none(), ("Builds a volume mesh and writes GLB and OBJ files.", "Move the generated shape into a 3D editor.", "Adjust 3D shape settings, then click. Missing views may reuse the first image; neither selection currently creates an articulated rig."));
                    app.automation.add_response(
                        "export.glb",
                        "button",
                        "Build 4-Direction .GLB",
                        &r_glb,
                    );
                    if r_glb.clicked() {
                        let st = app.ui_state.clone();
                        let out = format!("exports/blender_ingest/{slug}/{slug}_4dir.glb");
                        app.ui_state.last_export = Some(out);
                        app.exec_ui_command(UiCommand::MeshBuild4Dir {
                            recon_mode: st.recon_mode,
                            inflation_depth_pct: st.inflation_depth_pct,
                            relief_detail_pct: st.relief_detail_pct,
                            mesh_res: st.mesh_res,
                            proj_blend_pct: st.proj_blend_pct as u32,
                            bump_intensity: st.bump_strength as u32,
                        });
                    }
                    let r_blend = widgets::action_card(ui, "Blender", "Render animation through the connected Blender app.", "Requires a live Blender connection", "Render animation", app.ui_state.busy_command.is_none(), ("Runs the Blender character pipeline through its live connection.", "Render a character animation in Blender.", "Requires the Blender service on port 9876. The current pipeline uses character IDs; verify source identity for custom imports before running."));
                    app.automation.add_response(
                        "export.blender",
                        "button",
                        "Sync Blender 5.3 (:9876)",
                        &r_blend,
                    );
                    if r_blend.clicked() {
                        let out = format!("exports/blender_characters/{slug}/{slug}_blender53.mp4");
                        app.ui_state.last_export = Some(out.clone());
                        app.exec_ui_command(UiCommand::ExportBlender {
                            output: out,
                            script: "pipeline/blender53_bridge.py".to_string(),
                        });
                    }

                    ui.separator();
                    if let Some(path) = &app.ui_state.last_export {
                        help(ui.add(egui::Label::new(format!("Requested output: {path}")).wrap()), "The most recently requested output location.", "Find the intended export destination.", "Check the command result and file before treating this path as a completed export.");
                    }
                    if let Some(report) = app.ui_state.last_motion_validation.clone() {
                        ui.add_space(4.0);
                        motion_validation_card(app, ui, &report);
                    }
                });
        });
}

fn motion_validation_card(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    report: &serde_json::Value,
) {
    let overall_ok = report
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let badge_color = if overall_ok {
        egui::Color32::from_rgb(76, 209, 146)
    } else {
        egui::Color32::from_rgb(242, 109, 109)
    };
    let rect = chrome::card()
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Motion validation checks")
                        .strong()
                        .color(EXPORT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(if overall_ok { "PASS" } else { "ISSUES" })
                            .small()
                            .strong()
                            .color(badge_color),
                    );
                });
            });

            if let Some(tracks) = report.get("tracks").and_then(serde_json::Value::as_array) {
                for track in tracks {
                    let tid = track
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("track");
                    if let Some(val) = track.get("validation") {
                        render_validation_entry(ui, tid, val);
                    }
                }
            } else {
                let label = report
                    .get("video")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("clip");
                render_validation_entry(ui, label, report);
            }

            if let Some(lint) = report.get("spec_lint") {
                let issues = lint
                    .as_array()
                    .or_else(|| lint.get("issues").and_then(serde_json::Value::as_array));
                if let Some(issues) = issues
                    && !issues.is_empty()
                {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("Spec lint: {} aliasing warning(s)", issues.len()))
                            .small()
                            .color(egui::Color32::from_rgb(245, 184, 81)),
                    );
                }
            }
        })
        .response
        .rect;
    app.automation
        .add("panel.motion_validation", "panel", "Motion Validation Checks", rect);
}

fn render_validation_entry(ui: &mut Ui, title: &str, val: &serde_json::Value) {
    let ok = val
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let status_color = if ok {
        egui::Color32::from_rgb(76, 209, 146)
    } else {
        egui::Color32::from_rgb(242, 109, 109)
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(if ok { "✓" } else { "✗" })
                .strong()
                .color(status_color),
        );
        ui.label(RichText::new(title).small().strong());
        if let Some(metrics) = val.get("metrics") {
            let cov = metrics
                .get("coverage_pct")
                .and_then(serde_json::Value::as_array)
                .and_then(|a| a.first())
                .and_then(serde_json::Value::as_f64);
            let step = metrics
                .get("max_step_px")
                .and_then(serde_json::Value::as_f64);
            if let (Some(c), Some(s)) = (cov, step) {
                ui.label(
                    RichText::new(format!("cov {c:.1}% · step {s:.1}px"))
                        .small()
                        .color(chrome::MUTED),
                );
            }
        }
    });
    if let Some(checks) = val.get("checks").and_then(serde_json::Value::as_object) {
        ui.horizontal_wrapped(|ui| {
            for (name, check) in checks {
                let c_ok = check
                    .get("ok")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let col = if c_ok {
                    egui::Color32::from_rgb(76, 209, 146)
                } else {
                    egui::Color32::from_rgb(242, 109, 109)
                };
                let short = name.replace('_', " ");
                ui.label(
                    RichText::new(format!("{} {short}", if c_ok { "✓" } else { "✗" }))
                        .small()
                        .color(col),
                );
            }
        });
    } else if let Some(failed) = val.get("failed").and_then(serde_json::Value::as_array)
        && !failed.is_empty()
    {
        for f in failed {
            if let Some(s) = f.as_str() {
                ui.label(
                    RichText::new(format!("✗ {s}"))
                        .small()
                        .color(egui::Color32::from_rgb(242, 109, 109)),
                );
            }
        }
    }
}

fn expression_controls(app: &mut AnimationCraftApp, ui: &mut Ui) {
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Pose starting points").strong().color(POSE));
        ui.add(
            egui::Label::new(
                RichText::new("Choose a starting pose, then fine-tune below.")
                    .small()
                    .color(chrome::MUTED),
            )
            .wrap(),
        );
        ui.horizontal_wrapped(|ui| {
            for (label, id, tip) in [
                (
                    "A pose",
                    "a_pose",
                    "Reset the pose and spread the arms to 35.",
                ),
                (
                    "Hop",
                    "hop",
                    "Set knees to 30, elbows to 25 and ear tilt to −15.",
                ),
                ("Alert", "alert", "Set ear spread to 20 and blink to 80."),
            ] {
                let response = help(
                    chrome::button(ui, label, false),
                    tip,
                    "Explore a pose starting point.",
                    "Click, then adjust the values below. Supported rigs can respond differently.",
                );
                app.automation.add_response(
                    format!("pose.preset.{id}"),
                    "button",
                    label,
                    &response,
                );
                if response.clicked() {
                    match id {
                        "a_pose" => {
                            app.ui_state.reset_pose_sliders();
                            app.ui_state.arm_spread = 35.0;
                        }
                        "hop" => {
                            app.ui_state.knee_bend = 30.0;
                            app.ui_state.elbow_bend = 25.0;
                            app.ui_state.ear_pitch = -15.0;
                        }
                        _ => {
                            app.ui_state.ear_splay = 20.0;
                            app.ui_state.eye_blink = 80.0;
                        }
                    }
                }
            }
        });
        let response = help(
            chrome::button(ui, "Reset pose", false),
            "Restore the default pose, part separation and stage offset.",
            "Return to a neutral starting point.",
            "Click to reset the preview adjustments. The source artwork is unchanged.",
        );
        app.automation
            .add_response("pose.reset", "button", "Reset pose", &response);
        if response.clicked() {
            app.ui_state.reset_pose_sliders();
        }
    });
    ui.add_space(5.0);
    chrome::card().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("Mouth expression").strong().color(POSE));
                        ui.add(egui::Label::new(RichText::new("PhotoCraft facial preview · requires a detected face").small().color(chrome::MUTED)).wrap());
                        ui.horizontal_wrapped(|ui| {
                            for (id, label) in [("smile", "Smile"), ("laugh", "Laugh"), ("ooh", "Ooh"), ("mbp", "M/B/P"), ("talk", "Talk")] {
                                let response = help(chrome::tab(ui, label, app.ui_state.mouth_viseme == id),
                                    "Deform the detected mouth using PhotoCraft.", "Preview the expression on the actual artwork.",
                                    "Click to render a preview. Talk loops mouth deformation; it is not audio-driven lip-sync. Review the detected face before export.");
                                app.automation.add_response(format!("pose.mouth.{id}"), "button", label, &response);
                                if response.clicked() { app.ui_state.mouth_viseme = id.into(); app.ui_state.center_tab = crate::CenterTab::TwoD; if id == "talk" { app.ui_state.playing = true; } }
                            }
                        });
                        if !app.face_preview.status.is_empty() { ui.add(egui::Label::new(egui::RichText::new(&app.face_preview.status).small()).wrap()); }
                    });
    ui.add_space(5.0);
    chrome::card().show(ui, |ui| {
ui.set_width(ui.available_width());
ui.label(RichText::new("Face & head").strong().color(POSE));
ui.add(egui::Label::new(RichText::new("Drag a slider to shape the expression.").small().color(chrome::MUTED)).wrap());
let response = chrome::slider_row(ui, "Eye blink", &mut app.ui_state.eye_blink, 0.0..=100.0, "%", POSE,
                        ("Controls the blink-related preview offset.", "Check the head pose during an expressive moment.", "Drag from 0 to 100. The current layered preview shifts the head; this does not close painted eyelids."));
if response.changed() { app.ui_state.center_tab = crate::CenterTab::TwoD; }
app.automation.add_response("pose.eye_blink", "slider", "Eye blink", &response);
let response = chrome::slider_row(ui, "Eye gaze", &mut app.ui_state.eye_gaze, -45.0..=45.0, "", POSE,
                        ("Controls a gaze-related horizontal head offset.", "Suggest a change of attention while composing a pose.", "Try small positive or negative values. This does not redraw eyes or move pupils independently."));
if response.changed() { app.ui_state.center_tab = crate::CenterTab::TwoD; }
app.automation.add_response("pose.eye_gaze", "slider", "Eye gaze", &response);
let response = chrome::slider_row(ui, "Mouth opening", &mut app.ui_state.smile_open, 0.0..=100.0, "%", POSE,
                        ("Deforms the mouth region through PhotoCraft Liquify.", "Adjust expression opening while preserving the original file.", "20% preserves the source opening. Release the slider to render; inspect the result because detected landmarks can need correction."));
if response.changed() { app.ui_state.center_tab = crate::CenterTab::TwoD; }
app.automation.add_response("pose.smile_open", "slider", "Mouth opening", &response);
let response = chrome::slider_row(ui, "Head turn", &mut app.ui_state.head_yaw, -45.0..=45.0, "", POSE,
                        ("Adjusts head placement or rotation in supported preview paths.", "Suggest a look to the side and check neck overlap.", "Drag either side of zero. It cannot reveal artwork hidden behind the head."));
app.automation.add_response("pose.head_yaw", "slider", "Head turn", &response);
let response = chrome::slider_row(ui, "Head tilt", &mut app.ui_state.head_pitch, -45.0..=45.0, "", POSE,
                        ("Changes the vertical head offset in the layered preview.", "Explore the head position relative to the body.", "Use small values while watching the neck. This does not reconstruct an up or down view."));
app.automation.add_response("pose.head_pitch", "slider", "Head tilt", &response);
});
    ui.add_space(5.0);
    chrome::card().show(ui, |ui| {
ui.set_width(ui.available_width());
ui.label(RichText::new("Body & ears").strong().color(POSE));
ui.add(egui::Label::new(RichText::new("Preview adjustments; support varies by rig.").small().color(chrome::MUTED)).wrap());
let response = chrome::slider_row(ui, "Arm spread", &mut app.ui_state.arm_spread, -45.0..=90.0, "", POSE,
                        ("Moves the arms outward or changes their rotation in supported preview actions.", "Check how far the arms can move without exposing gaps at the shoulders.", "Start near zero and increase gradually; A-Pose sets this to 35."));
app.automation.add_response("pose.arm_spread", "slider", "Arm spread", &response);
let response = chrome::slider_row(ui, "Elbow bend", &mut app.ui_state.elbow_bend, -45.0..=90.0, "", POSE,
                        ("Adjusts arm placement in the layered preview.", "Check arm spacing and overlap with the torso.", "Try a small value first. It is not a separate elbow joint solver and may not affect every rig."));
app.automation.add_response("pose.elbow_bend", "slider", "Elbow bend", &response);
let response = chrome::slider_row(ui, "Knee bend", &mut app.ui_state.knee_bend, -45.0..=90.0, "", POSE,
                        ("Offsets the legs in the current preview; some cutout actions also use it in leg rotation.", "Explore a crouched or lifted-leg pose and inspect foot placement.", "Increase gradually while watching both feet. Negative values move the adjustment in the opposite direction."));
app.automation.add_response("pose.knee_bend", "slider", "Knee bend", &response);
let response = chrome::slider_row(ui, "Ear spread", &mut app.ui_state.ear_splay, -45.0..=45.0, "", POSE,
                        ("Adjusts ear spacing or rotation, depending on the active preview rig.", "Make an expression more alert and inspect overlap near the head.", "Drag either side of zero and watch the ear bases. Reset Pose restores the default."));
app.automation.add_response("pose.ear_splay", "slider", "Ear spread", &response);
let response = chrome::slider_row(ui, "Ear tilt", &mut app.ui_state.ear_pitch, -45.0..=45.0, "", POSE,
                        ("Changes the vertical ear offset in the layered preview.", "Explore the silhouette of raised or lowered ears.", "Use small positive or negative values. This is an approximate offset, not a full 3D ear rotation."));
app.automation.add_response("pose.ear_pitch", "slider", "Ear tilt", &response);
});
    ui.add_space(5.0);
}

fn shape_controls(app: &mut AnimationCraftApp, ui: &mut Ui) {
    chrome::card().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("Reconstruction").strong().color(MESH));
                        ui.horizontal_wrapped(|ui| {
                            for (id, label) in [("volumetric", "Volume"), ("rig", "Rig")] {
                                let response = help(chrome::tab(ui, label, app.ui_state.recon_mode == id),
                                    "Select the reconstruction mode stored in the project.", "Describe the intended construction method.",
                                    "Both selections currently use the volume builder. Rig mode does not produce an articulated mesh yet.");
                                app.automation.add_response(format!("shape.mode.{id}"), "button", label, &response);
                                if response.clicked() { app.ui_state.recon_mode = id.into(); }
                            }
                        });
                        ui.add(egui::Label::new(RichText::new("Rig mode currently uses the volume builder.").small().color(chrome::MUTED)).wrap());
                    });
    ui.add_space(5.0);
    chrome::card().show(ui, |ui| {
ui.set_width(ui.available_width());
ui.label(RichText::new("Form & resolution").strong().color(MESH));
ui.add(egui::Label::new(RichText::new("Drag a value, or click to enter an exact amount.").small().color(chrome::MUTED)).wrap());
let response = chrome::number_row(ui, "Depth", &mut app.ui_state.inflation_depth_pct, 10..=100, "%", MESH, ("Controls how thick the generated volume is.", "Give a flat silhouette depth without making it excessively round.", "Start near 58%. Compare the side view and rebuild the mesh after adjusting."));
app.automation.add_response("shape.inflation_depth_pct", "number", "Depth", &response);
let response = chrome::number_row(ui, "Mesh detail", &mut app.ui_state.mesh_res, 12..=64, "", MESH, ("Controls the number of height slices and radial segments.", "Higher values create a smoother mesh but increase geometry and processing cost.", "Start at 32. Raise only if the exported silhouette looks faceted; rebuild to apply."));
app.automation.add_response("shape.mesh_res", "number", "Mesh detail", &response);
let response = chrome::number_row(ui, "View blending", &mut app.ui_state.proj_blend_pct, 0.0..=100.0, "%", MESH, ("Weights the front and quarter-view silhouette spans used during mesh generation.", "Adjust how much the quarter view contributes to the shape.", "Compare low and high values on a rebuilt mesh. This does not generate missing views."));
app.automation.add_response("shape.proj_blend_pct", "number", "View blending", &response);
});
    ui.add_space(5.0);
    chrome::card().show(ui, |ui| {
ui.set_width(ui.available_width());
ui.label(RichText::new("Surface").strong().color(MESH));
ui.add(egui::Label::new(RichText::new("Generated relief, not recovered image detail.").small().color(chrome::MUTED)).wrap());
let response = chrome::number_row(ui, "Surface relief", &mut app.ui_state.relief_detail_pct, 0..=100, "%", MESH, ("Adds procedural surface variation to the volume.", "Break up a perfectly smooth surface.", "Start low and increase gradually. This is generated relief, not detail recovered from the artwork."));
app.automation.add_response("shape.relief_detail_pct", "number", "Surface relief", &response);
let response = chrome::number_row(ui, "Surface bump", &mut app.ui_state.bump_strength, 0.0..=100.0, "%", MESH, ("Adds to the procedural relief amplitude during mesh export.", "Introduce more surface variation when a smooth result looks too plain.", "Use small values first. This is geometric variation, not a physically based felt material."));
app.automation.add_response("shape.bump_strength", "number", "Surface bump", &response);
});
    ui.add_space(5.0);
    ui.add(
        egui::Label::new(
            RichText::new("Use Build mesh in Export to apply these settings to a new file.")
                .small()
                .color(chrome::MUTED),
        )
        .wrap(),
    );
}

fn quality_cards(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let asset = app.session().current_asset();
    let turnaround_count = asset
        .and_then(|a| a.turnaround_manifest.as_ref())
        .map(|m| m.frames.len());
    let pose_count = asset
        .and_then(|a| a.pose_manifest.as_ref())
        .map(|m| m.frames.len());
    let part_count = asset.map(|a| a.parts.len()).unwrap_or(0);
    let verified_count = asset
        .map(|a| a.parts.iter().filter(|p| p.verified).count())
        .unwrap_or(0);
    let reg_passed = asset
        .and_then(|a| a.turnaround_registration.as_ref())
        .map(|r| r.registration_passed);

    chrome::card().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("View alignment").strong().color(chrome::TEXT));
                        let (label, tone) = match reg_passed { Some(true) => ("Passed", READY), Some(false) => ("Needs review", MESH), None => ("Not measured", chrome::MUTED) };
                        chrome::badge(ui, label, tone).on_hover_text("Result recorded in the loaded registration report. This does not certify the entire rig.");
                        if let Some(reg) = asset.and_then(|a| a.turnaround_registration.as_ref()) {
                            ui.label(RichText::new(format!("{:.1} px drift", reg.max_vertical_drift_px)).size(20.0).color(chrome::TEXT));
                        }
                        ui.add(egui::Label::new(RichText::new("Alignment between reference views.").small().color(chrome::MUTED)).wrap());
                    });
    ui.add_space(5.0);
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new("Artwork & review")
                .strong()
                .color(chrome::TEXT),
        );
        ui.horizontal_wrapped(|ui| {
            chrome::badge(
                ui,
                &turnaround_count
                    .map(|n| format!("{n} views"))
                    .unwrap_or_else(|| "No views".into()),
                MOTION,
            );
            chrome::badge(
                ui,
                &pose_count
                    .map(|n| format!("{n} poses"))
                    .unwrap_or_else(|| "No poses".into()),
                POSE,
            );
        });
        if part_count > 0 {
            ui.add(
                egui::ProgressBar::new(verified_count as f32 / part_count as f32)
                    .desired_width(ui.available_width())
                    .fill(READY.gamma_multiply(0.5))
                    .text(format!("{verified_count} / {part_count} parts reviewed")),
            )
            .on_hover_text(
                "Manual review marks set in the Parts list. This is not automatic validation.",
            );
        } else {
            ui.label("No parts loaded");
        }
        ui.add(
            egui::Label::new(
                RichText::new("Review parts in the left panel.")
                    .small()
                    .color(chrome::MUTED),
            )
            .wrap(),
        );
    });
    ui.add_space(5.0);
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Other checks").strong().color(chrome::TEXT));
        help(
            ui.label(RichText::new("Topology · not measured").color(chrome::MUTED)),
            "No measured mesh or bone comparison is available here.",
            "Avoid treating a status label as proof that the rig is correct.",
            "Inspect and validate the exported rig before delivery.",
        );
        help(
            ui.label(RichText::new("AI / OCR · not checked").color(chrome::MUTED)),
            "Service availability is not tracked in this panel.",
            "Loaded artwork does not prove that AI analysis ran.",
            "Check the analysis result for this asset.",
        );
    });
}
