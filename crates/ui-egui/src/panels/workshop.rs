//! Workshop: the four stages a song goes through before animation (song, transcription,
//! reference art with the storyboard PDF, animate assets), each shown as inputs ──▶ outputs with an
//! agent brief. Every button runs the same engine command as the CLI/MCP.
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
        app.ui_state.workshop_files = app
            .session
            .execute("workshop.files", json!({}))
            .ok()
            .and_then(|f| f["files"].as_array().cloned())
            .unwrap_or_default();
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
    // Stages on the left (also the drop target); video project and storyboard on the right.
    let right_w = if app.ui_state.storyboard_open { (area.width() * 0.5).clamp(320.0, 760.0) } else { 220.0 };
    let right = Rect::from_min_max(pos2(area.right() - right_w, area.top()), area.max);
    let left = Rect::from_min_max(area.min, pos2(right.left() - 1.0, area.bottom()));
    ui.painter().line_segment([right.left_top(), right.left_bottom()], egui::Stroke::new(1.0, chrome::BORDER));

    let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right.shrink(10.0)));
    right_ui.set_clip_rect(right);
    video_project_panel(app, &mut right_ui, &status, ready);
    right_ui.add_space(10.0);
    storyboard_pane(app, &mut right_ui);

    let drop = ui.interact(left, ui.id().with("workshop.drop"), Sense::hover());
    let dragging = egui::DragAndDrop::has_payload_of_type::<String>(ui.ctx());
    if dragging && drop.contains_pointer() {
        ui.painter().rect_stroke(left.shrink(4.0), 8.0, egui::Stroke::new(2.0, chrome::TEAL), egui::StrokeKind::Inside);
    }
    if let Some(path) = drop.dnd_release_payload::<String>() {
        animate_image(app, &path);
    }
    let width = 640.0_f32.min(left.width() - 32.0);
    let column = Rect::from_min_size(pos2(left.left() + 16.0, left.top() + 16.0), vec2(width, left.height() - 24.0));
    let mut col = ui.new_child(egui::UiBuilder::new().max_rect(column));
    col.set_clip_rect(left);
    egui::ScrollArea::vertical().id_salt("workshop.column").auto_shrink([false, false]).show(&mut col, |ui| {
        // The studio's name and intent come from its own files (intent.md / STUDIO.md), not the app.
        let studio_name = status["studio"]["name"].as_str().unwrap_or("Workshop");
        ui.label(RichText::new(format!("{title} — {studio_name}")).size(24.0).color(TEXT));
        match status["studio"]["intent"].as_str() {
            Some(intent) => {
                let r = ui.add(egui::Label::new(RichText::new(intent.lines().next().unwrap_or_default()).size(11.5).italics().color(MUTED)).truncate());
                r.on_hover_text(format!("{intent}\n\n{}", status["studio"]["source"].as_str().unwrap_or_default()));
            }
            None => {
                ui.label(RichText::new("Add intent.md to this song's folder, or STUDIO.md beside your projects, to tell agents your audience, style and rules.").size(11.5).italics().color(MUTED));
            }
        }
        ui.add_space(12.0);
        let stages = status["stages"].as_array().cloned().unwrap_or_default();
        for (i, stage) in stages.iter().enumerate() {
            stage_row(app, ui, i + 1, stage, ready);
            ui.add_space(6.0);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let complete = status["complete"] == json!(true);
            let start = ui
                .add_enabled_ui(complete, |ui| chrome::button(ui, "Start animating", true))
                .inner
                .on_disabled_hover_text("Stages 1–3 need a green check first.");
            app.automation.add_response("workshop.start", "button", "Start animating", &start);
            if start.clicked() {
                app.ui_state.show_workshop = false;
            }
            if app.ui_state.busy_command.is_some() {
                ui.spinner();
            }
            ui.label(RichText::new(app.ui_state.status_line.clone()).size(11.0).color(MUTED));
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new("Drag an image from the storyboard onto this side, or right-click it, to animate it.")
                .size(11.0)
                .color(MUTED),
        );
    });
    confirm_remove(app, ui.ctx());
}

