//! Focused pose/reference inspection. Presentation state is scoped to each asset.
use crate::{AnimationCraftApp, CenterTab, UiCommand};
use animationcraft_engine::{Asset, ManifestFrame};
use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui, Vec2};
use std::path::{Path, PathBuf};

const TEXT: Color32 = Color32::from_rgb(235, 240, 248);
const MUTED: Color32 = Color32::from_rgb(164, 175, 191);
const ACCENT: Color32 = Color32::from_rgb(112, 196, 255);
const SURFACE: Color32 = Color32::from_rgb(25, 30, 39);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    Pose,
    #[default]
    All,
    Source,
    Parts,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Background {
    #[default]
    Checker,
    Light,
    Dark,
}
#[derive(Clone)]
struct ViewerState {
    mode: Mode,
    background: Background,
    fit: bool,
    zoom: f32,
    pan: Vec2,
    crop_bounds: bool,
    labels: bool,
    anchors: bool,
    source_index: usize,
    source_open: bool,
    rig_parts: bool,
    selected_part: Option<String>,
    last_pose: usize,
}
impl Default for ViewerState {
    fn default() -> Self {
        Self {
            mode: Mode::All,
            background: Background::Checker,
            fit: true,
            zoom: 1.0,
            pan: Vec2::ZERO,
            crop_bounds: false,
            labels: false,
            anchors: false,
            source_index: 0,
            source_open: false,
            rig_parts: false,
            selected_part: None,
            last_pose: usize::MAX,
        }
    }
}
impl ViewerState {
    fn fit_image(&mut self) {
        self.fit = true;
        self.pan = Vec2::ZERO;
    }
    fn select_mode(&mut self, mode: Mode) {
        if self.mode != mode {
            self.mode = mode;
            self.fit_image();
        }
    }
    fn change_zoom(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(0.05, 8.0);
        self.fit = false;
    }
}

struct Source {
    label: String,
    path: PathBuf,
}
fn sources(asset: &Asset) -> Vec<Source> {
    let mut result = Vec::new();
    for (label, path) in [
        (
            "Pose sheet",
            asset
                .pose_manifest
                .as_ref()
                .and_then(|m| m.source_image.as_ref()),
        ),
        (
            "Exploded parts sheet",
            asset
                .cutout_rig
                .as_ref()
                .and_then(|r| r.cutout_asset_sheet.as_ref()),
        ),
        (
            "Turnaround reference",
            asset.attached_turnaround_sheet.as_ref().or_else(|| {
                asset
                    .turnaround_manifest
                    .as_ref()
                    .and_then(|m| m.source_image.as_ref())
            }),
        ),
        ("Attached artwork", asset.attached_asset_sheet.as_ref()),
    ] {
        if let Some(path) = path
            && !result.iter().any(|s: &Source| s.path == *path)
        {
            result.push(Source {
                label: label.into(),
                path: path.clone(),
            });
        }
    }
    for reference in &asset.attached_references {
        if !result.iter().any(|s| s.path == reference.path) {
            result.push(Source {
                label: reference.label.clone(),
                path: reference.path.clone(),
            });
        }
    }
    result
}

fn can_map_crops(asset: &Asset, source: &Path, size: [usize; 2]) -> bool {
    asset.pose_manifest.as_ref().is_some_and(|m| {
        m.source_image.as_deref() == Some(source)
            && m.source_size == Some([size[0] as u32, size[1] as u32])
    })
}

fn readable(name: &str) -> String {
    match name {
        "arm_L" => "Left arm".into(),
        "arm_R" => "Right arm".into(),
        "leg_L" => "Left leg".into(),
        "leg_R" => "Right leg".into(),
        "prop_backpack" => "Backpack".into(),
        _ => {
            let text = name.replace('_', " ");
            let mut chars = text.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_else(|| "Unnamed".into())
        }
    }
}

fn navigate(index: usize, delta: isize, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        index.saturating_add_signed(delta).min(count - 1)
    }
}

fn button(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    id: &str,
    label: &str,
    enabled: bool,
    help: &str,
) -> bool {
    let response = ui
        .add_enabled(
            enabled,
            egui::Button::new(RichText::new(label).color(TEXT)).min_size(Vec2::new(0.0, 28.0)),
        )
        .on_hover_text(help);
    app.automation.add_response(id, "button", label, &response);
    response.clicked()
}
fn surface() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(49, 59, 74)))
        .corner_radius(7)
        .inner_margin(10)
}

