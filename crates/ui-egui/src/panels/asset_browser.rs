//! Reference browsing and explicit part visibility/review controls.
use super::chrome;
use crate::{AnimationCraftApp, CenterTab, UiCommand};
use egui::{Color32, FontId, Pos2, Rect, RichText, Ui, Vec2};

fn readable_name(name: &str) -> String {
    match name {
        "quarter_front" => "Front ¾".into(),
        "opposite_quarter" => "Back ¾".into(),
        _ => {
            let text = name.replace(['_', '-'], " ");
            let mut chars = text.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => "Unnamed".into(),
            }
        }
    }
}

fn view_angle(label: &str) -> Option<f32> {
    match label {
        "front" | "00_front" | "0° Front" => Some(0.0),
        "quarter_front" | "45°" | "45° 3/4" => Some(45.0),
        "side" | "90°" | "90° Side" => Some(90.0),
        "opposite_quarter" | "135°" | "135° Back-3/4" => Some(135.0),
        "back" | "180°" | "180° Back" => Some(180.0),
        _ => None,
    }
}

fn image_fit(ui: &Ui, texture: &egui::TextureHandle, bounds: Rect) {
    let size = texture.size_vec2();
    let scale = (bounds.width() / size.x.max(1.0)).min(bounds.height() / size.y.max(1.0));
    ui.painter().image(
        texture.id(),
        Rect::from_center_size(bounds.center(), size * scale),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn section(ui: &mut Ui, title: &str, count: usize) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(14.0).strong().color(chrome::TEXT));
        chrome::badge(ui, &count.to_string(), chrome::BLUE);
    });
}

pub(super) fn turnaround(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let frames = app
        .session()
        .current_asset()
        .and_then(|a| a.turnaround_manifest.as_ref())
        .map(|m| m.frames.clone())
        .unwrap_or_default();
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "Turnaround views", frames.len());
        ui.add(egui::Label::new(RichText::new("Choose a view to inspect in 3D.").small().color(chrome::MUTED)).wrap());
        if frames.is_empty() {
            ui.label("Attach and slice a turnaround sheet to see its views here.");
            return;
        }
        let columns = ((ui.available_width() / 82.0).floor() as usize).clamp(1, 3);
        let width = (ui.available_width() - ui.spacing().item_spacing.x * (columns - 1) as f32) / columns as f32;
        let ctx = ui.ctx().clone();
        for row in frames.chunks(columns) {
            ui.horizontal(|ui| {
                for frame in row {
                    ui.push_id(frame.index, |ui| {
                        let angle = view_angle(&frame.label);
                        let selected = angle.is_some_and(|a| ((app.ui_state.yaw_deg - a + 180.0).rem_euclid(360.0) - 180.0).abs() < 15.0);
                        let name = readable_name(&frame.label);
                        let response = ui.add_sized([width, 112.0], egui::Button::new("").selected(selected).corner_radius(5));
                        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name));
                        let rect = response.rect;
                        let bounds = Rect::from_min_max(rect.min + Vec2::splat(5.0), Pos2::new(rect.right() - 5.0, rect.bottom() - 30.0));
                        if let Some(texture) = app.load_texture(&ctx, &frame.resolved_path.to_string_lossy()) {
                            image_fit(ui, &texture, bounds);
                        } else {
                            ui.painter().text(bounds.center(), egui::Align2::CENTER_CENTER, "No image", FontId::proportional(10.0), chrome::MUTED);
                        }
                        let galley = ui.painter().layout(name.clone(), FontId::proportional(11.0), chrome::TEXT, width - 8.0);
                        ui.painter_at(rect).galley(Pos2::new(rect.center().x - galley.size().x * 0.5, rect.bottom() - 26.0), galley, chrome::TEXT);
                        app.automation.add_response(format!("turnaround.view.{}", frame.index), "button", &name, &response);
                        let response = chrome::help(response, &format!("{} reference image.", frame.label),
                            "Compare the character silhouette from different directions.",
                            "Click to open the corresponding 3D angle. An unrecognized direction opens the reference sheet instead.");
                        if response.clicked() {
                            app.ui_state.auto_turntable = false;
                            if let Some(angle) = angle {
                                app.ui_state.center_tab = CenterTab::ThreeD;
                                app.ui_state.yaw_deg = angle;
                            } else { app.ui_state.center_tab = CenterTab::TurnaroundSheet; }
                        }
                    });
                }
            });
        }
    });
    ui.add_space(8.0);
}


