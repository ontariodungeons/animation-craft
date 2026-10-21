//! Workshop: the four stages a song goes through before animation (song, transcription,
//! reference art with the storyboard PDF, animate assets), each shown as inputs ──▶ outputs with an
//! agent brief. Every button runs the same engine command as the CLI/MCP.
use super::chrome::{self, MUTED, TEXT};
use crate::{AnimationCraftApp, UiCommand};
use egui::{FontId, Rect, RichText, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Instant;

/// Where new song projects go unless the user picks another folder.
pub(crate) fn default_projects_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Work/merriweather-videos/projects")
}

fn music_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Music"))
}

/// The open project's checklist, cached until a project/workshop command invalidates it.
pub(crate) fn status(app: &mut AnimationCraftApp) -> Option<Value> {
    app.session.project_path.as_ref()?;
    if app.ui_state.workshop_status.is_none() {
        let value = app.session.execute("workshop.status", json!({})).ok()?;
        app.ui_state.workshop_status = Some((Instant::now(), value));
        app.ui_state.workshop_files = app
            .session
            .execute("workshop.files", json!({}))
            .ok()
            .and_then(|f| f["files"].as_array().cloned())
            .unwrap_or_default();
    }
    app.ui_state.workshop_status.as_ref().map(|(_, v)| v.clone())
}

/// Whether the 2-column `AnimationProject` screen is active this frame.
pub(crate) fn showing(app: &mut AnimationCraftApp) -> bool {
    if !app.ui_state.show_workshop
        || app.ui_state.standalone_studio
        || app.session.project_path.is_none()
    {
        return false;
    }
    let _ = status(app);
    if app.ui_state.active_stage == 0 || app.ui_state.active_stage > 4 {
        app.ui_state.active_stage = 1;
    }
    true
}

fn run(app: &mut AnimationCraftApp, id: &'static str, params: Value) {
    app.exec_ui_command(UiCommand::Engine { id, params });
    app.ui_state.workshop_status = None;
}

/// Top Header Bar (Row 2) when `AnimationProject` mode is active.
pub(crate) fn header_project_bar(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let title = app
        .session
        .project_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled")
        .to_string();
    ui.separator();
    ui.label(RichText::new(&title).size(13.0).strong().color(TEXT));
    if !app.session.project_assets.is_empty() {
        ui.separator();
        ui.label(RichText::new("Subjects:").size(12.0).color(MUTED));
        let entries = app.session.project_assets.clone();
        for entry in entries {
            let active = app.session.active_asset.as_deref() == Some(entry.id.as_str());
            let r = ui
                .selectable_label(active, RichText::new(&entry.name).size(12.0).strong())
                .on_hover_text(format!(
                    "Open '{}' in AnimationStudio",
                    entry.name
                ));
            app.automation.add_response(
                format!("header.subject.{}", entry.id),
                "button",
                &entry.name,
                &r,
            );
            if r.clicked() {
                run(app, "project.load_asset", json!({"id": entry.id}));
                app.ui_state.show_workshop = false;
                app.ui_state.standalone_studio = true;
            }
        }
    }
    ui.separator();
    super::agent::toggle(app, ui);
    if super::header::action_button(app, ui, "toolbar.import", "+ Import Asset", true) {
        app.ui_state.connect_asset_open = true;
    }
}

