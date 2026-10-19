//! Shared surfaces and controls for the studio's navigation and source artwork.
use egui::{Color32, Response, RichText, Stroke, Ui};

pub(super) const SURFACE: Color32 = Color32::from_rgb(25, 30, 39);
pub(super) const BORDER: Color32 = Color32::from_rgb(49, 59, 74);
pub(super) const TEXT: Color32 = Color32::from_rgb(235, 240, 248);
pub(super) const MUTED: Color32 = Color32::from_rgb(164, 175, 191);
pub(super) const BLUE: Color32 = Color32::from_rgb(112, 196, 255);
pub(super) const TEAL: Color32 = Color32::from_rgb(100, 215, 194);

pub(super) fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(7)
        .inner_margin(10)
}

pub(super) fn button(ui: &mut Ui, label: &str, primary: bool) -> Response {
    ui.scope(|ui| {
        let visuals = &mut ui.style_mut().visuals;
        let base = if primary {
            Color32::from_rgb(29, 83, 83)
        } else {
            Color32::from_rgb(37, 46, 60)
        };
        let hover = if primary {
            Color32::from_rgb(38, 111, 108)
        } else {
            Color32::from_rgb(51, 66, 85)
        };
        visuals.widgets.inactive.bg_fill = base;
        visuals.widgets.inactive.weak_bg_fill = base;
        visuals.widgets.inactive.bg_stroke = Stroke::new(
            1.0,
            if primary {
                TEAL.gamma_multiply(0.65)
            } else {
                BORDER
            },
        );
        visuals.widgets.hovered.bg_fill = hover;
        visuals.widgets.hovered.weak_bg_fill = hover;
        ui.add(
            egui::Button::new(RichText::new(label).size(12.0).strong().color(TEXT))
                .min_size(egui::vec2(0.0, 30.0))
                .wrap(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

pub(super) fn tab(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let response = ui.add(
        egui::Button::selectable(
            selected,
            RichText::new(label)
                .size(12.0)
                .color(if selected { TEXT } else { MUTED }),
        )
        .min_size(egui::vec2(0.0, 32.0)),
    );
    if selected {
        ui.painter().line_segment(
            [
                response.rect.left_bottom() + egui::vec2(4.0, -1.0),
                response.rect.right_bottom() + egui::vec2(-4.0, -1.0),
            ],
            Stroke::new(2.0, BLUE),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn badge(ui: &mut Ui, label: &str, color: Color32) -> Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(10.0).color(color))
        })
        .inner
}

pub(super) fn help(response: Response, what: &str, why: &str, how: &str) -> Response {
    response.on_hover_ui(|ui| {
        ui.set_max_width(290.0);
        for (heading, body) in [("What", what), ("Why", why), ("How", how)] {
            ui.label(RichText::new(heading).strong());
            ui.add(egui::Label::new(body).wrap());
            ui.add_space(4.0);
        }
    })
}

/// Compact, keyboard-editable property row. Native DragValue supports drag and direct entry.
pub(super) fn number_row<T: egui::emath::Numeric>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    suffix: &str,
    accent: Color32,
    explanation: (&str, &str, &str),
) -> Response {
    ui.push_id(label, |ui| {
        ui.horizontal(|ui| {
            let width = ui.available_width();
            let number_width = (width * 0.4).clamp(44.0, 72.0);
            let label_width = (width - number_width - ui.spacing().item_spacing.x).max(1.0);
            help(
                ui.add_sized(
                    [label_width, 28.0],
                    egui::Label::new(RichText::new(label).size(12.0).color(MUTED)).wrap(),
                ),
                explanation.0,
                explanation.1,
                explanation.2,
            );
            let response = ui
                .scope(|ui| {
                    ui.style_mut().visuals.override_text_color = Some(accent);
                    ui.add_sized(
                        [number_width, 28.0],
                        egui::DragValue::new(value)
                            .range(range)
                            .speed(0.5)
                            .max_decimals(1)
                            .suffix(suffix),
                    )
                })
                .inner;
            help(
                response,
                explanation.0,
                explanation.1,
                &format!(
                    "Drag sideways or click the value to type. {}",
                    explanation.2
                ),
            )
        })
        .inner
    })
    .inner
}

/// Visible range control with a quiet readout, preserving native slider interaction.
pub(super) fn slider_row(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
    accent: Color32,
    explanation: (&str, &str, &str),
) -> Response {
    ui.push_id(label, |ui| {
        ui.horizontal(|ui| {
            help(
                ui.label(RichText::new(label).size(12.0).color(MUTED)),
                explanation.0,
                explanation.1,
                explanation.2,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{:.1}{}", *value, suffix))
                        .monospace()
                        .size(11.0)
                        .color(accent),
                );
            });
        });
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = ui.available_width();
                ui.spacing_mut().interact_size.y = 18.0;
                ui.visuals_mut().selection.bg_fill = accent.gamma_multiply(0.65);
                ui.visuals_mut().widgets.inactive.bg_fill = BORDER;
                ui.visuals_mut().widgets.inactive.fg_stroke = Stroke::new(2.0, accent);
                ui.visuals_mut().widgets.hovered.bg_fill = accent;
                ui.visuals_mut().widgets.active.bg_fill = accent;
                ui.add(
                    egui::Slider::new(value, range)
                        .show_value(false)
                        .trailing_fill(true)
                        .handle_shape(egui::style::HandleShape::Circle),
                )
            })
            .inner;
        ui.add_space(4.0);
        help(response, explanation.0, explanation.1, explanation.2)
    })
    .inner
}
