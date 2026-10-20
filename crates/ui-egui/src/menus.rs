//! Desktop menus use the same project/workflow commands as the toolbar and CLI.
//!
//! Layout follows the craft-ui principles: section headers group related
//! commands, icons lead each item so text length never drives button width,
//! and keyboard shortcuts are shown inline. Menus are deliberately narrow
//! and stable so opening/closing them doesn't reflow the surrounding UI.
use crate::panels::header::{action_button, command, export_mp4, reload};
use crate::{AnimationCraftApp, CenterTab, UiCommand};
use egui::{Color32, RichText, Ui};
use serde_json::json;

/// Small section header inside a menu, visually separating groups of commands.
fn menu_section(ui: &mut Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(title)
            .size(9.0)
            .strong()
            .color(Color32::from_rgb(112, 196, 255)),
    );
    ui.add_space(2.0);
    ui.separator();
}

/// Consistent icon prefix for menu items. Fixed-width geometric glyphs keep
/// button width stable regardless of label length (no emoji, which render
/// inconsistently across platforms).
const ICON_NEW: &str = "▸";
const ICON_OPEN: &str = "⌘";
const ICON_CLOSE: &str = "×";
const ICON_ADD: &str = "+";
const ICON_REFRESH: &str = "↻";
const ICON_EXPORT: &str = "⇥";
const ICON_RIG: &str = "◇";
const ICON_PLAY: &str = "▶";
const ICON_VERIFY: &str = "✓";
const ICON_SETTINGS: &str = "⚙";

