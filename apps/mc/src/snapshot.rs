//! One Studio observation shared by `mc exec center.snapshot`, `mc exec center.tail` and both
//! Command Center views. Reads existing sources only (ui.feedback, ui.state, motion.tracks, the
//! saved project file and the center.events journal); anything unobserved stays null.
use serde_json::{Value, json};

pub const SNAPSHOT_SCHEMA: &str = "mc.center.snapshot/1";
pub const TAIL_SCHEMA: &str = "mc.center.tail/1";

/// Page state and rig fps from the matched Studio window; empty when no window is connected.
/// `previous` is the last snapshot, so fps is re-read only when the subject changes.
pub fn live(ui: &Value, previous: &Value) -> Value {
    if crate::scope::current()
        .ok()
        .flatten()
        .is_some_and(|s| !s.permits("ui.state"))
    {
        return json!({});
    }
    let Some(endpoint) = ui["endpoint"]
        .as_str()
        .filter(|_| ui["connection"] == "connected")
    else {
        return json!({});
    };
    let page =
        crate::bridge_call(endpoint, "ui.state", json!({})).unwrap_or_else(|e| json!({"error": e}));
    let subject = page["animation"]["character"].as_str();
    let fps = if subject.is_some()
        && subject == previous["subject"].as_str()
        && !previous["fps"].is_null()
    {
        previous["fps"].clone()
    } else if let Some(character) = subject.filter(|_| ui["engine_unsaved"].is_boolean()) {
        // motion.tracks waits for the engine lock; ask only while ui.feedback found it free.
        // A rig without its own fps is sampled at motion.sample's default of 30.
        crate::bridge_call(endpoint, "motion.tracks", json!({"character": character}))
            .map(|t| {
                if t["fps"].is_number() {
                    t["fps"].clone()
                } else {
                    json!(30)
                }
            })
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    json!({"page": page, "fps": fps})
}

/// The snapshot object agents read. Keys are stable; values are null when unknown.
pub fn build(project: Option<&str>, ui: &Value, live: &Value, activity: &Value) -> Value {
    let connected = ui["connection"] == "connected";
    let project = ui["active_project"]
        .as_str()
        .filter(|_| connected)
        .or(project);
    // Page state counts only when the page shows the same project ui.feedback reported.
    let animation = if connected && live["page"]["project"] == ui["active_project"] {
        &live["page"]["animation"]
    } else {
        &Value::Null
    };
    // Studio builds before the edit counter report only reasons; their count stays null.
    let unsaved = if connected && ui["unsaved_edits"].is_u64() {
        ui["unsaved_edits"].clone()
    } else {
        Value::Null
    };
    let reasons = if connected {
        ui["dirty_reasons"].clone()
    } else {
        Value::Null
    };
    // workshop.set_target saves immediately, so the project file holds the live target.
    let hyperframes = project
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .map(|doc| doc["workshop"]["target"].as_str().unwrap_or("remotion") == "hyperframes");
    let error = |source: &str, command: &Value, message: &Value, at: &Value| json!({"source": source, "command": command, "message": message, "unix_micros": at});
    let last_error = project
        .map(|p| tail(activity, Some(p)))
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|e| e["type"] == "error")
        .map(|e| error("engine", &e["command"], &e["error"], &e["unix_micros"]))
        .or_else(|| {
            ui["errors"]
                .as_array()
                .and_then(|v| v.last())
                .map(|e| error("studio page", &Value::Null, &e["message"], &Value::Null))
        })
        .or_else(|| {
            ui["error"]
                .as_str()
                .or(live["page"]["error"].as_str())
                .map(|m| error("connection", &Value::Null, &json!(m), &Value::Null))
        });
    // A clip is converting between its clip_sheet_started and its converted/failed event.
    let mut converting: Vec<Value> = Vec::new();
    for e in project.map(|p| tail(activity, Some(p))).unwrap_or_default() {
        match e["type"].as_str() {
            Some("clip_sheet_started") => converting.push(e["clip"].clone()),
            Some("clip_sheet_converted" | "clip_sheet_failed") => {
                converting.retain(|c| *c != e["clip"])
            }
            _ => {}
        }
    }
    json!({
        "schema": SNAPSHOT_SCHEMA,
        "observed_unix_ms": now_ms(),
        "connection": ui["connection"].as_str().unwrap_or("unknown"),
        "project": project,
        "subject": animation["character"],
        // The rest pose keeps the last track selected underneath; report the pose itself.
        "clip": if animation["rest"] == true { json!("@rest") } else { animation["track"].clone() },
        "playhead_sec": animation["time"],
        "selected_part": animation["selected_layer"],
        "unsaved_edits": unsaved,
        "unsaved_reasons": reasons,
        "hyperframes": hyperframes,
        "fps": if animation.is_null() { Value::Null } else { live["fps"].clone() },
        "last_error": last_error,
        // Theatre's active sheet, as the page reports it (one sheet per clip: "<asset> / <clip>").
        "sheet": if animation.is_null() { Value::Null } else { ui["theatre"]["selected_objects"][0]["sheet"].clone() },
        "converting": converting.last().cloned().unwrap_or(Value::Null),
        "theatre": if connected && ui["theatre"].is_object() {
            let t = &ui["theatre"];
            json!({"availability": t["availability"], "error": t["error"], "persistence": t["persistence"],
                "selected_object": t["selected_objects"][0], "playing": t["sequence_playing"]})
        } else { Value::Null },
    })
}

