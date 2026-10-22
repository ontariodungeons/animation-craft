//! Terminal presentation only. All project operations execute the shared engine registry.
use animationcraft_engine::Session;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use serde_json::{Value, json};
use std::{
    io::{self, IsTerminal},
    time::Duration,
};
const LABELS: [&str; 8] = [
    "Existing project (.acproj)",
    "New song root",
    "Song name",
    "Existing audio",
    "Transcript JSON",
    "Original captions SRT (optional)",
    "Existing audiomap (optional)",
    "Notes / references (; separated)",
];
struct Center {
    session: Session,
    fields: [String; 8],
    selected: usize,
    editing: bool,
    state: Value,
    message: String,
    scroll: u16,
    show_form: bool,
    control_view: bool,
    command_selected:usize,
    started:std::time::Instant,
    seen_cursor:u64,
    flash_after:u64,
    flash_started:Option<std::time::Instant>,
    pinned_instance:Option<String>,
}
impl Center {
    fn new(p: &Value) -> Self {
        let value = |k: &str| p[k].as_str().unwrap_or_default().to_string();
        Self{session:Session::user_session(),fields:[value("project"),p["root"].as_str().unwrap_or("/run/media/jesse/Speedy/media").to_string(),value("name"),value("audio"),value("transcript"),value("captions"),value("audiomap"),value("materials")],selected:0,editing:false,show_form:false,control_view:false,command_selected:0,started:std::time::Instant::now(),seen_cursor:0,flash_after:0,flash_started:None,pinned_instance:None,state:json!({}),message:"Choose inputs. Nothing runs until you select an action. F3 opens an existing project; F5 creates; F6 resumes explicitly.".into(),scroll:0}
    }
    fn refresh(&mut self) {
        match crate::dispatch(&mut self.session,"center.status",json!({"_origin":"Booth inspection","_expected_instance":self.pinned_instance})) {
            Ok(mut value)=>{
                let project=self.session.project_path.as_ref().map(|p|p.to_string_lossy().into_owned());
                value["ui"]=crate::local_ui::call("auto","ui.feedback",json!({"project":project,"_expected_instance":self.pinned_instance})).unwrap_or_else(|e|json!({"connection":"error","error":e}));
                if self.pinned_instance.is_none(){self.pinned_instance=value["ui"]["instance"].as_str().map(str::to_owned);}
                value["catalog"]=self.session.execute("project.catalog",json!({})).unwrap_or_else(|e|json!({"error":e.to_string()}));
                let cursor=value["activity"]["next_cursor"].as_u64().unwrap_or(0);
                if self.seen_cursor>0&&cursor>self.seen_cursor{self.flash_after=self.seen_cursor;self.flash_started=Some(std::time::Instant::now());}
                self.seen_cursor=cursor;self.state=value;
            },Err(error)=>{self.message=error.clone();self.state["ui"]=json!({"connection":"stale_or_unavailable","ui_ready":false,"error":error,"recovery":"F8 explicitly reattaches the same project; Studio is never switched."});},
        }
    }
    fn execute(&mut self, id: &str, mut p: Value) {
        if id=="project.open"{self.pinned_instance=None;}else if let Some(instance)=&self.pinned_instance{p["_expected_instance"]=json!(instance);}
        p["_origin"]=json!("Booth action");p["_client"]=json!("Booth supplied label");
        match crate::dispatch(&mut self.session,id,p) {
            Ok(v) => {
                if let Some(path) = v["project"].as_str() {
                    self.fields[0] = path.into();
                }
                self.message = if let Some(errors) =
                    v["problems"].as_array().filter(|e| !e.is_empty())
                {
                    format!(
                        "RECEIVED / NEEDS WORK: {}",
                        errors
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" | ")
                    )
                } else {
                    if id=="project.intake" {format!("INTAKE {}. Storyboard readiness: pending. Studio verification: pending.",if v["intake_ready"]==true{"READY"}else{"NEEDS WORK"})} else {format!("COMMAND SUCCEEDED: {id}. This is not storyboard or Studio readiness.")}
                };
            }
            Err(e) => {
                self.message = format!(
                    "BLOCKED: {e}. See command details/manual below. Existing files retained."
                )
            }
        };
        self.refresh();
    }
    fn intake(&mut self, resume: bool) {
        let mut p = json!({"resume":resume});
        if !self.fields[0].trim().is_empty() {
            p["path"] = json!(self.fields[0].trim());
        } else {
            p["root"] = json!(self.fields[1].trim());
            p["name"] = json!(self.fields[2].trim());
        }
        for (i, key) in [
            (3, "audio"),
            (4, "transcript"),
            (5, "captions"),
            (6, "audiomap"),
        ] {
            if !self.fields[i].trim().is_empty() {
                p[key] = json!(self.fields[i].trim());
            }
        }
        p["materials"] = json!(
            self.fields[7]
                .split(';')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
        );
        self.execute("project.intake", p);
    }
    fn details(&self) -> String {
        let id = match self.selected {
            0 => "project.open",
            1 | 2 => "project.intake",
            3 => "workshop.set_audio",
            4..=6 => "workshop.import_transcript",
            _ => "project.intake",
        };
        let spec = self.state["commands"]
            .as_array()
            .and_then(|v| v.iter().find(|v| v["id"] == id));
        let mut text = format!(
            "COMMAND CONTRACT\n{id}\n{}\n\n",
            spec.map(|v| v["params"].as_str().unwrap_or_default())
                .unwrap_or("registry unavailable")
        );
        if let Some(spec) = spec {
            text.push_str(&format!(
                "{}\n\n",
                spec["block_reason"]
                    .as_str()
                    .unwrap_or("Available; required arguments still validated by engine")
            ));
        }
        text.push_str(&format!("LIVE STUDIO: {}\nProject: {}\nPhase: {} · ready: {} · unsaved: {}\nEngine revision: {} · UI acknowledgement: {}\n{}\n\n",self.state["ui"]["connection"],self.state["ui"]["active_project"],self.state["ui"]["phase"],self.state["ui"]["ui_ready"],self.state["ui"]["unsaved_state"],self.state["ui"]["engine_revision"],self.state["ui"]["ui_acknowledgement"],self.state["ui"]["recovery"].as_str().unwrap_or_default()));
        text.push_str("RECENT PROJECT ACTIVITY (polling)\n");
        for event in self.state["activity"]["events"].as_array().into_iter().flatten().rev().take(8){text.push_str(&format!("#{} {} · {} · {} · {}\n",event["sequence"],event["client_label"].as_str().unwrap_or("UI"),event["stage"].as_str().unwrap_or_default(),event["command"].as_str().unwrap_or_default(),event["error"].as_str().unwrap_or_default()));}
        text.push_str("STUDIO → CLI (source verified)\n");
        for map in self.state["ui_mapping"].as_array().into_iter().flatten() {
            text.push_str(&format!(
                "{} → {}\n{}\n",
                map["control"].as_str().unwrap_or_default(),
                map["command"].as_str().unwrap_or_default(),
                map["status"].as_str().unwrap_or_default()
            ));
        }
        text.push_str("\nREAD THE MANUAL\nanimationcraft-cli commands workshop.import_transcript\ncrates/engine/src/workshop.rs\n~/.agents/skills/media-use/audio/references/transcribe.md\n\nF7 creates native scene Markdown/TOC; it does not build or register the separate DeckCraft template.\n\nNo ASR, transcoding, install, Studio launch or project switching is automatic. Missing or invalid inputs block dependent steps only.\n");
        text
    }
}
fn render_form(f: &mut Frame, c: &Center) {
    let area = f.area();
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(12),
        Constraint::Length(6),
        Constraint::Length(3),
    ])
    .split(area);
    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "  ANIMATIONCRAFT ",
                Style::default()
                    .fg(Color::Rgb(102, 226, 205))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("/ COMMAND CENTER · THE BOOTH  ·  F4 Monitor",Style::default().fg(BLUE)),
        ]),
        Line::from(format!(
            "  {}  ·  HEADLESS PROJECT SESSION  ·  human visual review pending",
            c.state["project"].as_str().unwrap_or("No project selected")
        )),
    ])
    .style(Style::default().bg(Color::Rgb(20, 30, 42)));
    f.render_widget(title, layout[0]);
    let cols = Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(layout[1]);
    let mut lines = Vec::new();
    for (i, label) in LABELS.iter().enumerate() {
        let selected = i == c.selected;
        let mark = if selected {
            if c.editing { "✎" } else { "›" }
        } else {
            " "
        };
        let color = if selected {
            Color::Rgb(102, 226, 205)
        } else {
            Color::Gray
        };
        lines.push(Line::styled(
            format!("{mark} {label}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(format!(
            "  {}",
            if c.fields[i].is_empty() {
                "— choose file / enter value —"
            } else {
                &c.fields[i]
            }
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        format!(
            "Transcript: {} · {} words",
            if c.state["transcript"]["valid"] == true {
                "VALID TIMING"
            } else {
                "MISSING / NEEDS REPAIR"
            },
            c.state["transcript"]["words"]
        ),
        Style::default().fg(if c.state["transcript"]["valid"] == true {
            Color::Green
        } else {
            Color::Yellow
        }),
    ));
    for step in c.state["steps"].as_array().into_iter().flatten() {
        lines.push(Line::from(format!(
            "✓ {} — {}",
            step["command"].as_str().unwrap_or_default(),
            step["status"].as_str().unwrap_or_default()
        )));
    }
    let offset = (c.selected as u16 * 2).saturating_sub(cols[0].height.saturating_sub(5));
    f.render_widget(
        Paragraph::new(lines)
            .scroll((offset, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" PROJECT / INPUTS · ↑↓ select · Enter edit "),
            )
            .wrap(Wrap { trim: false }),
        cols[0],
    );
    f.render_widget(
        Paragraph::new(c.details())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" LIVE REGISTRY / MAPPING "),
            )
            .wrap(Wrap { trim: false })
            .scroll((c.scroll, 0)),
        cols[1],
    );
    let prov = &c.state["provenance"];
    let text = format!(
        "{}\nProvenance: {} | provider: {} | model: {}\n{}",
        c.message,
        prov["mode"].as_str().unwrap_or("unknown"),
        prov["provider"].as_str().unwrap_or("unknown"),
        prov["model"].as_str().unwrap_or("unknown"),
        c.state["problems"]
            .as_array()
            .map(|v| v
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" | "))
            .unwrap_or_default()
    );
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" RECEIPT / REPAIR GUIDANCE "),
            )
            .style(Style::default().fg(Color::Yellow)),
        layout[2],
    );
    f.render_widget(Paragraph::new("F2 New form  ·  F3 Open  ·  F5 Create  ·  F6 Resume  ·  F7 Scene scaffold  ·  F8 Refresh\nCtrl+S Save  ·  PgUp/PgDn Details  ·  Esc Cancel edit  ·  Ctrl+Q Exit (saved project stays)").block(Block::default().borders(Borders::TOP)).style(Style::default().fg(Color::Rgb(102,226,205))),layout[3]);
}
const TEAL: Color=Color::Rgb(124,248,247);
const BLUE: Color=Color::Rgb(130,157,212);
const GREEN: Color=Color::Rgb(79,232,143);
const AMBER: Color=Color::Rgb(246,193,104);
const RED: Color=Color::Rgb(255,118,130);
const MUTED: Color=Color::Rgb(181,197,219);
fn shell_quote(value:&str)->String{format!("'{}'",value.replace('\'',"'\\''"))}
fn styled(value:impl Into<String>,color:Color)->Line<'static>{Line::styled(value.into(),Style::default().fg(color))}
impl Center{
 fn copy_commands(&self)->Vec<(&'static str,String)>{
  let exe=shell_quote(&std::env::args().next().unwrap_or_else(||"animationcraft-cli".into()));
  let project=self.state["project"].as_str().unwrap_or("");
  let cursor=self.state["activity"]["next_cursor"].as_u64().unwrap_or(0);
  let inspection=if let (Some(endpoint),Some(instance))=(self.state["ui"]["endpoint"].as_str(),self.pinned_instance.as_deref()){
   format!("{exe} --bridge {} exec workshop.status {} {}",shell_quote(endpoint),shell_quote(&format!("_expected_project={project}")),shell_quote(&format!("_expected_instance={instance}")))
  }else if !project.is_empty(){
   let attach=json!({"id":"project.attach","params":{"path":project}}).to_string();
   let status=json!({"id":"center.status","params":{}}).to_string();
   format!("printf '%s\\n' {} {} | {exe} run -",shell_quote(&attach),shell_quote(&status))
  }else{format!("{exe} exec project.catalog")};
  let commands=vec![
   ("Inspect selected project's Studio connection; never opens or switches",format!("{exe} exec ui.feedback {}",if project.is_empty(){String::new()}else{shell_quote(&format!("project={project}"))})),
   ("Read this project's next persisted action stages",format!("{exe} exec center.events after={cursor} limit=192 {}",if project.is_empty(){String::new()}else{shell_quote(&format!("project={project}"))})),
   ("Inspect project state; closed projects use read-only disk metadata",inspection),
   ("Read the actual transcript import contract; no processing",format!("{exe} commands workshop.import_transcript"))];
  commands
 }
 fn copy_command(&mut self){
  use std::io::Write;
  let commands=self.copy_commands();let text=&commands[self.command_selected%commands.len()].1;
  match std::process::Command::new("wl-copy").arg("--").stdin(std::process::Stdio::piped()).spawn(){
   Ok(mut child)=>{let written=child.stdin.take().ok_or_else(||io::Error::other("clipboard pipe unavailable")).and_then(|mut input|input.write_all(text.as_bytes()));let status=child.wait();self.message=if written.is_ok()&&status.is_ok_and(|s|s.success()){"COPIED command text only. Nothing executed.".into()}else{"Clipboard failed. Select terminal text and use Ctrl+Shift+C in Foot; nothing executed.".into()};},
   Err(_)=>self.message="Clipboard unavailable. Select terminal text and use Ctrl+Shift+C in Foot; nothing executed.".into()
  }
 }
}
// Hackerman: ~/.local/state/omarchy/current/theme/{colors.toml,foot.ini,btop.theme}.
// The theme remaps red/yellow to green/teal; keep distinct semantic red/amber below.
const DESK_BG: Color = Color::Rgb(11, 12, 22);
const DESK_FG: Color = Color::Rgb(221, 247, 255);
const DESK_LINE: Color = Color::Rgb(45, 52, 80);
const DESK_SELECTED: Color = Color::Rgb(31, 37, 58);

