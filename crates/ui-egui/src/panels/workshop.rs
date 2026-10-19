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
    // Stages on the left; the Storyboard pane docks on the right and can be collapsed.
    let pane_w = if app.ui_state.storyboard_open { (area.width() * 0.42).clamp(320.0, 620.0) } else { 36.0 };
    let pane = Rect::from_min_max(pos2(area.right() - pane_w, area.top()), area.max);
    let main = Rect::from_min_max(area.min, pos2(pane.left() - 1.0, area.bottom()));
    ui.painter().line_segment([pane.left_top(), pane.left_bottom()], egui::Stroke::new(1.0, chrome::BORDER));
    let mut pane_ui = ui.new_child(egui::UiBuilder::new().max_rect(pane.shrink(8.0)));
    pane_ui.set_clip_rect(pane);
    storyboard_pane(app, &mut pane_ui);

    let width = 760.0_f32.min(main.width() - 40.0);
    let column = Rect::from_min_size(pos2(main.center().x - width / 2.0, main.top() + 24.0), vec2(width, main.height() - 32.0));
    let mut col = ui.new_child(egui::UiBuilder::new().max_rect(column));
    col.set_clip_rect(main);
    egui::ScrollArea::vertical().id_salt("workshop.column").auto_shrink([false, false]).show(&mut col, |ui| {
        ui.label(RichText::new(format!("{title} — workshop")).size(26.0).color(TEXT));
        ui.label(
            RichText::new("Each stage shows what goes in and what comes out. Copy a stage's agent brief to hand it to an agent.")
                .size(12.5)
                .color(MUTED),
        );
        ui.add_space(14.0);
        let stages = status["stages"].as_array().cloned().unwrap_or_default();
        for (i, stage) in stages.iter().enumerate() {
            stage_card(app, ui, i + 1, stage, &status, ready);
            ui.add_space(10.0);
        }
        ui.add_space(4.0);
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
                ui.label(RichText::new(app.ui_state.status_line.clone()).size(11.5).color(MUTED));
            }
        });
        ui.add_space(18.0);
        files_panel(app, ui, ready);
    });
    confirm_remove(app, ui.ctx());
}

fn stage_card(app: &mut AnimationCraftApp, ui: &mut Ui, number: usize, stage: &Value, status: &Value, ready: bool) {
    let id = stage["id"].as_str().unwrap_or_default().to_string();
    let done = stage["done"] == json!(true);
    let optional = stage["optional"] == json!(true);
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (badge, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            ui.painter().circle_filled(badge.center(), 11.0, chrome::BORDER);
            ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, number.to_string(), FontId::proportional(12.0), TEXT);
            ui.label(RichText::new(stage["title"].as_str().unwrap_or_default()).size(15.0).color(TEXT));
            if !optional {
                let (b, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                chrome::paint_status_badge(ui.painter(), b.center(), done);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let brief = stage["brief"].as_str().unwrap_or_default().to_string();
                let copy = chrome::button(ui, "Copy agent brief", false).on_hover_text(&brief);
                app.automation.add_response(format!("workshop.{id}.brief"), "button", "Copy agent brief", &copy);
                if copy.clicked() {
                    ui.ctx().copy_text(brief);
                    app.ui_state.status_line = format!("Copied the stage {number} brief for an agent");
                }
            });
        });
        ui.label(RichText::new(stage["why"].as_str().unwrap_or_default()).size(12.0).color(if done { MUTED } else { TEXT }));
        ui.add_space(6.0);
        flow(app, ui, &id, stage);
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if !ready {
                ui.disable();
            }
            stage_actions(app, ui, &id, stage, status);
        });
    });
}