pub(super) fn poses(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let frames = app
        .session()
        .current_asset()
        .and_then(|a| a.pose_manifest.as_ref())
        .map(|m| m.frames.clone())
        .unwrap_or_default();
    let lifts: std::collections::HashMap<String, f32> = app
        .session()
        .current_asset()
        .and_then(|a| a.motion_range.as_ref())
        .map(|m| {
            m.poses
                .keys()
                .filter_map(|label| {
                    m.raw["poses"][label.as_str()]["metrics"]["footLiftPx"]
                        .as_f64()
                        .map(|v| (label.clone(), v as f32))
                })
                .collect()
        })
        .unwrap_or_default();
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "Pose references", frames.len());
        ui.add(egui::Label::new(RichText::new("Select a pose to inspect its source frame.").small().color(chrome::MUTED)).wrap());
        if frames.is_empty() { ui.label("Attach and slice a pose sheet to see its frames here."); return; }
        ui.scope(|ui| {
            let ctx = ui.ctx().clone();
            for (index, frame) in frames.iter().enumerate() {
                ui.push_id(index, |ui| {
                    let name = readable_name(&frame.label);
                    let selected = app.ui_state.selected_pose_idx == index;
                    let response = ui.add_sized([ui.available_width(), 54.0], egui::Button::new("").selected(selected).corner_radius(5));
                    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name));
                    let rect = response.rect;
                    let image_rect = Rect::from_min_size(rect.min + Vec2::splat(5.0), Vec2::new(34.0, 44.0));
                    if let Some(texture) = app.load_texture(&ctx, &frame.resolved_path.to_string_lossy()) { image_fit(ui, &texture, image_rect); }
                    let painter = ui.painter_at(rect.shrink(4.0));
                    let x = rect.left() + 47.0;
                    painter.text(Pos2::new(x, rect.top() + 10.0), egui::Align2::LEFT_TOP, &name, FontId::proportional(12.0), chrome::TEXT);
                    let lift = lifts.get(&frame.label).filter(|v| v.is_finite()).map(|v| format!("Foot lift {v:.0} px"))
                        .unwrap_or_else(|| "Lift not measured".into());
                    painter.text(Pos2::new(x, rect.top() + 30.0), egui::Align2::LEFT_TOP, &lift, FontId::proportional(10.0), chrome::MUTED);
                    app.automation.add_response(format!("pose.select.{index}"), "button", &name, &response);
                    let response = chrome::help(response, &format!("{} pose reference. {lift}.", frame.label),
                        "Inspect the actual source artwork before choosing an animation.", "Click to open this frame in Parts and pause playback. Recorded lift is measured from the sheet, not from the animated rig.");
                    if response.clicked() { super::center::pose_viewer::select_pose(app, ui.ctx(), index); }
                });
            }
        });
    });
    ui.add_space(8.0);
}

