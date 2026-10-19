//! Right panel: Motion Range Controls, Verified Telemetry, Visemes/Sliders, and 1-Click Export.
//!
//! All silhouette motion range and registration drift values are read strictly from
//! `app.session().current_asset()`. Unmeasured fields display `"not measured"`.

use crate::{AnimMode, AnimationCraftApp, UiCommand};
use egui::{CollapsingHeader, Color32, RichText, Ui};

// Inspector accents: labels carry meaning even when colors cannot be distinguished.
const MOTION: Color32 = Color32::from_rgb(112, 196, 255);
const POSE: Color32 = Color32::from_rgb(195, 164, 255);
const MESH: Color32 = Color32::from_rgb(240, 190, 112);
const EXPORT: Color32 = Color32::from_rgb(100, 215, 194);
const READY: Color32 = Color32::from_rgb(128, 218, 154);

fn help(response: egui::Response, what: &str, why: &str, how: &str) -> egui::Response {
    response.on_hover_ui(|ui| {
        ui.set_max_width(290.0);
        for (heading, body) in [("What", what), ("Why", why), ("How", how)] {
            ui.label(RichText::new(heading).strong());
            ui.add(egui::Label::new(body).wrap());
            ui.add_space(4.0);
        }
    })
}

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

fn export_button(ui: &mut Ui, label: &str, explanation: (&str, &str, &str)) -> egui::Response {
    let (what, why, how) = explanation;
    help(
        ui.add_sized(
            [ui.available_width(), 28.0],
            egui::Button::new(RichText::new(label).color(EXPORT)).wrap(),
        ),
        what,
        why,
        how,
    )
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
                    let rows = if let Some(mr) = motion_opt {
                        vec![
                            ("Foot lift", format!("{:.1} px", mr.foot_lift_max_px * scale), format!("Maximum measured lift: {:.1} px in {}. Use it to judge foot clearance.", mr.foot_lift_max_px, mr.foot_lift_max_pose)),
                            ("Width change", format!("{:.1} px", mr.width_max_delta_px * scale), format!("Maximum measured width change: {:.1} px in {}. Allow this much room beside the character.", mr.width_max_delta_px, mr.width_max_pose)),
                            ("Left / right reach", format!("{:.1} / {:+.1} px", mr.left_min_delta_px * scale, mr.right_max_delta_px * scale), "Horizontal changes relative to the reference silhouette. Negative values extend left; positive values extend right.".into()),
                            ("Reference height", format!("{:.0} px", mr.still_height_px), format!("Height of the still reference: {}. Used to compare the scale of different poses.", mr.still_label)),
                        ]
                    } else {
                        vec![("Motion report", "Not measured".into(), "Load or generate a motion report for this asset before interpreting motion limits.".into())]
                    };
                    for (label, value, detail) in rows {
                        help(ui.add(egui::Label::new(format!("{label}: {value}")).wrap()), &detail,
                            "Use measured evidence when judging movement quality.", "Read this value after adjusting Motion amount; hover to see what it measures.");
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
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        help(ui.small("Mouth shape"), "A viseme is a mouth shape associated with speech.", "Choose an expression or speech shape for the character.", "These selections currently store a preview setting; they do not generate lip-sync or missing mouth artwork.");
                        for (id, label) in [
                            ("smile", "Smile"),
                            ("laugh", "Laugh"),
                            ("ooh", "Ooh"),
                            ("mbp", "M/B/P"),
                            ("talk", "Talk"),
                        ] {
                            let active = app.ui_state.mouth_viseme == id;
                            if help(ui.selectable_label(active, label),
                                &format!("Select {label} as the mouth-shape setting."),
                                "Record the desired expression for preview work.",
                                "Click to select. A stored selection does not guarantee a visible mouth change; the current renderer does not synthesize visemes.").clicked() {
                                app.ui_state.mouth_viseme = id.to_string();
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        if help(ui.small_button("Sheet Neutral"), "Reset all pose adjustments to their neutral defaults.", "Quickly explore a pose starting point.", "Click, then fine-tune the controls below. These are preview offsets and may not affect every rig.").clicked() {
                            app.ui_state.reset_pose_sliders();
                        }
                        if help(ui.small_button("A-Pose"), "Reset the pose, then set arm spread to 35.", "Quickly explore a pose starting point.", "Click, then fine-tune the controls below. These are preview offsets and may not affect every rig.").clicked() {
                            app.ui_state.reset_pose_sliders();
                            app.ui_state.arm_spread = 35.0;
                        }
                        if help(ui.small_button("Action Hop"), "Set knee bend to 30, elbow bend to 25 and ear pitch to -15.", "Quickly explore a pose starting point.", "Click, then fine-tune the controls below. These are preview offsets and may not affect every rig.").clicked() {
                            app.ui_state.knee_bend = 30.0;
                            app.ui_state.elbow_bend = 25.0;
                            app.ui_state.ear_pitch = -15.0;
                        }
                        if help(ui.small_button("Alert Ears"), "Set ear splay to 20 and eye blink to 80.", "Quickly explore a pose starting point.", "Click, then fine-tune the controls below. These are preview offsets and may not affect every rig.").clicked() {
                            app.ui_state.ear_splay = 20.0;
                            app.ui_state.eye_blink = 80.0;
                        }
                        if help(ui.small_button("Reset Pose"), "Reset all pose adjustments to their neutral defaults.", "Quickly explore a pose starting point.", "Click, then fine-tune the controls below. These are preview offsets and may not affect every rig.").clicked() {
                            app.ui_state.reset_pose_sliders();
                        }
                    });
                    slider(ui, "Eye blink", &mut app.ui_state.eye_blink, 0.0..=100.0, "%",
                        ("Controls the blink-related preview offset.", "Check the head pose during an expressive moment.", "Drag from 0 to 100. The current layered preview shifts the head; this does not close painted eyelids."));
                    slider(ui, "Eye gaze", &mut app.ui_state.eye_gaze, -45.0..=45.0, "",
                        ("Controls a gaze-related horizontal head offset.", "Suggest a change of attention while composing a pose.", "Try small positive or negative values. This does not redraw eyes or move pupils independently."));
                    slider(ui, "Knee bend", &mut app.ui_state.knee_bend, -45.0..=90.0, "",
                        ("Offsets the legs in the current preview; some cutout actions also use it in leg rotation.", "Explore a crouched or lifted-leg pose and inspect foot placement.", "Increase gradually while watching both feet. Negative values move the adjustment in the opposite direction."));
                    slider(ui, "Elbow bend", &mut app.ui_state.elbow_bend, -45.0..=90.0, "",
                        ("Adjusts arm placement in the layered preview.", "Check arm spacing and overlap with the torso.", "Try a small value first. It is not a separate elbow joint solver and may not affect every rig."));
                    slider(ui, "Ear spread", &mut app.ui_state.ear_splay, -45.0..=45.0, "",
                        ("Adjusts ear spacing or rotation, depending on the active preview rig.", "Make an expression more alert and inspect overlap near the head.", "Drag either side of zero and watch the ear bases. Reset Pose restores the default."));
                    slider(ui, "Ear tilt", &mut app.ui_state.ear_pitch, -45.0..=45.0, "",
                        ("Changes the vertical ear offset in the layered preview.", "Explore the silhouette of raised or lowered ears.", "Use small positive or negative values. This is an approximate offset, not a full 3D ear rotation."));
                    slider(ui, "Head turn", &mut app.ui_state.head_yaw, -45.0..=45.0, "",
                        ("Adjusts head placement or rotation in supported preview paths.", "Suggest a look to the side and check neck overlap.", "Drag either side of zero. It cannot reveal artwork hidden behind the head."));
                    slider(ui, "Head tilt", &mut app.ui_state.head_pitch, -45.0..=45.0, "",
                        ("Changes the vertical head offset in the layered preview.", "Explore the head position relative to the body.", "Use small values while watching the neck. This does not reconstruct an up or down view."));
                    slider(ui, "Mouth opening", &mut app.ui_state.smile_open, 0.0..=100.0, "%",
                        ("Stores a mouth-opening value used as a small head offset in the layered preview.", "Record an expression adjustment while inspecting the pose.", "20% is the current neutral value. It does not redraw the mouth or generate lip-sync."));
                    slider(ui, "Arm spread", &mut app.ui_state.arm_spread, -45.0..=90.0, "",
                        ("Moves the arms outward or changes their rotation in supported preview actions.", "Check how far the arms can move without exposing gaps at the shoulders.", "Start near zero and increase gradually; A-Pose sets this to 35."));
                });

            ui.separator();

            // --- 3b. 3D Volumetric Reconstruction Controls (from Web App) ---
            CollapsingHeader::new(RichText::new("4 · 3D shape").color(MESH).strong())
                .id_salt("3b. 3D Reconstruction & Multi-View")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.small("Mode:");
                        for (mode_id, label) in [
                            ("volumetric", "Volume"),
                            ("rig", "Rig (limited)"),
                        ] {
                            let active = app.ui_state.recon_mode == mode_id;
                            if help(ui.selectable_label(active, label),
                                "Select the reconstruction mode recorded in the project.",
                                "Volume approximates depth from silhouettes; an articulated rig would use separate movable parts.",
                                "Current mesh export uses the same volume builder for both selections. Rig mode does not yet produce a separate articulated mesh.").clicked() {
                                app.ui_state.recon_mode = mode_id.to_string();
                            }
                        }
                    });
                    slider(ui, "Depth", &mut app.ui_state.inflation_depth_pct, 10..=100, "%", ("Controls how thick the generated volume is.", "Give a flat silhouette depth without making it excessively round.", "Start near 58%. Compare the side view and rebuild the mesh after adjusting."));
                    slider(ui, "Surface relief", &mut app.ui_state.relief_detail_pct, 0..=100, "%", ("Adds procedural surface variation to the volume.", "Break up a perfectly smooth surface.", "Start low and increase gradually. This is generated relief, not detail recovered from the artwork."));
                    slider(ui, "Mesh detail", &mut app.ui_state.mesh_res, 12..=64, "", ("Controls the number of height slices and radial segments.", "Higher values create a smoother mesh but increase geometry and processing cost.", "Start at 32. Raise only if the exported silhouette looks faceted; rebuild to apply."));
                    slider(ui, "View blending", &mut app.ui_state.proj_blend_pct, 0.0..=100.0, "%", ("Weights the front and quarter-view silhouette spans used during mesh generation.", "Adjust how much the quarter view contributes to the shape.", "Compare low and high values on a rebuilt mesh. This does not generate missing views."));
                    slider(ui, "Surface bump", &mut app.ui_state.bump_strength, 0.0..=100.0, "%", ("Adds to the procedural relief amplitude during mesh export.", "Introduce more surface variation when a smooth result looks too plain.", "Use small values first. This is geometric variation, not a physically based felt material."));
                });

            ui.separator();

            // --- 4. Verified Asset & QA Status ---
            CollapsingHeader::new(RichText::new("5 · Quality checks").color(READY).strong())
                .id_salt("4. Asset & Registration QA")
                .default_open(true)
                .show(ui, |ui| {
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

                    match reg_passed {
                        Some(true) => {
                            ui.colored_label(
                                READY,
                                "PASS · View alignment",
                            );
                        }
                        Some(false) => {
                            ui.colored_label(
                                MESH,
                                "REVIEW · View alignment",
                            );
                        }
                        None => {
                            ui.label("NOT MEASURED · View alignment");
                        }
                    }

                    help(ui.label("Loaded artwork & review"), "Counts of loaded views, pose references and reviewed cutout parts.", "Find missing inputs before exporting. Loaded does not mean verified.", "Review cutout parts in the left panel. View alignment status comes from the registration report.");
                    match turnaround_count {
                        Some(n) => ui.label(format!("• Turnaround views: {n} loaded")),
                        None => ui.label("• Turnaround views: not loaded"),
                    };
                    match pose_count {
                        Some(n) => ui.label(format!("• Pose frames: {n} loaded")),
                        None => ui.label("• Pose frames: not loaded"),
                    };
                    if part_count > 0 {
                        ui.label(format!(
                            "• Cutout parts: {part_count} ({verified_count}/{part_count} verified)"
                        ));
                    } else {
                        ui.label("• Cutout parts: not measured");
                    }
                    help(ui.label("Topology check: Not measured"), "Mesh and bone parity has no measured result here.", "Avoid treating a status label as proof that the exported rig is correct.", "Inspect and validate the exported rig before delivery.");
                    help(ui.label("AI / OCR status: Not checked"), "This panel does not track whether the external image or text analysis service is available.", "Loaded artwork does not prove that AI analysis ran.", "Check the analysis command result for this asset.");
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
                    let r_iter = export_button(ui, "Action reel + layers", ("Runs the character-iteration pipeline to create layers and an action reel.", "Share a review package of supported character actions.", "Uses the selected character ID. This preset-oriented pipeline may not support a newly imported character."));
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
                    let r_ec = export_button(ui, "Run EffectCraft script", ("Executes the existing EffectCraft JSX script.", "Build the composition described by that script in EffectCraft.", "Requires exports/effectcraft_demo/build_scene.jsx. It does not generate a new script from the current preview."));
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
                    let r_fc = export_button(ui, "Send to FilmCraft", ("Imports media into a FilmCraft project.", "Continue editing a rendered animation on a timeline.", "This button currently uses the pipeline default Hopper video. For other media, use export.filmcraft with an explicit media path."));
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
                    let r_glb = export_button(ui, "Build mesh · GLB / OBJ", ("Builds a volume mesh and writes GLB and OBJ files.", "Move the generated shape into a 3D editor.", "Adjust 3D shape settings, then click. Missing views may reuse the first image; neither selection currently creates an articulated rig."));
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
                    let r_blend = export_button(ui, "Render with Blender", ("Runs the Blender character pipeline through its live connection.", "Render a character animation in Blender.", "Requires the Blender service on port 9876. The current pipeline uses character IDs; verify source identity for custom imports before running."));
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
                });
        });
}