pub(super) fn show(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let Some(asset) = app.session().current_asset().cloned() else {
        ui.heading("Pose browser");
        ui.label("Connect an asset to inspect its poses and parts.");
        return;
    };
    let frames = asset
        .pose_manifest
        .as_ref()
        .map(|m| m.frames.as_slice())
        .unwrap_or(&[]);
    app.ui_state.selected_pose_idx = navigate(app.ui_state.selected_pose_idx, 0, frames.len());
    let state_id = ui.id().with(("pose_viewer", &asset.id));
    let mut state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<ViewerState>(state_id))
        .unwrap_or_default();
    if ui
        .ctx()
        .data_mut(|d| d.remove_temp::<usize>(egui::Id::new(("pose_viewer_selection", &asset.id))))
        .is_some()
    {
        state.select_mode(Mode::Pose);
        state.fit_image();
    }
    if state.last_pose != app.ui_state.selected_pose_idx {
        state.last_pose = app.ui_state.selected_pose_idx;
        state.selected_part = None;
        state.fit_image();
    }
    surface().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            for (mode, id, label) in [
                (Mode::Source, "source", "Sheets"),
                (Mode::All, "all", "Poses"),
                (Mode::Parts, "parts", "Parts"),
            ] {
                let active = state.mode == mode || (mode == Mode::All && state.mode == Mode::Pose);
                let response = super::chrome::tab(ui, label, active);
                app.automation
                    .add_response(format!("pose.mode.{id}"), "tab", label, &response);
                if response.clicked() {
                    state.select_mode(mode);
                    if mode == Mode::Source {
                        state.source_open = false;
                    }
                }
            }
        });
        if state.mode == Mode::Pose {
            if button(
                app,
                ui,
                "pose.back",
                "Back to poses",
                true,
                "Return to the complete pose gallery, keeping this pose selected.",
            ) {
                state.select_mode(Mode::All);
            }
            ui.horizontal_wrapped(|ui| {
                let index = app.ui_state.selected_pose_idx;
                if button(
                    app,
                    ui,
                    "pose.previous",
                    "Previous",
                    !frames.is_empty() && index > 0,
                    "Inspect the previous pose. Arrow keys also work when the canvas is focused.",
                ) {
                    app.ui_state.selected_pose_idx = navigate(index, -1, frames.len());
                    app.ui_state.playing = false;
                    state.fit_image();
                }
                if let Some(frame) = frames.get(app.ui_state.selected_pose_idx) {
                    ui.label(
                        RichText::new(format!(
                            "{} · {} of {}",
                            readable(&frame.label),
                            app.ui_state.selected_pose_idx + 1,
                            frames.len()
                        ))
                        .strong()
                        .color(TEXT),
                    );
                } else {
                    ui.label("No pose frames loaded");
                }
                if button(
                    app,
                    ui,
                    "pose.next",
                    "Next",
                    index + 1 < frames.len(),
                    "Inspect the next pose.",
                ) {
                    app.ui_state.selected_pose_idx = navigate(index, 1, frames.len());
                    app.ui_state.playing = false;
                    state.fit_image();
                }
            });
        }
        if state.last_pose != app.ui_state.selected_pose_idx {
            state.last_pose = app.ui_state.selected_pose_idx;
            state.selected_part = None;
        }
    });
    ui.add_space(8.0);
    let current = frames.get(app.ui_state.selected_pose_idx);
    match state.mode {
        Mode::All => pose_grid(app, ui, frames, &mut state),
        Mode::Pose => {
            if let Some(frame) = current {
                image_controls(app, ui, &mut state, false, false);
                canvas(
                    app,
                    ui,
                    &frame.resolved_path,
                    &mut state,
                    None,
                    None,
                    frames.len(),
                );
            } else {
                empty(
                    ui,
                    "No poses yet",
                    "Import a pose sheet, or open Sheets to inspect attached artwork.",
                );
            }
        }
        Mode::Source => {
            if button(
                app,
                ui,
                "pose.add_sheets",
                "Add sheet…",
                app.ui_state.busy_command.is_none(),
                "Attach extra reference sheets without replacing the current pose or turnaround sheet. This processes the images for transparency; it does not slice them.",
            ) && let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
            {
                app.exec_ui_command(UiCommand::IntakeValidate {
                    image_path: path.to_string_lossy().into_owned(),
                    slot: "reference".into(),
                    attach_to_active: true,
                    auto_cut: false,
                    tolerance: app.ui_state.bg_tolerance,
                });
            }
            let menu = ui.menu_button("Sheet tools", |ui| {
                let ready = app.ui_state.busy_command.is_none();
                if button(app, ui, "pose.import_sheet", "Replace pose sheet…", ready, "Replace and slice the active pose sheet. Use Add sheet to keep existing references.")
                    && let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png","jpg","jpeg","webp"]).pick_file() {
                    app.exec_ui_command(UiCommand::IntakeValidate { image_path: path.to_string_lossy().into_owned(), slot: "asset_sheet".into(), attach_to_active: true, auto_cut: true, tolerance: app.ui_state.bg_tolerance }); ui.close();
                }
                if button(app, ui, "pose.recut", "Re-cut active sheet", ready, "Re-run the cutter for the active asset.") {
                    app.exec_ui_command(UiCommand::AssetCutSheet { asset_id: asset.id.clone(), tolerance: app.ui_state.bg_tolerance, use_ocr: app.ui_state.use_ocr }); ui.close();
                }
            });
            app.automation
                .add_response("pose.tools", "button", "Sheet tools", &menu.response);
            let choices = sources(&asset);
            if choices.is_empty() {
                empty(
                    ui,
                    "No source sheet",
                    "Attach a pose sheet or a rig parts sheet to inspect it here.",
                );
            } else {
                state.source_index = state.source_index.min(choices.len() - 1);
                if !state.source_open {
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .id_salt("sheet_gallery")
                        .show(ui, |ui| {
                            let columns = ((ui.available_width() / 190.0).floor() as usize).max(1);
                            let width = (ui.available_width()
                                - ui.spacing().item_spacing.x * (columns - 1) as f32)
                                / columns as f32;
                            for (row_index, row) in choices.chunks(columns).enumerate() {
                                ui.horizontal(|ui| {
                                    for (column, source) in row.iter().enumerate() {
                                        let i = row_index * columns + column;
                                        if tile(
                                            app,
                                            ui,
                                            &format!("pose.source.{i}"),
                                            &source.label,
                                            &source.path,
                                            false,
                                            width,
                                        )
                                        .clicked()
                                        {
                                            state.source_index = i;
                                            state.source_open = true;
                                            state.fit_image();
                                        }
                                    }
                                });
                            }
                        });
                } else {
                    if button(
                        app,
                        ui,
                        "pose.sheets.back",
                        "Back to sheets",
                        true,
                        "Show every attached sheet.",
                    ) {
                        state.source_open = false;
                    }
                    ui.label(RichText::new(&choices[state.source_index].label).strong());
                    let source = &choices[state.source_index];
                    let context = ui.ctx().clone();
                    let maps = app
                        .load_texture(&context, &source.path.to_string_lossy())
                        .is_some_and(|t| can_map_crops(&asset, &source.path, t.size()));
                    image_controls(app, ui, &mut state, maps, false);
                    if !maps {
                        ui.label(RichText::new("Crop overlays unavailable: this image has no matching source coordinates.").small().color(MUTED));
                    }
                    canvas(
                        app,
                        ui,
                        &source.path,
                        &mut state,
                        maps.then_some(frames),
                        None,
                        frames.len(),
                    );
                }
            }
        }
        Mode::Parts => parts_view(app, ui, &asset, current, &mut state, frames.len()),
    }
    ui.ctx().data_mut(|d| d.insert_temp(state_id, state));
}