/// Dedicated Video Project + Web Server card at the top of the Right Column in `AnimationProject`.
fn video_project_bar(app: &mut AnimationCraftApp, ui: &mut Ui, status: &Value, ready: bool) {
    let selected = status["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["selected"] == json!(true))
        .cloned();
    let server = &status["video_server"];
    let server_running = server["running"] == json!(true);
    let server_port = server["port"].as_u64();
    let server_url = server["url"].as_str().map(str::to_owned);

    egui::Frame::new()
        .fill(chrome::SURFACE)
        .stroke(egui::Stroke::new(1.0, chrome::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Video project").size(13.0).strong().color(TEXT));
                for target in status["targets"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                {
                    let name = target["target"].as_str().unwrap_or_default().to_string();
                    let label = match name.as_str() {
                        "remotion" => "Remotion",
                        "filmcraft" => "FilmCraft",
                        _ => "HyperFrames",
                    };
                    let selected_t = target["selected"] == json!(true);
                    let r = ui
                        .add_enabled(
                            ready,
                            egui::Button::selectable(selected_t, RichText::new(label).size(12.0)),
                        )
                        .on_hover_text(
                            target["path"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| format!("Switch video target to {label}")),
                        );
                    app.automation
                        .add_response(format!("workshop.target.{name}"), "button", label, &r);
                    let (b, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
                    chrome::paint_status_badge(ui.painter(), b.center(), target["exists"] == json!(true));
                    if r.clicked() && !selected_t {
                        run(app, "workshop.set_target", json!({"target": name}));
                    }
                }

                if let Some(t) = selected {
                    let target_name = t["target"].as_str().unwrap_or("remotion");
                    let exists = t["exists"] == json!(true);
                    ui.separator();
                    if !exists && button(app, ui, "video.init", "Init project", ready) {
                        run(app, "video.init", json!({"target": target_name}));
                    }
                    if target_name == "filmcraft" {
                        if let Some(fcproj) = t["path"].as_str().map(str::to_owned) {
                            let filmcraft = super::start::launcher_path("filmcraft");
                            if button(app, ui, "video.open", "Open in FilmCraft", filmcraft.is_some())
                                && let Some(exe) = filmcraft
                            {
                                app.ui_state.status_line =
                                    match std::process::Command::new(&exe).arg(&fcproj).spawn() {
                                        Ok(_) => format!("Opened {fcproj} in FilmCraft"),
                                        Err(e) => format!(
                                            "Could not open FilmCraft ({}): {e}",
                                            exe.display()
                                        ),
                                    };
                            }
                        }
                    } else if server_running {
                        let (dot_rect, _) = ui.allocate_exact_size(vec2(12.0, 16.0), Sense::hover());
                        ui.painter().circle_filled(
                            dot_rect.center(),
                            4.5,
                            egui::Color32::from_rgb(46, 204, 113),
                        );
                        let status_text = match server_port {
                            Some(p) => format!("Live :{p}"),
                            None => "Live".to_string(),
                        };
                        let badge_resp = ui.label(
                            RichText::new(&status_text)
                                .size(12.0)
                                .strong()
                                .color(egui::Color32::from_rgb(46, 204, 113)),
                        );
                        app.automation.add_response(
                            "workshop.server.status",
                            "label",
                            &status_text,
                            &badge_resp,
                        );
                        if let Some(url) = server_url
                            && button(app, ui, "video.server_open", "Open in Browser", true)
                        {
                            app.ui_state.status_line =
                                match std::process::Command::new("xdg-open").arg(&url).spawn() {
                                    Ok(_) => format!("Opened {url}"),
                                    Err(e) => format!("Could not open {url}: {e}"),
                                };
                        }
                        if button(app, ui, "video.server_stop", "Stop Server", ready) {
                            run(app, "video.server_stop", json!({}));
                        }
                    } else {
                        let (dot_rect, _) = ui.allocate_exact_size(vec2(12.0, 16.0), Sense::hover());
                        ui.painter().circle_filled(
                            dot_rect.center(),
                            4.5,
                            egui::Color32::from_rgb(120, 128, 140),
                        );
                        let badge_resp =
                            ui.label(RichText::new("Server off").size(12.0).color(MUTED));
                        app.automation.add_response(
                            "workshop.server.status",
                            "label",
                            "Server off",
                            &badge_resp,
                        );
                        if button(app, ui, "video.server_start", "Start Server", ready) {
                            run(app, "video.server_start", json!({"target": target_name}));
                        }
                    }
                }
            });
        });
}

pub(crate) fn workshop_screen(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let area = ui.max_rect();
    app.automation.add("workshop.screen", "panel", "Workshop", area);
    let Some(status) = status(app) else { return };
    let title = app
        .session
        .project_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let ready = app.ui_state.busy_command.is_none();

    let right_w = if app.ui_state.storyboard_open {
        (area.width() * 0.45).clamp(360.0, 700.0)
    } else {
        320.0
    };
    let right = Rect::from_min_max(pos2(area.right() - right_w, area.top()), area.max);
    let left = Rect::from_min_max(area.min, pos2(right.left() - 1.0, area.bottom()));
    ui.painter().line_segment(
        [right.left_top(), right.left_bottom()],
        egui::Stroke::new(1.0, chrome::BORDER),
    );

    let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right.shrink(10.0)));
    right_ui.set_clip_rect(right);
    video_project_bar(app, &mut right_ui, &status, ready);
    right_ui.add_space(8.0);
    assets_panel(app, &mut right_ui, &status, ready);
    right_ui.add_space(8.0);
    storyboard_pane(app, &mut right_ui);

    // Left Panel is a drop target for assets dragged from the Right Column.
    let drop = ui.interact(left, ui.id().with("workshop.drop"), Sense::hover());
    let dragging = egui::DragAndDrop::has_payload_of_type::<String>(ui.ctx());
    if dragging && drop.contains_pointer() {
        ui.painter().rect_stroke(
            left.shrink(4.0),
            8.0,
            egui::Stroke::new(2.0, chrome::TEAL),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(path) = drop.dnd_release_payload::<String>() {
        prompt_animate_image(app, &path);
    }

    let left_inner = left.shrink2(vec2(14.0, 10.0));
    let mut left_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(left_inner)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    left_ui.set_clip_rect(left);

    egui::ScrollArea::vertical()
        .id_salt("workshop.column")
        .auto_shrink([false, false])
        .show(&mut left_ui, |ui| {
            let studio_name = status["studio"]["name"].as_str().unwrap_or("Workshop");
            ui.label(
                RichText::new(format!("{title} — {studio_name}"))
                    .size(20.0)
                    .strong()
                    .color(TEXT),
            );
            match status["studio"]["intent"].as_str() {
                Some(intent) => {
                    let r = ui.add(
                        egui::Label::new(
                            RichText::new(intent.lines().next().unwrap_or_default())
                                .size(12.0)
                                .italics()
                                .color(MUTED),
                        )
                        .truncate(),
                    );
                    r.on_hover_text(format!(
                        "{intent}\n\n{}",
                        status["studio"]["source"].as_str().unwrap_or_default()
                    ));
                }
                None => {
                    ui.label(
                        RichText::new("Add intent.md to this song's folder, or STUDIO.md beside your projects, to tell agents your audience, style and rules.")
                            .size(12.0)
                            .italics()
                            .color(MUTED),
                    );
                }
            }
            ui.add_space(10.0);
            let stages = status["stages"].as_array().cloned().unwrap_or_default();
            for (i, stage) in stages.iter().enumerate() {
                stage_row(app, ui, i + 1, stage, ready);
                ui.add_space(6.0);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let start = ui
                    .add_enabled_ui(ready, |ui| {
                        chrome::button(ui, "Open AnimationStudio", true)
                    })
                    .inner;
                app.automation.add_response(
                    "workshop.start",
                    "button",
                    "Open AnimationStudio",
                    &start,
                );
                if start.clicked() {
                    app.ui_state.show_workshop = false;
                    app.ui_state.standalone_studio = true;
                }
                if app.ui_state.busy_command.is_some() {
                    ui.spinner();
                }
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new("Tip: Drag any cutout from Assets into Stage 4 to create an animated subject, or click a subject chip to open it in AnimationStudio.")
                    .size(11.5)
                    .color(MUTED),
            );
        });
    new_subject_dialog(app, ui.ctx());
    confirm_remove(app, ui.ctx());
}

/// Opens the New Animated Subject prompt for a dropped or selected image.
fn prompt_animate_image(app: &mut AnimationCraftApp, path: &str) {
    let default_name = std::path::Path::new(path)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("subject")
        .to_string();
    app.ui_state.new_subject_path = path.to_string();
    app.ui_state.new_subject_name = default_name;
    app.ui_state.new_subject_target_id = None;
    app.ui_state.new_subject_open = true;
}

/// Commits the dropped/selected image as a new or existing animated subject in Stage 4 using the
/// exact same engine commands as the CLI (`project.add_asset` with `load: true` or `intake.validate`).
fn commit_animate_subject(app: &mut AnimationCraftApp, path: &str, name: &str, target_id: Option<&str>, open_studio: bool) {
    let clean_name = if name.trim().is_empty() { "subject" } else { name.trim() };
    if let Some(existing_id) = target_id {
        run(
            app,
            "intake.validate",
            json!({"image": path, "asset": existing_id, "attach_to_active": true, "slot": "auto"}),
        );
        run(app, "project.load_asset", json!({"id": existing_id}));
    } else {
        run(
            app,
            "project.add_asset",
            json!({
                "path": path,
                "name": clean_name,
                "kind": "auto",
                "if_not_exists": true,
                "load": true
            }),
        );
    }
    if open_studio {
        app.ui_state.active_stage = 4;
        app.ui_state.show_workshop = false;
        app.ui_state.standalone_studio = true;
        app.ui_state.status_line = format!("Animating {clean_name} in AnimationStudio");
    } else {
        app.ui_state.status_line = format!("Added subject '{clean_name}' to Stage 4 (Animate assets).");
    }
}

/// Modal dialog when dropping an asset into Stage 4 to name/group the animated subject.
fn new_subject_dialog(app: &mut AnimationCraftApp, ctx: &egui::Context) {
    if !app.ui_state.new_subject_open {
        return;
    }
    let path = app.ui_state.new_subject_path.clone();
    let mut open = true;
    let mut closed = false;
    let existing_subjects: Vec<(String, String)> = app
        .session
        .project_assets
        .iter()
        .map(|a| (a.id.clone(), a.name.clone()))
        .collect();
    egui::Window::new("New Animated Subject")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(tex) = app.load_texture(ui.ctx(), &path) {
                    ui.add(egui::Image::new(&tex).max_size(vec2(72.0, 72.0)));
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new("Create or attach animated subject").size(13.0).strong().color(TEXT));
                    ui.label(RichText::new(&path).size(10.5).color(MUTED));
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Subject name:");
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut app.ui_state.new_subject_name)
                        .hint_text("e.g. bee, older-fish")
                        .desired_width(240.0),
                );
                app.automation.add_response("workshop.subject.name", "text", "Subject name", &edit);
            });
            if !existing_subjects.is_empty() {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Mode:").size(11.5).color(MUTED));
                    let is_new = app.ui_state.new_subject_target_id.is_none();
                    if ui.selectable_label(is_new, "New subject").clicked() {
                        app.ui_state.new_subject_target_id = None;
                    }
                    for (id, label) in &existing_subjects {
                        let selected = app.ui_state.new_subject_target_id.as_deref() == Some(id.as_str());
                        if ui.selectable_label(selected, format!("Attach to '{label}'")).clicked() {
                            app.ui_state.new_subject_target_id = Some(id.clone());
                        }
                    }
                });
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let name = app.ui_state.new_subject_name.clone();
                let target_id = app.ui_state.new_subject_target_id.clone();
                let add_stay = chrome::button(ui, "Add to Animate assets", true);
                app.automation.add_response("workshop.subject.confirm", "button", "Add to Animate assets", &add_stay);
                if add_stay.clicked() {
                    commit_animate_subject(app, &path, &name, target_id.as_deref(), false);
                    closed = true;
                }
                let add_open = chrome::button(ui, "Add & Open Rig View", false);
                app.automation.add_response("workshop.subject.open_rig", "button", "Add & Open Rig View", &add_open);
                if add_open.clicked() {
                    commit_animate_subject(app, &path, &name, target_id.as_deref(), true);
                    closed = true;
                }
                let cancel = chrome::button(ui, "Cancel", false);
                if cancel.clicked() {
                    closed = true;
                }
            });
        });
    if !open || closed {
        app.ui_state.new_subject_open = false;
    }
}

