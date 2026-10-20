//! Project connections and workflow actions shared by the menus and toolbar.
use super::chrome;
use crate::{AnimationCraftApp, CenterTab, UiCommand};
use egui::{RichText, Ui};
use serde_json::{Value, json};

pub(crate) fn command(app: &mut AnimationCraftApp, id: &'static str, params: Value) {
    app.exec_ui_command(UiCommand::Engine { id, params });
}

pub(crate) fn connect(app: &mut AnimationCraftApp, mut params: Value) {
    params["load"] = json!(true);
    params["if_not_exists"] = json!(true);
    command(app, "project.add_asset", params);
    app.ui_state.center_tab = CenterTab::OriginalView;
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
    let response = ui
        .add_enabled_ui(enabled && app.ui_state.busy_command.is_none(), |ui| {
            chrome::button(
                ui,
                label,
                matches!(id, "toolbar.import" | "toolbar.export_mp4" | "project.add"),
            )
        })
        .inner;
    let explanation = match id {
        "toolbar.import" | "project.add" => (
            "Import an image (with PhotoCraft background removal) or connect a manifest.",
            "Bring artwork into the project on a transparent background, or connect an existing pose sheet, turnaround, or cutout rig manifest.",
            "Choose a file and optional name. Images run PhotoCraft background removal via intake.validate; .json manifests connect directly.",
        ),
        "toolbar.transparent" => (
            "Process the active source to remove its background.",
            "Prepare artwork for compositing and rigging.",
            "Use on an imported image and inspect the result before continuing.",
        ),
        "toolbar.build_rig" => (
            "Create a proposed cutout rig for the active image.",
            "Separate movable parts for animation.",
            "Click after preparing the artwork. Review generated parts and joints; automatic placement needs checking.",
        ),
        "toolbar.reload" => (
            "Reload the connected source from disk.",
            "Refresh after changing its file externally.",
            "Click to reload; review any changes to the asset.",
        ),
        "toolbar.export_mp4" => (
            "Render the connected cutout project to MP4.",
            "Create a playable video for review.",
            "Choose an output path. The connected animation project determines the rendered motion.",
        ),
        _ => (
            label,
            "Manage this asset in your project.",
            "Click to perform the labeled action.",
        ),
    };
    let disabled_hint = if app.ui_state.busy_command.is_some() {
        "Wait for the active background task to finish."
    } else if id == "toolbar.export_mp4" && app.session.current_asset().is_some() {
        "Export MP4 renders a cutout rig — click 'Build Cutout Rig' first, or use 'Create action reel' in the right inspector."
    } else {
        "Connect or select an asset first."
    };
    let response = chrome::help(response, explanation.0, explanation.1, explanation.2)
        .on_disabled_hover_text(disabled_hint);
    app.automation.add_response(id, "button", label, &response);
    response.clicked()
}

