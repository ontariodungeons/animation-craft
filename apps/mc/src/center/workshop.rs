//! Project overview: selection never attaches or switches a Studio.
use super::*;

pub(super) fn projects(c: &Center) -> &[Value] {
    c.state["catalog"]["projects"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
pub(super) fn selected(c: &Center) -> Option<&Value> {
    projects(c).get(c.catalog_selected)
}
fn matches_project(project: &Value, value: &Value) -> bool {
    let identities = [project["path"].as_str(), project["project_id"].as_str()];
    ["project", "active_project", "canonical_project"]
        .iter()
        .any(|key| {
            value[*key]
                .as_str()
                .is_some_and(|id| identities.contains(&Some(id)))
        })
}
fn discovery_fresh(c: &Center) -> bool {
    !c.studios["observation_error"].is_string()
        && crate::worker::age_ms(&c.studios, "observed_at_ms").is_some_and(|age| age < 6000)
}
fn discovery_known(c: &Center) -> bool {
    discovery_fresh(c)
        && c.studios["errors"].as_array().is_some_and(|errors| {
            errors.iter().all(|e| {
                e["state"] == "stale endpoint; no listener"
                    || e["error"]
                        .as_str()
                        .is_some_and(|v| v.starts_with("stale endpoint:"))
            })
        })
}
fn live_ui(c: &Center, project: &Value) -> bool {
    c.state["ui"]["connection"] == "connected"
        && matches_project(project, &c.state["ui"])
        && crate::worker::age_ms(&c.state["snapshot"], "observed_unix_ms")
            .is_some_and(|age| age < 6000)
}
pub(super) fn activity_label(c: &Center) -> String {
    let (running, _) = counts(c);
    if discovery_known(c) {
        format!("{running} Active")
    } else if running > 0 {
        format!("≥{running} Active")
    } else {
        "? Active".into()
    }
}
fn sessions<'a>(c: &'a Center, project: &'a Value) -> Vec<&'a Value> {
    if !discovery_fresh(c) {
        return Vec::new();
    }
    c.studios["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| matches_project(project, s))
        .collect()
}
fn worker<'a>(c: &'a Center, project: &Value) -> Option<&'a Value> {
    let value = &c.state["animation_worker"];
    if matches_project(project, value) && value["state"] != "not_started" && !value.is_null() {
        Some(value)
    } else if matches_project(project, &value["worker_owner"]) {
        Some(&value["worker_owner"])
    } else {
        None
    }
}
fn active(c: &Center, project: &Value) -> bool {
    !sessions(c, project).is_empty()
        || live_ui(c, project)
        || worker(c, project)
            .is_some_and(|w| matches!(w["state"].as_str(), Some("running" | "cancelling")))
}
pub(super) fn counts(c: &Center) -> (usize, usize) {
    let all = projects(c);
    let running = all.iter().filter(|p| active(c, p)).count();
    (running, all.len() - running)
}
pub(super) fn expanded(c: &Center) -> bool {
    c.catalog_show_inactive.unwrap_or(counts(c).0 == 0)
}
pub(super) fn order(c: &Center) -> Vec<usize> {
    let mut rows: Vec<_> = projects(c)
        .iter()
        .enumerate()
        .filter(|(_, p)| active(c, p) || expanded(c))
        .map(|(i, _)| i)
        .collect();
    rows.sort_by_key(|i| !active(c, &projects(c)[*i]));
    rows
}
pub(super) fn reconcile(c: &mut Center, catalog: &Value) {
    let rows = catalog["projects"].as_array();
    if let Some(path) = selected(c).and_then(|p| p["path"].as_str())
        && let Some(index) =
            rows.and_then(|rows| rows.iter().position(|p| p["path"].as_str() == Some(path)))
    {
        c.catalog_selected = index;
    }
    c.catalog_selected = c
        .catalog_selected
        .min(rows.map_or(0, |rows| rows.len().saturating_sub(1)));
}
pub(super) fn select(c: &mut Center, index: usize) -> Result<(), String> {
    let project = projects(c)
        .get(index)
        .ok_or("Project index is out of range")?;
    if !active(c, project) {
        c.catalog_show_inactive = Some(true);
    }
    c.catalog_selected = index;
    c.command_selected = 0;
    c.scroll = 0;
    Ok(())
}
pub(super) fn cycle(c: &mut Center, delta: isize) {
    let rows = order(c);
    if rows.is_empty() {
        return;
    }
    let at = rows
        .iter()
        .position(|i| *i == c.catalog_selected)
        .unwrap_or(0);
    let target = (at as isize + delta).clamp(0, rows.len() as isize - 1) as usize;
    let _ = select(c, rows[target]);
}
pub(super) fn toggle(c: &mut Center) {
    c.catalog_show_inactive = Some(!expanded(c));
    let rows = order(c);
    if !rows.contains(&c.catalog_selected)
        && let Some(index) = rows.first()
    {
        c.catalog_selected = *index;
    }
    c.scroll = 0;
}
fn display_name(c: &Center, project: &Value) -> String {
    let name = project["name"].as_str().unwrap_or("Untitled");
    if projects(c)
        .iter()
        .filter(|p| p["name"].as_str() == Some(name))
        .count()
        < 2
    {
        return name.into();
    }
    let path = std::path::Path::new(project["path"].as_str().unwrap_or(""));
    let folder = path
        .parent()
        .filter(|p| p.file_name().and_then(|v| v.to_str()) != Some(name))
        .or_else(|| path.parent().and_then(|p| p.parent()));
    let context = folder
        .and_then(|p| p.file_name())
        .and_then(|v| v.to_str())
        .unwrap_or("project");
    format!("{name} · {context}")
}
fn next(project: &Value) -> &'static str {
    if project["readiness"]["state"] == "unreadable" {
        "Inspect"
    } else if project["exists"] == false {
        "Locate"
    } else if project["readiness"]["audio_available"] == false {
        "Audio"
    } else if project["readiness"]["transcript_valid"] == false {
        "Transcript"
    } else if project["readiness"]["editor_ready"] == false {
        "Setup"
    } else if project["readiness"].is_object() {
        "Review"
    } else {
        "Inspect"
    }
}
fn status(c: &Center, project: &Value) -> (&'static str, Color) {
    if project["exists"] == false {
        return ("Missing", AMBER);
    }
    if let Some(w) = worker(c, project) {
        if w["observation_error"].is_string() {
            return ("Unknown", AMBER);
        }
        match w["state"].as_str() {
            Some("running") => {
                return if crate::worker::stale(w) {
                    ("Stale", AMBER)
                } else {
                    ("Rendering", GREEN)
                };
            }
            Some("cancelling") => return ("Stopping", AMBER),
            Some("failed") => return ("Failed", RED),
            Some("completed") => return ("Rendered", TEAL),
            Some("cancelled") => return ("Cancelled", MUTED),
            _ => {}
        }
    }
    if active(c, project) {
        ("Open", GREEN)
    } else if !discovery_known(c) {
        ("Unknown", AMBER)
    } else {
        ("Offline", MUTED)
    }
}
fn save(c: &Center, project: &Value) -> &'static str {
    let found = sessions(c, project);
    let ui = &c.state["ui"];
    let observed = if live_ui(c, project) {
        Some(ui)
    } else {
        found.first().copied()
    };
    match observed.and_then(|v| v["engine_unsaved"].as_bool()) {
        Some(true) => "! Edits",
        Some(false) => "✓ Saved",
        None => "? Save",
    }
}
pub(super) fn observations(c: &Center) -> Vec<Value> {
    projects(c).iter().enumerate().map(|(index,project)|{let (state,_)=status(c,project);json!({"index":index,"path":project["path"],"name":project["name"],"active":active(c,project),"state":state,"save":save(c,project),"next":next(project),"worker":worker(c,project),"selected":index==c.catalog_selected})}).collect()
}
pub(super) fn commands(c: &Center) -> Vec<(&'static str, String)> {
    let Some(project) = selected(c) else {
        return vec![("Catalog", "mc exec project.catalog".into())];
    };
    let path = project["path"].as_str().unwrap_or("");
    vec![
        (
            "Inspect selected project",
            format!(
                "mc exec ui.feedback {}",
                shell_quote(&format!("project={path}"))
            ),
        ),
        (
            "Open selected project in Booth",
            format!("mc tui project {}", shell_quote(&format!("path={path}"))),
        ),
    ]
}
fn button(
    c: &Center,
    id: &str,
    label: &str,
    area: ratatui::layout::Rect,
    action: &str,
    params: Value,
) {
    if area.width > 0 && area.height > 0 {
        c.hits.borrow_mut().push(Hit {
            id: id.into(),
            label: label.into(),
            area,
            action: action.into(),
            params,
            enabled: c.poke.is_none() && !c.chat_editing && !c.show_form,
        });
    }
}
pub(super) fn render(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    if area.width < 3 || area.height < 2 {
        return;
    }
    let (_, inactive) = counts(c);
    let title = if c.state["catalog"]["projects"].is_array() {
        format!(
            " PROJECTS · {} · {inactive} Other · Recent catalog",
            activity_label(c)
        )
    } else {
        " PROJECTS · Catalog unavailable".into()
    };
    f.render_widget(
        Paragraph::new(fit_text(&title, area.width as usize)).style(Style::default().fg(TEAL)),
        ratatui::layout::Rect::new(area.x, area.y, area.width, 1),
    );
    let preview = ratatui::layout::Rect::new(area.x, area.y + 1, area.width, 1);
    let controls = if area.width >= 65 {
        "[ Open ↵ ] [ Details D ] [ Agent B ]"
    } else {
        "[ Open ↵ ] [ Details D ]"
    };
    let controls_width = Line::raw(controls).width().min(area.width as usize) as u16;
    let name = selected(c)
        .map(|p| display_name(c, p))
        .unwrap_or("No selection".into());
    f.render_widget(
        Paragraph::new(fit_text(
            &format!(" › {name}"),
            area.width.saturating_sub(controls_width + 1) as usize,
        ))
        .style(Style::default().fg(DESK_FG).bg(DESK_SELECTED)),
        preview,
    );
    let buttons =
        ratatui::layout::Rect::new(area.right() - controls_width, preview.y, controls_width, 1);
    f.render_widget(
        Paragraph::new(controls).style(Style::default().fg(TEAL).bg(DESK_SELECTED)),
        buttons,
    );
    if selected(c).is_some() {
        button(
            c,
            "project-open",
            "Open selected project in Booth",
            ratatui::layout::Rect::new(buttons.x, buttons.y, 10.min(buttons.width), 1),
            "project.open-selected",
            json!({}),
        );
    }
    button(
        c,
        "project-details",
        "Selected project details",
        ratatui::layout::Rect::new(
            buttons.x + 11.min(buttons.width),
            buttons.y,
            buttons.width.saturating_sub(11).min(13),
            1,
        ),
        "details",
        json!({}),
    );
    if area.width >= 65 {
        button(
            c,
            "project-agent-booth",
            "Prepare a project-locked agent Booth",
            ratatui::layout::Rect::new(buttons.x + 25, buttons.y, 11, 1),
            "booth.prepare",
            json!({}),
        );
    }
    let mut grid = ratatui::layout::Rect::new(
        area.x,
        area.y + 2,
        area.width,
        area.height.saturating_sub(2),
    );
    if c.details_open {
        let parts = if area.width >= 110 {
            Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                .spacing(1)
                .split(grid)
        } else {
            Layout::vertical([
                Constraint::Min(5),
                Constraint::Length(grid.height.saturating_sub(5).min(12)),
            ])
            .split(grid)
        };
        grid = parts[0];
        render_details(f, c, parts[1]);
    }
    let columns = ((grid.width as usize + 1) / 39).clamp(1, 4);
    c.catalog_columns.set(columns);
    // Page whole visual rows so crossing groups cannot clip the selected card.
    let rows = order(c);
    let active_ids: Vec<_> = rows
        .iter()
        .copied()
        .filter(|i| active(c, &projects(c)[*i]))
        .collect();
    let inactive_ids: Vec<_> = rows
        .iter()
        .copied()
        .filter(|i| !active(c, &projects(c)[*i]))
        .collect();
    let visual: Vec<(bool, Vec<usize>)> = active_ids
        .chunks(columns)
        .map(|row| (true, row.to_vec()))
        .chain(
            inactive_ids
                .chunks(columns)
                .map(|row| (false, row.to_vec())),
        )
        .collect();
    let capacity = (grid.height.saturating_sub(2) / 6).max(1) as usize;
    let selected_row = visual
        .iter()
        .position(|(_, row)| row.contains(&c.catalog_selected))
        .unwrap_or(0);
    let start = selected_row / capacity * capacity;
    let shown = &visual[start..visual.len().min(start + capacity)];
    let mut y = grid.y;
    for group_active in [true, false] {
        if y >= grid.bottom() {
            break;
        }
        let group = ratatui::layout::Rect::new(grid.x, y, grid.width, 1);
        let label = if group_active {
            format!(" ● {}", activity_label(c))
        } else {
            format!(
                " {} {} · {inactive} · I {}{}",
                if expanded(c) { "▾" } else { "▸" },
                if discovery_known(c) {
                    "INACTIVE"
                } else {
                    "INACTIVE / UNKNOWN"
                },
                if expanded(c) { "Hide" } else { "Show" },
                if visual.len() > capacity {
                    format!(
                        " · Page {}/{}",
                        start / capacity + 1,
                        visual.len().div_ceil(capacity)
                    )
                } else {
                    String::new()
                }
            )
        };
        f.render_widget(
            Paragraph::new(fit_text(&label, grid.width as usize))
                .style(Style::default().fg(if group_active { GREEN } else { MUTED })),
            group,
        );
        if !group_active {
            button(
                c,
                "projects-inactive",
                "Toggle inactive projects",
                group,
                "project.inactive",
                json!({}),
            );
        }
        y += 1;
        for (_, row) in shown.iter().filter(|(a, _)| *a == group_active) {
            if y >= grid.bottom() {
                break;
            }
            render_cards(
                f,
                c,
                row,
                ratatui::layout::Rect::new(grid.x, y, grid.width, 6.min(grid.bottom() - y)),
                columns,
            );
            y = y.saturating_add(6);
        }
    }
    if projects(c).is_empty() {
        let error = c.state["catalog"]["error"].as_str().unwrap_or(
            if c.state["catalog"]["projects"].is_array() {
                "No recent projects. F6 opens project intake."
            } else {
                "Catalog unavailable. F8 refreshes the observation."
            },
        );
        f.render_widget(
            Paragraph::new(fit_text(error, grid.width as usize)).style(Style::default().fg(AMBER)),
            grid,
        );
    }
}
fn render_cards(
    f: &mut Frame,
    c: &Center,
    rows: &[usize],
    area: ratatui::layout::Rect,
    columns: usize,
) {
    for (position, index) in rows.iter().enumerate() {
        let y = area.y + (position / columns) as u16 * 6;
        if y >= area.bottom() {
            break;
        }
        let column = position % columns;
        let x = area.x + (column * area.width as usize / columns) as u16;
        let right = area.x + ((column + 1) * area.width as usize / columns) as u16;
        let rect = ratatui::layout::Rect::new(
            x,
            y,
            right.saturating_sub(x + u16::from(column + 1 < columns)),
            6.min(area.bottom() - y),
        );
        let project = &projects(c)[*index];
        let picked = *index == c.catalog_selected;
        let (state, color) = status(c, project);
        let name = project["name"].as_str().unwrap_or("Untitled");
        let title = fit_text(
            &format!(
                "{} {}",
                if picked { "›" } else { " " },
                display_name(c, project)
            ),
            rect.width.saturating_sub(4) as usize,
        );
        let block = round_panel(&title, if picked { TEAL } else { DESK_LINE }, true)
            .style(Style::default().bg(if picked { DESK_SELECTED } else { DESK_BG }));
        let inner = block.inner(rect);
        let width = inner.width as usize;
        let mut lines = vec![Line::from(vec![
            Span::styled(
                format!("{} {state}", if active(c, project) { "●" } else { "○" }),
                color,
            ),
            Span::styled(
                if save(c, project) == "? Save" {
                    String::new()
                } else {
                    format!(" · {}", save(c, project))
                },
                MUTED,
            ),
        ])];
        if let Some(w) = worker(c, project) {
            let finished = matches!(
                w["state"].as_str(),
                Some("completed" | "failed" | "cancelled")
            );
            let phase = if finished {
                if w["partial"] == true {
                    "Preview"
                } else {
                    "Render"
                }
            } else {
                w["phase"].as_str().unwrap_or(state)
            };
            lines.push(styled(
                format!(
                    "{}  {}",
                    if finished { "Last" } else { "Task" },
                    fit_text(phase, width.saturating_sub(6))
                ),
                MUTED,
            ));
            if let (Some(done), Some(total)) =
                (w["frames_completed"].as_u64(), w["frames_total"].as_u64())
            {
                if total > 0 && done <= total {
                    let (meter, tone) =
                        meter(Some(done), Some(total), width.saturating_sub(2).min(20));
                    lines.push(styled(meter, tone));
                }
            }
        }
        lines.push(styled(
            format!(
                "→ {}{}",
                next(project),
                if picked { " · Enter Booth" } else { "" }
            ),
            if picked { TEAL } else { MUTED },
        ));
        f.render_widget(Paragraph::new(lines).block(block), rect);
        button(
            c,
            &format!("project-card-{index}"),
            name,
            rect,
            "project.select",
            json!({"index":index}),
        );
    }
}
fn render_details(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let Some(project) = selected(c) else {
        return;
    };
    let width = area.width.saturating_sub(2) as usize;
    let (state, color) = status(c, project);
    let mut lines = vec![
        styled(project["name"].as_str().unwrap_or("Untitled"), TEAL),
        styled(state, color),
        styled(
            project["path"].as_str().unwrap_or("Path unavailable"),
            MUTED,
        ),
        Line::raw(""),
        styled(
            project["readiness"]["next_step"]
                .as_str()
                .unwrap_or("Readiness unavailable"),
            DESK_FG,
        ),
        styled("Readiness is not approval", DIM),
        Line::raw(""),
        styled("C Copy · 1/2 Command · Pg Scroll", TEAL),
    ];
    if !discovery_known(c) {
        let reason = c.studios["observation_error"]
            .as_str()
            .or_else(|| {
                c.studios["errors"]
                    .as_array()
                    .and_then(|v| v.first())
                    .and_then(|v| v["error"].as_str())
            })
            .unwrap_or("Awaiting Studio discovery");
        lines.push(styled(format!("Studio: {reason}"), AMBER));
    }
    let commands = commands(c);
    let command = &commands[c.command_selected % commands.len()];
    lines.push(styled(command.0, MUTED));
    lines.push(styled(command.1.clone(), BLUE));
    let max = wrapped_rows(&lines, width as u16).saturating_sub(area.height.saturating_sub(2));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((c.scroll.min(max), 0))
            .block(round_panel("PROJECT DETAILS · D close", TEAL, true)),
        area,
    );
}
