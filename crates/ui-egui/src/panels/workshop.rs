//! Workshop: the checklist a song must pass before animation (audio, transcript, liner notes,
//! reference images, approved draft PDF). Every button runs the same engine command as the CLI/MCP.
use super::chrome::{self, MUTED, TEXT};
use crate::{AnimationCraftApp, UiCommand};
use egui::{FontId, Rect, RichText, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

/// The open project's checklist, refreshed at most once a second (it reads the project folder).
pub(crate) fn status(app: &mut AnimationCraftApp) -> Option<Value> {
    app.session.project_path.as_ref()?;
    let fresh = app
        .ui_state
        .workshop_status
        .as_ref()
        .is_some_and(|(at, _)| at.elapsed() < Duration::from_secs(1));
    if !fresh {
        let value = app.session.execute("workshop.status", json!({})).ok()?;
        app.ui_state.workshop_status = Some((Instant::now(), value));
    }
    app.ui_state.workshop_status.as_ref().map(|(_, v)| v.clone())
}

/// Whether the Workshop screen replaces the animation workspace this frame.
pub(crate) fn showing(app: &mut AnimationCraftApp) -> bool {
    let complete = status(app).is_some_and(|s| s["complete"] == json!(true));
    app.session.project_path.is_some() && (app.ui_state.show_workshop || !complete)
}

fn run(app: &mut AnimationCraftApp, id: &'static str, params: Value) {
    app.exec_ui_command(UiCommand::Engine { id, params });
    app.ui_state.workshop_status = None;
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
    let width = 620.0_f32.min(area.width() - 32.0);
    let column = Rect::from_min_size(pos2(area.center().x - width / 2.0, area.top() + 36.0), vec2(width, area.height() - 48.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
        ui.label(RichText::new(format!("{title} — workshop")).size(26.0).color(TEXT));
        ui.add_space(4.0);
        ui.label(
            RichText::new("Finish these before animating. Each step runs the same command an agent would use.")
                .size(13.0)
                .color(MUTED),
        );
        ui.add_space(16.0);
        let items = status["items"].as_array().cloned().unwrap_or_default();
        for item in &items {
            row(app, ui, item, ready);
            ui.add_space(8.0);
        }
        ui.add_space(12.0);
        let complete = status["complete"] == json!(true);
        ui.horizontal(|ui| {
            let start = ui
                .add_enabled_ui(complete, |ui| chrome::button(ui, "Start animating", true))
                .inner
                .on_disabled_hover_text("Every item above needs a green check first.");
            app.automation.add_response("workshop.start", "button", "Start animating", &start);
            if start.clicked() {
                app.ui_state.show_workshop = false;
            }
            if app.ui_state.busy_command.is_some() {
                ui.spinner();
            }
        });
    });
}

fn row(app: &mut AnimationCraftApp, ui: &mut Ui, item: &Value, ready: bool) {
    let id = item["id"].as_str().unwrap_or_default().to_string();
    let done = item["done"] == json!(true);
    let label = item["label"].as_str().unwrap_or_default();
    let path = item["path"].as_str().map(str::to_owned);
    let detail = match id.as_str() {
        "liner_notes" | "images" => format!("{} file(s) in {}", item["count"].as_u64().unwrap_or(0), path.clone().unwrap_or_default()),
        "draft" if item["exists"] == json!(true) && !done => "Built — open it, then tick Approved".into(),
        _ => path.clone().unwrap_or_else(|| "Not set".into()),
    };
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (badge, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            chrome::paint_status_badge(ui.painter(), badge.center(), done);
            ui.vertical(|ui| {
                ui.label(RichText::new(label).size(14.0).color(TEXT));
                let g = ui.painter().layout_job({
                    let mut job = egui::text::LayoutJob::simple_singleline(detail.clone(), FontId::proportional(11.5), MUTED);
                    job.wrap = egui::text::TextWrapping::truncate_at_width((ui.available_width() - 190.0).max(80.0));
                    job
                });
                ui.label(g).on_hover_text(&detail);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(ready, |ui| actions(app, ui, &id, item, path.as_deref()));
            });
        });
    });
}

fn button(app: &mut AnimationCraftApp, ui: &mut Ui, id: &str, label: &str, enabled: bool) -> bool {
    let response = ui.add_enabled_ui(enabled, |ui| chrome::button(ui, label, false)).inner;
    app.automation.add_response(format!("workshop.{id}"), "button", label, &response);
    response.clicked()
}

fn actions(app: &mut AnimationCraftApp, ui: &mut Ui, id: &str, item: &Value, path: Option<&str>) {
    match id {
        "audio" => {
            if button(app, ui, "audio.choose", "Choose…", true) {
                let mut dialog = rfd::FileDialog::new().set_title("Song audio").add_filter("Audio", &["wav", "mp3", "flac", "m4a", "aac", "ogg"]);
                if let Some(dir) = music_dir() {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(file) = dialog.pick_file() {
                    run(app, "workshop.set_audio", json!({"path": file}));
                }
            }
        }
        "transcript" => {
            let has_audio = app.session.song_audio.is_some();
            let label = if item["done"] == json!(true) { "Transcribe again" } else { "Transcribe" };
            if button(app, ui, "transcript.run", label, has_audio) {
                run(app, "audio.transcribe", json!({"overwrite": true}));
            }
        }
        "liner_notes" | "images" => {
            let (title, filter): (&str, &[&str]) = if id == "images" {
                ("Reference images", &["png", "jpg", "jpeg", "webp"])
            } else {
                ("Liner notes", &["md", "txt", "pdf", "png", "jpg", "jpeg"])
            };
            if button(app, ui, &format!("{id}.add"), "Add…", true)
                && let Some(files) = rfd::FileDialog::new().set_title(title).add_filter(title, filter).pick_files()
            {
                run(app, "workshop.add", json!({"kind": id, "paths": files}));
            }
        }
        "draft" => {
            let exists = item["exists"] == json!(true);
            let mut approved = item["approved"] == json!(true);
            let check = ui.add_enabled(exists, egui::Checkbox::new(&mut approved, "Approved"));
            app.automation.add_response("workshop.draft.approved", "checkbox", "Approved", &check);
            if check.changed() {
                run(app, "workshop.approve_draft", json!({"approved": approved}));
            }
            if button(app, ui, "draft.open", "Open", exists)
                && let Some(path) = path
            {
                app.ui_state.status_line = match std::process::Command::new("xdg-open").arg(path).spawn() {
                    Ok(_) => format!("Opened {path}"),
                    Err(e) => format!("Could not open {path}: {e}"),
                };
            }
            let label = if exists { "Rebuild" } else { "Build draft" };
            if button(app, ui, "draft.build", label, true) {
                run(app, "workshop.build_draft", json!({}));
            }
        }
        _ => {}
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
            created = app.session.project_path.as_ref() == Some(&target);
            super::start::record_open_project(app);
        }
    });
    app.ui_state.new_project_open = open && !created;
}