/// Adds an image from the storyboard as a project asset and loads it for animation.
fn animate_image(app: &mut AnimationCraftApp, path: &str) {
    let name = std::path::Path::new(path).file_stem().and_then(|n| n.to_str()).unwrap_or("asset").to_string();
    let existing = app.session.project_assets.iter().find(|a| a.path.as_ref().is_some_and(|p| p.display().to_string() == path)).map(|a| a.id.clone());
    let id = match existing {
        Some(id) => Some(id),
        None => app
            .dispatch("project.add_asset", json!({"path": path, "name": name, "kind": "image"}))
            .ok()
            .and_then(|r| r["asset_id"].as_str().map(str::to_owned)),
    };
    let Some(id) = id else {
        app.ui_state.status_line = format!("Could not add {path}");
        return;
    };
    run(app, "project.load_asset", json!({"id": id}));
    let complete = status(app).is_some_and(|s| s["complete"] == json!(true));
    app.ui_state.status_line = if complete {
        app.ui_state.show_workshop = false;
        format!("Animating {name}")
    } else {
        format!("Added {name} to Animate assets; finish stages 1–3 to start animating.")
    };
}

/// One compact stage: number, title, ✓/✗ and reason; inputs ─▶ outputs; brief and actions.
fn stage_row(app: &mut AnimationCraftApp, ui: &mut Ui, number: usize, stage: &Value, ready: bool) {
    let id = stage["id"].as_str().unwrap_or_default().to_string();
    let done = stage["done"] == json!(true);
    egui::Frame::new()
        .fill(chrome::SURFACE)
        .stroke(egui::Stroke::new(1.0, chrome::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (badge, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                ui.painter().circle_filled(badge.center(), 10.0, chrome::BORDER);
                ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, number.to_string(), FontId::proportional(11.5), TEXT);
                ui.label(RichText::new(stage["title"].as_str().unwrap_or_default()).size(14.0).color(TEXT));
                if stage["optional"] != json!(true) {
                    let (b, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    chrome::paint_status_badge(ui.painter(), b.center(), done);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let menu = ui.add_enabled_ui(ready, |ui| ui.menu_button(RichText::new("Actions").size(12.0), |ui| stage_actions(app, ui, &id, stage))).inner;
                    app.automation.add_response(format!("workshop.{id}.actions"), "button", "Stage actions", &menu.response);
                    let brief = stage["brief"].as_str().unwrap_or_default().to_string();
                    let copy = ui.small_button("Copy brief").on_hover_text(&brief);
                    app.automation.add_response(format!("workshop.{id}.brief"), "button", "Copy agent brief", &copy);
                    if copy.clicked() {
                        ui.ctx().copy_text(brief);
                        app.ui_state.status_line = format!("Copied the stage {number} brief for an agent");
                    }
                    if id == "reference" {
                        let has_pdf = stage["outputs"].as_array().and_then(|o| o.first()).is_some_and(|c| c["exists"] == json!(true));
                        let mut approved = stage["approved"] == json!(true);
                        let check = ui.add_enabled(has_pdf && ready, egui::Checkbox::new(&mut approved, "Approved"));
                        app.automation.add_response("workshop.storyboard.approved", "checkbox", "Approved", &check);
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
                flow_line(app, ui, &id, stage);
            });
        });
}

/// inputs ─▶ outputs as one line of short names; images preview on hover.
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
            ui.label(RichText::new("no animated assets yet").size(11.5).italics().color(MUTED));
        }
        for asset in assets {
            let parts = asset["parts"].as_u64().map(|p| format!(" · {p} parts")).unwrap_or_default();
            let motion = motion_symbols(asset["controls"].as_array().map(Vec::as_slice).unwrap_or(&[]));
            let motion = if motion.is_empty() { String::new() } else { format!(" · {motion}") };
            ui.label(RichText::new(format!("{}{parts}{motion}", asset["name"].as_str().unwrap_or_default())).size(11.5).color(TEXT));
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
    let r = if image && exists && let Some(tex) = app.load_texture(ui.ctx(), &path) {
        r.on_hover_ui(|ui| {
            ui.add(egui::Image::new(&tex).max_size(vec2(260.0, 200.0)));
            ui.label(RichText::new(&path).size(10.5).color(MUTED));
        })
    } else {
        r.on_hover_text(format!("{path}{}\nClick to copy the path", if exists { "" } else { "\n(not made yet)" }))
    };
    app.automation.add_response(format!("workshop.chip.{key}"), "button", &name, &r);
    if r.clicked() {
        ui.ctx().copy_text(path.clone());
        app.ui_state.status_line = format!("Copied {path}");
    }
}