fn empty(ui: &mut Ui, title: &str, message: &str) {
    surface().show(ui, |ui| {
        ui.heading(title);
        ui.add(egui::Label::new(message).wrap());
    });
}
fn image_controls(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    state: &mut ViewerState,
    crops: bool,
    anchors: bool,
) {
    ui.horizontal_wrapped(|ui| {
        let fit=ui.add(egui::Button::selectable(state.fit,"Fit").min_size(Vec2::new(0.0,28.0))).on_hover_text("Fit the complete artwork and reset panning.");
        app.automation.add_response("pose.zoom.fit","button","Fit artwork",&fit);
        if fit.clicked(){state.fit_image();}
        let actual=ui.add(egui::Button::selectable(!state.fit && (state.zoom-1.0).abs()<0.001,"100%").min_size(Vec2::new(0.0,28.0))).on_hover_text("Actual size: one image pixel per UI point.");
        app.automation.add_response("pose.zoom.actual","button","Actual size",&actual);
        if actual.clicked(){state.fit=false;state.zoom=1.0;state.pan=Vec2::ZERO;}
        if button(app,ui,"pose.zoom.out","−",true,"Zoom out. Drag the canvas to pan.") {state.change_zoom(0.8);}
        if button(app,ui,"pose.zoom.in","+",true,"Zoom in. Double-click the canvas to fit.") {state.change_zoom(1.25);}
        let menu=ui.menu_button("View options",|ui| {
            ui.label(RichText::new("Background").strong());
            for (bg,id,label) in [(Background::Checker,"checker","Checkerboard"),(Background::Light,"light","Light"),(Background::Dark,"dark","Dark")] {
                let response=ui.selectable_value(&mut state.background,bg,label);
                app.automation.add_response(format!("pose.background.{id}"),"button",label,&response);
            }
            ui.separator();ui.label(RichText::new("Overlays").strong());
            let response=ui.add_enabled(crops,egui::Checkbox::new(&mut state.crop_bounds,"Crop bounds"))
                .on_disabled_hover_text("Available only on the matching pose source with known dimensions.");
            app.automation.add_response("pose.overlay.crops","checkbox","Crop bounds",&response);
            let response=ui.add_enabled(crops || anchors,egui::Checkbox::new(&mut state.labels,"Part / pose names"))
                .on_hover_text("Name the displayed part or the pose crop boxes. Thumbnail captions always remain visible.");
            app.automation.add_response("pose.overlay.labels","checkbox","Part / pose names",&response);
            let response=ui.add_enabled(anchors,egui::Checkbox::new(&mut state.anchors,"Joint anchors"))
                .on_disabled_hover_text("Available for an individual part with a local pivot.");
            app.automation.add_response("pose.overlay.anchor","checkbox","Joint anchors",&response);
            ui.separator();ui.label(RichText::new("Zoom").strong());
            let response=ui.selectable_label(state.fit,"Fit artwork");
            app.automation.add_response("pose.options.fit","button","Fit artwork",&response);
            if response.clicked(){state.fit_image();}
            let response=ui.selectable_label(!state.fit && (state.zoom-1.0).abs()<0.001,"Actual size · 100%");
            app.automation.add_response("pose.options.actual","button","Actual size",&response);
            if response.clicked(){state.fit=false;state.zoom=1.0;state.pan=Vec2::ZERO;}
        });
        app.automation.add_response("pose.view_options","button","View options",&menu.response);
    });
    let background = match state.background {
        Background::Checker => "Checkerboard",
        Background::Light => "Light",
        Background::Dark => "Dark",
    };
    let overlays = (crops && state.crop_bounds) || (anchors && (state.anchors || state.labels));
    ui.label(
        RichText::new(format!(
            "{background} · {} · {}",
            if state.fit {
                "Fit artwork".into()
            } else {
                format!("{:.0}%", state.zoom * 100.0)
            },
            if overlays {
                "Overlays on"
            } else {
                "Overlays off"
            }
        ))
        .small()
        .color(MUTED),
    );
    ui.add_space(5.0);
}

