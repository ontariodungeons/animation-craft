//! Project connections and workflow actions shared by the menus and toolbar.
use crate::{AnimationCraftApp, CenterTab, DemoPreset, UiCommand};
use egui::Ui;
use serde_json::{Value, json};

pub(crate) const PRESETS: [DemoPreset; 11] = [
    DemoPreset::Hopper,
    DemoPreset::Whiskers,
    DemoPreset::Puddles,
    DemoPreset::Jenny,
    DemoPreset::Rusty,
    DemoPreset::MissHayley,
    DemoPreset::OldMac,
    DemoPreset::Scout,
    DemoPreset::MrPuddles,
    DemoPreset::Sam,
    DemoPreset::Maisy,
];

pub(crate) fn command(app: &mut AnimationCraftApp, id: &'static str, params: Value) {
    app.exec_ui_command(UiCommand::Engine { id, params });
}

pub(crate) fn connect(app: &mut AnimationCraftApp, params: Value) {
    if let Ok(result) = app.dispatch("project.add_asset", params)
        && let Some(id) = result.get("asset_id").and_then(Value::as_str)
    {
        command(app, "project.load_asset", json!({"id": id}));
        app.ui_state.center_tab = CenterTab::OriginalView;
    }
}

pub(crate) fn preset_picker(app: &mut AnimationCraftApp, ui: &mut Ui) {
    for preset in PRESETS {
        let response = ui.button(preset.label());
        app.automation.add_response(
            format!("preset.{}", preset.slug()),
            "button",
            preset.label(),
            &response,
        );
        if response.clicked() {
            let existing = app
                .session
                .project_assets
                .iter()
                .find(|a| a.preset.as_deref() == Some(preset.slug()))
                .map(|a| a.id.clone());
            if let Some(id) = existing {
                command(app, "project.load_asset", json!({"id": id}));
            } else {
                connect(
                    app,
                    json!({"preset": preset.slug(), "name": preset.label()}),
                );
            }
            ui.close();
        }
    }
}

pub(crate) fn reload(app: &mut AnimationCraftApp) {
    if let Some(id) = app.session.active_asset.clone() {
        command(app, "project.load_asset", json!({"id": id, "reload": true}));
    }
}

pub(crate) fn export_mp4(app: &mut AnimationCraftApp) {
    let id = app.session.active_asset.as_deref().unwrap_or("asset");
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("MP4", &["mp4"])
        .set_file_name(format!("{id}_cutout.mp4"))
        .save_file()
    {
        command(app, "export.cutout_mp4", json!({"output": path}));
    }
}

pub(crate) fn action_button(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    id: &str,
    label: &str,
    enabled: bool,
) -> bool {
    let response = ui.add_enabled(
        enabled && app.ui_state.busy_command.is_none(),
        egui::Button::new(label),
    );
    app.automation.add_response(id, "button", label, &response);
    response.clicked()
}

pub(crate) fn header(app: &mut AnimationCraftApp, ui: &mut Ui) {
    crate::menus::menu_bar(app, ui);
    ui.separator();
    let has_asset = app.session.current_asset().is_some();
    let has_rig = app
        .session
        .current_asset()
        .is_some_and(|a| a.cutout_rig.is_some());
    egui::ScrollArea::horizontal()
        .id_salt("header_actions")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("AnimationCraft");
                ui.separator();
                if action_button(app, ui, "toolbar.import", "+ Import Asset", true) {
                    app.ui_state.connect_asset_open = true;
                }
                if action_button(
                    app,
                    ui,
                    "toolbar.transparent",
                    "Make Transparent",
                    has_asset,
                ) {
                    command(
                        app,
                        "asset.make_transparent",
                        json!({"tolerance": app.ui_state.bg_tolerance}),
                    );
                }
                if action_button(app, ui, "toolbar.build_rig", "Build Cutout Rig", has_asset) {
                    command(app, "asset.build_cutout_rig", json!({}));
                }
                if action_button(app, ui, "toolbar.reload", "Reload Manifest", has_asset) {
                    reload(app);
                }
                if action_button(app, ui, "toolbar.export_mp4", "Export MP4", has_rig) {
                    export_mp4(app);
                }
            });
        });
    ui.horizontal(|ui| {
        if app.ui_state.busy_command.is_some() {
            ui.spinner();
        }
        ui.add(egui::Label::new(&app.ui_state.status_line).truncate());
    });
}

