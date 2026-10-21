//! 3D turnaround software preview and 5-pose lineup panels.

use super::center_motion::sanitize_dt;
use crate::{AnimationCraftApp, ShadingMode};
use animationcraft_engine::{Asset, ManifestFrame};
use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};

pub(super) fn select_turnaround_frame_for_yaw(
    asset: Option<&Asset>,
    yaw_deg: f32,
) -> Option<ManifestFrame> {
    let asset = asset?;
    let frames = if let Some(reg) = &asset.turnaround_registration
        && !reg.frames.is_empty()
    {
        &reg.frames
    } else if let Some(tm) = &asset.turnaround_manifest {
        &tm.frames
    } else {
        return None;
    };

    let norm = yaw_deg.rem_euclid(360.0);
    let desired_label = if !(22.5..337.5).contains(&norm) {
        "front"
    } else if (22.5..67.5).contains(&norm) || (292.5..337.5).contains(&norm) {
        "quarter_front"
    } else if (67.5..112.5).contains(&norm) || (247.5..292.5).contains(&norm) {
        "side"
    } else if (112.5..157.5).contains(&norm) || (202.5..247.5).contains(&norm) {
        "opposite_quarter"
    } else {
        "back"
    };

    frames
        .iter()
        .find(|f| f.label == desired_label)
        .or_else(|| frames.first())
        .cloned()
}

#[derive(Clone)]
struct ShapePreviewCache {
    sources: String,
    settings: animationcraft_engine::mesh_geometry::ShapeSettings,
    views: std::sync::Arc<Vec<(String, image::RgbaImage)>>,
    mesh: std::sync::Arc<animationcraft_engine::mesh_geometry::MeshGeometry>,
}

fn shape_preview(
    app: &AnimationCraftApp,
    ctx: &egui::Context,
) -> Option<std::sync::Arc<animationcraft_engine::mesh_geometry::MeshGeometry>> {
    use animationcraft_engine::mesh_geometry::{self, ShapeSettings};
    let asset = app.session().current_asset()?;
    let mut paths = vec![asset.path.clone()];
    for manifest in [&asset.turnaround_manifest, &asset.pose_manifest]
        .into_iter()
        .flatten()
    {
        paths.push(manifest.manifest_path.clone());
        paths.extend(manifest.frames.iter().map(|f| f.resolved_path.clone()));
    }
    let sources = format!(
        "{:?}:{:?}:{:?}",
        asset
            .turnaround_registration
            .as_ref()
            .map(|r| r.target_height_px),
        paths
            .iter()
            .map(|p| (
                p,
                std::fs::metadata(p)
                    .ok()
                    .map(|m| (m.len(), m.modified().ok()))
            ))
            .collect::<Vec<_>>(),
        [&asset.turnaround_manifest, &asset.pose_manifest]
            .into_iter()
            .flatten()
            .flat_map(|m| m.frames.iter().map(|f| &f.label))
            .collect::<Vec<_>>()
    );
    let settings = ShapeSettings {
        inflation_depth_pct: app.ui_state.inflation_depth_pct,
        relief_detail_pct: app.ui_state.relief_detail_pct,
        mesh_res: app.ui_state.mesh_res,
        proj_blend_pct: app.ui_state.proj_blend_pct as u32,
        bump_intensity: app.ui_state.bump_strength as u32,
    };
    let id = egui::Id::unique("shared-shape-preview");
    let old = ctx.data_mut(|d| d.get_temp::<ShapePreviewCache>(id));
    if let Some(cache) = &old
        && cache.sources == sources
        && cache.settings == settings
    {
        return Some(cache.mesh.clone());
    }
    let views = old
        .filter(|cache| cache.sources == sources)
        .map(|cache| cache.views)
        .unwrap_or_else(|| std::sync::Arc::new(mesh_geometry::load_views(asset)));
    if views.is_empty() {
        return None;
    }
    let mesh = std::sync::Arc::new(mesh_geometry::build(&views, settings));
    ctx.data_mut(|d| {
        d.insert_temp(
            id,
            ShapePreviewCache {
                sources,
                settings,
                views,
                mesh: mesh.clone(),
            },
        )
    });
    Some(mesh)
}