/// One compact stage: number, title, ✓/✗ and reason; inputs ─▶ outputs; brief and actions.
fn stage_row(app: &mut AnimationCraftApp, ui: &mut Ui, number: usize, stage: &Value, ready: bool) {
    let id = stage["id"].as_str().unwrap_or_default().to_string();
    let done = stage["done"] == json!(true);
    let selected = app.ui_state.active_stage == number;
    let border_col = if selected { chrome::BLUE } else { chrome::BORDER };
    let fill_col = if selected {
        egui::Color32::from_rgb(29, 36, 48)
    } else {
        chrome::SURFACE
    };
    let frame_resp = egui::Frame::new()
        .fill(fill_col)
        .stroke(egui::Stroke::new(if selected { 1.5 } else { 1.0 }, border_col))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (badge, badge_resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                ui.painter().circle_filled(
                    badge.center(),
                    10.0,
                    if selected { chrome::BLUE.gamma_multiply(0.35) } else { chrome::BORDER },
                );
                ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, number.to_string(), FontId::proportional(11.5), TEXT);
                let title_resp = ui.add(
                    egui::Label::new(
                        RichText::new(stage["title"].as_str().unwrap_or_default())
                            .size(14.0)
                            .strong()
                            .color(TEXT),
                    )
                    .sense(Sense::click()),
                );
                if badge_resp.clicked() || title_resp.clicked() {
                    app.ui_state.active_stage = number;
                }
                if stage["optional"] != json!(true) || done {
                    let (b, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    chrome::paint_status_badge(ui.painter(), b.center(), done);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let menu = ui.add_enabled_ui(ready, |ui| ui.menu_button(RichText::new("...").size(12.0), |ui| stage_actions(app, ui, &id, stage))).inner;
                    app.automation.add_response(format!("workshop.{id}.actions"), "button", "Stage actions", &menu.response);
                    let brief = stage["brief"].as_str().unwrap_or_default().to_string();
                    let copy = ui.small_button("Copy brief").on_hover_text(&brief);
                    app.automation.add_response(format!("workshop.row.{id}.brief"), "button", "Copy agent brief", &copy);
                    if copy.clicked() {
                        ui.ctx().copy_text(brief);
                        app.ui_state.status_line = format!("Copied the stage {number} brief for an agent");
                    }
                    if id == "reference" {
                        let has_pdf = stage["outputs"].as_array().and_then(|o| o.first()).is_some_and(|c| c["exists"] == json!(true));
                        let mut approved = stage["approved"] == json!(true);
                        let check = ui
                            .add_enabled(has_pdf && ready, egui::Checkbox::new(&mut approved, "Approved"))
                            .on_disabled_hover_text("Build storyboard.pdf first before approving.");
                        app.automation.add_response("workshop.row.storyboard.approved", "checkbox", "Approved", &check);
                        if check.changed() {
                            run(app, "workshop.approve_draft", json!({"approved": approved}));
                            app.ui_state.storyboard_info = None;
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add_space(26.0);
                let reason = RichText::new(stage["why"].as_str().unwrap_or_default()).size(11.5).color(if done { MUTED } else { TEXT });
                ui.add(egui::Label::new(reason).truncate());
            });
            ui.horizontal(|ui| {
                ui.add_space(26.0);
                ui.horizontal_wrapped(|ui| {
                    flow_line(app, ui, &id, stage);
                });
            });
        });
    frame_resp.response.context_menu(|ui| {
        stage_actions(app, ui, &id, stage);
    });
}