pub(crate) fn header(app: &mut AnimationCraftApp, ui: &mut Ui) {
    crate::menus::menu_bar(app, ui);
    ui.separator();
    let has_project = app.session.project_path.is_some();
    let in_workshop = has_project && app.ui_state.show_workshop && !app.ui_state.standalone_studio;
    let has_asset = app.session.current_asset().is_some();
    let has_rig = app
        .session
        .current_asset()
        .is_some_and(|a| a.cutout_rig.is_some());

    egui::ScrollArea::horizontal()
        .id_salt("header_actions")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("AnimationCraft")
                        .size(16.0)
                        .strong()
                        .color(chrome::TEXT),
                );
                if super::start::project_open(app) {
                    ui.separator();
                    let workshop_tab = ui
                        .add_enabled_ui(has_project, |ui| {
                            chrome::tab(ui, "AnimationProject", in_workshop)
                        })
                        .inner
                        .on_hover_text("AnimationProject Mode: 4-stage song checklist, Video Project server, Assets pool, and Storyboard PDF")
                        .on_disabled_hover_text("Open or create a song project (.acproj) to use AnimationProject mode");
                    app.automation.add_response(
                        "header.mode.workshop",
                        "button",
                        "AnimationProject",
                        &workshop_tab,
                    );
                    if workshop_tab.clicked() {
                        app.ui_state.show_workshop = true;
                        app.ui_state.standalone_studio = false;
                    }

                    let studio_label = match app.session.active_asset.as_deref() {
                        Some(active) => format!("AnimationStudio · {active}"),
                        None => "AnimationStudio".to_string(),
                    };
                    let studio_tab = chrome::tab(ui, &studio_label, !in_workshop).on_hover_text(
                        "AnimationStudio Mode: Full-window 3-column 2D/3D Rigging & Layered Motion Studio",
                    );
                    app.automation.add_response(
                        "header.mode.studio",
                        "button",
                        &studio_label,
                        &studio_tab,
                    );
                    if studio_tab.clicked() {
                        app.ui_state.show_workshop = false;
                        app.ui_state.standalone_studio = true;
                    }
                }

                if in_workshop {
                    super::workshop::header_project_bar(app, ui);
                } else {
                    if !app.session.project_assets.is_empty() {
                        ui.separator();
                        ui.label(RichText::new("Subject:").size(12.0).color(chrome::MUTED));
                        let entries = app.session.project_assets.clone();
                        for entry in entries {
                            let active = app.session.active_asset.as_deref() == Some(entry.id.as_str());
                            let r = ui
                                .selectable_label(
                                    active,
                                    RichText::new(&entry.name).size(12.0).strong(),
                                )
                                .on_hover_text(format!("Switch active animated subject to '{}'", entry.name));
                            app.automation.add_response(
                                format!("header.subject.{}", entry.id),
                                "button",
                                &entry.name,
                                &r,
                            );
                            if r.clicked() && !active {
                                command(app, "project.load_asset", json!({"id": entry.id}));
                            }
                        }
                    }
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
                    let export_label = if has_asset && !has_rig {
                        "Export MP4 (Build Rig First)"
                    } else {
                        "Export MP4"
                    };
                    if action_button(app, ui, "toolbar.export_mp4", export_label, has_rig) {
                        export_mp4(app);
                    }
                }
            });
        });
    ui.horizontal(|ui| {
        if app.ui_state.busy_command.is_some() {
            ui.spinner();
        }
        let message = if app.ui_state.status_line.contains("CLI Bridge listening") {
            "Ready"
        } else {
            &app.ui_state.status_line
        };
        ui.add(egui::Label::new(RichText::new(message).small().color(chrome::MUTED)).truncate())
            .on_hover_text(&app.ui_state.status_line);
    });
}