pub(crate) fn asset_dialogs(app: &mut AnimationCraftApp, ctx: &egui::Context) {
    if app.ui_state.connect_asset_open {
        let mut open = true;
        let mut connected = false;
        egui::Window::new("Add / Connect Asset").open(&mut open).collapsible(false)
            .default_width(460.0).show(ctx, |ui| {
                ui.add_enabled_ui(app.ui_state.busy_command.is_none(), |ui| {
                    ui.menu_button("Connect a preset", |ui| preset_picker(app, ui));
                    ui.separator();
                    ui.label("Image, pose sheet, turnaround, or cutout rig manifest");
                    ui.horizontal(|ui| {
                        ui.label("Path");
                        let response = ui.text_edit_singleline(&mut app.ui_state.connect_asset_path);
                        app.automation.add_response("project.path", "text", "Asset path", &response);
                        if ui.button("Browse…").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Images / Manifests", &["png", "jpg", "jpeg", "json"])
                                .pick_file()
                        { app.ui_state.connect_asset_path = path.to_string_lossy().into_owned(); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Name (optional)");
                        ui.text_edit_singleline(&mut app.ui_state.connect_asset_name);
                    });
                    egui::ComboBox::from_label("Asset type")
                        .selected_text(&app.ui_state.connect_asset_kind).show_ui(ui, |ui| {
                            for (kind, label) in [("auto", "Detect from file"), ("image", "Original image"),
                                ("poses", "Pose sheet"), ("turnaround", "Turnaround"), ("cutout_rig", "Cutout rig")] {
                                ui.selectable_value(&mut app.ui_state.connect_asset_kind, kind.into(), label);
                            }
                        });
                    if action_button(app, ui, "project.connect", "Connect Asset", !app.ui_state.connect_asset_path.trim().is_empty()) {
                        let mut params = json!({"path": app.ui_state.connect_asset_path.trim(), "kind": app.ui_state.connect_asset_kind});
                        if !app.ui_state.connect_asset_name.trim().is_empty() {
                            params["name"] = json!(app.ui_state.connect_asset_name.trim());
                        }
                        let count = app.session.project_assets.len();
                        connect(app, params);
                        connected = app.session.project_assets.len() > count;
                    }
                    ui.small("Connects the source as-is. Use Make Transparent or Build Cutout Rig when ready.");
                });
            });
        app.ui_state.connect_asset_open = open && !connected;
        if connected {
            app.ui_state.connect_asset_path.clear();
            app.ui_state.connect_asset_name.clear();
        }
    }
    if let Some((id, mut name)) = app.ui_state.rename_asset.clone() {
        let mut open = true;
        let mut saved = false;
        egui::Window::new("Rename Project Asset")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.text_edit_singleline(&mut name);
                if action_button(
                    app,
                    ui,
                    "project.rename_save",
                    "Save Name",
                    !name.trim().is_empty(),
                ) {
                    command(app, "project.update_asset", json!({"id": id, "name": name}));
                    saved = true;
                }
            });
        app.ui_state.rename_asset = (open && !saved).then_some((id, name));
    }
}

pub(crate) fn project_assets(app: &mut AnimationCraftApp, ui: &mut Ui) {
    ui.heading("Project Assets");
    let entries = app.session.project_assets.clone();
    ui.add_enabled_ui(app.ui_state.busy_command.is_none(), |ui| {
        for (index, entry) in entries.iter().enumerate() {
            ui.push_id(&entry.id, |ui| {
                ui.horizontal(|ui| {
                    let active = app.session.active_asset.as_deref() == Some(&entry.id);
                    let label = egui::Button::selectable(active, &entry.name);
                    let response = ui
                        .add_sized([ui.available_width().max(100.0) - 78.0, 22.0], label)
                        .on_hover_text(
                            entry
                                .path
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| {
                                    format!(
                                        "Preset: {}",
                                        entry.preset.as_deref().unwrap_or("unknown")
                                    )
                                }),
                        );
                    app.automation.add_response(
                        format!("project.load.{}", entry.id),
                        "button",
                        &entry.name,
                        &response,
                    );
                    if response.clicked() {
                        command(app, "project.load_asset", json!({"id": entry.id}));
                    }
                    response.context_menu(|ui| {
                        if ui.button("Rename…").clicked() {
                            app.ui_state.rename_asset =
                                Some((entry.id.clone(), entry.name.clone()));
                            ui.close();
                        }
                    });
                    for (suffix, label, enabled, target) in [
                        ("up", "^", index > 0, index.saturating_sub(1)),
                        ("down", "v", index + 1 < entries.len(), index + 1),
                    ] {
                        if action_button(
                            app,
                            ui,
                            &format!("project.{suffix}.{}", entry.id),
                            label,
                            enabled,
                        ) {
                            command(
                                app,
                                "project.update_asset",
                                json!({"id": entry.id, "index": target}),
                            );
                        }
                    }
                    if action_button(app, ui, &format!("project.remove.{}", entry.id), "x", true) {
                        command(app, "project.remove_asset", json!({"id": entry.id}));
                    }
                });
            });
        }
        if entries.is_empty() {
            ui.weak("No connected assets");
        }
        if action_button(app, ui, "project.add", "+ Add / Connect Asset…", true) {
            app.ui_state.connect_asset_open = true;
        }
    });
    ui.separator();
}
