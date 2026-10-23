//! The Magic Studio title card.
//!
//! Drawn with `bit` (Terminal ANSI Logo Designer) in the `dogica` face, then baked in as
//! plain glyphs so the app carries no runtime dependency and no escape codes. The colour
//! ramp is the same teal-to-gold gradient `bit` renders, interpolated per column.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

/// The studio mark: four rows of glyphs, straight from `bit -font dogica`.
pub const TITLE_CARD: [&str; 4] = [
    "█    █  ▄▀▀▀▄  ▄▀▀▀▀  ▀█▀  ▄▀▀▀▄     ▄▀▀▀▄ ▀▀█▀▀  █   █  ▀█▀▀▀▄  ▀█▀  ▄▀▀▀▄",
    "█▀▄▄▀█  █   █  █  ▄▄   █   █         ▀▄▄▄    █    █   █   █   █   █   █   █",
    "█    █  █▀▀▀█  █   █   █   █   ▄     ▄   █   █    █   █   █   █   █   █   █",
    "▀    ▀  ▀   ▀   ▀▀▀▀  ▀▀▀   ▀▀▀       ▀▀▀    ▀     ▀▀▀    ▀▀▀▀   ▀▀▀   ▀▀▀ ",
];

/// Interpolate the studio gradient across `width` columns.
fn ramp(width: usize) -> Vec<Color> {
    let (r0, g0, b0) = (100u8, 215u8, 194u8);
    let (r1, g1, b1) = (240u8, 190u8, 112u8);
    (0..width.max(1))
        .map(|i| {
            let t = if width <= 1 { 0.0 } else { i as f32 / (width - 1) as f32 };
            Color::Rgb(
                (r0 as f32 + (r1 as f32 - r0 as f32) * t).round() as u8,
                (g0 as f32 + (g1 as f32 - g0 as f32) * t).round() as u8,
                (b0 as f32 + (b1 as f32 - b0 as f32) * t).round() as u8,
            )
        })
        .collect()
}

/// The mark as styled lines, coloured with the studio gradient.
pub fn title_card() -> Vec<Line<'static>> {
    let width = TITLE_CARD[0].chars().count();
    let colours = ramp(width);
    TITLE_CARD
        .iter()
        .map(|row| {
            Line::from(
                row.chars()
                    .enumerate()
                    .map(|(index, ch)| Span::styled(ch.to_string(), Style::default().fg(colours[index.min(colours.len() - 1)])))
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

/// The mark, fitted to `width` columns by cropping. Empty when it will not fit.
pub fn title_card_fitted(width: usize) -> Vec<Line<'static>> {
    if width < 40 {
        return Vec::new();
    }
    let mut lines = title_card();
    for line in &mut lines {
        let mut spans = line.spans.clone();
        let mut used = 0usize;
        let mut cut = spans.len();
        for (index, span) in spans.iter().enumerate() {
            used += span.content.chars().count();
            if used > width {
                cut = index;
                break;
            }
        }
        if cut < spans.len() {
            let mut last = spans[cut - 1].clone();
            let keep: String = last.content.chars().take(width.saturating_sub(used - last.content.chars().count())).collect();
            last.content = keep.into();
            spans.truncate(cut - 1);
            spans.push(last);
        }
        *line = Line::from(spans);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_has_four_rows_of_equal_width() {
        assert_eq!(TITLE_CARD.len(), 4);
        let width = TITLE_CARD[0].chars().count();
        for row in TITLE_CARD {
            assert_eq!(row.chars().count(), width);
        }
    }

    #[test]
    fn the_gradient_runs_from_teal_to_gold() {
        let colours = ramp(10);
        assert_eq!(colours.first(), Some(&Color::Rgb(100, 215, 194)));
        assert_eq!(colours.last(), Some(&Color::Rgb(240, 190, 112)));
    }

    #[test]
    fn a_narrow_booth_gets_no_card() {
        assert!(title_card_fitted(20).is_empty());
    }
}
