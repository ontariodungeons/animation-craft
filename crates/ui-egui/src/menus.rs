//! Desktop menus use the same project/workflow commands as the toolbar and CLI.
use crate::panels::header::{action_button, command, export_mp4, reload};
use crate::{AnimationCraftApp, CenterTab, UiCommand};
use egui::Ui;
use serde_json::json;

pub(super) fn menu_bar(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let ready = app.ui_state.busy_command.is_none();
    let has_asset = app.session.current_asset().is_some();
    let has_rig = app
        .session
        .current_asset()
        .is_some_and(|a| a.cutout_rig.is_some());
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            let project = crate::panels::start::project_open(app);
            let new_label = format!("New Project…   {}", crate::panels::start::shortcut_text(ui.ctx(), egui::Key::N));
            if action_button(app, ui, "menu.new_project", &new_label, ready) {
                ui.close();
                crate::panels::start::new_project(app);
            }
            let open_label = format!("Open Project…   {}", crate::panels::start::shortcut_text(ui.ctx(), egui::Key::O));
            if action_button(app, ui, "menu.open_project", &open_label, ready) {
                ui.close();
                crate::panels::start::open_project(app);
            }
            ui.add_enabled_ui(ready && !app.ui_state.recent_projects.is_empty(), |ui| {
                ui.menu_button("Open Recent", |ui| {
                    for path in app.ui_state.recent_projects.clone() {
                        if ui.button(&path).clicked() {
                            crate::panels::start::open_recent(app, &path);
                            ui.close();
                        }
                    }
                });
            });
            if action_button(app, ui, "menu.close_project", "Close Project", ready && project) {
                command(app, "project.close", json!({}));
                ui.close();
            }
            ui.separator();
            if action_button(app, ui, "menu.import", "+ Import Asset…", ready && project) {
                app.ui_state.connect_asset_open = true;
                ui.close();
            }
            if action_button(app, ui, "menu.reload", "Reload Manifest", has_asset) {
                reload(app);
                ui.close();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button("Project Assets", |ui| {
            if action_button(app, ui, "menu.connect", "+ Add / Connect Asset…", crate::panels::start::project_open(app)) {
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
                if action_button(app, ui, "menu.rename", "Rename Active Asset…", has_asset)
                    && let Some(entry) = app
                        .session
                        .project_assets
                        .iter()
                        .find(|e| Some(&e.id) == app.session.active_asset.as_ref())
                {
                    app.ui_state.rename_asset = Some((entry.id.clone(), entry.name.clone()));
                    ui.close();
                }
                if action_button(app, ui, "menu.remove", "Remove Active Asset", has_asset)
                    && let Some(id) = app.session.active_asset.clone()
                {
                    command(app, "project.remove_asset", json!({"id": id}));
                    ui.close();
                }
            });
        });
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
            ui.checkbox(&mut app.ui_state.show_ref_planes, "Reference Planes");
            ui.checkbox(&mut app.ui_state.light_backdrop, "Light Backdrop");
            ui.checkbox(&mut app.ui_state.show_pivot_markers, "Joint Pivots");
            ui.checkbox(&mut app.ui_state.show_topology_numbers, "Topology Numbers");
        });
        ui.menu_button("Rig & Matting", |ui| {
            if action_button(app, ui, "menu.transparent", "Make Transparent", has_asset) {
                command(
                    app,
                    "asset.make_transparent",
                    json!({"tolerance": app.ui_state.bg_tolerance}),
                );
                ui.close();
            }
            if action_button(app, ui, "menu.build_rig", "Build Cutout Rig", has_asset) {
                command(app, "asset.build_cutout_rig", json!({}));
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.cut_sheet",
                "Cut Pose / Turnaround Sheet",
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
            if action_button(app, ui, "menu.register", "Register Turnaround", has_asset) {
                app.exec_ui_command(UiCommand::TurnaroundRegister);
                ui.close();
            }
            if action_button(app, ui, "menu.show_parts", "Show All Parts", has_asset) {
                app.exec_ui_command(UiCommand::PartShowAll);
                ui.close();
            }
            if ui.button("Reset Pose Sliders").clicked() {
                app.ui_state.reset_pose_sliders();
                ui.close();
            }
        });
        ui.menu_button("Export", |ui| {
            if action_button(app, ui, "menu.export_mp4", "Export MP4…", has_rig) {
                export_mp4(app);
                ui.close();
            }
            if action_button(
                app,
                ui,
                "menu.export_prompt",
                "Export Agent Rig Prompt…",
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
                "Export EffectCraft Script (.jsx)…",
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
                "Run EffectCraft Script (.jsx)…",
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
                "Export Layered Motion (.json)…",
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
            if action_button(
                app,
                ui,
                "menu.validate_motion",
                "Validate Motion Clip (.webm)…",
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
            if action_button(
                app,
                ui,
                "menu.export_filmcraft",
                "FilmCraft Project…",
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
