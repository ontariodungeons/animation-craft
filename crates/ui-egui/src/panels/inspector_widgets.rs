//! Read-only metric cards and explicit action cards for narrow inspectors.
use egui::{Color32, FontId, RichText, Sense, Stroke, Ui, Vec2};

use super::help;

const CARD: Color32 = Color32::from_rgb(25, 30, 39);
const BORDER: Color32 = Color32::from_rgb(49, 59, 74);
const MUTED: Color32 = Color32::from_rgb(164, 175, 191);
const TEXT: Color32 = Color32::from_rgb(235, 240, 248);
const TRACK: Color32 = Color32::from_rgb(51, 62, 79);

fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(7)
        .inner_margin(10)
}

pub(super) fn metric_card(
    ui: &mut Ui,
    title: &str,
    value: &str,
    unit: &str,
    caption: &str,
    samples: &[(&str, f32)],
    accent: Color32,
    explanation: (&str, &str, &str),
) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        super::help(
            ui.label(RichText::new(title).size(12.0).color(MUTED)),
            explanation.0,
            explanation.1,
            explanation.2,
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new(value).size(25.0).strong().color(TEXT));
            ui.label(RichText::new(unit).size(12.0).color(MUTED));
        });
        ui.add(egui::Label::new(RichText::new(caption).size(11.0).color(MUTED)).wrap());
        if !samples.is_empty() {
            ui.add_space(4.0);
            pose_chart(ui, title, samples, accent);
        }
    });
    ui.add_space(3.0);
}

fn chart_domain(samples: &[(&str, f32)]) -> (f32, f32) {
    let (lo, hi) = samples
        .iter()
        .filter(|(_, value)| value.is_finite())
        .fold((0.0_f32, 0.0_f32), |(lo, hi), (_, value)| {
            (lo.min(*value), hi.max(*value))
        });
    (lo, if hi > lo { hi } else { lo + 1.0 })
}

fn pose_chart(ui: &mut Ui, id: &str, samples: &[(&str, f32)], accent: Color32) {
    let samples: Vec<_> = samples
        .iter()
        .copied()
        .filter(|(_, value)| value.is_finite())
        .collect();
    if samples.is_empty() {
        return;
    }
    let (lo, hi) = chart_domain(&samples);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::hover());
    let chart = rect.shrink2(Vec2::new(0.0, 3.0));
    let y = |value: f32| chart.bottom() - (value - lo) / (hi - lo) * chart.height();
    let zero = y(0.0);
    let step = chart.width() / samples.len() as f32;
    for (index, (label, value)) in samples.iter().enumerate() {
        let x = chart.left() + index as f32 * step;
        let hit = egui::Rect::from_min_max(
            egui::pos2(x, rect.top()),
            egui::pos2(x + step, rect.bottom()),
        );
        let response = ui.interact(hit, ui.id().with((id, index)), Sense::hover());
        let bar = egui::Rect::from_min_max(
            egui::pos2(x + 1.0, y(*value).min(zero)),
            egui::pos2((x + step - 1.0).max(x + 1.5), y(*value).max(zero) + 1.0),
        );
        ui.painter()
            .rect_filled(bar, 1.0, if response.hovered() { TEXT } else { accent });
        response.on_hover_text(format!(
            "{label}\n{value:.1} px recorded\nEach bar is a pose reference, not a frame in time."
        ));
    }
    ui.painter().line_segment(
        [
            egui::pos2(chart.left(), zero),
            egui::pos2(chart.right(), zero),
        ],
        Stroke::new(1.0, TRACK),
    );
    ui.label(
        RichText::new("Recorded poses · hover a bar")
            .size(10.0)
            .color(MUTED),
    );
}

pub(super) fn action_card(
    ui: &mut Ui,
    title: &str,
    description: &str,
    detail: &str,
    button: &str,
    enabled: bool,
    explanation: (&str, &str, &str),
) -> egui::Response {
    card()
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).size(14.0).strong().color(TEXT));
            ui.add(egui::Label::new(RichText::new(description).size(12.0).color(MUTED)).wrap());
            ui.add(egui::Label::new(RichText::new(detail).size(10.0).color(super::MESH)).wrap());
            ui.add_space(4.0);
            // Native buttons preserve focus/keyboard behavior; only this explicit CTA is clickable.
            let response = ui
                .scope(|ui| {
                    let visuals = &mut ui.style_mut().visuals;
                    visuals.widgets.inactive.bg_fill = Color32::from_rgb(29, 83, 83);
                    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(29, 83, 83);
                    visuals.widgets.inactive.bg_stroke =
                        Stroke::new(1.0, Color32::from_rgb(71, 146, 142));
                    visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 111, 108);
                    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(38, 111, 108);
                    visuals.widgets.active.bg_fill = Color32::from_rgb(23, 66, 67);
                    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(23, 66, 67);
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.add_sized(
                            [ui.available_width(), 34.0],
                            egui::Button::new(
                                RichText::new(button)
                                    .font(FontId::proportional(13.0))
                                    .strong()
                                    .color(TEXT),
                            )
                            .wrap(),
                        )
                    })
                    .inner
                })
                .inner;
            let response = if enabled {
                response.on_hover_cursor(egui::CursorIcon::PointingHand)
            } else {
                response
            };
            help(response, explanation.0, explanation.1, explanation.2).on_disabled_hover_text(
                "Wait for the current task to finish before starting another export.",
            )
        })
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_domain_handles_zero_negative_and_unavailable_samples() {
        assert_eq!(chart_domain(&[("zero", 0.0)]), (0.0, 1.0));
        assert_eq!(
            chart_domain(&[("left", -39.0), ("right", 40.0)]),
            (-39.0, 40.0)
        );
        assert_eq!(
            chart_domain(&[("missing", f32::NAN), ("lift", 20.0)]),
            (0.0, 20.0)
        );
    }

    #[test]
    fn action_card_only_activates_its_button() {
        let ctx = egui::Context::default();
        let mut button_rect = egui::Rect::NOTHING;
        let mut clicked = false;
        let mut draw = |events, enabled| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(232.0, 300.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let response = action_card(
                        ui,
                        "3D mesh",
                        "Build a volume from your artwork.",
                        "GLB + OBJ",
                        "Build mesh",
                        enabled,
                        (
                            "Builds a mesh.",
                            "Use it in a 3D editor.",
                            "Click Build mesh.",
                        ),
                    );
                    button_rect = response.rect;
                    clicked = response.clicked();
                },
            );
            output.textures_delta.clear();
            (button_rect, clicked)
        };
        let (button, _) = draw(vec![], true);
        assert!(button.right() <= 232.0, "button must fit a narrow panel");
        let pointer = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let caption = egui::pos2(30.0, 20.0);
        draw(pointer(caption, true), true);
        assert!(
            !draw(pointer(caption, false), true).1,
            "card text must not start an export"
        );
        draw(pointer(button.center(), true), true);
        assert!(draw(pointer(button.center(), false), true).1);
        draw(pointer(button.center(), true), false);
        assert!(
            !draw(pointer(button.center(), false), false).1,
            "busy controls must not start another export"
        );
    }
}
