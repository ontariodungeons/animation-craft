//! Booth presentation consumes cached observations; drawing never drives the app.
use super::*;

pub(super) fn panel(title: impl Into<String>, active: bool) -> Block<'static> {
    quiet_panel(title, active)
}
pub(super) fn age_label(ms: u64) -> String {
    let seconds = ms / 1000;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, seconds / 60 % 60)
    }
}
pub(super) fn overview(c: &Center) -> Value {
    let ui = &c.state["ui"];
    let matched = ui["connection"] == "connected"
        && ui["active_project"] == c.state["project"]
        && c.state["project"].is_string();
    let connection = if matched {
        "Online"
    } else if ui["connection"] == "not_observed_open" {
        "Offline"
    } else {
        "Unknown"
    };
    let metrics = &c.state["metrics"];
    let name = c.state["project"]
        .as_str()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .and_then(|n| n.to_str())
        .unwrap_or("No project");
    json!({"name":name,"project":c.state["project"],"studio":connection,
  "save":if matched{match ui["engine_unsaved"].as_bool(){Some(true)=>"Edits",Some(false)=>"Saved",None=>"Unknown"}}else{"Unknown"},
  "target":c.state["target"].as_str().map(str::to_owned).or_else(||(c.state["snapshot"]["hyperframes"]==true).then(||"HyperFrames".into())),
  "audio_state":stage_evidence(c,0).and_then(|v|v["state"].as_str()),"audio_seconds":metrics["recording_duration_sec"],"words":metrics["timed_words"],"assets_available":metrics["available_assets"],"assets_registered":metrics["registered_assets"],
  "selected_stage":{"name":PROJECT_STAGES[c.stage_selected%PROJECT_STAGES.len()].0,"state":stage_evidence(c,c.stage_selected).and_then(|v|v["state"].as_str())},
  "problem":c.state["problems"].as_array().and_then(|p|p.first()),"chat_send_granted":c.scope.as_ref().is_none_or(|s|s.permits("agent.send")),"chat_available":matched&&c.pinned_instance.is_some()&&ui["endpoint"].is_string()&&c.scope.as_ref().is_none_or(|s|s.permits("agent.send"))})
}
fn row(label: &str, value: impl Into<String>, width: usize) -> Line<'static> {
    bounded_line(
        Line::from(vec![
            Span::styled(format!("{label:<10}"), MUTED),
            Span::styled(value.into(), DESK_FG),
        ]),
        width,
    )
}
pub(super) fn render_project(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let block = panel("PROJECT · F9 Sproket", false);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;
    if inner.height == 0 {
        return;
    }
    let data = overview(c);
    let studio = data["studio"].as_str().unwrap_or("Unknown");
    let light = if studio == "Online" { GREEN } else { AMBER };
    let mut lines = vec![
        row("Project", data["name"].as_str().unwrap_or("Unknown"), width),
        bounded_line(
            Line::from(vec![
                Span::styled(format!("{:<10}", "Studio"), MUTED),
                Span::styled(if studio == "Online" { "● " } else { "○ " }, light),
                Span::styled(studio.to_owned(), DESK_FG),
            ]),
            width,
        ),
        row("Save", data["save"].as_str().unwrap_or("Unknown"), width),
        row(
            "Selected",
            format!(
                "{} · {}",
                data["selected_stage"]["name"].as_str().unwrap_or("Stage"),
                data["selected_stage"]["state"]
                    .as_str()
                    .unwrap_or("unknown")
            ),
            width,
        ),
        row(
            "Target",
            match data["target"].as_str() {
                Some("hyperframes") => "HyperFrames",
                Some("remotion") => "Remotion",
                Some(value) => value,
                None => "Unknown",
            },
            width,
        ),
    ];
    let duration = data["audio_seconds"]
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|s| format!("{}:{:02}", s as u64 / 60, s as u64 % 60))
        .unwrap_or_else(|| {
            if data["audio_state"] == "missing" {
                "Missing".into()
            } else {
                "Unknown".into()
            }
        });
    let words = data["words"]
        .as_u64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".into());
    lines.push(row(
        "Media",
        format!("{duration} audio · {words} words"),
        width,
    ));
    let available = data["assets_available"]
        .as_u64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".into());
    let registered = data["assets_registered"]
        .as_u64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".into());
    lines.push(row(
        "Assets",
        format!("{available}/{registered} available"),
        width,
    ));
    if let Some(problem) = data["problem"].as_str() {
        lines.push(row("Review", problem, width));
    }
    f.render_widget(Paragraph::new(lines), inner);
    let button_width = Line::raw("F9 Sproket").width() as u16;
    if area.width >= button_width + 14 {
        c.hits.borrow_mut().push(Hit {
            id: "sproket-chat".into(),
            label: "Open Sproket project chat (F9)".into(),
            area: ratatui::layout::Rect::new(area.x + 12, area.y, button_width, 1),
            action: "chat".into(),
            params: json!({}),
            enabled: true,
        });
    }
}

