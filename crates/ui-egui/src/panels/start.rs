//! Home screen shown while no project is open, following the Craft suite's start screen
//! (PhotoCraft `canvas::start_screen`): title, New / Open, Recent, community links.
//! New and Open run `project.new` / `project.open`, the same engine commands as the CLI and MCP.
use super::chrome::{MUTED, TEXT};
use crate::{AnimationCraftApp, UiCommand};
use egui::{Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use std::path::{Path, PathBuf};

const FAINT: Color32 = Color32::from_rgb(112, 122, 138);
const HOVER: Color32 = Color32::from_rgb(37, 46, 60);
const PRIMARY: Color32 = Color32::from_rgb(29, 83, 83);
const PRIMARY_HOVER: Color32 = Color32::from_rgb(38, 111, 108);
const DISCORD: &str = "https://discord.gg/artcraft";
const ARTCRAFT_WEBSITE: &str = "https://getartcraft.com";
/// Recent projects listed on the Home screen.
const HOME_RECENT: usize = 6;
/// Recent projects remembered.
const RECENT_KEPT: usize = 10;
/// Height of one Home-screen recent-project row.
const HOME_RECENT_ROW: f32 = 30.0;

/// Whether a project is open (or assets were connected / standalone studio was opened without one, e.g. from the CLI).
pub(crate) fn project_open(app: &AnimationCraftApp) -> bool {
    app.session.project_path.is_some()
        || !app.session.project_assets.is_empty()
        || app.session.current_asset().is_some()
        || app.ui_state.standalone_studio
}

/// The user's recent-projects file (`$XDG_CONFIG_HOME/animationcraft/recent_projects.json`).
pub(crate) fn user_recent_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("animationcraft").join("recent_projects.json"))
}

pub(crate) fn load_recent(file: &Path) -> Vec<String> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes Recent only when the app was given a file (the desktop app; never tests or headless use).
fn save_recent(file: Option<&Path>, list: &[String]) {
    if let Some(file) = file {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, json!(list).to_string());
    }
}

/// Puts the open project at the top of Recent, whichever caller (Home, menu, bridge) opened it.
pub(crate) fn record_open_project(app: &mut AnimationCraftApp) {
    if app.session.project_path == app.ui_state.recorded_project {
        return;
    }
    app.ui_state.recorded_project = app.session.project_path.clone();
    if let Some(path) = &app.session.project_path {
        let path = path.display().to_string();
        let recent = &mut app.ui_state.recent_projects;
        recent.retain(|p| *p != path);
        recent.insert(0, path);
        recent.truncate(RECENT_KEPT);
        save_recent(app.ui_state.recent_file.as_deref(), recent);
    }
}

/// New song project: opens the dialog that asks for the song's name and audio.
pub(crate) fn new_project(app: &mut AnimationCraftApp) {
    if app.ui_state.new_project_root.is_empty() {
        app.ui_state.new_project_root = super::workshop::default_projects_root().display().to_string();
    }
    app.ui_state.new_project_open = true;
}

pub(crate) fn open_project(app: &mut AnimationCraftApp) {
    if let Some(path) = rfd::FileDialog::new()
        .set_title("Open Project")
        .add_filter("AnimationCraft project", &["acproj"])
        .pick_file()
    {
        run(app, "project.open", json!({"path": path}));
    }
}

pub(crate) fn open_recent(app: &mut AnimationCraftApp, path: &str) {
    run(app, "project.open", json!({"path": path}));
}

fn run(app: &mut AnimationCraftApp, id: &'static str, params: serde_json::Value) {
    app.exec_ui_command(UiCommand::Engine { id, params });
    record_open_project(app);
}

/// Ctrl+N / Ctrl+O (⌘ on macOS), as labelled on the Home buttons and File menu.
pub(crate) fn shortcuts(app: &mut AnimationCraftApp, ctx: &egui::Context) {
    if app.ui_state.busy_command.is_some() {
        return;
    }
    let new = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::N);
    let open = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O);
    if ctx.input_mut(|i| i.consume_shortcut(&new)) {
        new_project(app);
    } else if ctx.input_mut(|i| i.consume_shortcut(&open)) {
        open_project(app);
    }
}

