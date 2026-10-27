//! Compact title styling, retaining the teal-to-gold ramp from the original `bit` mark.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

/// Interpolate the studio gradient across `width` columns.
fn ramp(width: usize) -> Vec<Color> {
    let (r0, g0, b0) = (100u8, 215u8, 194u8);
    let (r1, g1, b1) = (240u8, 190u8, 112u8);
    (0..width.max(1))
        .map(|i| {
            let t = if width <= 1 {
                0.0
            } else {
                i as f32 / (width - 1) as f32
            };
            Color::Rgb(
                (r0 as f32 + (r1 as f32 - r0 as f32) * t).round() as u8,
                (g0 as f32 + (g1 as f32 - g0 as f32) * t).round() as u8,
                (b0 as f32 + (b1 as f32 - b0 as f32) * t).round() as u8,
            )
        })
        .collect()
}

pub fn compact_title(text: &str) -> Line<'static> {
    let colours = ramp(text.chars().count());
    Line::from(
        text.chars()
            .zip(colours)
            .map(|(ch, colour)| {
                Span::styled(
                    ch.to_string(),
                    Style::default()
                        .fg(colour)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                )
            })
            .collect::<Vec<_>>(),
    )
}