/// A command that runs a job: one start row, one outcome row and, when it measured one, a result
/// row carrying outputs. Unknown commands stay untyped rather than inventing progress.
fn job(command: &str) -> Option<&'static str> {
    match command {
        "image.generate" => Some("generation"),
        "motion.recipe" => Some("motion_recipe"),
        "video.place_track" => Some("place_track"),
        "video.snapshot" => Some("snapshot"),
        "video.render" => Some("render"),
        "export.layered_motion" => Some("layered_motion"),
        _ => None,
    }
}

/// Typed events from the journal's history merged with the rows the three-root journal still
/// holds (or its retained rows on an older engine), oldest first, for `project` when given.
/// The history is narrower than the journal — saves, errors, page reports and a few named jobs —
/// so job rows for other commands only survive in the journal's recent root actions; the union
/// keeps a start and its outcome observable together. Both sources are merged by sequence, once
/// per row. selection_changed / clip_switched appear once the page reports
/// `{stage:"selection_changed", part}` / `{stage:"clip_switched", clip}` events.
pub fn tail(activity: &Value, project: Option<&str>) -> Vec<Value> {
    let mut out = Vec::new();
    let mut rows: Vec<&Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // The journal copy comes first: retain() has already named its project and root request.
    for e in [&activity["events"], &activity["history"]]
        .into_iter()
        .flat_map(|source| source.as_array().into_iter().flatten())
    {
        if e["sequence"]
            .as_u64()
            .is_none_or(|sequence| seen.insert(sequence))
        {
            rows.push(e);
        }
    }
    rows.sort_by_key(|e| e["sequence"].as_u64());
    // One request can report its outcome twice: command_result carries measured outputs and
    // engine_success repeats it. The result row is that request's single finish.
    let measured: std::collections::HashSet<&str> = rows
        .iter()
        .filter(|e| e["stage"] == "command_result")
        .filter_map(|e| e["request_id"].as_str())
        .collect();
    for e in rows {
        if project.is_some_and(|p| e["project"].as_str() != Some(p)) {
            continue;
        }
        let command = e["command"].as_str().unwrap_or_default();
        let stage = e["stage"].as_str().unwrap_or_default();
        let failed = matches!(stage, "engine_error" | "handler_error");
        let request = e["request_id"].as_str();
        let kinds: Vec<String> = match (command, stage) {
            (_, "saved_state" | "theatre_saved") => vec!["saved".into()],
            (_, "theatre_selection" | "selection_changed") => vec!["selection_changed".into()],
            (_, "theatre_error") => vec!["error".into()],
            (_, "clip_switched") => vec!["clip_switched".into()],
            (_, "clip_sheet_started") => vec!["clip_sheet_started".into()],
            (_, "clip_sheet_converted") => vec!["clip_sheet_converted".into()],
            (_, "clip_sheet_failed") => vec!["clip_sheet_failed".into()],
            // Drawn motion paths are not Theatre keyframes; Theatre reports its own keyframe counts.
            ("motion.path", "engine_success") => vec!["motion_path_drawn".into()],
            ("motion.undo_path", "engine_success") => vec!["motion_path_undone".into()],
            (_, "theatre_keyframes") if e["count"].as_u64() > e["previous"].as_u64() => {
                vec!["keyframe_added".into()]
            }
            (_, "theatre_keyframes") => vec!["keyframe_removed".into()],
            // Every job reports progress and exactly one result, so a consumer never has to infer
            // completion from a row that never came.
            (_, "dispatch_running" | "command_result" | "engine_success" | "engine_needs_work")
                if job(command).is_some() =>
            {
                let stem = job(command).unwrap_or_default();
                match stage {
                    "dispatch_running" => vec![format!("{stem}_started")],
                    "command_result" => vec![format!("{stem}_finished")],
                    _ if measured.contains(request.unwrap_or_default()) => Vec::new(),
                    _ => vec![format!("{stem}_finished")],
                }
            }
            // A measured refusal already reported its single finish; only the error row is left.
            _ if job(command).is_some()
                && failed
                && measured.contains(request.unwrap_or_default()) =>
            {
                vec!["error".into()]
            }
            _ if job(command).is_some() && failed => vec![
                format!("{}_finished", job(command).unwrap_or_default()),
                "error".into(),
            ],
            _ if failed => vec!["error".into()],
            _ => Vec::new(),
        };
        for kind in kinds {
            let client = e["client_label"].as_str().unwrap_or("");
            let client_emoji = match client {
                c if c.contains("opencode") => "🤖",
                c if c.contains("cursor") => "🖱️",
                c if c.contains("claude") => "🐙",
                c if c.contains("grok") => "🤖",
                c if c.contains("codex") => "📜",
                c if c.contains("beaker") => "🧪",
                c if c.contains("fozzie") => "🐻",
                c if c.contains("rizzo") => "🐭",
                c if c.contains("bunsen") => "🔬",
                c if c.contains("statler") => "👴",
                c if c.contains("waldorf") => "👴",
                c if c.contains("gonzo") => "🚀",
                c if c.contains("sprocket") => "🐶",
                _ => "?",
            };
            out.push(json!({
                "sequence": e["sequence"],
                "unix_micros": e["unix_micros"],
                "type": kind,
                "command": e["command"],
                // A measured result can report a refusal the row's stage alone would call a success.
                "ok": !failed && e["outputs"]["ok"] != false,
                "error": e["error"],
                "origin": e["origin"],
                "pane_id": e["pane_id"],
                "client": e["client_label"],
                "client_emoji": client_emoji,
                "request_id": e["request_id"],
                // The root action ties a job's start and its result together when grouped.
                "root_request_id": e["root_request_id"].as_str().or(request),
                "project": e["project"],
                "control": e["control"],
                // Theatre reports its selection as [{sheet, part}]; the first object is the part.
                "part": if e["part"].is_null() { e["objects"][0]["part"].clone() } else { e["part"].clone() },
                "clip": e["clip"],
                "sheet": e["sheet"],
                // Measured evidence the result row carries; null when the row was only an outcome.
                "outputs": e["outputs"],
                "elapsed_ms": e["elapsed_ms"],
            }));
        }
    }
    out
}