pub(super) fn three_d_turnaround(app: &mut AnimationCraftApp, ui: &mut Ui) {
    if app.ui_state.auto_turntable {
        let dt = sanitize_dt(ui.ctx().input(|i| i.stable_dt));
        app.ui_state.yaw_deg = (app.ui_state.yaw_deg + dt * 36.0).rem_euclid(360.0);
        ui.ctx().request_repaint();
    }

    ui.horizontal_wrapped(|ui| {
        ui.strong("Turnaround Yaw:");
        let yaw = app.ui_state.yaw_deg;
        ui.add(
            egui::Slider::new(&mut app.ui_state.yaw_deg, 0.0..=360.0).text(format!("{yaw:.0}°")),
        );
        for (label, deg) in [
            ("POSE 1 • 0° Front", 0.0),
            ("POSE 2 • 45° 3/4", 45.0),
            ("POSE 3 • 90° Side", 90.0),
            ("POSE 4 • 135° Back-3/4", 135.0),
            ("POSE 5 • 180° Back", 180.0),
        ] {
            if ui.button(label).clicked() {
                app.ui_state.auto_turntable = false;
                app.ui_state.yaw_deg = deg;
            }
        }
        if ui
            .selectable_label(app.ui_state.auto_turntable, "⟳ Auto-Turntable")
            .clicked()
        {
            app.ui_state.auto_turntable = !app.ui_state.auto_turntable;
        }
    });

    ui.add_space(4.0);

    let frame_opt =
        select_turnaround_frame_for_yaw(app.session().current_asset(), app.ui_state.yaw_deg);
    let drift_label = app
        .session()
        .current_asset()
        .and_then(|a| a.turnaround_registration.as_ref())
        .map(|r| {
            format!(
                "Drift: {:.1} px (before crown {:.0}px / foot {:.0}px)",
                r.max_vertical_drift_px, r.before_crown_drift_px, r.before_foot_drift_px
            )
        })
        .unwrap_or_else(|| "Drift: not measured".to_string());

    let ctx = ui.ctx().clone();
    let reference_path = frame_opt
        .as_ref()
        .map(|f| f.resolved_path.clone())
        .or_else(|| app.session().current_asset().map(|a| a.path.clone()));
    let tex_turnaround = reference_path
        .as_ref()
        .and_then(|path| app.load_texture(&ctx, &path.to_string_lossy()));

    let avail = ui.available_size();
    let canvas_size = Vec2::new(avail.x.max(80.0), (avail.y - 8.0).max(220.0));
    let (rect, _resp) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, Color32::from_rgb(18, 22, 30));

    if let Some(tex) = tex_turnaround {
        let ts = tex.size_vec2();
        let max_h = (rect.height() - 80.0).max(140.0);
        let max_w = (rect.width() * 0.52 - 32.0).max(110.0);
        let scale = (max_h / ts.y.max(1.0)).min(max_w / ts.x.max(1.0));
        let draw_size = ts * scale;
        let left_center = Pos2::new(rect.left() + rect.width() * 0.28, rect.center().y + 10.0);
        let img_rect = Rect::from_center_size(left_center, draw_size);
        painter.image(
            tex.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        let hull_center = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y + 10.0);
        let hull_h = draw_size.y.max(150.0);
        let wire_col = match app.ui_state.shading_mode {
            ShadingMode::FeltPbr => Color32::from_rgba_unmultiplied(110, 210, 255, 155),
            ShadingMode::ClaySculpt => Color32::from_rgba_unmultiplied(225, 195, 165, 185),
            ShadingMode::Wireframe => Color32::from_rgba_unmultiplied(80, 235, 195, 220),
            ShadingMode::Normals => Color32::from_rgba_unmultiplied(195, 145, 255, 200),
        };
        if let Some(mesh) = shape_preview(app, &ctx) {
            let yaw = app.ui_state.yaw_deg.to_radians();
            let projected: Vec<_> = mesh
                .positions
                .iter()
                .map(|[x, y, z]| {
                    let horizontal = x * yaw.cos() - z * yaw.sin();
                    let depth = x * yaw.sin() + z * yaw.cos();
                    [horizontal, 0.5 - y + depth * 0.22]
                })
                .collect();
            let extent = projected.iter().map(|p| p[0].abs()).fold(0.01f32, f32::max);
            let scale = hull_h.min((rect.width() * 0.23 - 12.0).max(10.0) / extent);
            let points: Vec<_> = projected
                .iter()
                .map(|p| hull_center + Vec2::new(p[0], p[1]) * scale)
                .collect();
            for face in mesh.indices.chunks_exact(3) {
                for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
                    painter.line_segment(
                        [points[a as usize], points[b as usize]],
                        Stroke::new(0.7, wire_col),
                    );
                }
            }
        }
        painter.text(
            Pos2::new(hull_center.x, hull_center.y + hull_h * 0.5 + 16.0),
            egui::Align2::CENTER_TOP,
            format!(
                "3D Visual Hull ({}) · Depth {}% · Res {}",
                app.ui_state.recon_mode, app.ui_state.inflation_depth_pct, app.ui_state.mesh_res
            ),
            egui::FontId::monospace(10.5),
            Color32::from_rgb(150, 220, 250),
        );
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No turnaround manifest loaded for active asset",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(220, 175, 120),
        );
    }

    let angle_name = frame_opt
        .as_ref()
        .map(|f| f.label.as_str())
        .unwrap_or("none");
    painter.text(
        Pos2::new(rect.left() + 14.0, rect.top() + 14.0),
        egui::Align2::LEFT_TOP,
        format!(
            "TURNAROUND VIEW · Asset: {} · View: {angle_name} ({:.0}°) · {drift_label}",
            app.session().active_asset.as_deref().unwrap_or("none"),
            app.ui_state.yaw_deg
        ),
        egui::FontId::monospace(11.5),
        Color32::from_rgb(130, 215, 250),
    );
}