pub(crate) fn shortcut_text(ctx: &egui::Context, key: egui::Key) -> String {
    ctx.format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, key))
}

pub(crate) fn start_screen(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let area = ui.max_rect();
    app.automation.add("home.screen", "panel", "Home", area);
    let recent: Vec<String> = app.ui_state.recent_projects.iter().take(HOME_RECENT).cloned().collect();
    let recent_h = if recent.is_empty() { 0.0 } else { 34.0 + recent.len() as f32 * HOME_RECENT_ROW };
    let card_h = 330.0 + recent_h;
    let strip_h = workflow_height(area.width());
    // The card sits centred above the workflow strip; both centre together in the body.
    let top = area.top() + ((area.height() - card_h - 28.0 - strip_h) / 2.0).max(12.0);
    let card = Rect::from_min_size(pos2(area.center().x - 230.0, top), vec2(460.0, card_h));
    workflow_strip(app, ui, Rect::from_min_size(pos2(area.left(), card.bottom() + 28.0), vec2(area.width(), strip_h)));
    let new_label = format!("New project…   {}", shortcut_text(ui.ctx(), egui::Key::N));
    let open_label = format!("Open…   {}", shortcut_text(ui.ctx(), egui::Key::O));
    let ready = app.ui_state.busy_command.is_none();
    ui.scope_builder(egui::UiBuilder::new().max_rect(card), |ui| {
        ui.vertical_centered(|ui| {
            let title = ui.painter().layout_no_wrap("AnimationCraft".into(), FontId::proportional(38.0), TEXT);
            let (r, _) = ui.allocate_exact_size(title.size(), Sense::hover());
            ui.painter().galley(r.min, title, TEXT);
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Create a new project or open an existing one.").color(MUTED).size(14.0));
            ui.add_space(22.0);
            ui.horizontal(|ui| {
                ui.add_space(((card.width() - 2.0 * 190.0 - 12.0) / 2.0).max(0.0));
                ui.spacing_mut().item_spacing.x = 12.0;
                ui.add_enabled_ui(ready, |ui| {
                    let new = pill_button(ui, &new_label, 190.0, true);
                    app.automation.add_response("home.new_project", "button", "New project…", &new);
                    if new.clicked() {
                        new_project(app);
                    }
                    let open = pill_button(ui, &open_label, 190.0, false);
                    app.automation.add_response("home.open_project", "button", "Open…", &open);
                    if open.clicked() {
                        open_project(app);
                    }
                });
            });
            ui.add_space(12.0);
            let standalone = pill_button(ui, "Open AnimationStudio", 220.0, false)
                .on_hover_text("Open the 3-column 2D/3D rigging & layered motion studio directly without a song project");
            app.automation
                .add_response("home.standalone_studio", "button", "Open AnimationStudio", &standalone);
            if standalone.clicked() {
                app.ui_state.show_workshop = false;
                app.ui_state.standalone_studio = true;
            }
            ui.add_space(14.0);
            ui.label(
                egui::RichText::new("A project keeps your characters together. Import them one at a time.")
                    .size(12.5)
                    .color(FAINT),
            );
            if !recent.is_empty() {
                ui.add_space(22.0);
                home_recent(app, ui, &recent);
            }
            ui.add_space(26.0);
            let discord = pill_button(ui, "Join us on Discord", 190.0, true).on_hover_text(DISCORD);
            app.automation.add_response("home.discord", "button", "Join us on Discord", &discord);
            if discord.clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(DISCORD));
            }
            ui.add_space(10.0);
            let link = ui
                .link(egui::RichText::new("ArtCraft").size(12.5).color(super::chrome::BLUE))
                .on_hover_text(ARTCRAFT_WEBSITE);
            app.automation.add_response("home.link.artcraft", "link", "ArtCraft", &link);
            if link.clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(ARTCRAFT_WEBSITE));
            }
        });
    });
}

/// One step of the song-to-video workflow on Home: what happens, which app does it, and the exact
/// instructions an agent needs (copied by clicking the step).
struct Step {
    title: &'static str,
    app: &'static str,
    /// ArtCraft launcher app id to open, if the step happens in another *craft app.
    launch: Option<&'static str>,
    agent: &'static str,
}