/// One status line for both Command Center views.
pub fn status_line(s: &Value) -> String {
    let text = |key: &str| {
        s[key]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| "?".into())
    };
    let project = s["project"]
        .as_str()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .and_then(|v| v.to_str())
        .unwrap_or("no project");
    let playhead = s["playhead_sec"]
        .as_f64()
        .map(|t| format!("{t:.3}s"))
        .unwrap_or_else(|| "?".into());
    let unsaved = s["unsaved_edits"]
        .as_u64()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".into());
    let hyperframes = match s["hyperframes"].as_bool() {
        Some(true) => "on",
        Some(false) => "off",
        None => "?",
    };
    let fps = s["fps"]
        .as_f64()
        .map(|f| format!("{f}"))
        .unwrap_or_else(|| "?".into());
    let error = s["last_error"]["message"]
        .as_str()
        .map(|m| format!(" · err {m}"))
        .unwrap_or_default();
    let sheet = s["sheet"]
        .as_str()
        .map(|v| format!(" · sheet {v}"))
        .unwrap_or_default();
    let converting = s["converting"]
        .as_str()
        .map(|v| format!(" · converting {v}"))
        .unwrap_or_default();
    format!(
        "{project} · subject {} · clip {}{sheet} · ▶ {playhead} · part {} · unsaved {unsaved} · HF {hyperframes} · {fps}fps{converting}{error}",
        text("subject"),
        text("clip"),
        text("selected_part")
    )
}