fn paint_background(painter: &egui::Painter, rect: Rect, bg: Background) {
    match bg {
        Background::Checker => super::paint_transparent_checkerboard(painter, rect, false),
        Background::Light => {
            painter.rect_filled(rect, 5.0, Color32::from_rgb(227, 231, 237));
        }
        Background::Dark => {
            painter.rect_filled(rect, 5.0, Color32::from_rgb(15, 18, 24));
        }
    }
}

fn canvas(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    path: &Path,
    state: &mut ViewerState,
    crops: Option<&[ManifestFrame]>,
    part: Option<(&str, [f32; 2])>,
    count: usize,
) {
    let context = ui.ctx().clone();
    let texture = app.load_texture(&context, &path.to_string_lossy());
    ui.add(
        egui::Label::new(
            RichText::new(path.file_name().and_then(|s| s.to_str()).unwrap_or("Image"))
                .small()
                .color(MUTED),
        )
        .truncate(),
    )
    .on_hover_text(path.display().to_string());
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(
            ui.available_width().max(40.0),
            ui.available_height().max(120.0),
        ),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    paint_background(&painter, rect, state.background);
    app.automation
        .add_response("pose.canvas", "canvas", "Image canvas", &response);
    if response.clicked() {
        response.request_focus();
    }
    if response.double_clicked() {
        state.fit_image();
    }
    if response.has_focus() {
        let delta = ui.input(|i| {
            if i.key_pressed(egui::Key::ArrowRight) {
                1
            } else if i.key_pressed(egui::Key::ArrowLeft) {
                -1
            } else {
                0
            }
        });
        if delta != 0 {
            app.ui_state.selected_pose_idx = navigate(app.ui_state.selected_pose_idx, delta, count);
            app.ui_state.playing = false;
            state.fit_image();
        }
    }
    let Some(texture) = texture else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Image unavailable",
            egui::FontId::proportional(16.0),
            MUTED,
        );
        return;
    };
    let size = texture.size_vec2();
    if state.fit {
        state.zoom = ((rect.width() - 24.0).max(1.0) / size.x.max(1.0))
            .min((rect.height() - 24.0).max(1.0) / size.y.max(1.0));
        state.pan = Vec2::ZERO;
    }
    if response.dragged() {
        state.fit = false;
        state.pan += response.drag_delta();
    }
    let drawn = size * state.zoom;
    let limit = (drawn + rect.size()) * 0.5 - Vec2::splat(20.0);
    state.pan = state
        .pan
        .clamp(-limit.max(Vec2::ZERO), limit.max(Vec2::ZERO));
    let image_rect = Rect::from_center_size(rect.center() + state.pan, drawn);
    painter.image(
        texture.id(),
        image_rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    app.automation.add(
        "pose.active_image",
        "image",
        path.display().to_string(),
        image_rect.intersect(rect),
    );
    if let Some(frames) = crops {
        for (index, frame) in frames.iter().enumerate() {
            let Some([x0, y0, x1, y1]) = frame.source_box else {
                continue;
            };
            if x1 <= x0 || y1 <= y0 || x1 as f32 > size.x || y1 as f32 > size.y {
                continue;
            }
            let bounds = Rect::from_min_max(
                image_rect.min + Vec2::new(x0 as f32, y0 as f32) * state.zoom,
                image_rect.min + Vec2::new(x1 as f32, y1 as f32) * state.zoom,
            );
            let selected = app.ui_state.selected_pose_idx == index;
            if state.crop_bounds || selected {
                painter.rect_stroke(
                    bounds,
                    2.0,
                    Stroke::new(
                        if selected { 2.0 } else { 1.0 },
                        if selected { ACCENT } else { MUTED },
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            if state.labels {
                painter.text(
                    bounds.min + Vec2::splat(4.0),
                    egui::Align2::LEFT_TOP,
                    readable(&frame.label),
                    egui::FontId::proportional(11.0),
                    ACCENT,
                );
            }
            let hit = bounds.intersect(rect);
            if hit.is_positive() {
                let clicked = ui.interact(hit, ui.id().with(("crop", index)), egui::Sense::click());
                app.automation.add_response(
                    format!("pose.crop.{index}"),
                    "button",
                    &frame.label,
                    &clicked,
                );
                if clicked.clicked() {
                    app.ui_state.selected_pose_idx = index;
                    app.ui_state.playing = false;
                }
                clicked.context_menu(|ui| {
                    if button(
                        app,
                        ui,
                        &format!("pose.crop.open.{index}"),
                        "Open single pose",
                        true,
                        "Inspect this crop in the single-pose view.",
                    ) {
                        app.ui_state.selected_pose_idx = index;
                        app.ui_state.playing = false;
                        state.select_mode(Mode::Pose);
                        ui.close();
                    }
                });
            }
        }
    }
    if let Some((name, [x, y])) = part
        && x >= 0.0
        && y >= 0.0
        && x <= size.x
        && y <= size.y
    {
        let point = image_rect.min + Vec2::new(x, y) * state.zoom;
        if state.anchors {
            painter.circle_stroke(point, 7.0, Stroke::new(2.0, ACCENT));
            painter.line_segment(
                [point - Vec2::new(11.0, 0.0), point + Vec2::new(11.0, 0.0)],
                Stroke::new(1.0, ACCENT),
            );
            painter.line_segment(
                [point - Vec2::new(0.0, 11.0), point + Vec2::new(0.0, 11.0)],
                Stroke::new(1.0, ACCENT),
            );
        }
        if state.labels {
            painter.text(
                point + Vec2::new(10.0, 10.0),
                egui::Align2::LEFT_TOP,
                readable(name),
                egui::FontId::proportional(11.0),
                ACCENT,
            );
        }
    }
}

fn tile(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    id: &str,
    label: &str,
    path: &Path,
    selected: bool,
    width: f32,
) -> egui::Response {
    let response = ui.add_sized(
        [width, 160.0],
        egui::Button::new("")
            .fill(SURFACE)
            .stroke(Stroke::new(
                if selected { 1.5 } else { 1.0 },
                if selected {
                    ACCENT
                } else {
                    super::chrome::BORDER
                },
            ))
            .corner_radius(6),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
    let rect = response.rect;
    let bounds = Rect::from_min_max(
        rect.min + Vec2::splat(6.0),
        Pos2::new(rect.right() - 6.0, rect.bottom() - 34.0),
    );
    let painter = ui.painter_at(rect);
    paint_background(&painter, bounds, Background::Checker);
    let context = ui.ctx().clone();
    if let Some(texture) = app.load_texture(&context, &path.to_string_lossy()) {
        let size = texture.size_vec2();
        let scale = (bounds.width() / size.x.max(1.0)).min(bounds.height() / size.y.max(1.0));
        painter.image(
            texture.id(),
            Rect::from_center_size(bounds.center(), size * scale),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        painter.text(
            bounds.center(),
            egui::Align2::CENTER_CENTER,
            "Unavailable",
            egui::FontId::proportional(11.0),
            MUTED,
        );
    }
    let short_label = if label.chars().count() > 27 {
        format!("{}…", label.chars().take(26).collect::<String>())
    } else {
        label.into()
    };
    let text = painter.layout(
        short_label,
        egui::FontId::proportional(12.0),
        TEXT,
        width - 10.0,
    );
    painter.galley(
        Pos2::new(rect.center().x - text.size().x * 0.5, rect.bottom() - 29.0),
        text,
        TEXT,
    );
    app.automation.add_response(id, "button", label, &response);
    response.on_hover_text(path.display().to_string())
}
fn pose_grid(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    frames: &[ManifestFrame],
    state: &mut ViewerState,
) {
    if frames.is_empty() {
        empty(
            ui,
            "No poses yet",
            "Attach and slice a pose sheet to populate the contact sheet.",
        );
        return;
    }
    ui.label(
        RichText::new("Click to select · Right-click to open a single pose")
            .small()
            .color(MUTED),
    );
    egui::ScrollArea::vertical()
        .id_salt("pose_contact_sheet")
        .show(ui, |ui| {
            let columns = ((ui.available_width() / 132.0).floor() as usize).max(1);
            let width = (ui.available_width() - ui.spacing().item_spacing.x * (columns - 1) as f32)
                / columns as f32;
            for (row_index, row) in frames.chunks(columns).enumerate() {
                ui.horizontal(|ui| {
                    for (column, frame) in row.iter().enumerate() {
                        let index = row_index * columns + column;
                        let response = tile(
                            app,
                            ui,
                            &format!("pose.tile.{index}"),
                            &readable(&frame.label),
                            &frame.resolved_path,
                            app.ui_state.selected_pose_idx == index,
                            width,
                        );
                        if response.clicked() {
                            app.ui_state.selected_pose_idx = index;
                            app.ui_state.playing = false;
                        }
                        response.context_menu(|ui| {
                            if button(
                                app,
                                ui,
                                &format!("pose.open.{index}"),
                                "Open single pose",
                                true,
                                "Inspect this pose at a larger size.",
                            ) {
                                app.ui_state.selected_pose_idx = index;
                                app.ui_state.playing = false;
                                state.select_mode(Mode::Pose);
                                ui.close();
                            }
                        });
                    }
                });
            }
        });
}

struct Part {
    id: String,
    path: PathBuf,
    pivot: [f32; 2],
}
fn parts_view(
    app: &mut AnimationCraftApp,
    ui: &mut Ui,
    asset: &Asset,
    frame: Option<&ManifestFrame>,
    state: &mut ViewerState,
    count: usize,
) {
    ui.horizontal_wrapped(|ui| {
        for (rig, id, label) in [
            (false, "pose", "Selected pose parts"),
            (true, "rig", "Connected rig parts"),
        ] {
            let response = ui.selectable_label(state.rig_parts == rig, label);
            app.automation.add_response(
                format!("pose.parts_source.{id}"),
                "button",
                label,
                &response,
            );
            if response.clicked() {
                state.rig_parts = rig;
                state.selected_part = None;
                state.fit_image();
            }
        }
    });
    let parts: Vec<Part> = if state.rig_parts {
        asset
            .cutout_rig
            .as_ref()
            .map(|r| {
                r.parts
                    .iter()
                    .map(|p| Part {
                        id: p.id.clone(),
                        path: p.cropped_path.clone(),
                        pivot: [
                            p.pivot[0] - p.crop_box[0] as f32,
                            p.pivot[1] - p.crop_box[1] as f32,
                        ],
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        frame
            .map(|f| {
                f.parts
                    .iter()
                    .map(|(name, p)| Part {
                        id: name.clone(),
                        path: p.resolved_path.clone(),
                        pivot: p.pivot,
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    if parts.is_empty() {
        empty(
            ui,
            "No parts in this source",
            if state.rig_parts {
                "Connect or build a rig to inspect its parts."
            } else {
                "This pose has no extracted parts. You can inspect the connected rig separately."
            },
        );
        return;
    }
    if let Some(part) = parts
        .iter()
        .find(|p| Some(&p.id) == state.selected_part.as_ref())
    {
        ui.horizontal_wrapped(|ui| {
            if button(
                app,
                ui,
                "pose.parts.back",
                "All parts",
                true,
                "Return to the parts grid.",
            ) {
                state.selected_part = None;
                state.fit_image();
            }
            ui.label(RichText::new(readable(&part.id)).strong());
        });
        image_controls(app, ui, state, false, true);
        canvas(
            app,
            ui,
            &part.path,
            state,
            None,
            Some((&part.id, part.pivot)),
            count,
        );
    } else {
        ui.label(
            RichText::new("Select a part for a larger inspection view.")
                .small()
                .color(MUTED),
        );
        egui::ScrollArea::vertical()
            .id_salt("pose_parts_grid")
            .show(ui, |ui| {
                let columns = ((ui.available_width() / 132.0).floor() as usize).max(1);
                let width = (ui.available_width()
                    - ui.spacing().item_spacing.x * (columns - 1) as f32)
                    / columns as f32;
                for row in parts.chunks(columns) {
                    ui.horizontal(|ui| {
                        for part in row {
                            if tile(
                                app,
                                ui,
                                &format!("pose.part.{}", part.id),
                                &readable(&part.id),
                                &part.path,
                                false,
                                width,
                            )
                            .clicked()
                            {
                                state.selected_part = Some(part.id.clone());
                                state.fit_image();
                            }
                        }
                    });
                }
            });
    }
}

pub(in crate::panels) fn select_pose(
    app: &mut AnimationCraftApp,
    ctx: &egui::Context,
    index: usize,
) {
    app.ui_state.selected_pose_idx = index;
    app.ui_state.playing = false;
    app.ui_state.center_tab = CenterTab::AssetSheet;
    if let Some(asset_id) = app.session.active_asset.as_ref() {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(("pose_viewer_selection", asset_id)), index));
    }
}

pub(in crate::panels) fn sidebar(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let frames = app
        .session()
        .current_asset()
        .and_then(|a| a.pose_manifest.as_ref())
        .map(|m| m.frames.clone())
        .unwrap_or_default();
    let history_id = ui.id().with((
        "pose_sidebar_last_selection",
        app.session.active_asset.clone(),
    ));
    let previous = ui.ctx().data_mut(|d| d.get_temp::<usize>(history_id));
    let selection_changed = previous.is_some_and(|p| p != app.ui_state.selected_pose_idx);
    surface().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(format!("Pose references · {}", frames.len()))
                .size(14.0)
                .strong()
                .color(TEXT),
        );
        ui.add(
            egui::Label::new(
                RichText::new("Select a thumbnail to open its single view.")
                    .small()
                    .color(MUTED),
            )
            .wrap(),
        );
        if frames.is_empty() {
            ui.label("No pose frames loaded.");
            return;
        }
        let context = ui.ctx().clone();
        for (index, frame) in frames.iter().enumerate() {
            let selected = app.ui_state.selected_pose_idx == index;
            let name = readable(&frame.label);
            let response = ui.add_sized(
                [ui.available_width(), 56.0],
                egui::Button::new("").selected(selected).corner_radius(5),
            );
            if selected && selection_changed {
                response.scroll_to_me(Some(egui::Align::Center));
            }
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name)
            });
            let rect = response.rect;
            let picture = Rect::from_min_size(rect.min + Vec2::splat(5.0), Vec2::new(36.0, 46.0));
            let painter = ui.painter_at(rect.shrink(4.0));
            if let Some(texture) =
                app.load_texture(&context, &frame.resolved_path.to_string_lossy())
            {
                let size = texture.size_vec2();
                let scale =
                    (picture.width() / size.x.max(1.0)).min(picture.height() / size.y.max(1.0));
                painter.image(
                    texture.id(),
                    Rect::from_center_size(picture.center(), size * scale),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            painter.text(
                Pos2::new(rect.left() + 49.0, rect.top() + 10.0),
                egui::Align2::LEFT_TOP,
                &name,
                egui::FontId::proportional(12.0),
                TEXT,
            );
            painter.text(
                Pos2::new(rect.left() + 49.0, rect.top() + 31.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "{} of {}{}",
                    index + 1,
                    frames.len(),
                    if selected { " · selected" } else { "" }
                ),
                egui::FontId::proportional(10.0),
                MUTED,
            );
            app.automation
                .add_response(format!("pose.select.{index}"), "button", &name, &response);
            if response
                .on_hover_text(format!(
                    "Inspect {name}. This opens its source image and pauses playback."
                ))
                .clicked()
            {
                select_pose(app, ui.ctx(), index);
            }
        }
    });
    ui.ctx()
        .data_mut(|d| d.insert_temp(history_id, app.ui_state.selected_pose_idx));
}

#[cfg(test)]
mod tests {
    use super::*;
    use animationcraft_engine::LoadedManifest;

    #[test]
    fn navigation_clamps_at_edges_and_handles_no_frames() {
        assert_eq!(navigate(0, -1, 11), 0);
        assert_eq!(navigate(10, 1, 11), 10);
        assert_eq!(navigate(3, 1, 11), 4);
        assert_eq!(navigate(99, 0, 0), 0);
    }

    #[test]
    fn source_selection_never_maps_pose_boxes_onto_an_unrelated_parts_sheet() {
        let pose_source = PathBuf::from("pose-source.png");
        let unrelated = PathBuf::from("exploded-rig.png");
        let asset = Asset {
            attached_asset_sheet: Some(unrelated.clone()),
            attached_turnaround_sheet: Some(PathBuf::from("turnaround.png")),
            cutout_rig: Some(animationcraft_engine::session::LoadedCutoutRigPlan {
                cutout_asset_sheet: Some(unrelated.clone()),
                ..Default::default()
            }),
            pose_manifest: Some(LoadedManifest {
                source_image: Some(pose_source.clone()),
                source_size: Some([640, 480]),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(sources(&asset)[0].path, pose_source);
        assert_eq!(
            sources(&asset)
                .iter()
                .map(|s| s.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Pose sheet", "Exploded parts sheet", "Turnaround reference"]
        );
        assert!(can_map_crops(&asset, &pose_source, [640, 480]));
        assert!(!can_map_crops(&asset, &unrelated, [640, 480]));
        assert!(!can_map_crops(&asset, &pose_source, [320, 240]));
        let no_dimensions = Asset {
            pose_manifest: Some(LoadedManifest {
                source_image: Some(pose_source.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(!can_map_crops(&no_dimensions, &pose_source, [640, 480]));
    }

    fn fixture(name: &str) -> (PathBuf, AnimationCraftApp) {
        let root = std::env::temp_dir().join(format!(
            "animationcraft-viewer-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        for (name, size, color) in [
            ("first.png", [20, 40], [255, 0, 0, 255]),
            ("second.png", [40, 20], [0, 255, 0, 255]),
            ("head.png", [12, 16], [0, 0, 255, 255]),
        ] {
            image::RgbaImage::from_pixel(size[0], size[1], image::Rgba(color))
                .save(root.join(name))
                .unwrap();
        }
        let mut app = AnimationCraftApp::new();
        app.session.assets.clear();
        app.session.active_asset = Some("fixture".into());
        let mut part_map = std::collections::BTreeMap::new();
        part_map.insert(
            "head".into(),
            animationcraft_engine::session::ManifestPart {
                name: "head".into(),
                resolved_path: root.join("head.png"),
                pivot: [6.0, 8.0],
                size: [12, 16],
                ..Default::default()
            },
        );
        app.session.assets.insert(
            "fixture".into(),
            Asset {
                id: "fixture".into(),
                pose_manifest: Some(LoadedManifest {
                    frames: vec![
                        ManifestFrame {
                            label: "standing".into(),
                            resolved_path: root.join("first.png"),
                            parts: part_map.clone(),
                            ..Default::default()
                        },
                        ManifestFrame {
                            index: 1,
                            label: "jumping".into(),
                            resolved_path: root.join("second.png"),
                            parts: part_map,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        (root, app)
    }

    fn render(
        app: &mut AnimationCraftApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::Id {
        app.automation.begin_frame();
        let mut state_id = egui::Id::NULL;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                events,
                ..Default::default()
            },
            |ui| {
                let mut left = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_max(Pos2::ZERO, Pos2::new(210.0, 700.0))),
                );
                sidebar(app, &mut left);
                let mut main = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(
                    Pos2::new(220.0, 0.0),
                    Pos2::new(900.0, 700.0),
                )));
                state_id = main.id().with(("pose_viewer", "fixture"));
                show(app, &mut main);
            },
        );
        output.textures_delta.clear();
        state_id
    }

    fn click(app: &mut AnimationCraftApp, ctx: &egui::Context, id: &str) -> egui::Id {
        pointer_click(app, ctx, id, egui::PointerButton::Primary)
    }

    fn pointer_click(
        app: &mut AnimationCraftApp,
        ctx: &egui::Context,
        id: &str,
        mouse_button: egui::PointerButton,
    ) -> egui::Id {
        render(app, ctx, vec![]);
        let control = app
            .automation
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing control {id}"));
        let pos = Pos2::new(
            control.rect[0] + control.rect[2] * 0.5,
            control.rect[1] + control.rect[3] * 0.5,
        );
        for pressed in [true, false] {
            render(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: mouse_button,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        render(app, ctx, vec![])
    }

    fn displayed_image(app: &AnimationCraftApp) -> &str {
        &app.automation
            .elements
            .iter()
            .find(|e| e.id == "pose.active_image")
            .unwrap()
            .label
    }

    #[test]
    fn sidebar_and_previous_next_select_the_same_frame() {
        let (root, mut app) = fixture("selection");
        let ctx = egui::Context::default();
        click(&mut app, &ctx, "pose.mode.source");
        click(&mut app, &ctx, "pose.select.1");
        assert_eq!(app.ui_state.selected_pose_idx, 1);
        assert_eq!(app.ui_state.center_tab, CenterTab::AssetSheet);
        assert!(!app.ui_state.playing);
        assert_eq!(
            displayed_image(&app),
            root.join("second.png").to_string_lossy()
        );
        assert_eq!(
            app.automation
                .elements
                .iter()
                .find(|e| e.id == "pose.next")
                .unwrap()
                .enabled,
            Some(false)
        );
        click(&mut app, &ctx, "pose.previous");
        assert_eq!(app.ui_state.selected_pose_idx, 0);
        assert_eq!(
            displayed_image(&app),
            root.join("first.png").to_string_lossy()
        );
        click(&mut app, &ctx, "pose.next");
        assert_eq!(app.ui_state.selected_pose_idx, 1);
        assert_eq!(
            displayed_image(&app),
            root.join("second.png").to_string_lossy()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn contact_sheet_and_part_tiles_open_the_matching_large_image() {
        let (root, mut app) = fixture("tiles");
        let ctx = egui::Context::default();
        click(&mut app, &ctx, "pose.mode.all");
        let id = click(&mut app, &ctx, "pose.tile.1");
        assert!(
            ctx.data_mut(|d| d.get_temp::<ViewerState>(id))
                .unwrap()
                .mode
                == Mode::All
        );
        assert!(
            !app.automation
                .elements
                .iter()
                .any(|e| e.id == "pose.previous")
        );
        pointer_click(
            &mut app,
            &ctx,
            "pose.tile.1",
            egui::PointerButton::Secondary,
        );
        click(&mut app, &ctx, "pose.open.1");
        assert_eq!(app.ui_state.selected_pose_idx, 1);
        assert_eq!(
            displayed_image(&app),
            root.join("second.png").to_string_lossy()
        );
        click(&mut app, &ctx, "pose.back");
        assert_eq!(app.ui_state.selected_pose_idx, 1);
        assert!(
            ctx.data_mut(|d| d.get_temp::<ViewerState>(id))
                .unwrap()
                .mode
                == Mode::All
        );
        click(&mut app, &ctx, "pose.mode.parts");
        click(&mut app, &ctx, "pose.part.head");
        assert_eq!(
            displayed_image(&app),
            root.join("head.png").to_string_lossy()
        );
        click(&mut app, &ctx, "pose.parts.back");
        assert!(
            app.automation
                .elements
                .iter()
                .any(|e| e.id == "pose.part.head")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sheets_include_extra_references_and_return_to_gallery() {
        let (root, mut app) = fixture("sheets");
        let asset = app.session.assets.get_mut("fixture").unwrap();
        asset.attached_asset_sheet = Some(root.join("first.png"));
        asset
            .attached_references
            .push(animationcraft_engine::session::AttachedReference {
                label: "Extra animation poses".into(),
                kind: "reference".into(),
                path: root.join("second.png"),
                raw_path: None,
                width: 20,
                height: 20,
                report: None,
            });
        assert_eq!(sources(asset).len(), 2);
        let ctx = egui::Context::default();
        click(&mut app, &ctx, "pose.mode.source");
        assert!(
            app.automation
                .elements
                .iter()
                .any(|e| e.id == "pose.add_sheets")
        );
        click(&mut app, &ctx, "pose.source.1");
        assert_eq!(
            displayed_image(&app),
            root.join("second.png").to_string_lossy()
        );
        click(&mut app, &ctx, "pose.sheets.back");
        assert!(
            app.automation
                .elements
                .iter()
                .any(|e| e.id == "pose.source.0")
        );
        assert!(
            app.automation
                .elements
                .iter()
                .any(|e| e.id == "pose.source.1")
        );
        assert!(
            !app.automation
                .elements
                .iter()
                .any(|e| e.id == "pose.active_image")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn options_keep_overlays_off_until_requested_and_zoom_choices_are_real() {
        let (root, mut app) = fixture("options");
        let ctx = egui::Context::default();
        let id = render(&mut app, &ctx, vec![]);
        let initial = ctx.data_mut(|d| d.get_temp::<ViewerState>(id)).unwrap();
        assert!(!initial.crop_bounds && !initial.labels && !initial.anchors);
        click(&mut app, &ctx, "pose.select.0");
        click(&mut app, &ctx, "pose.zoom.actual");
        let actual = ctx.data_mut(|d| d.get_temp::<ViewerState>(id)).unwrap();
        assert!(!actual.fit);
        assert_eq!(actual.zoom, 1.0);
        click(&mut app, &ctx, "pose.zoom.in");
        assert!(
            ctx.data_mut(|d| d.get_temp::<ViewerState>(id))
                .unwrap()
                .zoom
                > 1.0
        );
        click(&mut app, &ctx, "pose.zoom.fit");
        assert!(ctx.data_mut(|d| d.get_temp::<ViewerState>(id)).unwrap().fit);
        click(&mut app, &ctx, "pose.view_options");
        for control in [
            "pose.background.checker",
            "pose.background.light",
            "pose.background.dark",
            "pose.overlay.crops",
            "pose.overlay.labels",
            "pose.overlay.anchor",
            "pose.options.fit",
            "pose.options.actual",
        ] {
            assert!(
                app.automation.elements.iter().any(|e| e.id == control),
                "missing view option {control}"
            );
        }
        click(&mut app, &ctx, "pose.background.light");
        assert!(
            ctx.data_mut(|d| d.get_temp::<ViewerState>(id))
                .unwrap()
                .background
                == Background::Light
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