/// inputs ──▶ outputs, as file chips (thumbnails for images).
fn flow(app: &mut AnimationCraftApp, ui: &mut Ui, id: &str, stage: &Value) {
    let inputs = stage["inputs"].as_array().cloned().unwrap_or_default();
    let mut outputs = stage["outputs"].as_array().cloned().unwrap_or_default();
    if id == "animate" {
        outputs.clear();
    }
    ui.horizontal_wrapped(|ui| {
        if inputs.is_empty() {
            ui.label(RichText::new("nothing yet").size(11.5).italics().color(MUTED));
        }
        for (i, chip) in inputs.iter().enumerate() {
            file_chip(app, ui, chip, &format!("{id}.in.{i}"));
        }
        if !outputs.is_empty() || id == "animate" {
            let (r, _) = ui.allocate_exact_size(vec2(44.0, 30.0), Sense::hover());
            let y = r.center().y;
            let stroke = egui::Stroke::new(1.6, MUTED);
            ui.painter().line_segment([pos2(r.left() + 4.0, y), pos2(r.right() - 6.0, y)], stroke);
            ui.painter().line_segment([pos2(r.right() - 12.0, y - 5.0), pos2(r.right() - 6.0, y)], stroke);
            ui.painter().line_segment([pos2(r.right() - 12.0, y + 5.0), pos2(r.right() - 6.0, y)], stroke);
        }
        for (i, chip) in outputs.iter().enumerate() {
            file_chip(app, ui, chip, &format!("{id}.out.{i}"));
        }
        if id == "animate" {
            animate_list(ui, stage);
        }
    });
}