fn fit_text(text: &str, width: usize) -> String {
    if Line::raw(text).width() <= width { return text.into(); }
    if width == 0 { return String::new(); }
    let mut result = String::new();
    for ch in text.chars() {
        if Line::raw(format!("{result}{ch}…")).width() > width { break; }
        result.push(ch);
    }
    format!("{result}…")
}
fn compact_path(path: &str, width: usize) -> String {
    if Line::raw(path).width() <= width { return path.into(); }
    let mut tail = String::new();
    for ch in path.chars().rev() {
        if Line::raw(format!("…{ch}{tail}")).width() > width { break; }
        tail.insert(0, ch);
    }
    format!("…{tail}")
}
fn padded(text: &str, width: usize) -> String {
    let text = fit_text(text, width);
    format!("{text}{}", " ".repeat(width.saturating_sub(Line::raw(&text).width())))
}
fn wrapped_rows(lines: &[Line<'_>], width: u16) -> u16 {
    lines.iter().map(|line| line.width().max(1).div_ceil(usize::from(width.max(1))) as u16).sum()
}
fn round_panel(title: impl Into<String>, accent: Color, active: bool) -> Block<'static> {
    Block::default().borders(Borders::ALL)
        .border_type(if active { ratatui::widgets::BorderType::Thick } else { ratatui::widgets::BorderType::Rounded })
        .border_style(Style::default().fg(if active { accent } else { DESK_LINE }))
        .title(Span::styled(format!(" {} ", title.into()), Style::default().fg(accent).add_modifier(Modifier::BOLD)))
}
fn root_actions(c: &Center) -> Vec<&Value> {
    c.state["activity"]["actions"].as_array().into_iter().flatten().rev().take(3).collect()
}
fn stages(action: &Value) -> Vec<&Value> {
    action["events"].as_array().into_iter().flatten().collect()
}
fn target(event: &Value) -> &str {
    event["command"].as_str().or(event["control"].as_str()).unwrap_or("unmapped interaction")
}
fn root_start(action: &Value) -> Option<&Value> {
    let events = action["events"].as_array()?;
    events.iter().find(|e| e["stage"] == "dispatch_running" && e["request_id"] == action["root_request_id"])
        .or_else(|| events.iter().find(|e| e["stage"] == "dispatch_running"))
        .or_else(|| events.first())
}
fn root_finish(action: &Value) -> Option<&Value> {
    let start = root_start(action)?;
    action["events"].as_array()?.iter().rev().find(|e| {
        matches!(e["stage"].as_str(), Some("engine_success" | "engine_error" | "engine_needs_work" | "handler_error"))
            && (e["command"] == start["command"] || !start["command"].is_string())
            && (!start["request_id"].is_string() || e["request_id"] == start["request_id"])
    })
}
fn engine_badge(action: &Value) -> (&'static str, Color) {
    match root_finish(action).and_then(|e| e["stage"].as_str()) {
        Some("engine_success") => ("✓ ENGINE OK", GREEN),
        Some("engine_error" | "handler_error") => ("✗ FAILED", RED),
        Some("engine_needs_work") => ("! NEEDS WORK", AMBER),
        _ => ("! COMPLETION UNKNOWN", AMBER),
    }
}
fn action_ack(action: &Value) -> bool {
    stages(action).iter().any(|e| e["stage"] == "frontend_ack" && (
        e["request_id"].as_str().is_some_and(|id| Some(id) == root_start(action).and_then(|s| s["request_id"].as_str())) ||
        root_finish(action).is_some_and(|finish| finish["engine_revision"].is_u64() && finish["engine_revision"] == e["engine_revision"])
    ))
}
fn stamp(event: &Value) -> String {
    event["unix_micros"].as_u64().map(|us| {
        let s = us / 1_000_000; format!("{:02}:{:02}:{:02}", s / 3600 % 24, s / 60 % 60, s % 60)
    }).unwrap_or_else(|| "--:--:--".into())
}
fn revision(value: &Value) -> String {
    value.as_u64().map(|n| n.to_string()).unwrap_or_else(|| "unknown".into())
}
fn lag(ui: &Value, matched: bool) -> String {
    if !matched { return "lag unknown".into(); }
    match (ui["engine_revision"].as_u64(), ui["ack_engine_revision"].as_u64()) {
        (Some(engine), Some(ack)) if engine >= ack => format!("lag {} rev", engine - ack),
        _ => "lag unknown".into(),
    }
}
// Actual receipt counts in eight fixed one-second buckets. No audio/progress claim.
fn activity_wave(c: &Center) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut bins = [0usize; 8];
    let mut seen = std::collections::HashSet::new();
    for action in root_actions(c) {
        for event in stages(action) {
            if let (Some(us), Some(sequence)) = (event["unix_micros"].as_u64(), event["sequence"].as_u64()) {
                if seen.insert(sequence) {
                    let second = us / 1_000_000;
                    if second <= now && now - second < 8 { bins[7 - (now - second) as usize] += 1; }
                }
            }
        }
    }
    let levels = ['▁','▂','▃','▄','▅','▆','▇','█'];
    let wave: String = bins.iter().map(|n| levels[(*n).min(7)]).collect();
    format!("activity {wave} · {} receipts/8s", bins.iter().sum::<usize>())
}
fn action_lines(c: &Center, action: &Value, compact: bool) -> Vec<Line<'static>> {
    let Some(start) = root_start(action) else { return vec![styled("— action details unavailable", MUTED)]; };
    let first = action["first_sequence"].as_u64(); let last = action["last_sequence"].as_u64();
    let sequence = match (first,last) { (Some(a),Some(b)) if a != b => format!("#{a}–{b}"), (Some(a),_) => format!("#{a}"), _ => "#?".into() };
    let (engine, color) = engine_badge(action);
    let mut heading = styled(format!("{sequence}  {}  ·  {} UTC", target(start), stamp(start)), if color == RED { RED } else { BLUE });
    if c.flash_started.is_some_and(|at| at.elapsed() < Duration::from_millis(800)) && last.is_some_and(|seq| seq > c.flash_after) {
        heading = heading.style(Style::default().fg(if color == RED { RED } else { BLUE }).bg(DESK_SELECTED));
    }
    let ack = action_ack(action);
    let mut lines = vec![heading];
    if compact {
        lines.push(Line::from(vec![Span::styled(format!("  {engine}  "), Style::default().fg(color)),
            Span::styled(if ack { "▣ UI APPLIED" } else { "— NO UI RECEIPT" }, Style::default().fg(if ack { GREEN } else { MUTED }))]));
    } else {
        lines.push(styled(format!(" ├─ ENGINE: {engine}"), color));
        lines.push(styled(if ack { " ├─ STUDIO: ▣ DOM APPLIED · visual review separate" } else { " ├─ STUDIO: — no correlated UI receipt; valid for headless work" }, if ack { GREEN } else { MUTED }));
        lines.push(styled(format!(" └─ AGENT: {} · {}", start["origin"].as_str().unwrap_or("unknown"), start["client_label"].as_str().unwrap_or("unverified")), MUTED));
    }
    if let Some(error) = root_finish(action).and_then(|e| e["error"].as_str()).filter(|_| !compact) { lines.push(styled(format!("    {}", error.strip_prefix("invalid parameters: ").unwrap_or(error)), RED)); }
    lines
}
fn command_preview(command: &str) -> String {
    let exe = std::env::current_exe().ok().map(|p| shell_quote(&p.to_string_lossy()));
    exe.as_deref().and_then(|exe| command.strip_prefix(exe)).map(|rest| format!("<ac-cli>{rest}")).unwrap_or_else(|| command.into())
}
fn render_commands(f: &mut Frame, c: &Center, area: ratatui::layout::Rect, narrow: bool) {
    let actions = root_actions(c);
    let failure = actions.iter().find(|a| engine_badge(a).1 == RED);
    let tone = if failure.is_some() { RED } else { BLUE };
    let block = round_panel(if narrow { "WHAT TO DO NEXT · numbers select · C copies" } else { "WHAT TO DO NEXT · COPY COMMANDS · ↑↓ / numbers select" }, tone, true);
    let inner = block.inner(area); f.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 { return; }
    let commands = c.copy_commands();
    if commands.is_empty() { f.render_widget(Paragraph::new("— no commands supplied"), inner); return; }
    let selected = c.command_selected % commands.len();
    let compact_commands = narrow && inner.height < 20;
    let (effect, literal) = &commands[selected];
    let payload = vec![styled(format!("LITERAL PAYLOAD · {} bytes · C copies full text", literal.len()), MUTED), styled(literal.clone(), BLUE)];
    let payload_height = wrapped_rows(&payload, inner.width).min(if narrow { 4 } else { 6 }).min(inner.height.saturating_sub(3));
    let parts = Layout::vertical([Constraint::Min(1), Constraint::Length(payload_height)]).split(inner);
    let mut lines = Vec::new();
    if let Some(action) = failure {
        if let Some(start) = root_start(action) { lines.push(styled(format!("✗ BLOCKER: {}", target(start)), RED)); }
        if let Some(error) = root_finish(action).and_then(|e| e["error"].as_str()) { lines.push(styled(fit_text(error.strip_prefix("invalid parameters: ").unwrap_or(error), inner.width as usize), RED)); }
    } else if let Some(error) = c.state["ui"]["error"].as_str() { lines.push(styled(format!("! STUDIO: {}", fit_text(error, inner.width.saturating_sub(12) as usize)), AMBER)); }
    else { lines.push(styled("Inspection only · selection/C never executes · engine and UI receipts are separate", MUTED)); }
    if !compact_commands { lines.push(Line::raw("")); }
    let mut selected_row = 0;
    if compact_commands {
        lines.push(styled(format!("{}  ·  ↑↓ choose", (0..commands.len()).map(|i|format!("{}[{}]", if i == selected { "▶" } else { " " }, i + 1)).collect::<Vec<_>>().join(" ")), BLUE));
        lines.push(styled(format!("▶ [{}] {effect}", selected + 1), BLUE));
        lines.push(styled(format!("    {}", fit_text(&command_preview(literal), inner.width.saturating_sub(4) as usize)), MUTED));
    } else {
        for (i,(effect,command)) in commands.iter().enumerate() {
            if i == selected { selected_row = wrapped_rows(&lines, parts[0].width); }
            lines.push(styled(format!("{} [{}] {}", if i == selected { "▶" } else { " " }, i + 1, fit_text(effect, inner.width.saturating_sub(7) as usize)), if i == selected { BLUE } else { DESK_FG }));
            lines.push(styled(format!("       {}", fit_text(&command_preview(command), inner.width.saturating_sub(7) as usize)), BLUE));
            lines.push(Line::raw(""));
        }
    }
    let scroll = if compact_commands { 0 } else { selected_row.saturating_sub(parts[0].height.saturating_sub(5)) };
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0)), parts[0]);
    let payload_scroll = c.scroll.min(wrapped_rows(&payload, parts[1].width).saturating_sub(parts[1].height));
    f.render_widget(Paragraph::new(payload).wrap(Wrap { trim: false }).scroll((payload_scroll, 0)), parts[1]);
}
fn render_actions(f: &mut Frame, c: &Center, area: ratatui::layout::Rect, compact: bool) {
    let actions = root_actions(c);
    let mut lines = Vec::new();
    if actions.is_empty() { lines.push(styled("— No root actions recorded for this project.", MUTED)); }
    for (i, action) in actions.iter().enumerate() {
        if i > 0 && !compact { lines.push(Line::raw("")); }
        lines.extend(action_lines(c, action, compact));
    }
    let block = round_panel("LAST 3 ROOT ACTIONS · newest first · Pg scroll", MUTED, false);
    let inner = block.inner(area);
    let max_scroll = wrapped_rows(&lines, inner.width).saturating_sub(inner.height);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((c.scroll.min(max_scroll), 0)).block(block), area);
}
fn render_catalog(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let projects: Vec<&Value> = c.state["catalog"]["projects"].as_array().into_iter().flatten().collect();
    let block = round_panel("ALL PROJECTS · shared Home recent catalog", TEAL, false);
    let inner = block.inner(area); let width = inner.width as usize;
    let mut lines = vec![styled("Existing Home catalog · newest ten paths · readiness is not approval", MUTED)];
    let widths = if width >= 100 { [width * 24 / 100, width * 20 / 100, width * 25 / 100, width * 12 / 100, width * 19 / 100] } else { [width,0,0,0,0] };
    if width >= 100 { lines.push(styled(format!("{}{}{}{}{}", padded("PROJECT",widths[0]), padded("ENGINE",widths[1]), padded("STUDIO UI",widths[2]), padded("REV LAG",widths[3]), padded("LAST ACTION",widths[4])), MUTED)); }
    let actions = root_actions(c);
    for project in projects {
        let current = project["path"] == c.state["project"] && c.state["project"].is_string();
        let ui = &c.state["ui"];
        let matched = current && ui["active_project"] == c.state["project"] && ui["connection"] == "connected";
        let engine = if current { actions.first().map(|a|engine_badge(a)).unwrap_or(("— UNKNOWN", DESK_FG)) } else { ("— UNKNOWN", DESK_FG) };
        let ack = if matched && ui["ui_ready"] == true { ("▣ UI UPDATED",GREEN) } else { ("! ACK UNKNOWN / MISSING",AMBER) };
        let name = format!("{} {}", if current { "▶" } else { " " }, project["name"].as_str().unwrap_or("unnamed"));
        if width >= 100 {
            lines.push(Line::from(vec![Span::styled(padded(&name,widths[0]),Style::default().fg(if current { BLUE } else { DESK_FG })), Span::styled(padded(engine.0,widths[1]),Style::default().fg(engine.1)), Span::styled(padded(ack.0,widths[2]),Style::default().fg(ack.1)), Span::styled(padded(&lag(ui,matched),widths[3]),Style::default().fg(if matched && ui["ui_ready"] == true { GREEN } else { AMBER })), Span::styled(padded(if current { actions.first().and_then(|a|root_start(a)).map(target).unwrap_or("—") } else { "— unobserved" },widths[4]),Style::default().fg(MUTED))]));
        } else {
            lines.push(styled(name, BLUE));
            lines.push(Line::from(vec![Span::styled(format!("  {}  ",engine.0),Style::default().fg(engine.1)), Span::styled(ack.0,Style::default().fg(ack.1))]));
        }
        lines.push(styled(format!("  └─ {}",compact_path(project["path"].as_str().unwrap_or("path unavailable"), width.saturating_sub(5))),MUTED));
        if let Some(next) = project["readiness"]["next_step"].as_str() { lines.push(styled(format!("     {}",fit_text(next,width.saturating_sub(5))),MUTED)); }
        lines.push(Line::raw(""));
    }
    if lines.len() <= 2 { lines.push(styled("— No projects returned by the shared Home catalog.",MUTED)); }
    if let Some(error) = c.state["catalog"]["error"].as_str() { lines.push(styled(format!("✗ Catalog: {error}"),RED)); }
    let scroll = c.scroll.min(wrapped_rows(&lines,inner.width).saturating_sub(inner.height));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll,0)).block(block),area);
}
fn render_desk(f: &mut Frame, c: &Center, control: bool) {
    let area = f.area(); let narrow = area.width < 110;
    f.render_widget(Block::default().style(Style::default().bg(DESK_BG).fg(DESK_FG)),area);
    let chunks = Layout::vertical([Constraint::Length(if narrow { 6 } else { 5 }),Constraint::Min(1),Constraint::Length(if area.height >= 28 { 2 } else { 1 })]).split(area);
    let project = c.state["project"].as_str();
    let name = project.and_then(|p|std::path::Path::new(p).file_stem()).and_then(|n|n.to_str()).unwrap_or("No project");
    let ui = &c.state["ui"];
    let matched = project.is_some() && ui["connection"] == "connected" && ui["active_project"].as_str() == project;
    let mismatch = project.is_some() && ui["active_project"].is_string() && ui["active_project"].as_str() != project;
    let actions = root_actions(c);
    let engine = actions.first().map(|a|engine_badge(a)).unwrap_or(("— UNKNOWN", DESK_FG));
    let projects = c.state["catalog"]["projects"].as_array().map(Vec::len).unwrap_or(0);
    let mut tabs = vec![Span::styled(format!(" [F1] CONTROL ({projects}) "),Style::default().fg(if control { TEAL } else { MUTED }).add_modifier(if control { Modifier::BOLD } else { Modifier::empty() })), Span::styled("┬",Style::default().fg(DESK_LINE)), Span::styled(format!(" [F2] BOOTH: {}{} ",fit_text(name,if narrow { 12 } else { 24 }),if engine.1 == RED { " [!]" } else { "" }),Style::default().fg(if control { MUTED } else if engine.1 == RED { RED } else { TEAL }).add_modifier(if control { Modifier::empty() } else { Modifier::BOLD })), Span::styled("┬ [F4] INTAKE ",Style::default().fg(MUTED))];
    if area.width >= 118 { tabs.push(Span::styled(" · snapshots 1s · ",Style::default().fg(MUTED))); }
    let mut header = vec![Line::from(vec![Span::styled(if control { " WORKSPACE: " } else { " FILE: " },Style::default().fg(MUTED)),Span::styled(compact_path(if control { c.state["catalog"]["scope"].as_str().unwrap_or("shared Home recent projects") } else { project.unwrap_or("no project attached") },area.width.saturating_sub(16) as usize),Style::default().fg(DESK_FG))]),Line::from(vec![Span::styled(format!(" ENGINE: {}  │  ",engine.0),Style::default().fg(engine.1)),Span::styled(format!("STUDIO: {} · {}",if mismatch { "✗ UNMATCHED" } else if matched { "✓ MATCHED" } else { "— SELECTED PROJECT NOT OPEN / UNOBSERVED" },lag(ui,matched)),Style::default().fg(if mismatch { RED } else if matched && ui["ui_ready"] == true { GREEN } else { AMBER }))])];
    let ack = if !matched { "UI RECEIPT: N/A · no matched Studio" } else if ui["ui_ready"] == true { "▣ ACK APPLIED" } else { "! ACK PENDING / UNKNOWN" };
    let busy = matched && ui["pending_invokes"].as_u64().is_some_and(|n| n > 0);
    let work = if busy { format!(" · {} RUNNING", ["|", "/", "-", "\\"][(c.started.elapsed().as_millis() / 120 % 4) as usize]) } else { String::new() };
    let status = format!(" {ack} · engine rev {} / ack rev {} · unsaved {}{work}",revision(&ui["engine_revision"]),revision(&ui["ack_engine_revision"]),ui["unsaved_state"].as_str().unwrap_or("unknown"));
    if narrow {
        let short_rev = |v: &Value| v.as_u64().map(|n|n.to_string()).unwrap_or_else(||"?".into());
        header.push(styled(format!(" {} · rev {}/{} · {} · unsaved {}", if matched && ui["ui_ready"] == true { "▣ UI APPLIED" } else { "— NO UI RECEIPT" }, short_rev(&ui["engine_revision"]), short_rev(&ui["ack_engine_revision"]), lag(ui,matched), ui["unsaved_state"].as_str().unwrap_or("unknown")),if matched && ui["ui_ready"] == true { GREEN } else { AMBER }));
    }
    else { header.push(Line::from(vec![Span::styled(fit_text(&status,area.width.saturating_sub(42) as usize),Style::default().fg(if matched && ui["ui_ready"] == true { GREEN } else { AMBER })),Span::styled(format!("  {}",activity_wave(c)),Style::default().fg(TEAL))])); }
    if narrow { header.push(styled(activity_wave(c),TEAL)); }
    let block = Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded).border_style(Style::default().fg(DESK_LINE)).title(Line::from(tabs));
    f.render_widget(Paragraph::new(header).block(block),chunks[0]);
    if control {
        let parts = Layout::vertical([Constraint::Min(5),Constraint::Length(if area.height >= 35 { 11 } else { 8 })]).split(chunks[1]);
        render_catalog(f,c,parts[0]); render_commands(f,c,parts[1],true);
    } else if narrow {
        let commands_height = chunks[1].height.saturating_sub(if chunks[1].height >= 22 { 11 } else { 8 }).max(5);
        let parts = Layout::vertical([Constraint::Length(commands_height),Constraint::Min(1)]).split(chunks[1]);
        render_commands(f,c,parts[0],true); render_actions(f,c,parts[1],true);
    } else {
        let parts = Layout::horizontal([Constraint::Percentage(47),Constraint::Percentage(53)]).spacing(1).split(chunks[1]);
        render_actions(f,c,parts[0],false); render_commands(f,c,parts[1],false);
    }
    let keys = if area.width < 90 { "F1 Control F2 Booth F4 Inputs 1–9 Select C Copy Pg Scroll ^Q Exit" } else { "[F1] Control  [F2] Booth  [F4] Intake   [1–9 / ↑↓] Select  [C] Copy literal   [PgUp/Dn] Scroll  [F8] Refresh  [Ctrl+Q] Exit" };
    let mut footer = vec![styled(keys,BLUE)];
    if chunks[2].height > 1 { footer.push(styled(c.message.clone(),if c.message.starts_with("COPIED") || c.message.starts_with("COMMAND SUCCEEDED") { GREEN } else if c.message.starts_with("BLOCKED") || c.message.contains("failed") { RED } else { MUTED })); }
    f.render_widget(Paragraph::new(footer),chunks[2]);
}
fn render(f: &mut Frame,c: &Center) {
    if c.show_form { render_form(f,c); return; }
    render_desk(f,c,c.control_view || !c.state["project"].is_string());
}