const WORKFLOW: [Step; 6] = [
    Step {
        title: "Transcribe the song",
        app: "FilmCraft",
        launch: Some("filmcraft"),
        agent: "Transcribe the song headless with FilmCraft's speech-to-text through AnimationCraft:\n\
animationcraft-cli exec audio.transcribe audio=<song.wav> out_dir=<project folder> name=<song>\n\
It writes <song>.transcript.json (timed words and lines) and <song>.fcproj (the song on a FilmCraft sequence).",
    },
    Step {
        title: "Reference images",
        app: "Codex agent",
        launch: None,
        agent: "Ask the Codex agent in herdr for reference images from the transcript: one storyboard grid first,\n\
no text or numerals in any image, the whole character visible and separate from the background.\n\
Save the prompt as assets/references/CODEX_PROMPT_*.md next to the result.",
    },
    Step {
        title: "Approve the design",
        app: "PDFCraft",
        launch: Some("pdfcraft"),
        agent: "Assemble a design PDF with pdfcraft-cli (tools doc_create, page_add_image, page_add_text, doc_save):\n\
storyboard, lyrics with timings, cast and palette. Wait for the user to approve it before animating.",
    },
    Step {
        title: "Crop & transparency",
        app: "PhotoCraft",
        launch: Some("photocraft"),
        agent: "Cut each element out with PhotoCraft through AnimationCraft (no custom matting):\n\
animationcraft-cli exec intake.validate image=<panel.png> asset=<name>\n\
Check every cut-out at full size before using it.",
    },
    Step {
        title: "Animate",
        app: "AnimationCraft",
        launch: None,
        agent: "Open the project in AnimationCraft (project.open), one animated subject at a time over its background,\n\
then render the layered motion: animationcraft-cli exec export.layered_motion spec=<spec.json>",
    },
    Step {
        title: "Edit to the song",
        app: "FilmCraft",
        launch: Some("filmcraft"),
        agent: "Place the rendered clips on the song's sequence headless:\n\
filmcraft-cli --project <song>.fcproj --save import <clip.mp4>",
    },
];

const STEP_W: f32 = 150.0;
const STEP_H: f32 = 92.0;
const STEP_GAP: f32 = 26.0;

fn steps_per_row(width: f32) -> usize {
    if width >= 6.0 * STEP_W + 5.0 * STEP_GAP + 32.0 { 6 } else { 3 }
}

fn workflow_height(width: f32) -> f32 {
    let rows = WORKFLOW.len().div_ceil(steps_per_row(width)) as f32;
    24.0 + rows * STEP_H + (rows - 1.0) * 18.0
}

/// The app's install path from the ArtCraft launcher (`~/.local/share/artcraft-launcher/installed.json`).
pub(crate) fn launcher_path(app: &str) -> Option<PathBuf> {
    use std::sync::OnceLock;
    static INSTALLED: OnceLock<Option<serde_json::Value>> = OnceLock::new();
    let installed = INSTALLED
        .get_or_init(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from)?;
            let text = std::fs::read_to_string(home.join(".local/share/artcraft-launcher/installed.json")).ok()?;
            serde_json::from_str(&text).ok()
        })
        .as_ref()?;
    installed.pointer(&format!("/apps/{app}/path")).and_then(|p| p.as_str()).map(PathBuf::from)
}