/// inputs ─▶ outputs as wrapped short names; images preview on hover.
fn flow_line(app: &mut AnimationCraftApp, ui: &mut Ui, id: &str, stage: &Value) {
    let inputs = stage["inputs"].as_array().cloned().unwrap_or_default();
    let outputs = if id == "animate" { Vec::new() } else { stage["outputs"].as_array().cloned().unwrap_or_default() };
    ui.spacing_mut().item_spacing.x = 6.0;
    let summarize = |ui: &mut Ui, app: &mut AnimationCraftApp, items: &[Value], key: &str| {
        if items.is_empty() {
            ui.label(RichText::new("—").size(11.5).color(MUTED));
        } else if items.len() > 3 {
            // Many files read as a count; each still previews on hover.
            let label = if key.starts_with("reference") { format!("{} panels", items.len()) } else { format!("{} files", items.len()) };
            let r = ui.label(RichText::new(label).size(11.5).color(chrome::BLUE)).interact(Sense::hover());
            r.on_hover_ui(|ui| {
                for item in items {
                    let begins = item["begins"].as_array().map(|b| b.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" · ")).unwrap_or_default();
                    ui.label(format!("{}  {begins}", item["name"].as_str().unwrap_or_default()));
                }
            });
        } else {
            for (i, item) in items.iter().enumerate() {
                file_name_chip(app, ui, item, &format!("{key}.{i}"));
            }
        }
    };
    summarize(ui, app, &inputs, &format!("{id}.in"));
    if !outputs.is_empty() {
        arrow(ui);
        summarize(ui, app, &outputs, &format!("{id}.out"));
    }
    if id == "animate" {
        let assets = stage["assets"].as_array().cloned().unwrap_or_default();
        arrow(ui);
        if assets.is_empty() {
            ui.label(RichText::new("no animated subjects yet (drag from Assets)").size(11.5).italics().color(MUTED));
        }
        for asset in assets {
            let asset_id = asset["id"].as_str().unwrap_or_default().to_string();
            let asset_name = asset["name"].as_str().unwrap_or_default().to_string();
            let animatable = asset["animatable"] == json!(true);
            let parts = asset["parts"].as_u64().map(|p| format!(" · {p} parts")).unwrap_or_default();
            let tracks = asset["tracks"]
                .as_u64()
                .filter(|&t| t > 0)
                .map(|t| format!(" · {t} tracks"))
                .unwrap_or_default();
            let motion = motion_symbols(asset["controls"].as_array().map(Vec::as_slice).unwrap_or(&[]));
            let motion = if motion.is_empty() { String::new() } else { format!(" · {motion}") };
            let chip_label = if animatable {
                format!("{asset_name}{parts}{tracks}{motion}")
            } else {
                format!("{asset_name} · static image (needs parts & rig)")
            };
            let fill = if animatable {
                egui::Color32::from_rgb(34, 44, 58)
            } else {
                egui::Color32::from_rgb(48, 36, 36)
            };
            let hover_tip = if animatable {
                "Rigged subject (✓ parts, pivots & motion tracks) — click to open in AnimationStudio"
            } else {
                "Single static image (0 cut parts). Static layers stay on video tracks; click to break apart or rig in AnimationStudio."
            };
            let r = ui
                .add(egui::Button::new(RichText::new(chip_label).size(11.5).color(TEXT)).fill(fill))
                .on_hover_text(hover_tip);
            app.automation.add_response(format!("workshop.subject.{asset_id}"), "button", &asset_name, &r);
            if r.clicked() {
                run(app, "project.load_asset", json!({"id": asset_id}));
                app.ui_state.active_stage = 4;
                app.ui_state.show_workshop = false;
                app.ui_state.standalone_studio = true;
            }
        }
    }
}

/// A short painted arrow (the UI font has no box-drawing characters).
fn arrow(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(26.0, 14.0), Sense::hover());
    let y = r.center().y;
    let stroke = egui::Stroke::new(1.4, MUTED);
    ui.painter().line_segment([pos2(r.left() + 2.0, y), pos2(r.right() - 3.0, y)], stroke);
    ui.painter().line_segment([pos2(r.right() - 8.0, y - 4.0), pos2(r.right() - 3.0, y)], stroke);
    ui.painter().line_segment([pos2(r.right() - 8.0, y + 4.0), pos2(r.right() - 3.0, y)], stroke);
}

