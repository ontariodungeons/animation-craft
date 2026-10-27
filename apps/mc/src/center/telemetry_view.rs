use super::*;

pub(super) fn usage_readout_lines(_c: &Center) -> Vec<Line<'static>> {
    Vec::new()
}

pub(super) fn render_pulse(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    if let Some(worker) = worker_observation(c) {
        lines.push(styled(
            format!("ARTWORK IN MOTION · {}", crate::worker::headline(worker)),
            worker_colour(worker),
        ));
        for (label, key) in [
            ("Job", "job"),
            ("Project", "project"),
            ("Unit", "unit"),
            ("Invocation", "invocation_id"),
        ] {
            lines.push(styled(
                format!("{label}: {}", worker[key].as_str().unwrap_or("?")),
                MUTED,
            ));
        }
        lines.push(styled(crate::worker::resource_line(worker), DESK_FG));
        for key in ["error", "detail", "log", "clip"] {
            if let Some(text) = worker[key].as_str() {
                lines.push(styled(
                    format!("{key}: {text}"),
                    if key.contains("error") { RED } else { MUTED },
                ));
            }
        }
    } else {
        lines.push(styled("No active animation worker.", MUTED));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((c.scroll, 0))
            .block(round_panel("WORKER DETAILS · D closes", BLUE, false)),
        area,
    );
}

pub(super) fn render_config(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let lines = vec![
        styled("TUI & App Settings", TEAL),
        Line::raw(""),
        styled(
            format!("Process count: {}", c.settings.process_count),
            DESK_FG,
        ),
        styled(
            format!("Agent refresh interval: {}s", c.settings.agent_refresh_secs),
            DESK_FG,
        ),
        styled(
            format!("Log tail lines: {}", c.settings.log_tail_lines),
            DESK_FG,
        ),
        Line::raw(""),
        styled("Edit ~/.config/magicstudio/tui-settings.json to customize.", MUTED),
    ];
    f.render_widget(
        Paragraph::new(lines).block(round_panel("CONFIG · F5 toggle", TEAL, false)),
        area,
    );
}

pub(super) fn render_telemetry(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let cols = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
        .spacing(1)
        .split(area);
    render_host(f, c, cols[0]);
    render_logs(f, c, cols[1]);
}

fn render_host(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    if let Some(host) = &c.dials {
        let cpu = host
            .cpu_pct
            .map(|v| format!("{v:.0}%"))
            .unwrap_or_else(|| "?".into());
        lines.push(styled(format!("CPU   {cpu} · load {}", host.load), DESK_FG));
        lines.push(styled(
            format!(
                "RAM   {:.1}G / {:.1}G",
                host.mem_used_kb as f64 / 1048576.0,
                host.mem_total_kb as f64 / 1048576.0
            ),
            DESK_FG,
        ));
        lines.push(styled(
            format!(
                "SWAP  {:.1}G / {:.1}G",
                host.swap_used_kb as f64 / 1048576.0,
                host.swap_total_kb as f64 / 1048576.0
            ),
            MUTED,
        ));
    } else {
        lines.push(styled("Reading /proc…", MUTED));
    }
    lines.push(Line::raw(""));
    lines.push(styled("ACTIVE PROCESSES", TEAL));
    for row in c.procs.iter().take(8) {
        lines.push(styled(
            format!(
                "{:<7} {:<18} {:>6.1}M",
                row.pid,
                fit_text(&row.name, 18),
                row.rss_kb as f64 / 1024.0
            ),
            MUTED,
        ));
    }
    f.render_widget(
        Paragraph::new(lines).block(round_panel("HOST · /proc", TEAL, false)),
        area,
    );
}

fn render_logs(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let inner = area.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut section = |title: &str, values: &Value| {
        lines.push(styled(title.to_string(), TEAL));
        for line in values.as_array().into_iter().flatten().take(8) {
            lines.push(styled(fit_text(line.as_str().unwrap_or(""), inner), MUTED));
        }
        lines.push(Line::raw(""));
    };
    section("BLENDER · worker.log", &c.logs["blender"]);
    section("MAGIC STUDIO · activity", &c.logs["studio"]);
    f.render_widget(
        Paragraph::new(lines).block(round_panel("LOGS · local activity", TEAL, false)),
        area,
    );
}

pub(super) fn render_muppets(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let tiles = gang_tiles(c);
    let selected = c.gang_selected.min(tiles.len().saturating_sub(1));
    let mut lines = vec![styled("AGENT ROSTER", TEAL)];
    for (idx, (name, col, state_col, word)) in tiles.iter().enumerate() {
        let mark = if idx == selected { "›" } else { " " };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{mark} {:<2} {:<14} ", agent_number(idx), name),
                Style::default().fg(*col),
            ),
            Span::styled(
                format!("{} {}", agent_glyph(word), word.to_lowercase()),
                Style::default().fg(*state_col),
            ),
        ]));
    }
    f.render_widget(
        Paragraph::new(lines).block(round_panel("MOPS · 1–9 / 0 select", TEAL, true)),
        area,
    );
}