fn workflow_strip(app: &mut AnimationCraftApp, ui: &mut Ui, rect: Rect) {
    let per_row = steps_per_row(rect.width());
    let row_w = per_row as f32 * STEP_W + (per_row - 1) as f32 * STEP_GAP;
    let left = rect.center().x - row_w / 2.0;
    ui.painter().text(
        pos2(left, rect.top() + 6.0),
        egui::Align2::LEFT_CENTER,
        "Song to video — click a step to copy its agent instructions",
        FontId::proportional(12.5),
        MUTED,
    );
    for (i, step) in WORKFLOW.iter().enumerate() {
        let (row, col) = (i / per_row, i % per_row);
        let min = pos2(left + col as f32 * (STEP_W + STEP_GAP), rect.top() + 24.0 + row as f32 * (STEP_H + 18.0));
        let r = Rect::from_min_size(min, vec2(STEP_W, STEP_H));
        let resp = ui.interact(r, ui.id().with(("workflow", i)), Sense::click());
        let hot = resp.hovered();
        ui.painter().rect_filled(r, 6.0, if hot { HOVER } else { super::chrome::SURFACE });
        ui.painter().rect_stroke(r, 6.0, Stroke::new(1.0, if hot { MUTED } else { super::chrome::BORDER }), StrokeKind::Inside);
        let badge = pos2(r.left() + 16.0, r.top() + 16.0);
        ui.painter().circle_filled(badge, 9.0, PRIMARY);
        ui.painter().text(badge, egui::Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(11.5), TEXT);
        ui.painter().text(pos2(r.left() + 12.0, r.top() + 42.0), egui::Align2::LEFT_CENTER, step.title, FontId::proportional(13.0), TEXT);
        ui.painter().text(pos2(r.left() + 12.0, r.top() + 62.0), egui::Align2::LEFT_CENTER, step.app, FontId::proportional(12.0), super::chrome::BLUE);
        if col + 1 < per_row && i + 1 < WORKFLOW.len() {
            let y = r.center().y;
            let (a, b) = (r.right() + 6.0, r.right() + STEP_GAP - 6.0);
            ui.painter().line_segment([pos2(a, y), pos2(b, y)], Stroke::new(1.4, FAINT));
            ui.painter().line_segment([pos2(b - 5.0, y - 4.0), pos2(b, y)], Stroke::new(1.4, FAINT));
            ui.painter().line_segment([pos2(b - 5.0, y + 4.0), pos2(b, y)], Stroke::new(1.4, FAINT));
        }
        let resp = resp.on_hover_text(step.agent).on_hover_cursor(egui::CursorIcon::PointingHand);
        app.automation.add_response(format!("home.workflow.{}", i + 1), "button", step.title, &resp);
        let mut opened = false;
        if let Some(id) = step.launch
            && let Some(path) = launcher_path(id)
        {
            let open = Rect::from_min_size(pos2(r.left() + 12.0, r.bottom() - 20.0), vec2(48.0, 14.0));
            let o = ui.interact(open, ui.id().with(("workflow-open", i)), Sense::click());
            let color = if o.hovered() { TEXT } else { FAINT };
            ui.painter().text(open.left_center(), egui::Align2::LEFT_CENTER, "Open", FontId::proportional(11.5), color);
            let o = o.on_hover_text(format!("Open {} ({})", step.app, path.display()));
            app.automation.add_response(format!("home.workflow.{}.open", i + 1), "link", &format!("Open {}", step.app), &o);
            if o.clicked() {
                opened = true;
                app.ui_state.status_line = match std::process::Command::new(&path).spawn() {
                    Ok(_) => format!("Opened {}", step.app),
                    Err(e) => format!("Could not open {}: {e}", path.display()),
                };
            }
        }
        if resp.clicked() && !opened {
            ui.ctx().copy_text(step.agent.to_owned());
            app.ui_state.status_line = format!("Copied step {} instructions for the agent", i + 1);
        }
    }
}

/// Fully rounded button: primary filled, secondary outlined (the suite's pro-theme buttons).
fn pill_button(ui: &mut Ui, label: &str, min_width: f32, primary: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(label.into(), FontId::proportional(13.0), TEXT);
    let size = vec2((galley.size().x + 28.0).max(min_width), 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    let hot = resp.hovered() || resp.is_pointer_button_down_on();
    let radius = rect.height() / 2.0;
    if primary {
        ui.painter().rect_filled(rect, radius, if hot { PRIMARY_HOVER } else { PRIMARY });
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, super::chrome::TEAL.gamma_multiply(0.65)), StrokeKind::Inside);
    } else {
        if hot {
            ui.painter().rect_filled(rect, radius, HOVER);
        }
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.5, if hot { TEXT } else { MUTED }), StrokeKind::Inside);
    }
    let color = if ui.is_enabled() { TEXT } else { FAINT };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn file_icon(ui: &Ui, centre: egui::Pos2, color: Color32) {
    let body = Rect::from_center_size(centre, vec2(10.0, 13.0));
    let fold = 3.5;
    let stroke = Stroke::new(1.2, color);
    let p = ui.painter();
    p.line_segment([body.left_top(), pos2(body.right() - fold, body.top())], stroke);
    p.line_segment([pos2(body.right() - fold, body.top()), pos2(body.right(), body.top() + fold)], stroke);
    p.line_segment([pos2(body.right(), body.top() + fold), body.right_bottom()], stroke);
    p.line_segment([body.right_bottom(), body.left_bottom()], stroke);
    p.line_segment([body.left_bottom(), body.left_top()], stroke);
}

