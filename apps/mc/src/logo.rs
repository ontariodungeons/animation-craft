//! The Magic Studio title card.
//!
//! Drawn with `bit` (Terminal ANSI Logo Designer) in the `dogica` face, then baked in as
//! plain glyphs so the app carries no runtime dependency and no escape codes.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

/// The word "MAGIC" ends at this column; "STUDIO" follows.
const SPLIT: usize = 32;

/// The studio mark: four rows of glyphs.
pub const TITLE_CARD: [&str; 4] = [
    "█    █  ▄▀▀▀▄  ▄▀▀▀▀  ▀█▀  ▄▀▀▀▄     ▄▀▀▀▄ ▀▀█▀▀  █   █  ▀█▀▀▀▄  ▀█▀  ▄▀▀▀▄",
    "█▀▄▄▀█  █   █  █  ▄▄   █   █         ▀▄▄▄    █    █   █   █   █   █   █   █",
    "█    █  █▀▀▀█  █   █   █   █   ▄     ▄   █   █    █   █   █   █   █   █   █",
    "▀    ▀  ▀   ▀   ▀▀▀▀  ▀▀▀   ▀▀▀       ▀▀▀    ▀     ▀▀▀    ▀▀▀▀   ▀▀▀   ▀▀▀ ",
];

/// The mark as styled lines: "MAGIC" in the studio teal, "STUDIO" in the reel gold.
pub fn title_card() -> Vec<Line<'static>> {
    TITLE_CARD
        .iter()
        .map(|row| {
            let (left, right) = row
                .char_indices()
                .nth(SPLIT)
                .map(|(at, _)| row.split_at(at))
                .unwrap_or((row, ""));
            Line::from(vec![
                Span::styled(left.to_string(), Style::default().fg(Color::Rgb(100, 215, 194))),
                Span::styled(right.to_string(), Style::default().fg(Color::Rgb(240, 190, 112))),
            ])
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
    fn a_narrow_booth_gets_no_card() {
        assert!(title_card_fitted(20).is_empty());
    }
}