fn file_name_chip(app: &mut AnimationCraftApp, ui: &mut Ui, chip: &Value, key: &str) {
    let path = chip["path"].as_str().unwrap_or_default().to_string();
    let name = chip["name"].as_str().unwrap_or_default().to_string();
    let exists = chip["exists"] != json!(false);
    let text = RichText::new(&name).size(11.5).color(if exists { chrome::BLUE } else { MUTED });
    let r = ui.add(egui::Label::new(if exists { text } else { text.italics() }).sense(Sense::click()));
    let image = ["png", "jpg", "jpeg", "webp"].iter().any(|e| name.to_ascii_lowercase().ends_with(e));
    let r = if image && exists && r.hovered() && let Some(tex) = app.load_texture(ui.ctx(), &path) {
        r.on_hover_ui(|ui| {
            ui.add(egui::Image::new(&tex).max_size(vec2(260.0, 200.0)));
            ui.label(RichText::new(&path).size(10.5).color(MUTED));
        })
    } else {
        r.on_hover_text(format!(
            "{path}{}",
            if exists { "\nClick to open · right-click to copy the path" } else { "\n(not made yet)" }
        ))
    };
    app.automation.add_response(format!("workshop.chip.{key}"), "button", &name, &r);
    if r.clicked() && exists {
        app.ui_state.status_line = match std::process::Command::new("xdg-open").arg(&path).spawn() {
            Ok(_) => format!("Opened {path}"),
            Err(e) => format!("Could not open {path}: {e}"),
        };
    }
    if r.secondary_clicked() {
        ui.ctx().copy_text(path.clone());
        app.ui_state.status_line = format!("Copied {path}");
    }
}

/// Dedicated right-side Assets block: shows static/cutout layers and lets the user drag them into
/// Stage 4 ("Animate assets") to create or attach to an animated subject.
fn assets_panel(app: &mut AnimationCraftApp, ui: &mut Ui, status: &Value, ready: bool) {
    let assets = status["asset_files"].as_array().cloned().unwrap_or_default();
    egui::Frame::new()
        .fill(chrome::SURFACE)
        .stroke(egui::Stroke::new(1.0, chrome::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Assets ({})", assets.len())).size(13.5).strong().color(TEXT));
                ui.label(RichText::new("· click or drag to animate").size(11.0).color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(app, ui, "assets.panel_add", "+ Add files…", ready)
                        && let Some(files) = pick_files("Static & cutout assets", &["png", "jpg", "jpeg", "webp"])
                    {
                        run(app, "workshop.add", json!({"kind": "assets", "paths": files}));
                    }
                    let import_resp = ui
                        .add_enabled(
                            ready,
                            egui::Button::new(RichText::new("+ Import").size(11.5).strong().color(TEXT))
                                .fill(egui::Color32::from_rgb(29, 83, 83))
                                .stroke(egui::Stroke::new(1.0, chrome::TEAL.gamma_multiply(0.65)))
                                .min_size(vec2(0.0, 26.0)),
                        )
                        .on_hover_text("Import an image (with PhotoCraft background removal) or connect a rig/motion manifest (.json).");
                    app.automation.add_response("toolbar.import", "button", "+ Import Asset", &import_resp);
                    if import_resp.clicked() {
                        app.ui_state.connect_asset_open = true;
                    }
                });
            });
            ui.add_space(4.0);
            if assets.is_empty() {
                ui.label(
                    RichText::new("No static or cutout assets in assets/ yet. Click '+ Add files…' or extract cutouts from storyboard panels.")
                        .size(11.5)
                        .italics()
                        .color(MUTED),
                );
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("workshop.assets_scroll")
                .max_height(135.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let mut decoded_this_frame = 0usize;
                        for (idx, item) in assets.iter().enumerate() {
                            let path = item["path"].as_str().unwrap_or_default().to_string();
                            let name = item["name"].as_str().unwrap_or_default().to_string();
                            let thumb = app.load_thumbnail_progressive(
                                ui.ctx(),
                                &path,
                                &mut decoded_this_frame,
                            );
                            let card_size = vec2(96.0, 68.0);
                            let (rect, response) = ui.allocate_exact_size(card_size, Sense::click_and_drag());
                            app.automation.add_response(format!("workshop.asset_card.{idx}"), "button", &name, &response);
                            let stroke_col = if response.hovered() || response.dragged() { chrome::TEAL } else { chrome::BORDER };
                            ui.painter().rect(
                                rect,
                                5.0,
                                egui::Color32::from_rgb(24, 30, 40),
                                egui::Stroke::new(if response.hovered() { 1.8 } else { 1.0 }, stroke_col),
                                egui::StrokeKind::Inside,
                            );
                            if let Some(tex) = &thumb {
                                let [tw, th] = tex.size();
                                let box_r = Rect::from_min_max(rect.min + vec2(4.0, 3.0), pos2(rect.right() - 4.0, rect.bottom() - 18.0));
                                let scale = (box_r.width() / tw.max(1) as f32).min(box_r.height() / th.max(1) as f32);
                                let img_size = vec2(tw as f32 * scale, th as f32 * scale);
                                let img_rect = Rect::from_center_size(box_r.center(), img_size);
                                ui.painter().image(
                                    tex.id(),
                                    img_rect,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            }
                            let short_name = if name.chars().count() > 13 {
                                format!("{}…", name.chars().take(12).collect::<String>())
                            } else {
                                name.clone()
                            };
                            ui.painter().text(
                                pos2(rect.center().x, rect.bottom() - 8.0),
                                egui::Align2::CENTER_CENTER,
                                short_name,
                                FontId::proportional(10.5),
                                TEXT,
                            );
                            response.dnd_set_drag_payload(path.clone());
                            if response.dragged()
                                && let (Some(tex), Some(pointer)) = (&thumb, ui.ctx().pointer_interact_pos())
                            {
                                let [tw, th] = tex.size();
                                let s = 96.0 / tw.max(th).max(1) as f32;
                                let r = Rect::from_min_size(pointer + vec2(12.0, 12.0), vec2(tw as f32 * s, th as f32 * s));
                                ui.ctx()
                                    .layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("assets.drag")))
                                    .image(
                                        tex.id(),
                                        r,
                                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                        egui::Color32::from_white_alpha(230),
                                    );
                            }
                            let response = match &thumb {
                                Some(tex) if !response.dragged() => response.on_hover_ui(|ui| {
                                    ui.add(egui::Image::new(tex).max_size(vec2(240.0, 180.0)));
                                    ui.label(RichText::new(&name).size(11.5).strong().color(TEXT));
                                    ui.label(RichText::new(&path).size(10.0).color(MUTED));
                                    ui.label(RichText::new("Drag onto Stage 4 or click/right-click to animate").size(10.5).color(chrome::TEAL));
                                }),
                                _ => response,
                            };
                            if response.clicked() {
                                prompt_animate_image(app, &path);
                            }
                            response.context_menu(|ui| {
                                if ui.button("Animate as new subject…").clicked() {
                                    prompt_animate_image(app, &path);
                                    ui.close();
                                }
                                if ui.button("Copy path").clicked() {
                                    ui.ctx().copy_text(path.clone());
                                    app.ui_state.status_line = format!("Copied {path}");
                                    ui.close();
                                }
                                if ui.button("Open").clicked() {
                                    let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Remove…").clicked() {
                                    app.ui_state.workshop_remove = Some(path.clone());
                                    ui.close();
                                }
                            });
                        }
                    });
                });
        });
}