fn elided(ui: &Ui, text: &str, size: f32, color: Color32, max_w: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), FontId::proportional(size), color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w);
    ui.painter().layout_job(job)
}

/// The Home screen's "Recent" list: project name, its folder on the right, click to open. The
/// hovered row shows a × that takes it off the list; right-click offers Clear Recent Projects.
fn home_recent(app: &mut AnimationCraftApp, ui: &mut Ui, recent: &[String]) {
    let width = 380.0;
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0) + 8.0);
        ui.label(egui::RichText::new("Recent").size(12.5).strong().color(MUTED));
    });
    ui.add_space(4.0);
    let mut open = None;
    let mut remove = None;
    let mut clear = false;
    for (i, path) in recent.iter().enumerate() {
        let file = Path::new(path);
        let name = file.file_stem().and_then(|n| n.to_str()).unwrap_or(path).to_owned();
        let folder = file.parent().map(|d| d.display().to_string()).unwrap_or_default();
        let (row, resp) = ui.allocate_exact_size(vec2(width, HOME_RECENT_ROW), Sense::click());
        let hovered = ui.rect_contains_pointer(row);
        if hovered {
            ui.painter().rect_filled(row, 3.0, HOVER);
        }
        let close = Rect::from_center_size(pos2(row.right() - 16.0, row.center().y), vec2(22.0, 22.0));
        let right = if hovered { close.left() - 4.0 } else { row.right() - 10.0 };
        file_icon(ui, pos2(row.left() + 16.0, row.center().y), MUTED);
        let x = row.left() + 32.0;
        let name_g = elided(ui, &name, 12.5, TEXT, 170.0);
        let name_w = name_g.size().x;
        ui.painter().galley(pos2(x, row.center().y - name_g.size().y / 2.0), name_g, TEXT);
        let folder_max = (right - (x + name_w + 14.0)).max(0.0);
        if folder_max > 20.0 && !folder.is_empty() {
            let fg = elided(ui, &folder, 11.5, FAINT, folder_max);
            ui.painter().galley(pos2(right - fg.size().x, row.center().y - fg.size().y / 2.0), fg, FAINT);
        }
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
        app.automation.add_response(format!("home.recent.{i}"), "button", &name, &resp);
        let resp = resp.on_hover_text(path).on_hover_cursor(egui::CursorIcon::PointingHand);
        if hovered {
            let x = ui.interact(close, resp.id.with("remove"), Sense::click());
            let color = if x.hovered() { TEXT } else { MUTED };
            let c = close.center();
            for (a, b) in [(vec2(-4.0, -4.0), vec2(4.0, 4.0)), (vec2(-4.0, 4.0), vec2(4.0, -4.0))] {
                ui.painter().line_segment([c + a, c + b], Stroke::new(1.4, color));
            }
            if x.on_hover_text("Remove from Recent").clicked() {
                remove = Some(path.clone());
            }
        }
        if resp.clicked() && remove.is_none() {
            open = Some(path.clone());
        }
        resp.context_menu(|ui| {
            if ui.button("Remove from Recent").clicked() {
                remove = Some(path.clone());
                ui.close();
            }
            if ui.button("Clear Recent Projects").clicked() {
                clear = true;
                ui.close();
            }
        });
    }
    let file = app.ui_state.recent_file.clone();
    let list = &mut app.ui_state.recent_projects;
    if clear {
        list.clear();
        save_recent(file.as_deref(), list);
    } else if let Some(path) = remove {
        list.retain(|p| *p != path);
        save_recent(file.as_deref(), list);
    }
    if let Some(path) = open {
        open_recent(app, &path);
    }
}