/// Where the song is edited: each target with ✓ when its project file exists, ✗ when it does not.
fn video_project_panel(app: &mut AnimationCraftApp, ui: &mut Ui, status: &Value, ready: bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Video project").size(14.0).color(TEXT));
        ui.add_space(6.0);
        for target in status["targets"].as_array().cloned().unwrap_or_default() {
            let name = target["target"].as_str().unwrap_or_default().to_string();
            let label = match name.as_str() {
                "remotion" => "Remotion",
                "filmcraft" => "FilmCraft",
                _ => "HyperFrames",
            };
            let selected = target["selected"] == json!(true);
            let r = ui.add_enabled(ready, egui::Button::selectable(selected, RichText::new(label).size(12.0)));
            let (b, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
            chrome::paint_status_badge(ui.painter(), b.center(), target["exists"] == json!(true));
            let hint = target["path"].as_str().map(str::to_owned).unwrap_or_else(|| format!("No {label} project file yet"));
            let r = r.on_hover_text(hint);
            app.automation.add_response(format!("workshop.target.{name}"), "button", label, &r);
            if r.clicked() && !selected {
                run(app, "workshop.set_target", json!({"target": name}));
            }
        }
    });
    let selected = status["targets"].as_array().into_iter().flatten().find(|t| t["selected"] == json!(true)).cloned();
    if let Some(t) = selected
        && t["target"] == json!("filmcraft")
        && let Some(fcproj) = t["path"].as_str().map(str::to_owned)
    {
        ui.horizontal(|ui| {
            let filmcraft = super::start::launcher_path("filmcraft");
            if button(app, ui, "video.open", "Open in FilmCraft", filmcraft.is_some())
                && let Some(exe) = filmcraft
            {
                app.ui_state.status_line = match std::process::Command::new(&exe).arg(&fcproj).spawn() {
                    Ok(_) => format!("Opened {fcproj} in FilmCraft"),
                    Err(e) => format!("Could not open FilmCraft ({}): {e}", exe.display()),
                };
            }
            if button(app, ui, "video.place", "Place panels at their lines", ready) {
                run(app, "video.place_references", json!({}));
            }
        });
    }
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
    let toggle = ui.button(if open { "Hide storyboard" } else { "📖" }).on_hover_text("Show or hide the storyboard");
    app.automation.add_response("workshop.storyboard.toggle", "button", "Toggle storyboard", &toggle);
    if toggle.clicked() {
        app.ui_state.storyboard_open = !open;
    }
    if !open {
        return;
    }
    if app.ui_state.storyboard_info.is_none() && app.ui_state.busy_command.is_none() {
        app.ui_state.storyboard_info = app.session.execute("workshop.storyboard", json!({})).ok();
    }
    let Some(info) = app.ui_state.storyboard_info.clone() else { return };
    let previews: Vec<String> = info["previews"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
    ui.add_space(6.0);
    ui.label(RichText::new("Storyboard").size(16.0).color(TEXT));
    if previews.is_empty() {
        ui.label(RichText::new("Build the storyboard in stage 3 to see it here.").size(12.0).color(MUTED));
        return;
    }
    let pages = previews.len();
    let page = app.ui_state.storyboard_page.min(pages - 1);
    ui.horizontal_wrapped(|ui| {
        let titles = info["scene_titles"].as_array().cloned().unwrap_or_default();
        let mut nav = vec![("Overview".to_string(), 0usize)];
        nav.extend(titles.iter().enumerate().filter_map(|(i, t)| Some((t.as_str()?.split(" · ").take(2).collect::<Vec<_>>().join(" · "), i + 1))));
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
        if ui.add_enabled(page > 0, egui::Button::new("◀")).clicked() {
            app.ui_state.storyboard_page = page - 1;
        }
        ui.label(RichText::new(format!("page {} of {pages}", page + 1)).size(11.5).color(MUTED));
        if ui.add_enabled(page + 1 < pages, egui::Button::new("▶")).clicked() {
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
        let shown = ui.add(egui::Image::new(&tex).fit_to_exact_size(size)).rect;
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
            let thumb = app.load_texture(ui.ctx(), &path);
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
                if ui.button("Animate this").clicked() {
                    animate_image(app, &path);
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
            created = app.session.project_path.as_ref() == Some(&target);
            super::start::record_open_project(app);
        }
    });
    app.ui_state.new_project_open = open && !created;
}