pub(crate) fn asset_dialogs(app: &mut AnimationCraftApp, ctx: &egui::Context) {
    if app.ui_state.connect_asset_open {
        let mut open = true;
        let mut connected = false;
        egui::Window::new("Import / Connect Asset").open(&mut open).collapsible(false)
            .default_width(460.0).show(ctx, |ui| {
                ui.add_enabled_ui(app.ui_state.busy_command.is_none(), |ui| {
                    ui.label("Image (runs PhotoCraft background removal) or manifest (.json)");
                    ui.horizontal(|ui| {
                        ui.label("Path");
                        let response = ui.text_edit_singleline(&mut app.ui_state.connect_asset_path);
                        app.automation.add_response("project.path", "text", "Asset path", &response);
                        if ui.button("Browse…").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Images / Manifests", &["png", "jpg", "jpeg", "webp", "json"])
                                .pick_file()
                        { app.ui_state.connect_asset_path = path.to_string_lossy().into_owned(); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Name (optional)");
                        let name_resp = ui.text_edit_singleline(&mut app.ui_state.connect_asset_name);
                        app.automation.add_response("project.name", "text", "Asset name", &name_resp);
                    });
                    egui::ComboBox::from_label("Asset type")
                        .selected_text(&app.ui_state.connect_asset_kind).show_ui(ui, |ui| {
                            for (kind, label) in [("auto", "Detect from file"), ("image", "Original image"),
                                ("poses", "Pose sheet"), ("turnaround", "Turnaround"), ("cutout_rig", "Cutout rig")] {
                                ui.selectable_value(&mut app.ui_state.connect_asset_kind, kind.into(), label);
                            }
                        });
                    if action_button(app, ui, "project.connect", "Import Asset", !app.ui_state.connect_asset_path.trim().is_empty()) {
                        let path = app.ui_state.connect_asset_path.trim().to_string();
                        let name = app.ui_state.connect_asset_name.trim().to_string();
                        let path_ref = std::path::Path::new(&path);
                        let is_manifest = path_ref.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
                            || path_ref.is_dir();
                        let count = app.session.project_assets.len();
                        if is_manifest {
                            let mut params = json!({"path": path, "kind": app.ui_state.connect_asset_kind});
                            if !name.is_empty() {
                                params["name"] = json!(name);
                            }
                            connect(app, params);
                            connected = app.session.project_assets.len() > count;
                        } else {
                            let asset_id = animationcraft_engine::commands::asset_id_for_import(
                                (!name.is_empty()).then_some(name.as_str()),
                                path_ref,
                            );
                            command(
                                app,
                                "intake.validate",
                                json!({
                                    "image": path,
                                    "asset": asset_id,
                                    "slot": "auto",
                                    "auto_cut": false
                                }),
                            );
                            connected = app.ui_state.busy_command.is_some()
                                || app.session.project_assets.len() > count;
                        }
                    }
                    ui.small("Images run PhotoCraft background removal via intake.validate; .json manifests connect directly.");
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
    let entries = app.session.project_assets.clone();
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Project assets").size(14.0).strong().color(chrome::TEXT));
            chrome::badge(ui, &entries.len().to_string(), chrome::BLUE);
        });
        ui.add_space(4.0);
        ui.add_enabled_ui(app.ui_state.busy_command.is_none(), |ui| {
            for (index, entry) in entries.iter().enumerate() {
                ui.push_id(&entry.id, |ui| {
                    ui.horizontal(|ui| {
                        let active = app.session.active_asset.as_deref() == Some(&entry.id);
                        let response = ui.add_sized([(ui.available_width() - 40.0).max(40.0), 34.0],
                            egui::Button::selectable(active, RichText::new(&entry.name).size(13.0).strong()).wrap())
                            .on_hover_text(entry.path.as_ref().map(|p| p.display().to_string())
                                .unwrap_or_else(|| format!("Preset: {}", entry.preset.as_deref().unwrap_or("unknown"))));
                        app.automation.add_response(format!("project.load.{}", entry.id), "button", &entry.name, &response);
                        if response.clicked() { command(app, "project.load_asset", json!({"id": entry.id})); }
                        let menu = ui.menu_button("•••", |ui| {
                            if action_button(app, ui, &format!("project.rename.{}", entry.id), "Rename…", true) {
                                app.ui_state.rename_asset = Some((entry.id.clone(), entry.name.clone()));
                                ui.close();
                            }
                            for (suffix, label, enabled, target) in [
                                ("up", "Move up", index > 0, index.saturating_sub(1)),
                                ("down", "Move down", index + 1 < entries.len(), index + 1),
                            ] {
                                if action_button(app, ui, &format!("project.{suffix}.{}", entry.id), label, enabled) {
                                    command(app, "project.update_asset", json!({"id": entry.id, "index": target}));
                                    ui.close();
                                }
                            }
                            ui.separator();
                            if action_button(app, ui, &format!("project.remove.{}", entry.id), "Remove from project", true) {
                                command(app, "project.remove_asset", json!({"id": entry.id}));
                                ui.close();
                            }
                        });
                        app.automation.add_response(format!("project.manage.{}", entry.id), "button", "Manage asset", &menu.response);
                        menu.response.on_hover_text("Rename, reorder, or remove this connection. Removing it does not delete its source file.");
                    });
                });
            }
            if entries.is_empty() { ui.label(RichText::new("No assets connected yet").color(chrome::MUTED)); }
            ui.add_space(4.0);
            if action_button(app, ui, "project.add", "+ Add asset…", true) { app.ui_state.connect_asset_open = true; }
        });
    });
    ui.add_space(10.0);
}