pub fn start(p: Value, print: bool) -> Result<(), String> {
    let mut c = Center::new(&p);
    if !c.fields[0].is_empty() {
        c.session.execute("project.attach",json!({"path":c.fields[0]})).map_err(|e|e.to_string())?;
        c.refresh();
    } else {
        c.refresh();
    }
    if print {
        println!(
            "ANIMATIONCRAFT / COMMAND CENTER\nProject: {}\nTranscript valid: {}\n{}\n{}",
            c.state["project"],
            c.state["transcript"]["valid"],
            c.message,
            c.details()
        );
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("Command Center needs an interactive terminal. Use center --print for inspection or run - for JSONL commands.".into());
    }
    let mut terminal = ratatui::init();
    let result = (|| -> io::Result<()> {
        let mut refreshed=std::time::Instant::now();
        loop {
            if refreshed.elapsed()>Duration::from_secs(1){c.refresh();refreshed=std::time::Instant::now();}
            terminal.draw(|f| render(f, &c))?;
            if !event::poll(Duration::from_millis(120))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
                break;
            }
            if key.code==KeyCode::F(1){c.control_view=true;c.show_form=false;continue;}
            if key.code==KeyCode::F(2) && !c.show_form{c.control_view=false;continue;}
            if key.code==KeyCode::F(4){c.show_form=!c.show_form;continue;}
            if !c.show_form {
                match key.code{
                    KeyCode::Char(n @ '1'..='9') if !key.modifiers.contains(KeyModifiers::CONTROL)=>{let index=(n as usize)-('1' as usize);if index<c.copy_commands().len(){c.command_selected=index;}},
                    KeyCode::Up=>c.command_selected=c.command_selected.saturating_sub(1),
                    KeyCode::Down|KeyCode::Tab=>c.command_selected=(c.command_selected+1)%c.copy_commands().len(),
                    KeyCode::Char('c')=>c.copy_command(),
                    KeyCode::PageDown=>c.scroll=c.scroll.saturating_add(4),
                    KeyCode::PageUp=>c.scroll=c.scroll.saturating_sub(4),
                    KeyCode::F(8)=>{c.pinned_instance=None;c.refresh();},
                    _=>{}
                }
                continue;
            }
            if c.editing {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => c.editing = false,
                    KeyCode::Backspace => {
                        c.fields[c.selected].pop();
                    }
                    KeyCode::Char(x) => c.fields[c.selected].push(x),
                    _ => {}
                }
                continue;
            }
            match key.code {
                KeyCode::Up => c.selected = c.selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Tab => c.selected = (c.selected + 1) % 8,
                KeyCode::Enter => c.editing = true,
                KeyCode::PageDown => c.scroll = c.scroll.saturating_add(6),
                KeyCode::PageUp => c.scroll = c.scroll.saturating_sub(6),
                KeyCode::F(2) => {
                    c.fields[0].clear();
                    c.selected = 2;
                    c.message="NEW PROJECT: choose name/root, retain or replace the displayed source inputs, then F5. Existing project remains saved.".into();
                }
                KeyCode::F(3) => c.execute("project.open", json!({"path":c.fields[0]})),
                KeyCode::F(5) => c.intake(false),
                KeyCode::F(6) => c.intake(true),
                KeyCode::F(7) => {
                    if c.state["transcript"]["valid"] == true {
                        c.execute("storyboard.init", json!({}));
                    } else {
                        c.message="BLOCKED: import or repair a timed transcript first. Received files are preserved.".into();
                    }
                }
                KeyCode::F(8) => {c.pinned_instance=None;c.refresh();},
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    c.execute("project.save", json!({}))
                }
                _ => {}
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result.map_err(|e| e.to_string())
}
