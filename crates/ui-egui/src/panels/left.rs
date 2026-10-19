//! Left panel: Intake, Asset/Sheet Slicer, Turnaround cutouts, Layer/Part Outliner.
//!
//! All data is read from `Session.current_asset()` manifests — never hardcoded to
//! Hopper/Hardcoded paths. Every button dispatches a [`UiCommand`].

use super::chrome;
use crate::{AnimationCraftApp, CenterTab, UiCommand};
use egui::Ui;

fn source_sheet_card(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    title: &str,
    filename: &str,
    count_label: &str,
    tab: CenterTab,
    slot: &str,
    view_id: &str,
    import_id: &str,
) {
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(title).strong().color(chrome::TEXT));
            chrome::badge(ui, count_label, chrome::BLUE);
        });
        ui.add(egui::Label::new(egui::RichText::new(if filename == "none" { "No source attached" } else { filename }).small().color(chrome::MUTED)).truncate())
            .on_hover_text(filename);
        ui.add_enabled_ui(app.ui_state.busy_command.is_none(), |ui| {
            ui.horizontal_wrapped(|ui| {
                let response = chrome::button(ui, "View sheet", false)
                    .on_hover_text("Open this character's connected reference sheet in the center workspace.");
                app.automation.add_response(view_id, "button", format!("View {title}"), &response);
                if response.clicked() {
                    if let Some(path) = app.session().current_asset().and_then(|a| super::center::pose_viewer::sheet_path(a, slot)) {
                        super::center::pose_viewer::open_source(app, ui.ctx(), &path);
                    } else { app.ui_state.status_line = "Attach a sheet to open it here.".into(); }
                }
                let response = chrome::help(chrome::button(ui, if filename == "none" { "Attach…" } else { "Replace…" }, false),
                    "Attach an image sheet to the currently selected character.",
                    "Add pose or turnaround references without creating a different character.",
                    "Choose a file. Intake processing and automatic slicing will run; review the result.");
                app.automation.add_response(import_id, "button", format!("Attach {title}"), &response);
                if response.clicked()
                    && let Some(file) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file()
                {
                    let path = file.display().to_string();
                    app.ui_state.image_path = Some(path.clone());
                    app.ui_state.center_tab = tab;
                    app.exec_ui_command(UiCommand::IntakeValidate {
                        image_path: path, slot: slot.into(), attach_to_active: true,
                        auto_cut: true, tolerance: app.ui_state.bg_tolerance,
                        use_ocr: app.ui_state.use_ocr,
                    });
                }
            });
        });
    });
    ui.add_space(6.0);
}

pub(crate) fn left_panel(app: &mut AnimationCraftApp, ui: &mut Ui) {
    egui::ScrollArea::vertical()
        .id_salt(if app.ui_state.center_tab == CenterTab::AssetSheet { "left_pose_browser" } else { "left_scroll" })
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.spacing_mut().item_spacing.y = 6.0;
            super::header::project_assets(app, ui);
            if app.ui_state.center_tab == CenterTab::AssetSheet {
                super::center::pose_viewer::sidebar(app, ui);
                ui.add_space(8.0);
            }
            ui.label(egui::RichText::new("Source artwork").size(14.0).strong().color(chrome::TEXT));
            // Attached Sheets per Character (Zero Context Switching)
            let (active_char, asset_sheet_name, pose_count, turn_sheet_name, turn_count) = {
                let asset = app.session().current_asset();
                let id = app
                    .session()
                    .active_asset
                    .clone()
                    .unwrap_or_else(|| "none".to_string());
                let a_name = asset
                    .and_then(|a| a.pose_manifest.as_ref().and_then(|m| m.source_image.as_ref()).or(a.attached_asset_sheet.as_ref()))
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

            ui.label(egui::RichText::new(format!("References for {active_char}")).small().color(chrome::MUTED));
            source_sheet_card(app, ui, "Pose sheet", &asset_sheet_name, &format!("{pose_count} poses"),
                CenterTab::AssetSheet, "asset_sheet", "btn.view_asset_sheet", "btn.import_asset_sheet");
            source_sheet_card(app, ui, "Turnaround", &turn_sheet_name, &format!("{turn_count} views"),
                CenterTab::TurnaroundSheet, "turnaround_sheet", "btn.view_turnaround_sheet", "btn.import_turnaround");
            ui.add_space(6.0);
            let settings = egui::CollapsingHeader::new(egui::RichText::new("Preparation tools").color(chrome::BLUE))
                .id_salt("left.preparation_tools").default_open(false).show(ui, |ui| {
            // Universal archetype buttons
            ui.horizontal_wrapped(|ui| {
                ui.label("Character type:");
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

            ui.checkbox(&mut app.ui_state_mut().strip_costume, "Request a base mannequin")
                .on_hover_text("Requests costume removal in the intake workflow. Support depends on the downstream pipeline; this toggle alone does not remove clothing.");

            ui.label("Background tolerance");
            ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(32.0);
            ui.add(egui::Slider::new(&mut app.ui_state.bg_tolerance, 1..=100))
                .on_hover_text("Higher values remove a broader range of background colors. Start low and inspect pale fur, fine edges and holes before increasing.");
            ui.checkbox(
                &mut app.ui_state_mut().use_ocr,
                "Read pose labels automatically",
            ).on_hover_text("Request OCR when cutting a sheet. Check the detected labels against the artwork; this setting does not prove that text recognition is available.");

            ui.separator();

            // Primary intake / pipeline buttons
            ui.horizontal_wrapped(|ui| {
                let r_val = chrome::button(ui, "Process image", false);
                app.automation.add_response(
                    "btn.validate_intake",
                    "button",
                    "Validate Intake",
                    &r_val,
                );
                if r_val.clicked() {
                    let path = app.ui_state.image_path.clone().unwrap_or_default();
                    let tolerance = app.ui_state.bg_tolerance;
                    let use_ocr = app.ui_state.use_ocr;
                    app.exec_ui_command(UiCommand::IntakeValidate {
                        image_path: path,
                        slot: "auto".to_string(),
                        attach_to_active: true,
                        auto_cut: true,
                        tolerance,
                        use_ocr,
                    });
                }
                let r_3q = chrome::button(ui, "Analyze intake", false);
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
                let r_cut = chrome::button(ui, "Slice sheet", false);
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
                let r_reg = chrome::button(ui, "Align views", false);
                app.automation.add_response(
                    "btn.drift_register",
                    "button",
                    "0px Drift Register",
                    &r_reg,
                );
                if r_reg.clicked() {
                    app.exec_ui_command(UiCommand::TurnaroundRegister);
                }
                let r_prompt = chrome::button(ui, "Export agent brief", false);
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

            });
            app.automation.add_response("left.preparation_tools", "button", "Preparation tools", &settings.header_response);
            settings.header_response.on_hover_text("Character type, background settings, intake analysis, slicing, registration and agent prompts.");
            ui.add_space(8.0);

            super::asset_browser::turnaround(app, ui);
            if app.ui_state.center_tab != CenterTab::AssetSheet {
                super::asset_browser::poses(app, ui);
            }
            super::asset_browser::parts(app, ui);
        });
}
