//! The one place the terminal look is defined: palette, panel frames and text helpers.
//! Screens take colours and frames from here instead of writing their own, so a change
//! to the look is one edit. Colours follow the Hackerman Omarchy theme
//! (~/.local/state/omarchy/current/theme/{colors.toml,foot.ini,btop.theme}); the theme
//! remaps red/yellow to green/teal, so semantic red and amber are kept distinct below.
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};

/// Accent: headings, labels that matter, the selected tab.
pub const TEAL: Color = Color::Rgb(124, 248, 247);
pub const BLUE: Color = Color::Rgb(130, 157, 212);
/// Semantic state: in use / needs you / problem.
pub const GREEN: Color = Color::Rgb(79, 232, 143);
pub const AMBER: Color = Color::Rgb(246, 193, 104);
pub const RED: Color = Color::Rgb(255, 118, 130);
/// Secondary text and the quietest text.
pub const MUTED: Color = Color::Rgb(181, 197, 219);
pub const DIM: Color = Color::Rgb(120, 132, 155);
pub const FILM: Color = Color::Rgb(92, 74, 42);
/// The desk: background, body text, hairlines, selected row.
pub const DESK_BG: Color = Color::Rgb(11, 12, 22);
pub const DESK_FG: Color = Color::Rgb(221, 247, 255);
pub const DESK_LINE: Color = Color::Rgb(45, 52, 80);
pub const DESK_SELECTED: Color = Color::Rgb(31, 37, 58);

/// One line in one colour.
pub fn styled(value: impl Into<String>, color: Color) -> Line<'static> {
    Line::styled(value.into(), Style::default().fg(color))
}

/// A rounded frame whose title and border share an accent.
pub fn round_panel(title: impl Into<String>, accent: Color, active: bool) -> Block<'static> {
    let weight = if active {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(accent).add_modifier(weight))
        .title(Span::styled(
            format!(" {} ", title.into()),
            Style::default().fg(accent).add_modifier(weight),
        ))
}

/// A hairline frame with a quiet title, for panels that sit behind the main one.
pub fn quiet_panel(title: impl Into<String>, active: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(DESK_LINE).add_modifier(if active {
            Modifier::BOLD
        } else {
            Modifier::empty()
        }))
        .title(Span::styled(format!(" {} ", title.into()), MUTED))
}

/// `text` cut to `width` cells with an ellipsis.
pub fn fit_text(text: &str, width: usize) -> String {
    if Line::raw(text).width() <= width {
        return text.into();
    }
    if width == 0 {
        return String::new();
    }
    let mut result = String::new();
    for ch in text.chars() {
        if Line::raw(format!("{result}{ch}…")).width() > width {
            break;
        }
        result.push(ch);
    }
    format!("{result}…")
}

/// `text` cut or padded to exactly `width` cells: one cell of a table row.
pub fn cell(text: &str, width: usize) -> String {
    let fitted = fit_text(text, width);
    let pad = width.saturating_sub(Line::raw(&fitted).width());
    format!("{fitted}{}", " ".repeat(pad))
}

/// A `label  value` row with a fixed label column; the value carries the meaning.
pub fn fact(label: &str, value: impl Into<String>, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<11}"), Style::default().fg(DIM)),
        Span::styled(value.into(), Style::default().fg(color)),
    ])
}