/// Motion symbols for the controls an asset's rig actually drives.
fn motion_symbols(controls: &[Value]) -> String {
    controls
        .iter()
        .filter_map(Value::as_str)
        .map(|c| match c {
            "head_yaw" => "head ↔",
            "head_pitch" => "head ↕",
            "eye_blink" | "eye_gaze" => "eyes ↔",
            "smile_open" => "mouth ↕",
            "arm_spread" | "elbow_bend" => "arms ↕",
            "knee_bend" => "legs ↕",
            "ear_splay" | "ear_pitch" => "ears ↔",
            other => other,
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("  ")
}

/// A secondary button as a single widget (not a nested scope), so wrapped rows can wrap it.
fn button(app: &mut AnimationCraftApp, ui: &mut Ui, id: &str, label: &str, enabled: bool) -> bool {
    let widget = egui::Button::new(RichText::new(label).size(12.0).strong().color(TEXT))
        .fill(egui::Color32::from_rgb(37, 46, 60))
        .stroke(egui::Stroke::new(1.0, chrome::BORDER))
        .min_size(vec2(0.0, 28.0));
    let response = ui.add_enabled(enabled, widget).on_hover_cursor(egui::CursorIcon::PointingHand);
    app.automation.add_response(format!("workshop.{id}"), "button", label, &response);
    response.clicked()
}

fn pick_files(title: &str, filter: &[&str]) -> Option<Vec<PathBuf>> {
    // The desktop file picker can hand back an empty list when closed: treat that as cancel.
    rfd::FileDialog::new().set_title(title).add_filter(title, filter).pick_files().filter(|f| !f.is_empty())
}

fn stage_actions(app: &mut AnimationCraftApp, ui: &mut Ui, id: &str, stage: &Value) {
    match id {
        "song" => {
            if button(app, ui, "audio.choose", "Choose audio…", true) {
                let mut dialog = rfd::FileDialog::new().set_title("Song audio").add_filter("Audio", &["wav", "mp3", "flac", "m4a", "aac", "ogg"]);
                if let Some(dir) = music_dir() {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(file) = dialog.pick_file() {
                    run(app, "workshop.set_audio", json!({"path": file}));
                }
            }
            if button(app, ui, "liner_notes.add", "Add liner notes…", true)
                && let Some(files) = pick_files("Liner notes", &["md", "txt", "pdf", "png", "jpg", "jpeg"])
            {
                run(app, "workshop.add", json!({"kind": "liner_notes", "paths": files}));
            }
        }
        "transcript" => {
            let has_audio = app.session.song_audio.is_some();
            let label = if stage["done"] == json!(true) { "Transcribe again" } else { "Transcribe" };
            if button(app, ui, "transcript.run", label, has_audio) {
                run(app, "audio.transcribe", json!({"overwrite": true}));
            }
            if button(app, ui, "transcript.import", "Import transcript…", true)
                && let Some(file) = rfd::FileDialog::new().set_title("Transcript (.transcript.json)").add_filter("Transcript", &["json"]).pick_file()
            {
                // Bring the FilmCraft project made with it, when it sits beside the transcript.
                let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default().replace(".transcript.json", ".fcproj");
                let fcproj = file.with_file_name(name);
                let mut params = json!({"path": file});
                if fcproj.is_file() {
                    params["filmcraft_project"] = json!(fcproj);
                }
                run(app, "workshop.import_transcript", params);
            }
        }
        "reference" => {
            if button(app, ui, "panels.split", "Split storyboard grid…", true)
                && let Some(grid) = rfd::FileDialog::new().set_title("Storyboard grid (3 × 2)").add_filter("Image", &["png", "jpg", "jpeg", "webp"]).pick_file()
            {
                run(app, "workshop.split_panels", json!({"image": grid, "columns": 3, "rows": 2}));
            }
            if button(app, ui, "panels.add", "Add panels…", true)
                && let Some(files) = pick_files("Storyboard panels", &["png", "jpg", "jpeg", "webp"])
            {
                run(app, "workshop.add", json!({"kind": "panels", "paths": files}));
            }
            if button(app, ui, "assets.add", "Add static assets…", true)
                && let Some(files) = pick_files("Static assets", &["png", "jpg", "jpeg", "webp"])
            {
                run(app, "workshop.add", json!({"kind": "assets", "paths": files}));
            }
            let has_pdf = stage["outputs"].as_array().and_then(|o| o.first()).is_some_and(|c| c["exists"] == json!(true));
            if button(app, ui, "storyboard.build", if has_pdf { "Rebuild storyboard" } else { "Build storyboard" }, true) {
                run(app, "workshop.build_storyboard", json!({}));
                app.ui_state.storyboard_info = None;
            }
        }
        _ => {
            if button(app, ui, "asset.import", "Import asset…", true) {
                app.ui_state.connect_asset_open = true;
            }
        }
    }
}

/// The storyboard PDF, page by page (previews rendered by pdfcraft-cli), with scene navigation.
fn storyboard_pane(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let open = app.ui_state.storyboard_open;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Storyboard").size(14.5).strong().color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let toggle = ui
                .button(if open { "Hide Storyboard" } else { "Show Storyboard" })
                .on_hover_text("Show or hide the interactive Storyboard PDF pane");
            app.automation
                .add_response("workshop.storyboard.toggle", "button", "Toggle storyboard", &toggle);
            if toggle.clicked() {
                app.ui_state.storyboard_open = !open;
            }
        });
    });
    if !open {
        return;
    }
    if app.ui_state.storyboard_info.is_none() && app.ui_state.busy_command.is_none() {
        app.ui_state.storyboard_checked = Some(Instant::now());
        app.ui_state.storyboard_info = app.session.execute("workshop.storyboard", json!({})).ok();
    }
    let Some(info) = app.ui_state.storyboard_info.clone() else { return };
    let previews: Vec<String> = info["previews"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
    ui.add_space(4.0);
    if previews.is_empty() {
        ui.label(RichText::new("Build the storyboard in Stage 3 to see it here.").size(12.0).color(MUTED));
        return;
    }
    let pages = previews.len();
    let page = app.ui_state.storyboard_page.min(pages - 1);
    ui.horizontal_wrapped(|ui| {
        let titles = info["scene_titles"].as_array().cloned().unwrap_or_default();
        // Page 1 is the lyrics index, then the "Cast & sets" overview, then one page per scene.
        let first_scene = info["first_scene_page"].as_u64().unwrap_or(2) as usize - 1;
        let mut nav = vec![("Lyrics".to_string(), 0usize), ("Cast & sets".to_string(), 1)];
        nav.extend(titles.iter().enumerate().filter_map(|(i, t)| Some((t.as_str()?.split(" · ").take(2).collect::<Vec<_>>().join(" · "), first_scene + i))));
        nav.push(("Agent brief".into(), pages - 1));
        for (label, target) in nav {
            let r = ui.selectable_label(page == target, RichText::new(label).size(11.5));
            app.automation.add_response(format!("workshop.storyboard.page.{target}"), "button", "Storyboard page", &r);
            if r.clicked() {
                app.ui_state.storyboard_page = target;
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.add_enabled(page > 0, egui::Button::new("<")).clicked() {
            app.ui_state.storyboard_page = page - 1;
        }
        ui.label(RichText::new(format!("page {} of {pages}", page + 1)).size(11.5).color(MUTED));
        if ui.add_enabled(page + 1 < pages, egui::Button::new(">")).clicked() {
            app.ui_state.storyboard_page = page + 1;
        }
        let status = if info["approved"] == json!(true) { "approved" } else { "draft — needs review" };
        ui.label(RichText::new(status).size(11.5).color(if info["approved"] == json!(true) { chrome::TEAL } else { MUTED }));
        if let Some(pdf) = info["pdf"].as_str()
            && ui.button("Open PDF").clicked()
        {
            app.ui_state.status_line = match std::process::Command::new("xdg-open").arg(pdf).spawn() {
                Ok(_) => format!("Opened {pdf}"),
                Err(e) => format!("Could not open {pdf}: {e}"),
            };
        }
    });
    let Some(tex) = app.load_texture(ui.ctx(), &previews[page]) else { return };
    let [w, h] = tex.size();
    let width = ui.available_width();
    let size = vec2(width, width * h as f32 / w.max(1) as f32);
    let page_size = info["layout"]["page_size"].as_array().map(|a| (a[0].as_f64().unwrap_or(842.0) as f32, a[1].as_f64().unwrap_or(595.0) as f32));
    let images: Vec<Value> = info["layout"]["images"].as_array().into_iter().flatten().filter(|i| i["page"] == json!(page + 1)).cloned().collect();
    egui::ScrollArea::vertical().id_salt("workshop.storyboard.page").show(ui, |ui| {
        let page_view = ui.add(egui::Image::new(&tex).fit_to_exact_size(size));
        app.automation.add_response("workshop.storyboard.view", "image", "Storyboard page", &page_view);
        let shown = page_view.rect;
        let Some((pw, ph)) = page_size else { return };
        // Each image on the page is a live item: hover previews it, drag it to the left side or
        // right-click it to animate, copy, open or remove it.
        for (n, item) in images.iter().enumerate() {
            let Some(r) = item["rect"].as_array().filter(|r| r.len() == 4) else { continue };
            let at = |x: &Value, y: &Value| pos2(shown.left() + x.as_f64().unwrap_or(0.0) as f32 / pw * shown.width(), shown.top() + y.as_f64().unwrap_or(0.0) as f32 / ph * shown.height());
            let rect = Rect::from_two_pos(at(&r[0], &r[1]), at(&r[2], &r[3]));
            let path = item["path"].as_str().unwrap_or_default().to_string();
            let response = ui.interact(rect, ui.id().with(("storyboard.image", page, n)), Sense::click_and_drag());
            app.automation.add_response(format!("workshop.storyboard.image.{n}"), "image", &path, &response);
            if response.hovered() || response.dragged() {
                ui.painter().rect_stroke(rect.expand(2.0), 3.0, egui::Stroke::new(2.0, chrome::TEAL), egui::StrokeKind::Outside);
            }
            response.dnd_set_drag_payload(path.clone());
            let thumb = if response.hovered() || response.dragged() {
                app.load_texture(ui.ctx(), &path)
            } else {
                None
            };
            if response.dragged()
                && let (Some(tex), Some(pointer)) = (&thumb, ui.ctx().pointer_interact_pos())
            {
                let [tw, th] = tex.size();
                let s = 120.0 / tw.max(th).max(1) as f32;
                let r = Rect::from_min_size(pointer + vec2(12.0, 12.0), vec2(tw as f32 * s, th as f32 * s));
                ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("storyboard.drag")))
                    .image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::from_white_alpha(230));
            }
            let response = match &thumb {
                Some(tex) if !response.dragged() => response.on_hover_ui(|ui| {
                    ui.add(egui::Image::new(tex).max_size(vec2(280.0, 220.0)));
                    ui.label(RichText::new(&path).size(10.5).color(MUTED));
                    ui.label(RichText::new("Drag to the left side or right-click to animate").size(10.5).color(MUTED));
                }),
                _ => response,
            };
            response.context_menu(|ui| {
                if ui.button("Animate this…").clicked() {
                    prompt_animate_image(app, &path);
                    ui.close();
                }
                if ui.button("Copy path").clicked() {
                    ui.ctx().copy_text(path.clone());
                    app.ui_state.status_line = format!("Copied {path}");
                    ui.close();
                }
                if ui.button("Open").clicked() {
                    app.ui_state.status_line = match std::process::Command::new("xdg-open").arg(&path).spawn() {
                        Ok(_) => format!("Opened {path}"),
                        Err(e) => format!("Could not open {path}: {e}"),
                    };
                    ui.close();
                }
                ui.separator();
                if ui.button("Remove…").clicked() {
                    app.ui_state.workshop_remove = Some(path.clone());
                    ui.close();
                }
            });
        }
    });
}