pub(super) fn motion_active(value: &Value) -> bool {
    value["state"] == "running"
        && !crate::worker::stale(value)
        && !value["observation_error"].is_string()
}
fn border_cells(area: ratatui::layout::Rect, tick: u64) -> Vec<(u16, u16, &'static str)> {
    if area.width < 28 || area.height < 3 {
        return Vec::new();
    }
    let top = area.width - 25;
    let side = area.height - 2;
    let bottom = area.width - 2;
    let length = top + side + bottom + side;
    (0..3)
        .map(|offset| {
            let at = ((tick + offset) % u64::from(length)) as u16;
            if at < top {
                (area.x + 24 + at, area.y, "━")
            } else if at < top + side {
                (area.right() - 1, area.y + 1 + at - top, "┃")
            } else if at < top + side + bottom {
                (area.right() - 2 - (at - top - side), area.bottom() - 1, "━")
            } else {
                (area.x, area.bottom() - 2 - (at - top - side - bottom), "┃")
            }
        })
        .collect()
}
pub(super) fn motion_border(f: &mut Frame, value: &Value, area: ratatui::layout::Rect) {
    if !motion_active(value) {
        return;
    }
    for (x, y, glyph) in border_cells(area, crate::worker::now_ms() / 250) {
        f.render_widget(
            Span::styled(glyph, BLUE),
            ratatui::layout::Rect::new(x, y, 1, 1),
        );
    }
}
pub(super) fn render_chat(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let block = panel("SPROKET · Enter send · F9/Esc close", true);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 {
        return;
    }
    let data = overview(c);
    let connected = data["chat_available"] == true;
    let mut lines = vec![
        row(
            "Project",
            data["name"].as_str().unwrap_or("Unknown"),
            inner.width as usize,
        ),
        Line::from(vec![
            Span::styled(
                if connected { "● " } else { "○ " },
                if connected { GREEN } else { AMBER },
            ),
            Span::styled(
                if connected {
                    "Connected to this project's Studio".into()
                } else {
                    format!(
                        "Chat unavailable · {}",
                        if data["chat_send_granted"] == false {
                            "send not granted"
                        } else if data["studio"] == "Offline" {
                            "Studio offline"
                        } else {
                            "control unavailable"
                        }
                    )
                },
                DESK_FG,
            ),
        ]),
    ];
    if c.message.starts_with("Chat ") {
        lines.push(styled(
            c.message.clone(),
            if c.message.starts_with("Chat blocked") || c.message.starts_with("Chat error") {
                AMBER
            } else {
                MUTED
            },
        ));
    }
    let request = &c.state["chat"]["request"];
    if let Some(status) = request["status"].as_str() {
        lines.push(styled(format!("Reply  {status}"), MUTED));
    }
    for turn in c.state["chat_turns"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        lines.push(Line::raw(""));
        lines.push(styled(
            format!(
                "{}: {}",
                if turn["role"] == "user" {
                    "You"
                } else {
                    "Sproket"
                },
                conversation_text(turn["text"].as_str().unwrap_or(""))
            ),
            DESK_FG,
        ));
    }
    let readonly = data["chat_send_granted"] == false;
    let draft_lines = if readonly {
        vec![styled(
            "Read-only · Sproket send is not granted · F9/Esc close",
            MUTED,
        )]
    } else {
        vec![
            styled("Message · Enter sends · F9/Esc keeps draft", MUTED),
            styled(format!("> {}▏", c.chat_draft), DESK_FG),
        ]
    };
    let draft_height = wrapped_rows(&draft_lines, inner.width)
        .max(2)
        .min(inner.height.saturating_sub(1));
    let heading: Vec<_> = lines.drain(..2).collect();
    let parts = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(draft_height),
    ])
    .split(inner);
    f.render_widget(Paragraph::new(heading), parts[0]);
    if !connected {
        lines.insert(
            0,
            styled(
                "Open this project in Studio to connect. Your draft is retained.",
                MUTED,
            ),
        );
    }
    let skip = wrapped_rows(&lines, parts[1].width)
        .saturating_sub(parts[1].height)
        .saturating_sub(c.scroll);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((skip, 0)),
        parts[1],
    );
    f.render_widget(
        Paragraph::new(draft_lines.clone())
            .wrap(Wrap { trim: false })
            .scroll((
                wrapped_rows(&draft_lines, parts[2].width).saturating_sub(parts[2].height),
                0,
            )),
        parts[2],
    );
}
pub(super) fn action_title(c: &Center, literal: &str) -> &'static str {
    if literal.contains("animation.status") || literal.contains("blender.status") {
        "INSPECT RENDER"
    } else if literal.contains("ui.feedback") {
        "INSPECT STUDIO"
    } else if literal.contains("center.events") {
        "INSPECT ACTIVITY"
    } else if root_actions(c).iter().any(|a| engine_badge(a).1 == RED) {
        "INSPECT FAILURE"
    } else {
        "PROJECT ACTION"
    }
}