pub(super) fn menu_bar(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let ready = app.ui_state.busy_command.is_none();
    let has_asset = app.session.current_asset().is_some();
    let has_rig = app
        .session
        .current_asset()
        .is_some_and(|a| a.cutout_rig.is_some());
    egui::MenuBar::new().ui(ui, |ui| {
        // ━━━ File ━━━
        ui.menu_button("File", |ui| {
            let project = crate::panels::start::project_open(app);
            let new_label = format!(
                "{} New Project…   {}",
                ICON_NEW,
                crate::panels::start::shortcut_text(ui.ctx(), egui::Key::N)
            );
            if action_button(app, ui, "menu.new_project", &new_label, ready) {
                ui.close();
                crate::panels::start::new_project(app);
            }
            let open_label = format!(
                "{} Open Project…   {}",
                ICON_OPEN,
                crate::panels::start::shortcut_text(ui.ctx(), egui::Key::O)
            );
            if action_button(app, ui, "menu.open_project", &open_label, ready) {
                ui.close();
                crate::panels::start::open_project(app);
            }
            ui.add_enabled_ui(ready && !app.ui_state.recent_projects.is_empty(), |ui| {
                menu_section(ui, "Open Recent");
                for path in app.ui_state.recent_projects.clone() {
                    if ui.button(&path).clicked() {
                        crate::panels::start::open_recent(app, &path);
                        ui.close();
                    }
                }
            });
            ui.separator();
            if action_button(app, ui, "menu.close_project", "Close Project", ready && project) {
                command(app, "project.close", json!({}));
                ui.close();
            }
            ui.separator();
            menu_section(ui, "Pipeline");
            if action_button(
                app,
                ui,
                "menu.workshop",
                &format!("{} AnimationProject (Song Workshop)", ICON_SETTINGS),
                ready && app.session.project_path.is_some(),
            ) {
                app.ui_state.standalone_studio = false;
                app.ui_state.show_workshop = true;
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.standalone_studio",
                &format!("{} AnimationStudio (2D/3D Rigging)", ICON_RIG),
                ready,
            ) {
                app.ui_state.show_workshop = false;
                app.ui_state.standalone_studio = true;
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.transcribe",
                &format!("{} Transcribe Song…", ICON_PLAY),
                ready && project,
            ) {
                ui.close();
                if let Some(audio) = rfd::FileDialog::new()
                    .set_title("Transcribe Song")
                    .add_filter("Audio", &["wav", "mp3", "flac", "m4a", "aac", "ogg"])
                    .pick_file()
                {
                    command(app, "audio.transcribe", json!({"audio": audio}));
                }
            }
            if action_button(
                app,
                ui,
                "menu.import",
                &format!("{} + Import Asset…", ICON_ADD),
                ready && project,
            ) {
                app.ui_state.connect_asset_open = true;
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.reload",
                &format!("{} Reload Manifest", ICON_REFRESH),
                has_asset,
            ) {
                reload(app);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{} Quit", ICON_CLOSE)).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        // ━━━ Project Assets ━━━
        ui.menu_button("Project Assets", |ui| {
            if action_button(
                app,
                ui,
                "menu.connect",
                &format!("{} + Add / Connect Asset…", ICON_ADD),
                crate::panels::start::project_open(app),
            ) {
                app.ui_state.connect_asset_open = true;
                ui.close();
            }
            ui.add_enabled_ui(ready, |ui| {
                let entries = app.session.project_assets.clone();
                ui.separator();
                for entry in entries {
                    if action_button(
                        app,
                        ui,
                        &format!("menu.load.{}", entry.id),
                        &entry.name,
                        true,
                    ) {
                        command(app, "project.load_asset", json!({"id": entry.id}));
                        ui.close();
                    }
                }
                ui.separator();
                menu_section(ui, "Manage");
                if action_button(
                    app,
                    ui,
                    "menu.rename",
                    &format!("{} Rename Active Asset…", ICON_SETTINGS),
                    has_asset,
                ) && let Some(entry) = app
                    .session
                    .project_assets
                    .iter()
                    .find(|e| Some(&e.id) == app.session.active_asset.as_ref())
                {
                    app.ui_state.rename_asset = Some((entry.id.clone(), entry.name.clone()));
                    ui.close();
                }
                if action_button(
                    app,
                    ui,
                    "menu.remove",
                    &format!("{} Remove Active Asset", ICON_CLOSE),
                    has_asset,
                ) && let Some(id) = app.session.active_asset.clone()
                {
                    command(app, "project.remove_asset", json!({"id": id}));
                    ui.close();
                }
            });
        });

        // ━━━ View ━━━
        ui.menu_button("View", |ui| {
            for (tab, id, label) in [
                (CenterTab::OriginalView, "original", "1. Original"),
                (CenterTab::AssetSheet, "poses", "2. 2D Poses"),
                (CenterTab::ThreeD, "turnaround", "3. 3D Turnaround"),
                (CenterTab::TwoD, "animated", "4. Animated Cutout"),
                (
                    CenterTab::TurnaroundSheet,
                    "turnaround_sheet",
                    "Turnaround Reference Sheet",
                ),
                (CenterTab::FivePoseLineup, "lineup", "5-Pose Lineup"),
            ] {
                let response = ui.selectable_label(app.ui_state.center_tab == tab, label);
                app.automation
                    .add_response(format!("menu.view.{id}"), "button", label, &response);
                if response.clicked() {
                    app.ui_state.center_tab = tab;
                    ui.close();
                }
            }
            ui.separator();
            menu_section(ui, "Overlays");
            ui.checkbox(&mut app.ui_state.show_ref_planes, "Reference Planes");
            ui.checkbox(&mut app.ui_state.light_backdrop, "Light Backdrop");
            ui.checkbox(&mut app.ui_state.show_pivot_markers, "Joint Pivots");
            ui.checkbox(&mut app.ui_state.show_topology_numbers, "Topology Numbers");
        });

        // ━━━ Rig & Matting ━━━
        ui.menu_button("Rig & Matting", |ui| {
            menu_section(ui, "Prepare");
            if action_button(
                app,
                ui,
                "menu.transparent",
                &format!("{} Make Transparent", ICON_EXPORT),
                has_asset,
            ) {
                command(
                    app,
                    "asset.make_transparent",
                    json!({"tolerance": app.ui_state.bg_tolerance}),
                );
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.build_rig",
                &format!("{} Build Cutout Rig", ICON_RIG),
                has_asset,
            ) {
                command(app, "asset.build_cutout_rig", json!({}));
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.cut_sheet",
                &format!("{} Cut Pose / Turnaround Sheet", ICON_EXPORT),
                has_asset,
            ) && let Some(id) = app.session.active_asset.clone()
            {
                app.exec_ui_command(UiCommand::AssetCutSheet {
                    asset_id: id,
                    tolerance: app.ui_state.bg_tolerance,
                    use_ocr: app.ui_state.use_ocr,
                });
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.register",
                &format!("{} Register Turnaround", ICON_REFRESH),
                has_asset,
            ) {
                app.exec_ui_command(UiCommand::TurnaroundRegister);
                ui.close();
            }
            if action_button(app, ui, "menu.show_parts", "{} Show All Parts", has_asset) {
                app.exec_ui_command(UiCommand::PartShowAll);
                ui.close();
            }
            ui.separator();
            menu_section(ui, "Pose");
            if ui.button(format!("{} Reset Pose Sliders", ICON_REFRESH)).clicked() {
                app.ui_state.reset_pose_sliders();
                ui.close();
            }
        });

        // ━━━ Export ━━━
        ui.menu_button("Export", |ui| {
            menu_section(ui, "Video Output");
            // Primary action first.
            if action_button(app, ui, "menu.export_mp4", &format!("{} Export MP4…", ICON_EXPORT), has_rig) {
                export_mp4(app);
                ui.close();
            }
            ui.separator();
            menu_section(ui, "Scripts & Specs");
            if action_button(
                app,
                ui,
                "menu.export_prompt",
                &format!("{} Export Agent Rig Prompt…", ICON_EXPORT),
                has_asset,
            ) && let Some(path) = rfd::FileDialog::new()
                .set_file_name("AGENT_RIG_PROMPT.md")
                .save_file()
            {
                command(app, "export.agent_prompt", json!({"output": path}));
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.export_effectcraft",
                &format!("{} Export EffectCraft Script (.jsx)…", ICON_EXPORT),
                has_rig,
            ) && let Some(path) = rfd::FileDialog::new()
                .add_filter("EffectCraft Script", &["jsx"])
                .save_file()
            {
                app.exec_ui_command(UiCommand::ExportEffectcraft {
                    output: path.to_string_lossy().into_owned(),
                });
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.run_effectcraft_script",
                &format!("{} Run EffectCraft Script (.jsx)…", ICON_PLAY),
                ready,
            ) && let Some(path) = rfd::FileDialog::new()
                .add_filter("EffectCraft Script", &["jsx"])
                .pick_file()
            {
                command(app, "effectcraft.run_script", json!({"script": path}));
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.export_layered_motion",
                &format!("{} Export Layered Motion (.json)…", ICON_EXPORT),
                ready,
            ) && let Some(path) = rfd::FileDialog::new()
                .add_filter("Layered Motion Spec", &["json"])
                .pick_file()
            {
                app.exec_ui_command(UiCommand::ExportLayeredMotion {
                    spec: path.to_string_lossy().into_owned(),
                    out_dir: None,
                });
                ui.close();
            }
            ui.separator();
            menu_section(ui, "Validation");
            if action_button(
                app,
                ui,
                "menu.validate_motion",
                &format!("{} Validate Motion Clip (.webm)…", ICON_VERIFY),
                ready,
            ) && let Some(path) = rfd::FileDialog::new()
                .add_filter("Rendered Clip", &["webm", "mp4"])
                .pick_file()
            {
                let reference = app
                    .session
                    .current_asset()
                    .filter(|a| a.path.is_file())
                    .map(|a| a.path.to_string_lossy().into_owned());
                let spec = app
                    .session
                    .current_asset()
                    .and_then(|a| a.cutout_rig.as_ref())
                    .filter(|r| r.rig_type == "layered_motion")
                    .map(|r| r.plan_path.to_string_lossy().into_owned());
                app.exec_ui_command(UiCommand::ValidateMotion {
                    video: path.to_string_lossy().into_owned(),
                    reference,
                    spec,
                    loop_clip: false,
                });
                ui.close();
            }
            ui.separator();
            menu_section(ui, "Integration");
            if action_button(
                app,
                ui,
                "menu.export_filmcraft",
                &format!("{} FilmCraft Project…", ICON_EXPORT),
                has_asset,
            ) && let Some(path) = rfd::FileDialog::new()
                .add_filter("FilmCraft", &["fcproj"])
                .save_file()
            {
                app.exec_ui_command(UiCommand::ExportFilmcraft {
                    output: path.to_string_lossy().into_owned(),
                });
                ui.close();
            }
        });
    });
}