fn confirm_remove(app: &mut AnimationCraftApp, ctx: &egui::Context) {
    let Some(path) = app.ui_state.workshop_remove.clone() else { return };
    let audio = app.session.song_audio.as_ref().is_some_and(|a| a.display().to_string() == path);
    let mut open = true;
    let mut decided = false;
    egui::Window::new("Remove file?")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
        ui.label(&path);
        ui.add_space(4.0);
        ui.label(if audio {
            "The project stops pointing to this song. The audio file itself is not touched."
        } else {
            "The file moves to the desktop trash, where it can be restored."
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let remove = chrome::button(ui, if audio { "Unlink" } else { "Move to trash" }, true);
            app.automation.add_response("workshop.remove.confirm", "button", "Confirm remove", &remove);
            if remove.clicked() {
                run(app, "workshop.remove", json!({"path": path}));
                decided = true;
            }
            let cancel = chrome::button(ui, "Cancel", false);
            app.automation.add_response("workshop.remove.cancel", "button", "Cancel remove", &cancel);
            if cancel.clicked() {
                decided = true;
            }
        });
    });
    if !open || decided {
        app.ui_state.workshop_remove = None;
    }
}

/// New song project: a name and the song's audio; the folder is created under the projects root.
pub(crate) fn new_project_dialog(app: &mut AnimationCraftApp, ctx: &egui::Context) {
    if !app.ui_state.new_project_open {
        return;
    }
    let mut open = true;
    let mut created = false;
    egui::Window::new("New song project").open(&mut open).collapsible(false).resizable(false).default_width(480.0).show(ctx, |ui| {
        egui::Grid::new("new_project_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label("Song name");
            let name = ui.add(egui::TextEdit::singleline(&mut app.ui_state.new_project_name).hint_text("Tideo").desired_width(300.0));
            app.automation.add_response("new_project.name", "text", "Song name", &name);
            ui.end_row();
            ui.label("Song audio");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut app.ui_state.new_project_audio).desired_width(230.0));
                if ui.button("Browse…").clicked() {
                    let mut dialog = rfd::FileDialog::new().add_filter("Audio", &["wav", "mp3", "flac", "m4a", "aac", "ogg"]);
                    if let Some(dir) = music_dir() {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(file) = dialog.pick_file() {
                        app.ui_state.new_project_audio = file.display().to_string();
                    }
                }
            });
            ui.end_row();
            ui.label("Folder");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut app.ui_state.new_project_root).desired_width(230.0));
                if ui.button("Browse…").clicked()
                    && let Some(dir) = rfd::FileDialog::new().set_directory(&app.ui_state.new_project_root).pick_folder()
                {
                    app.ui_state.new_project_root = dir.display().to_string();
                }
            });
            ui.end_row();
        });
        let name = app.ui_state.new_project_name.trim().to_string();
        let slug = animationcraft_engine::commands::sanitize_asset_id(&name).replace('_', "-");
        let target = PathBuf::from(app.ui_state.new_project_root.trim()).join(&slug).join(format!("{name}.acproj"));
        ui.add_space(6.0);
        if !name.is_empty() {
            ui.label(RichText::new(format!("Creates {}", target.display())).size(11.5).color(MUTED));
        }
        ui.small("The audio stays where it is; the project only points to it.");
        ui.add_space(6.0);
        let ok = !name.is_empty() && !app.ui_state.new_project_audio.trim().is_empty();
        let create = ui.add_enabled_ui(ok, |ui| chrome::button(ui, "Create project", true)).inner;
        app.automation.add_response("new_project.create", "button", "Create project", &create);
        if create.clicked() {
            run(app, "project.new", json!({"path": target, "audio": app.ui_state.new_project_audio.trim()}));
            run(app, "video.init", json!({}));
            app.ui_state.show_workshop = true;
            created = app.session.project_path.as_ref() == Some(&target);
            super::start::record_open_project(app);
        }
    });
    app.ui_state.new_project_open = open && !created;
}