pub(super) fn parts(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let parts = app
        .session()
        .current_asset()
        .map(|a| a.parts.clone())
        .unwrap_or_default();
    let reviewed = parts.iter().filter(|p| p.verified).count();
    chrome::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "Parts", parts.len());
        chrome::badge(ui, &format!("{reviewed} / {} reviewed", parts.len()), chrome::TEAL)
            .on_hover_text("Manual review marks. These are not automatic validation results.");
        ui.label(RichText::new("Preview separation").small().color(chrome::MUTED));
        ui.spacing_mut().slider_width = (ui.available_width() - 64.0).max(32.0);
        let response = chrome::help(ui.add(egui::Slider::new(&mut app.ui_state.explode_pct, 0.0..=100.0).suffix("%")),
            "Spreads supported layered previews apart.", "Inspect overlap and see which artwork belongs to each part.", "Start at 0%, increase to separate layers, then return to 0% to reassemble the preview. It does not change the source artwork.");
        app.automation.add_response("parts.separation", "slider", "Preview separation", &response);
        ui.horizontal_wrapped(|ui| {
            let response = ui.checkbox(&mut app.ui_state.show_pivot_markers, "Joint anchors").on_hover_text("Display joint markers in supported preview views.");
            app.automation.add_response("parts.anchors", "checkbox", "Joint anchors", &response);
            let response = ui.checkbox(&mut app.ui_state.show_topology_numbers, "Part labels").on_hover_text("Display part names or numbers in supported views.");
            app.automation.add_response("parts.labels", "checkbox", "Part labels", &response);
        });
        ui.add_space(4.0);
        ui.add_enabled_ui(!parts.is_empty() && app.ui_state.busy_command.is_none(), |ui| {
            ui.horizontal_wrapped(|ui| {
                let response = chrome::button(ui, "Show all", false).on_hover_text("Make all parts visible and clear every Solo selection.");
                app.automation.add_response("parts.show_all", "button", "Show all parts", &response);
                if response.clicked() {
                    app.exec_ui_command(UiCommand::Engine { id: "part.show_all", params: serde_json::json!({}) });
                }
                let response = chrome::help(chrome::button(ui, "Mark all reviewed", false),
                    "Marks every part as manually reviewed.", "Record that you have inspected the cutouts and their joints.", "Click only after inspecting all parts. This does not run validation; individual review marks can be cleared below.");
                app.automation.add_response("btn.verify_all_parts", "button", "Mark all reviewed", &response);
                if response.clicked() {
                    app.exec_ui_command(UiCommand::Engine { id: "part.set_state", params: serde_json::json!({"part": "*", "verified": true}) });
                }
            });
        });
        ui.add_space(4.0);
        if parts.is_empty() { ui.label("Build a rig or load a manifest to inspect its parts."); return; }
        ui.scope(|ui| {
            for (index, part) in parts.iter().enumerate() {
                ui.push_id(&part.name, |ui| {
                    egui::Frame::new().fill(Color32::from_rgb(20, 25, 33)).corner_radius(4).inner_margin(7).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{:02}", index + 1)).small().color(chrome::MUTED));
                            ui.add(egui::Label::new(RichText::new(readable_name(&part.name)).strong().color(chrome::TEXT)).wrap())
                                .on_hover_text(format!("Part ID: {}", part.name));
                        });
                        ui.add_enabled_ui(app.ui_state.busy_command.is_none(), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for (kind, label, current, what, why, how) in [
                                    ("visible", "Visible", part.visible, "Show or hide this part in supported previews.", "Inspect what is behind it or temporarily remove it from view.", "Uncheck to hide. Show all restores visibility; Solo selections can override which parts are shown."),
                                    ("solo", "Solo", part.solo, "Isolate this part together with any other soloed parts.", "Inspect a layer without the rest of the character obscuring it.", "Check Solo on the parts you want to see. Show all clears all Solo selections."),
                                    ("reviewed", "Reviewed", part.verified, "Record your manual review of this part.", "Track which cutouts and joints you have inspected.", "Check after inspecting the part. This is a review mark, not an automatic quality check."),
                                ] {
                                    let mut value = current;
                                    let response = chrome::help(ui.checkbox(&mut value, label), what, why, how);
                                    app.automation.add_response(format!("part.{kind}.{}", part.name), "checkbox", format!("{}: {label}", part.name), &response);
                                    if response.changed() {
                                        app.exec_ui_command(UiCommand::Engine {
                                            id: "part.set_state",
                                            params: serde_json::json!({
                                                "part": part.name,
                                                "visible": (kind == "visible").then_some(value),
                                                "solo": (kind == "solo").then_some(value),
                                                "verified": (kind == "reviewed").then_some(value),
                                            }),
                                        });
                                    }
                                }
                            });
                        });
                    });
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_view_does_not_silently_select_front() {
        assert_eq!(view_angle("quarter_front"), Some(45.0));
        assert_eq!(view_angle("custom_profile"), None);
    }

    #[test]
    fn pose_inspection_preserves_index_and_pauses_without_running_pipeline() {
        let mut app = AnimationCraftApp::new();
        app.session.assets.clear();
        let previous = app.session.last_result.clone();
        super::super::center::pose_viewer::select_pose(&mut app, &egui::Context::default(), 7);
        assert_eq!(app.ui_state.selected_pose_idx, 7);
        assert!(!app.ui_state.playing);
        assert_eq!(app.session.last_result, previous);
    }
}