fn file_chip(app: &mut AnimationCraftApp, ui: &mut Ui, chip: &Value, key: &str) {
    let path = chip["path"].as_str().unwrap_or_default().to_string();
    let name = chip["name"].as_str().unwrap_or_default().to_string();
    let exists = chip["exists"] != json!(false);
    let begins = chip["begins"].as_array().map(|b| b.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" · "));
    let image = ["png", "jpg", "jpeg", "webp"].iter().any(|e| name.to_ascii_lowercase().ends_with(e));
    let frame = egui::Frame::new()
        .fill(if exists { chrome::SURFACE } else { egui::Color32::TRANSPARENT })
        .stroke(egui::Stroke::new(1.0, if exists { chrome::BORDER } else { MUTED }))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(6, 4));
    // A fixed footprint lets the wrapped row move whole chips to the next line.
    let chip_w = if begins.is_some() || image { 168.0 } else { (name.len() as f32 * 7.0 + 24.0).clamp(80.0, 260.0) };
    let response = ui
        .allocate_ui_with_layout(vec2(chip_w, 46.0), egui::Layout::left_to_right(egui::Align::Center), |ui| frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                if image && exists && let Some(tex) = app.load_texture(ui.ctx(), &path) {
                    let [w, h] = tex.size();
                    let size = vec2(32.0 * w as f32 / h.max(1) as f32, 32.0).min(vec2(48.0, 32.0));
                    ui.add(egui::Image::new(&tex).fit_to_exact_size(size));
                }
                // Labels keep their width so a full row wraps the whole chip to the next line.
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(RichText::new(&name).size(11.5).color(if exists { TEXT } else { MUTED })).extend());
                    if let Some(b) = &begins {
                        ui.add(egui::Label::new(RichText::new(format!("begins {b}")).size(10.5).color(chrome::BLUE)).extend());
                    } else if !exists {
                        ui.add(egui::Label::new(RichText::new("not made yet").size(10.5).italics().color(MUTED)).extend());
                    }
                });
            });
        }))
        .inner
        .response
        .interact(Sense::click())
        .on_hover_text(format!("{path}\nClick to copy the path"));
    app.automation.add_response(format!("workshop.chip.{key}"), "button", &name, &response);
    if response.clicked() {
        ui.ctx().copy_text(path.clone());
        app.ui_state.status_line = format!("Copied {path}");
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

fn animate_list(ui: &mut Ui, stage: &Value) {
    let assets = stage["assets"].as_array().cloned().unwrap_or_default();
    if assets.is_empty() {
        ui.label(RichText::new("no animated assets yet").size(11.5).italics().color(MUTED));
    }
    for asset in assets {
        let parts = asset["parts"].as_u64().map(|p| format!("{p} parts")).unwrap_or_else(|| "static".into());
        let motion = motion_symbols(asset["controls"].as_array().map(Vec::as_slice).unwrap_or(&[]));
        let line = if motion.is_empty() { format!("{} · {parts}", asset["name"].as_str().unwrap_or_default()) } else { format!("{} · {parts} · {motion}", asset["name"].as_str().unwrap_or_default()) };
        egui::Frame::new().stroke(egui::Stroke::new(1.0, chrome::BORDER)).corner_radius(5).inner_margin(egui::Margin::symmetric(8, 5)).show(ui, |ui| {
            ui.label(RichText::new(line).size(11.5).color(TEXT));
        });
    }
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

fn stage_actions(app: &mut AnimationCraftApp, ui: &mut Ui, id: &str, stage: &Value, status: &Value) {
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
            ui.separator();
            ui.label(RichText::new("Edit in").size(12.0).color(MUTED));
            let current = status["video_target"].as_str().unwrap_or("remotion").to_string();
            for (target, label) in [("remotion", "Remotion"), ("filmcraft", "FilmCraft"), ("hyperframes", "HyperFrames")] {
                let r = ui.selectable_label(current == target, label);
                app.automation.add_response(format!("workshop.target.{target}"), "button", label, &r);
                if r.clicked() && current != target {
                    run(app, "workshop.set_target", json!({"target": target}));
                }
            }
            let target = &stage["target"];
            if let Some(note) = target["note"].as_str() {
                ui.label(RichText::new(note).size(11.5).italics().color(MUTED));
            }
            if current == "filmcraft"
                && let Some(fcproj) = target["path"].as_str().map(str::to_owned)
            {
                let filmcraft = super::start::launcher_path("filmcraft");
                if button(app, ui, "video.open", "Open in FilmCraft", filmcraft.is_some())
                    && let Some(exe) = filmcraft
                {
                    app.ui_state.status_line = match std::process::Command::new(&exe).arg(&fcproj).spawn() {
                        Ok(_) => format!("Opened {fcproj} in FilmCraft"),
                        Err(e) => format!("Could not open FilmCraft ({}): {e}", exe.display()),
                    };
                }
                let has_panels = !app.ui_state.workshop_files.iter().all(|f| f["group"] != json!("storyboard"));
                if button(app, ui, "video.place", "Place panels at their lines", has_panels) {
                    run(app, "video.place_references", json!({}));
                }
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
            let mut approved = stage["approved"] == json!(true);
            let check = ui.add_enabled(has_pdf, egui::Checkbox::new(&mut approved, "Approved"));
            app.automation.add_response("workshop.storyboard.approved", "checkbox", "Approved", &check);
            if check.changed() {
                run(app, "workshop.approve_draft", json!({"approved": approved}));
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
    if let Some(tex) = app.load_texture(ui.ctx(), &previews[page]) {
        let [w, h] = tex.size();
        let width = ui.available_width();
        let size = vec2(width, width * h as f32 / w.max(1) as f32);
        egui::ScrollArea::vertical().id_salt("workshop.storyboard.page").show(ui, |ui| {
            ui.add(egui::Image::new(&tex).fit_to_exact_size(size));
        });
    }
}

const GROUPS: [(&str, &str); 7] = [
    ("audio", "Song audio"),
    ("liner_notes", "Liner notes"),
    ("transcript", "Transcript and video project"),
    ("storyboard", "Storyboard (panels, prompts, PDF)"),
    ("assets", "Static assets"),
    ("other", "Other files in the project folder"),
    ("images", "Images"),
];

fn human_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / f64::from(1 << 20)),
        b if b >= 1 << 10 => format!("{:.0} KB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

/// Every workshop file with Copy path / Open / Remove, grouped like the checklist.
fn files_panel(app: &mut AnimationCraftApp, ui: &mut Ui, ready: bool) {
    let files = app.ui_state.workshop_files.clone();
    ui.label(RichText::new("Files").size(16.0).color(TEXT));
    ui.label(RichText::new("Copy a path to share it, open a file, or remove what does not belong.").size(11.5).color(MUTED));
    ui.add_space(8.0);
    egui::ScrollArea::vertical().id_salt("workshop.files").auto_shrink([false, true]).max_height(f32::INFINITY).show(ui, |ui| {
        for (group, title) in GROUPS {
            let entries: Vec<&Value> = files.iter().filter(|f| f["group"] == json!(group)).collect();
            if entries.is_empty() {
                continue;
            }
            ui.label(RichText::new(title).size(12.0).strong().color(MUTED));
            for (i, entry) in entries.iter().enumerate() {
                file_row(app, ui, entry, &format!("{group}.{i}"), ready);
            }
            ui.add_space(8.0);
        }
        if files.is_empty() {
            ui.label(RichText::new("No files yet.").color(MUTED));
        }
    });
}

fn file_row(app: &mut AnimationCraftApp, ui: &mut Ui, entry: &Value, key: &str, ready: bool) {
    let path = entry["path"].as_str().unwrap_or_default().to_string();
    let name = entry["name"].as_str().unwrap_or_default();
    let size = entry["size"].as_u64().map(human_size).unwrap_or_else(|| "missing".into());
    chrome::card().inner_margin(6).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let thumb = ["png", "jpg", "jpeg", "webp"].iter().any(|e| path.to_ascii_lowercase().ends_with(e));
            let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            if thumb && let Some(tex) = app.load_texture(ui.ctx(), &path) {
                let [w, h] = tex.size();
                let scale = 36.0 / w.max(h).max(1) as f32;
                let size = vec2(w as f32 * scale, h as f32 * scale);
                ui.painter().image(tex.id(), Rect::from_center_size(r.center(), size), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
            } else {
                // A page with a folded corner, so it never reads as a checkbox.
                let page = Rect::from_center_size(r.center(), vec2(16.0, 21.0));
                let fold = 5.0;
                let stroke = egui::Stroke::new(1.3, MUTED);
                let p = ui.painter();
                p.line_segment([page.left_top(), pos2(page.right() - fold, page.top())], stroke);
                p.line_segment([pos2(page.right() - fold, page.top()), pos2(page.right(), page.top() + fold)], stroke);
                p.line_segment([pos2(page.right(), page.top() + fold), page.right_bottom()], stroke);
                p.line_segment([page.right_bottom(), page.left_bottom()], stroke);
                p.line_segment([page.left_bottom(), page.left_top()], stroke);
                p.line_segment([pos2(page.right() - fold, page.top()), pos2(page.right() - fold, page.top() + fold)], stroke);
                p.line_segment([pos2(page.right() - fold, page.top() + fold), pos2(page.right(), page.top() + fold)], stroke);
            }
            ui.vertical(|ui| {
                ui.label(RichText::new(name).size(12.5).color(TEXT)).on_hover_text(&path);
                ui.label(RichText::new(size).size(11.0).color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(ready, |ui| {
                    if button(app, ui, &format!("file.{key}.remove"), "Remove", entry["removable"] == json!(true)) {
                        app.ui_state.workshop_remove = Some(path.clone());
                    }
                });
                if button(app, ui, &format!("file.{key}.open"), "Open", entry["exists"] == json!(true)) {
                    app.ui_state.status_line = match std::process::Command::new("xdg-open").arg(&path).spawn() {
                        Ok(_) => format!("Opened {path}"),
                        Err(e) => format!("Could not open {path}: {e}"),
                    };
                }
                if button(app, ui, &format!("file.{key}.copy"), "Copy path", true) {
                    ui.ctx().copy_text(path.clone());
                    app.ui_state.status_line = format!("Copied {path}");
                }
            });
        });
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