pub(super) fn five_pose_lineup(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let frames: Vec<ManifestFrame> = app
        .session()
        .current_asset()
        .and_then(|a| {
            a.turnaround_registration
                .as_ref()
                .map(|r| r.frames.clone())
                .or_else(|| a.turnaround_manifest.as_ref().map(|m| m.frames.clone()))
        })
        .unwrap_or_default();

    let avail = ui.available_size();
    let canvas_size = Vec2::new(avail.x.max(80.0), (avail.y - 8.0).max(220.0));
    let (rect, _resp) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, Color32::from_rgb(18, 22, 30));

    if frames.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No 5-view turnaround manifest loaded for active asset",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(220, 175, 120),
        );
        return;
    }

    let ctx = ui.ctx().clone();
    let count = frames.len().max(1) as f32;
    let slot_w = (rect.width() - 24.0) / count;
    let baseline_y = rect.bottom() - 48.0;
    let crown_y = rect.top() + 56.0;

    painter.line_segment(
        [
            Pos2::new(rect.left() + 12.0, crown_y),
            Pos2::new(rect.right() - 12.0, crown_y),
        ],
        Stroke::new(1.5, Color32::from_rgb(255, 202, 72)),
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + 12.0, baseline_y),
            Pos2::new(rect.right() - 12.0, baseline_y),
        ],
        Stroke::new(1.5, Color32::from_rgb(88, 217, 176)),
    );

    for (idx, f) in frames.iter().enumerate() {
        let p = f.resolved_path.to_string_lossy().into_owned();
        let x_min = rect.left() + 12.0 + (idx as f32) * slot_w;
        let slot_rect = Rect::from_min_max(
            Pos2::new(x_min, crown_y),
            Pos2::new(x_min + slot_w, baseline_y),
        );
        if let Some(tex) = app.load_texture(&ctx, &p) {
            let ts = tex.size_vec2();
            let scale = (slot_rect.height() / ts.y.max(1.0)).min(slot_rect.width() / ts.x.max(1.0));
            let draw_size = ts * scale;
            let img_rect = Rect::from_center_size(slot_rect.center(), draw_size);
            painter.image(
                tex.id(),
                img_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        painter.text(
            Pos2::new(slot_rect.center().x, baseline_y + 8.0),
            egui::Align2::CENTER_TOP,
            &f.label,
            egui::FontId::monospace(11.0),
            Color32::from_rgb(210, 225, 245),
        );
    }
}
