//! Panel rows from the on-disk UI config. A Studio fills signal and busy only.
use super::*;
use magicstudio_engine::panels::{Config, Panel, PanelReport, Reason};

pub(super) fn load_config() -> Result<Config, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../magicstudio-shell/ui/components.json");
    Config::load(&path).map_err(|error| format!("{}: {error}", path.display()))
}

/// Rebuild the cached lines only when the live revision or availability changes.
pub(super) fn sync(c: &mut Center) {
    let report = PanelReport::from_feedback(&c.state["ui"]);
    if c.last_panels_revision == Some(report.revision) && c.panel_report.available == report.available {
        return;
    }
    let (rows, detail) = built(c, &report);
    c.last_panels_revision = Some(report.revision);
    c.panel_report = report;
    c.panel_rows = rows;
    c.panel_detail = detail;
}

fn built(c: &Center, report: &PanelReport) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    if let Some(error) = &c.panel_config_error {
        return (vec![styled(format!("UI config unreadable: {error}"), RED)], Vec::new());
    }
    let mut live: std::collections::BTreeMap<&str, &Panel> = std::collections::BTreeMap::new();
    if report.available {
        for item in &report.items {
            live.insert(item.id.as_str(), item);
        }
    }
    let mut rows = vec![health_line(report)];
    for (id, entry) in &c.panel_config.elements {
        let label = entry.label.as_deref().filter(|text| !text.is_empty()).unwrap_or(id.as_str());
        let (dot, color, busy) = match live.get(id.as_str()) {
            Some(item) => ("●", signal_color(item.signal), item.busy),
            None => ("·", MUTED, false),
        };
        let mut spans = vec![
            Span::styled(dot.to_owned(), color),
            Span::styled(format!(" {label}"), DESK_FG),
        ];
        if busy {
            spans.push(Span::styled(" busy".to_owned(), AMBER));
        }
        rows.push(Line::from(spans));
    }
    (rows, detail_lines(report))
}

fn health_line(report: &PanelReport) -> Line<'static> {
    if !report.available {
        return styled("UI health  Panels unavailable", AMBER);
    }
    let untracked = report.count(Reason::Untracked);
    let unstyled = report.count(Reason::Unstyled);
    if untracked == 0 && unstyled == 0 {
        return styled("UI health  every element tracked and styled", GREEN);
    }
    let mut parts = Vec::new();
    if untracked > 0 {
        parts.push(format!("untracked {untracked}"));
    }
    if unstyled > 0 {
        parts.push(format!("unstyled {unstyled}"));
    }
    styled(format!("UI health  {}", parts.join(" · ")), AMBER)
}

fn detail_lines(report: &PanelReport) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (reason, name) in [(Reason::Untracked, "untracked"), (Reason::Unstyled, "unstyled")] {
        let ids: Vec<&str> = report.unconfigured.iter().filter(|row| row.reason == reason).map(|row| row.id.as_str()).collect();
        if ids.is_empty() {
            continue;
        }
        lines.push(styled(format!("{name}  {}", ids.join(" ")), MUTED));
    }
    lines
}

pub(super) fn render(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let block = booth::panel("PANELS", false);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let mut lines = Vec::new();
    if let Some(health) = c.panel_rows.first() {
        lines.push(health.clone());
    }
    if c.details_open {
        lines.extend(c.panel_detail.clone());
    }
    lines.extend(c.panel_rows.iter().skip(1).cloned());
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