/// The same facts, dense: a status strip a person can scan in one pass.
pub fn status_strip(s: &Value) -> String {
    let clip = s["clip"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| "?".into());
    let time = s["playhead_sec"]
        .as_f64()
        .map(|t| format!("{t:.2}s"))
        .unwrap_or_else(|| "?".into());
    let part = s["selected_part"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| "?".into());
    let unsaved = s["unsaved_edits"]
        .as_u64()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".into());
    let mut parts = vec![
        format!("@{}", clip.trim_start_matches('@')),
        format!("▶{time}"),
        format!("part {part}"),
    ];
    if unsaved != "0" {
        parts.push(format!("unsaved {unsaved}"));
    }
    if let Some(hf) = s["hyperframes"].as_bool() {
        parts.push(if hf { "HF on" } else { "HF off" }.into());
    }
    if let Some(conv) = s["converting"].as_str() {
        parts.push(format!("converting {conv}"));
    }
    if let Some(err) = s["last_error"]["message"].as_str() {
        parts.push(format!("err {err}"));
    }
    parts.join(" · ")
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// `mc exec center.snapshot` / `center.tail`: observe the Studio for `project` (or the only
/// open Studio) and read its journal. Never launches, switches or writes anything.
pub fn collect(project: Option<&str>, bridge: Option<&str>) -> (Value, Value) {
    let target = bridge.unwrap_or("auto");
    let mut ui = crate::bridge_call(target, "ui.feedback", json!({"project": project}))
        .unwrap_or_else(|e| json!({"connection": "error", "error": e}));
    if bridge.is_some() && ui["connection"] == "connected" {
        if let Some(expected) = project {
            let canonical =
                |p: &str| magicstudio_engine::studio_sessions::canonical(std::path::Path::new(p));
            if canonical(expected)
                .ok()
                .zip(
                    ui["active_project"]
                        .as_str()
                        .and_then(|p| canonical(p).ok()),
                )
                .is_none_or(|(a, b)| a != b)
            {
                ui = json!({"connection":"error","error":"Selected Studio does not show the requested project; no fallback observation used"});
            }
        }
        if ui["connection"] == "connected" {
            ui["endpoint"] = json!(target);
        }
    }
    let project = ui["active_project"].as_str().or(project).map(str::to_owned);
    let mut session = magicstudio_engine::Session::default();
    let activity = session
        .execute("center.events", json!({"project": project, "limit": 192}))
        .unwrap_or_else(|e| json!({"error": e.to_string()}));
    let live = live(&ui, &Value::Null);
    (build(project.as_deref(), &ui, &live, &activity), activity)
}
