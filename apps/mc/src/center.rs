//! Terminal presentation only. All project operations execute the shared engine registry.
mod booth;
mod control;
mod panels;
mod roles;
mod telemetry_view;
mod workshop;
use telemetry_view::*;
pub(crate) use crate::theme::*;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use magicstudio_engine::Session;
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
    path::Path,
    time::Duration,
};
const LABELS: [&str; 8] = [
    "Existing project (.magic)",
    "New song root",
    "Song name",
    "Existing audio",
    "Transcript JSON",
    "Original captions SRT (optional)",
    "Existing audiomap (optional)",
    "Notes / references (; separated)",
];
/// A message waiting to be sent to an agent pane.
#[derive(Clone)]
struct Poke {
    target: String,
    name: String,
    text: String,
}

#[derive(Clone)]
struct Hit {
    id: String,
    label: String,
    area: ratatui::layout::Rect,
    action: String,
    params: Value,
    enabled: bool,
}
struct Center {
    session: Session,
    scope: Option<crate::scope::Scope>,
    onboarding: Option<Value>,
    fields: [String; 8],
    selected: usize,
    editing: bool,
    state: Value,
    github_health: Value,
    github_health_receiver: Option<std::sync::mpsc::Receiver<Value>>,
    message: String,
    agent_gang: Value,
    agent_gang_at: Option<std::time::Instant>,
    studios: Value,
    studios_at: Option<std::time::Instant>,
    studios_receiver: Option<std::sync::mpsc::Receiver<Value>>,
    telemetry_view: bool,
    config_view: bool,
    dials: Option<crate::dials::Host>,
    dials_at: Option<std::time::Instant>,
    metrics_at: Option<std::time::Instant>,
    proc_snapshot: Vec<crate::telemetry::Proc>,
    mcp_servers: Vec<crate::mcp_watch::Server>,
    mcp_sessions: Vec<Value>,
    touches: Vec<Value>,
    skills: Vec<Value>,
    settings: crate::settings::TuiSettings,
    procs: Vec<crate::dials::ProcRow>,
    history: Vec<f64>,
    logs: Value,
    logs_at: Option<std::time::Instant>,
    usage: Vec<Value>,
    gang_selected: usize,
    event_index: usize,
    event_focus: bool,
    event_open: bool,
    event_detail: Option<Value>,
    hits: std::cell::RefCell<Vec<Hit>>,
    poke: Option<Poke>,
    muppets_view: bool,
    catalog_selected: usize,
    catalog_show_inactive: Option<bool>,
    catalog_columns: std::cell::Cell<usize>,
    spy: Option<String>,
    spy_at: Option<std::time::Instant>,
    pulse: Option<crate::telemetry::Sample>,
    pulse_at: Option<std::time::Instant>,
    scroll: u16,
    show_form: bool,
    control_view: bool,
    command_selected: usize,
    seen_cursor: u64,
    flash_after: u64,
    flash_started: Option<std::time::Instant>,
    pinned_instance: Option<String>,
    stage_selected: usize,
    details_open: bool,
    chat_draft: String,
    chat_draft_project: Option<String>,
    chat_editing: bool,
    chat_sent: String,
    chat_sent_request: Option<String>,
    doctor_receiver: Option<std::sync::mpsc::Receiver<Result<Value, String>>>,
    doctor_error: Option<String>,
    chat_receiver: Option<std::sync::mpsc::Receiver<Result<Value, String>>>,
    panel_config: magicstudio_engine::panels::Config,
    panel_config_error: Option<String>,
    panel_report: magicstudio_engine::panels::PanelReport,
    last_panels_revision: Option<u64>,
    panel_rows: Vec<Line<'static>>,
    panel_detail: Vec<Line<'static>>,
}
impl Center {
    fn new(p: &Value) -> Result<Self, String> {
        let value = |k: &str| p[k].as_str().unwrap_or_default().to_string();
        let root = magicstudio_engine::project_settings::root().map_err(|e| e.to_string())?;
        let mut center = Self {
            session: Session::user_session(),
            scope: None,
            onboarding: None,
            fields: [
                value("project"),
                p["root"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| root.to_string_lossy().into_owned()),
                value("name"),
                value("audio"),
                value("transcript"),
                value("captions"),
                value("audiomap"),
                value("materials"),
            ],
            selected: 0,
            editing: false,
            show_form: false,
            control_view: false,
            command_selected: 0,
            seen_cursor: 0,
            flash_after: 0,
            flash_started: None,
            pinned_instance: None,
            stage_selected: 0,
            details_open: false,
            chat_draft: String::new(),
            chat_draft_project: None,
            chat_editing: false,
            chat_sent: String::new(),
            chat_sent_request: None,
            chat_receiver: None,
            doctor_receiver: None,
            doctor_error: None,
            state: json!({}),
            github_health: crate::github_health::empty(),
            github_health_receiver: None,
            agent_gang: json!([]),
            agent_gang_at: None,
            studios: json!({"sessions":[],"errors":[]}),
            studios_at: None,
            studios_receiver: None,
            telemetry_view: false,
            config_view: false,
            dials: None,
            dials_at: None,
            metrics_at: None,
            proc_snapshot: Vec::new(),
            mcp_servers: Vec::new(),
            mcp_sessions: Vec::new(),
            touches: Vec::new(),
            skills: Vec::new(),
            settings: crate::settings::TuiSettings::load(),
            procs: Vec::new(),
            history: Vec::new(),
            logs: json!({"blender":[],"studio":[],"omarchy":[],"commits":[]}),
            logs_at: None,
            usage: crate::dials::agent_usage(),
            gang_selected: 0,
            event_index: 0,
            event_focus: false,
            event_open: false,
            event_detail: None,
            hits: Default::default(),
            poke: None,
            muppets_view: false,
            catalog_selected: 0,
            catalog_show_inactive: None,
            catalog_columns: std::cell::Cell::new(1),
            pulse: None,
            pulse_at: None,
            spy: None,
            spy_at: None,
            message: format!(
                "Blue not started / red failed / yellow stalled / green working / [OK] completed. Production root: {}.",
                root.display()
            ),
            scroll: 0,
            panel_config: magicstudio_engine::panels::Config::default(),
            panel_config_error: None,
            panel_report: magicstudio_engine::panels::PanelReport::default(),
            last_panels_revision: None,
            panel_rows: Vec::new(),
            panel_detail: Vec::new(),
        };
        match panels::load_config() {
            Ok(config) => center.panel_config = config,
            Err(error) => center.panel_config_error = Some(error),
        }
        center.scope = crate::scope::current()?.cloned();
        if let Some(scope) = &center.scope {
            if !center.fields[0].is_empty()
                && std::fs::canonicalize(&center.fields[0]).map_err(|e| e.to_string())?
                    != scope.project
            {
                return Err("Restricted: Booth project differs from assignment".into());
            }
            center.fields[0] = scope.project.to_string_lossy().into_owned();
            center.fields[1] = scope
                .project
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            center.control_view = false;
            center.message = "Assigned project Booth · BOOTH / MOPS / MAYHEM".into();
        } else {
            center.control_view = true;
        }
        Ok(center)
    }
    fn catalog(&mut self) -> Value {
        if let Some(scope) = &self.scope {
            scope.catalog()
        } else {
            self.session
                .execute("project.catalog", json!({}))
                .unwrap_or_else(|e| json!({"error":e.to_string()}))
        }
    }
    fn refresh(&mut self) {
        self.poll_github_health();
        // No project is attached: the booth waits in the workshop for you to choose one.
        if self.session.project_path.is_none() {
            self.control_view = true;
        }
        let project = self
            .session
            .project_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        if self.state["project"].as_str() != project.as_deref() {
            let catalog = self.state["catalog"].clone();
            self.state = json!({"project":project,"catalog":catalog,"agent_gang":self.agent_gang});
            self.pinned_instance = None;
        }
        let observe = |pin: Option<&str>| {
            crate::local_ui::call(
                "auto",
                "ui.feedback",
                json!({"project":project,"_expected_instance":pin.map(str::to_owned)}),
            )
            .unwrap_or_else(|e| json!({"connection":"error","error":e}))
        };
        let mut ui = observe(self.pinned_instance.as_deref());
        // Observation is read-only and project-matched, so a Studio that restarted under us is
        // re-discovered rather than pinned out. Commands still refuse to switch a live Studio.
        if ui["connection"] != "connected" && self.pinned_instance.is_some() {
            self.pinned_instance = None;
            ui = observe(None);
        }
        // Keep the last project inspection while the engine is busy; read its real journal directly.
        let inspection = if ui["connection"] == "connected"
            && ui["engine_unsaved"].is_null()
            && self.state["project"].is_string()
        {
            let mut value = self.state.clone();
            value["activity"] = self
                .session
                .execute("backstage.events", json!({"project":project,"limit":192}))
                .unwrap_or_else(|e| json!({"error":e.to_string()}));
            Ok(value)
        } else {
            crate::dispatch(
                &mut self.session,
                "backstage.status",
                json!({"_origin":"Booth inspection","_expected_instance":self.pinned_instance}),
            )
        };
        match inspection {
            Ok(mut value) => {
                if self.session.project_path.is_some() && ui["engine_unsaved"].is_boolean() {
                    let params = json!({"_expected_instance":self.pinned_instance});
                    match crate::dispatch(
                        &mut self.session,
                        "agent.log",
                        json!({"refresh":false,"_expected_instance":self.pinned_instance}),
                    ) {
                        Ok(turns) => {
                            value["chat_turns"] = turns;
                        }
                        Err(error) => {
                            value["chat_error"] = json!(error);
                        }
                    }
                    value["chat"] = crate::dispatch(&mut self.session, "agent.session", params)
                        .unwrap_or_else(|error| json!({"error":error}));
                } else {
                    value["chat"] = self.state["chat"].clone();
                    value["chat_turns"] = self.state["chat_turns"].clone();
                }
                value["ui"] = ui;
                if self.pinned_instance.is_none() {
                    self.pinned_instance = value["ui"]["instance"].as_str().map(str::to_owned);
                }
                value["catalog"] = self.catalog();
                workshop::reconcile(self, &value["catalog"]);
                value["animation_worker"] = self.state["animation_worker"].clone();
                let live = crate::snapshot::live(&value["ui"], &self.state["snapshot"]);
                value["snapshot"] = crate::snapshot::build(
                    value["project"].as_str(),
                    &value["ui"],
                    &live,
                    &value["activity"],
                );
                value["tail"] = merge_tail(
                    &self.state["tail"],
                    crate::snapshot::tail(
                        &value["activity"],
                        value["snapshot"]["project"].as_str(),
                    ),
                );
                let cursor = value["activity"]["next_cursor"].as_u64().unwrap_or(0);
                if self.seen_cursor > 0 && cursor > self.seen_cursor {
                    self.flash_after = self.seen_cursor;
                    self.flash_started = Some(std::time::Instant::now());
                }
                self.seen_cursor = cursor;
                self.state = value;
                let visible = workshop::order(self);
                if !visible.contains(&self.catalog_selected)
                    && let Some(index) = visible.first()
                {
                    self.catalog_selected = *index;
                }
                // Which Studio instances exist, and what each one has open. The roster
                // needs real evidence per project instead of a placeholder, so this is
                // read off the instances themselves and refreshed on its own cadence.
                // The telemetry tab is on demand: nothing is sampled while it is closed,
                // so the dials can be lavish without costing anything the rest of the time.
                if self.telemetry_view && self.scope.is_none() {
                    if self
                        .dials_at
                        .map(|t| t.elapsed().as_secs() >= 1)
                        .unwrap_or(true)
                    {
                        let current = crate::telemetry::all_procs();
                        self.procs = crate::dials::top_processes(&self.proc_snapshot, &current, 18);
                        self.proc_snapshot = current;
                        self.mcp_servers = crate::mcp_watch::scan();
                        if let Some(host) = crate::dials::sample_host(self.dials.as_ref()) {
                            if let Some(cpu) = host.cpu_pct {
                                self.history.push(cpu);
                                if self.history.len() > 240 {
                                    self.history.remove(0);
                                }
                            }
                            self.dials = Some(host);
                        }
                        self.dials_at = Some(std::time::Instant::now());
                    }
                    // One measured sample per minute: enough to see a trend, cheap to keep.
                    if self
                        .metrics_at
                        .map(|t| t.elapsed().as_secs() >= 60)
                        .unwrap_or(true)
                    {
                        let studios = self.studios["sessions"]
                            .as_array()
                            .map(|v| v.len())
                            .unwrap_or(0);
                        let stale = self.studios["errors"]
                            .as_array()
                            .map(|v| v.len())
                            .unwrap_or(0);
                        crate::mayhem::record(&crate::mayhem::Sample::capture(
                            &self.mcp_servers,
                            studios,
                            stale,
                        ));
                        self.metrics_at = Some(std::time::Instant::now());
                    }
                    if self
                        .logs_at
                        .map(|t| t.elapsed().as_secs() >= 5)
                        .unwrap_or(true)
                    {
                        let blender = self.state["animation_worker"]["log"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let studio: Vec<String> = self.state["tail"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .rev()
                            .take(8)
                            .map(|e| {
                                format!(
                                    "#{} {} {}",
                                    e["sequence"],
                                    e["type"].as_str().unwrap_or("?"),
                                    e["command"].as_str().unwrap_or("")
                                )
                            })
                            .collect();
                        self.logs = json!({
                            "blender":crate::dials::file_tail(&blender,10),
                            "studio":studio,
                            "omarchy":crate::dials::journal_tail(10),
                            "commits":crate::dials::commits(&std::env::current_dir().unwrap_or_default(),8),
                        });
                        self.usage = crate::dials::agent_usage();
                        self.logs_at = Some(std::time::Instant::now());
                    }
                }
                self.poll_studios();
                if self.chat_sent_request.is_some()
                    && self.message.starts_with("Chat ")
                    && !self.message.starts_with("Chat error:")
                    && !self.message.starts_with("Chat blocked:")
                    && self.chat_receiver.is_none()
                {
                    self.message = chat_outcome(&self.state);
                }
                self.observe_chat_delivery();
            }
            Err(error) => {
                let catalog = self.catalog();
                workshop::reconcile(self, &catalog);
                self.state["catalog"] = catalog;
                self.message = error.clone();
                self.state["ui"] = json!({"connection":"stale_or_unavailable","ui_ready":false,"error":error,"recovery":"F8 explicitly reattaches the same project; Studio is never switched."});
            }
        }
        if self.muppets_view
            && self
                .agent_gang_at
                .map(|t| t.elapsed().as_secs() >= self.settings.agent_refresh_secs)
                .unwrap_or(true)
        {
            self.usage = crate::dials::agent_usage();
            if let Ok(out) = std::process::Command::new("herdr")
                .args(["agent", "list"])
                .output()
            {
                if let Ok(v) = serde_json::from_slice::<Value>(&out.stdout) {
                    let mut agents = v["result"]["agents"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    if let Some(scope) = &self.scope {
                        agents = scope.roster(agents);
                    }
                    self.agent_gang = json!(agents);
                    self.agent_gang_at = Some(std::time::Instant::now());
                }
            }
        }
        crate::studios::annotate_agents(&mut self.agent_gang, &self.studios);
        self.state["agent_gang"] = self.agent_gang.clone();
        self.sample_studio();
        panels::sync(self);
        if let Some(scope) = &self.scope {
            self.state = scope.redact(self.state.clone());
        }
    }
    fn refresh_github_health(&mut self) {
        self.github_health = crate::github_health::empty();
    }
    fn poll_github_health(&mut self) {}
    fn poll_studios(&mut self) {
        self.studios = magicstudio_engine::studio_sessions::discover();
        self.studios_at = Some(std::time::Instant::now());
    }
    fn studio_pid(&self) -> Option<u32> {
        let ui = &self.state["ui"];
        if ui["connection"] != "connected"
            || !self.state["project"].is_string()
            || ui["active_project"] != self.state["project"]
        {
            return None;
        }
        let instance = ui["instance"].as_str()?;
        if self
            .pinned_instance
            .as_deref()
            .is_some_and(|pin| pin != instance)
        {
            return None;
        }
        crate::telemetry::pid_from_socket(std::path::Path::new(instance))
    }
    fn sample_studio(&mut self) {
        let Some(pid) = self.studio_pid() else {
            self.pulse = None;
            self.pulse_at = None;
            return;
        };
        let instance = self.state["ui"]["instance"].as_str().unwrap_or_default();
        let changed = self
            .pulse
            .as_ref()
            .is_none_or(|p| p.pid != pid || p.instance != instance);
        if changed
            || self
                .pulse_at
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
        {
            self.pulse = crate::telemetry::Sample::capture(instance, self.pulse.as_ref());
            self.pulse_at = Some(std::time::Instant::now());
        }
    }
    fn studio_sample(&self) -> Option<&crate::telemetry::Sample> {
        self.pulse.as_ref().filter(|p| {
            Some(p.pid) == self.studio_pid()
                && self.state["ui"]["instance"].as_str() == Some(p.instance.as_str())
                && p.at.elapsed() < Duration::from_secs(6)
        })
    }
    /// Which view the desk is showing, so a restart can come back to it.
    fn view_name(&self) -> &'static str {
        if self.telemetry_view {
            "animal"
        } else if self.muppets_view {
            "mops"
        } else if self.scope.is_none() && (self.control_view || !self.state["project"].is_string())
        {
            "workshop"
        } else {
            "booth"
        }
    }
    /// The agent the gang strip has selected, with its pane and its live state.
    fn selected_agent(&self) -> Option<&Value> {
        self.agent_gang
            .as_array()
            .and_then(|agents| agents.get(self.gang_selected))
            .filter(|agent| agent["name"].is_string())
    }
    fn cycle_agent(&mut self, step: isize) {
        let count = self.agent_gang.as_array().map_or(0, Vec::len) + 1;
        self.gang_selected =
            (self.gang_selected as isize + step).rem_euclid(count as isize) as usize;
        self.spy = None;
        self.spy_at = None;
        self.scroll = 0;
    }
    fn pokeable(_status: &str) -> bool {
        false
    }
    fn begin_poke(&mut self) {
        self.message = "TUI is for reviewing data and settings; it cannot send prompts to agents.".into();
    }
    fn check_in_all(&mut self) {
        self.message = "TUI is for reviewing data and settings; it cannot send prompts to agents.".into();
    }
    fn quick_poke(&mut self) {
        self.message = "TUI is for reviewing data and settings; it cannot send prompts to agents.".into();
    }
    fn send_poke(&mut self, _target: &str, _text: &str) -> bool {
        self.message = "TUI is for reviewing data and settings; it cannot send prompts to agents.".into();
        false
    }
    /// Attach the project chosen in the workshop and watch it from the booth.
    fn observe_selected(&mut self) {
        let Some(path) = self.state["catalog"]["projects"]
            .as_array()
            .and_then(|p| p.get(self.catalog_selected))
            .and_then(|p| p["path"].as_str().map(str::to_owned))
        else {
            return;
        };
        if let Err(error) = self.session.execute("project.attach", json!({"path":path})) {
            self.message = format!("Project observation failed: {error}");
            return;
        }
        self.fields[0] = path;
        self.message.clear();
        let catalog = self.state["catalog"].clone();
        self.state =
            json!({"project":self.fields[0],"catalog":catalog,"agent_gang":self.agent_gang});
        self.control_view = false;
        self.details_open = false;
        self.command_selected = 0;
        self.muppets_view = false;
        self.telemetry_view = false;
        self.event_open = false;
        self.event_focus = false;
        self.event_detail = None;
        self.state["tail"] = json!([]);
        self.state["snapshot"] = Value::Null;
        self.scroll = 0;
        self.pinned_instance = None;
        self.refresh();
    }
    fn observe_chat_delivery(&mut self) {
        let receipt = &self.state["chat"]["request"];
        if self.chat_sent_request.as_deref() == receipt["request_id"].as_str()
            && self.chat_sent_request.is_some()
            && matches!(
                receipt["status"].as_str(),
                Some("running" | "replied" | "stalled")
            )
        {
            if self.chat_draft == self.chat_sent {
                self.chat_draft.clear();
            }
            self.message = chat_outcome(&self.state);
        }
    }
    fn poll_chat(&mut self) {
        if let Some(result) = self.chat_receiver.as_ref().and_then(|r| r.try_recv().ok()) {
            self.chat_receiver = None;
            self.message = match result {
                Ok(_r) => {
                    if self.chat_draft == self.chat_sent {
                        self.chat_draft.clear();
                    }
                    chat_outcome(&self.state)
                }
                Err(e) => format!(
                    "Chat error: {e}. Draft retained; inspect existing receipt before retry."
                ),
            };
            self.refresh();
        }
    }
    fn send_chat(&mut self) {
        self.message = "TUI is for reviewing data and settings; it cannot send prompts to agents.".into();
    }
    fn poll_doctor(&mut self) {
        if let Some(result) = self
            .doctor_receiver
            .as_ref()
            .and_then(|r| r.try_recv().ok())
        {
            self.doctor_receiver = None;
            match result {
                Ok(value) => {
                    self.state["storyboard_doctor"] = value;
                    self.doctor_error = None;
                }
                Err(e) => {
                    self.doctor_error = Some(e.clone());
                    self.message = format!("Doctor check failed: {e}");
                }
            }
            self.refresh();
        }
    }
    fn check_storyboard(&mut self) {
        if self.doctor_receiver.is_some() {
            return;
        }
        self.stage_selected = STORYBOARD_STAGE;
        self.details_open = true;
        self.doctor_error = None;
        let (Some(endpoint), Some(project), Some(instance)) = (
            self.state["ui"]["endpoint"].as_str(),
            self.state["project"].as_str(),
            self.pinned_instance.as_deref(),
        ) else {
            self.doctor_error =
                Some("No matched Studio project; F8 reattach before checking".into());
            return;
        };
        if self.state["ui"]["active_project"].as_str() != Some(project) {
            self.doctor_error = Some("Studio project differs; no check launched".into());
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            self.doctor_error = Some("Current CLI executable unavailable".into());
            return;
        };
        let assignment = self.scope.clone();
        let endpoint = endpoint.to_owned();
        let project = project.to_owned();
        let instance = instance.to_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        self.doctor_receiver = Some(rx);
        std::thread::spawn(move || {
            let mut command = std::process::Command::new(exe);
            if let Some(scope) = assignment {
                scope.inherit(&mut command);
            }
            let result = command
                .args([
                    "--bridge",
                    &endpoint,
                    "exec",
                    "storyboard.doctor",
                    &format!("_expected_project={project}"),
                    &format!("_expected_instance={instance}"),
                ])
                .output()
                .map_err(|e| e.to_string())
                .and_then(|o| {
                    serde_json::from_slice::<Value>(&o.stdout)
                        .map_err(|_| String::from_utf8_lossy(&o.stderr).into_owned())
                });
            let _ = tx.send(result);
        });
    }
    fn execute(&mut self, id: &str, mut p: Value) {
        if id == "project.open" {
            self.pinned_instance = None;
        } else if let Some(instance) = &self.pinned_instance {
            p["_expected_instance"] = json!(instance);
        }
        p["_origin"] = json!("Booth action");
        p["_client"] = json!("Booth supplied label");
        match crate::dispatch(&mut self.session, id, p) {
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
                    if id == "project.intake" {
                        format!(
                            "INTAKE {}. Native storyboard: {}. Studio verification: pending.",
                            if v["intake_ready"] == true {
                                "READY"
                            } else {
                                "NEEDS WORK"
                            },
                            if v["storyboard_ready"] == true {
                                "created or reopened"
                            } else {
                                "not ready"
                            }
                        )
                    } else {
                        format!(
                            "COMMAND SUCCEEDED: {id}. This is not storyboard or Studio readiness."
                        )
                    }
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
        for event in self.state["activity"]["events"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .take(8)
        {
            text.push_str(&format!(
                "#{} {} · {} · {} · {}\n",
                event["sequence"],
                event["client_label"].as_str().unwrap_or("UI"),
                event["stage"].as_str().unwrap_or_default(),
                event["command"].as_str().unwrap_or_default(),
                event["error"].as_str().unwrap_or_default()
            ));
        }
        text.push_str("STUDIO → CLI (source verified)\n");
        for map in self.state["ui_mapping"].as_array().into_iter().flatten() {
            text.push_str(&format!(
                "{} → {}\n{}\n",
                map["control"].as_str().unwrap_or_default(),
                map["command"].as_str().unwrap_or_default(),
                map["status"].as_str().unwrap_or_default()
            ));
        }
        text.push_str("\nREAD THE MANUAL\nmc commands workshop.import_transcript\ncrates/engine/src/workshop.rs\n~/.agents/skills/media-use/audio/references/transcribe.md\n\nF7 invokes the existing native DeckCraft storyboard.init command.\n\nThis monitor does not initiate production. Explicit project.intake can run configured audio analysis when valid timing is missing; supplied valid timing is reused. Missing prerequisites remain visible.\n");
        text
    }
    /// The same evidence as the desk, as plain text with nothing abbreviated. The panes
    /// truncate to fit their column; this is where a long path can still be read in full.
    fn cli_text(&mut self) -> String {
        let mut out = String::new();
        out.push_str("WORKER SLOT (machine-global)\n");
        match self.session.execute("blender.list", json!({})) {
            Ok(value) => {
                let owner = &value["worker_owner"];
                if owner.is_object() {
                    out.push_str(&format!(
                        "  owner job: {}\n  owner project: {}\n  owner unit: {}\n  owner invocation: {}\n  owner state: {} · service {}\n",
                        owner["job"].as_str().unwrap_or("?"),
                        owner["project"].as_str().unwrap_or("?"),
                        owner["unit"].as_str().unwrap_or("?"),
                        owner["invocation_id"].as_str().unwrap_or("?"),
                        owner["state"].as_str().unwrap_or("?"),
                        owner["service_state"].as_str().unwrap_or("?")
                    ));
                    out.push_str(&format!(
                        "  owner limits: {}\n",
                        limits_text(&owner["limits"])
                    ));
                    out.push_str(&format!(
                        "  owner submitted_at: {} · started_at: {} · cancellation_requested_at: {}\n",
                        stamp_text(&owner["submitted_at"]),
                        stamp_text(&owner["started_at"]),
                        stamp_text(&owner["cancellation_requested_at"])
                    ));
                } else {
                    out.push_str("  owner: none observed\n");
                }
                out.push_str(&format!(
                    "  slot status: {}\n",
                    value["worker_owner_status"]
                        .as_str()
                        .unwrap_or("not reported by this engine build")
                ));
                if let Some(error) = value["worker_owner_error"].as_str() {
                    out.push_str(&format!("  slot error: {error}\n"));
                }
            }
            Err(error) => out.push_str(&format!(
                "  slot: not reported by this engine build ({error})\n"
            )),
        }
        if let Some(project) = self.state["project"].as_str() {
            let observation = crate::worker::observe(project);
            out.push_str("\nSELECTED PROJECT WORKER\n");
            if let Some(error) = observation["observation_error"].as_str() {
                out.push_str(&format!("  observation: {error}\n"));
            }
            out.push_str(&format!(
                "  job: {} · state: {} · phase: {}\n",
                observation["job"].as_str().unwrap_or("?"),
                observation["state"].as_str().unwrap_or("?"),
                observation["phase"].as_str().unwrap_or("?")
            ));
            out.push_str(&format!(
                "  frames: {}/{} · updated_at: {}\n",
                observation["frames_completed"]
                    .as_u64()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into()),
                observation["frames_total"]
                    .as_u64()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into()),
                stamp_text(&observation["updated_at"])
            ));
            out.push_str(&format!(
                "  limits: {}\n",
                limits_text(&observation["limits"])
            ));
            out.push_str(&format!(
                "  submitted_at: {} · started_at: {} · cancellation_requested_at: {}\n",
                stamp_text(&observation["submitted_at"]),
                stamp_text(&observation["started_at"]),
                stamp_text(&observation["cancellation_requested_at"])
            ));
            out.push_str(&format!(
                "  telemetry: {}\n",
                crate::worker::resource_line(&observation)
            ));
            if let Some(error) = observation["worker_owner_error"].as_str() {
                out.push_str(&format!("  owner error: {error}\n"));
            }
        }
        out.push_str("\nALL PROJECTS\n");
        match self.state["catalog"]["projects"].as_array() {
            Some(projects) => {
                for project in projects {
                    out.push_str(&format!(
                        "  {} ({})\n",
                        project["name"].as_str().unwrap_or("unnamed"),
                        project["path"].as_str().unwrap_or("path unavailable")
                    ));
                    if let Some(next) = project["readiness"]["next_step"].as_str() {
                        out.push_str(&format!("    next step: {next}\n"));
                    }
                    for problem in project["readiness"]["problems"]
                        .as_array()
                        .into_iter()
                        .flatten()
                    {
                        out.push_str(&format!("    problem: {problem}\n"));
                    }
                }
            }
            None => out.push_str("  — catalog unavailable\n"),
        }
        if let Some(error) = self.state["catalog"]["error"].as_str() {
            out.push_str(&format!("  catalog error: {error}\n"));
        }
        if let Some(sample) = self.studio_sample() {
            out.push_str(&format!(
                "\nSTUDIO PROCESS (/proc)\n  pid {} · {} procs · cpu {} · rss sum {:.0}MiB · sampled {:.1}s ago\n",
                sample.pid,
                sample.procs.len(),
                sample.cpu_pct.map(|v| format!("{v:.0}%")).unwrap_or_else(|| "?".into()),
                crate::telemetry::rss_mb(&sample.procs),
                sample.at.elapsed().as_secs_f32()
            ));
        }
        out.push_str("\nPROJECT ACTION (literal, unabbreviated)\n");
        for (effect, literal) in self.copy_commands() {
            out.push_str(&format!("  {effect}\n    {literal}\n"));
        }
        out
    }
}
/// The requested limits as recorded, or an honest "not recorded" for older jobs.
fn limits_text(limits: &Value) -> String {
    if !limits.is_object() {
        return "not recorded".into();
    }
    let mib = |value: &Value| {
        value
            .as_u64()
            .map(|v| format!("{:.0}MiB", v as f64 / 1048576.0))
            .unwrap_or_else(|| "?".into())
    };
    format!(
        "memory {} · swap {} · cpu {}% · timeout {}",
        mib(&limits["memory_max_bytes"]),
        mib(&limits["swap_max_bytes"]),
        limits["cpu_quota_percent"]
            .as_u64()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into()),
        limits["timeout_seconds"]
            .as_u64()
            .map(|v| format!("{v}s"))
            .unwrap_or_else(|| "?".into())
    )
}
/// A timestamp field exactly as recorded: decimal epoch-ms string, or null.
fn stamp_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| "null".into())
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
                "  MAGIC STUDIO ",
                Style::default()
                    .fg(Color::Rgb(102, 226, 205))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "/ COMMAND CENTER · THE BOOTH  ·  F6 Monitor",
                Style::default().fg(BLUE),
            ),
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
fn signal_color(signal: magicstudio_engine::panels::Signal) -> Color {
    match signal {
        magicstudio_engine::panels::Signal::Idle => MUTED,
        magicstudio_engine::panels::Signal::InUse => GREEN,
        magicstudio_engine::panels::Signal::Attention => AMBER,
        magicstudio_engine::panels::Signal::Problem => RED,
    }
}
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
impl Center {
    fn copy_commands(&self) -> Vec<(&str, String)> {
        if let Some(value) = &self.onboarding {
            return vec![(
                "Start the assigned project Booth",
                value["launch"].as_str().unwrap_or("").into(),
            )];
        }
        if let Some(scope) = &self.scope {
            let prefix = format!(
                "{}={} mc",
                crate::scope::ENV,
                shell_quote(&scope.file.to_string_lossy())
            );
            return vec![
                (
                    "Inspect assigned project",
                    format!("{prefix} exec center.status"),
                ),
                (
                    "Read project receipts",
                    format!("{prefix} exec center.events"),
                ),
                ("Show granted actions", format!("{prefix} commands")),
            ];
        }
        if self.view_name() == "workshop" && !self.event_open {
            return workshop::commands(self);
        }
        if self.event_open
            && let Some(command) = selected_event(self).and_then(|e| e["command"].as_str())
        {
            return vec![(
                "Inspect this event’s command contract",
                format!("mc commands {}", shell_quote(command)),
            )];
        }
        let exe = shell_quote(
            &std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "mc".into()),
        );
        let exe = if let Some(scope) = &self.scope {
            format!(
                "{}={} {exe}",
                crate::scope::ENV,
                shell_quote(&scope.file.to_string_lossy())
            )
        } else {
            exe
        };
        let project = self.state["project"].as_str().unwrap_or("");
        if project.is_empty() {
            return vec![(
                "List projects to observe in WORKSHOP",
                format!("{exe} exec project.catalog"),
            )];
        }
        if let Some(worker) = worker_observation(self).filter(|v| {
            v["state"] == "failed"
                || v["state"] == "cancelling"
                || v["observation_error"].is_string()
        }) && let Some(job) = worker["job"].as_str()
        {
            return vec![(
                "Inspect this render's authoritative status and receipt",
                format!(
                    "{exe} exec animation.status {} {}",
                    shell_quote(&format!("project={project}")),
                    shell_quote(&format!("job={job}"))
                ),
            )];
        }
        let cursor = self.state["activity"]["next_cursor"].as_u64().unwrap_or(0);
        let inspection = if let (Some(endpoint), Some(instance)) = (
            self.state["ui"]["endpoint"].as_str(),
            self.pinned_instance.as_deref(),
        ) {
            format!(
                "{exe} --bridge {} exec workshop.status {} {}",
                shell_quote(endpoint),
                shell_quote(&format!("_expected_project={project}")),
                shell_quote(&format!("_expected_instance={instance}"))
            )
        } else if !project.is_empty() {
            let attach = json!({"id":"project.attach","params":{"path":project}}).to_string();
            let status = json!({"id":"center.status","params":{}}).to_string();
            format!(
                "printf '%s\\n' {} {} | {exe} run -",
                shell_quote(&attach),
                shell_quote(&status)
            )
        } else {
            format!("{exe} exec project.catalog")
        };
        let feedback = format!(
            "{exe} exec ui.feedback {}",
            shell_quote(&format!("project={project}"))
        );
        let receipts = format!(
            "{exe} exec center.events after={cursor} limit=192 {}",
            shell_quote(&format!("project={project}"))
        );
        let actions = root_actions(self);
        if let Some(failed) = actions.iter().find(|a| {
            root_finish(a).is_some_and(|e| {
                matches!(
                    e["stage"].as_str(),
                    Some("engine_error" | "handler_error" | "engine_needs_work")
                )
            })
        }) {
            let command = root_start(failed).map(target).unwrap_or("");
            return vec![
                ("Read the failed action's actual receipts", receipts),
                (
                    "Inspect the failed command's supported parameters",
                    format!("{exe} commands {}", shell_quote(command)),
                ),
            ];
        }
        if actions.iter().any(|a| {
            stages(a).iter().any(|e| e["stage"] == "dispatch_running") && root_finish(a).is_none()
        }) {
            return vec![("Read completion of the running action", receipts)];
        }
        if self.state["ui"]["connection"] != "connected" || self.state["ui"]["ui_ready"] != true {
            return vec![
                ("Check this project's missing Studio observation", feedback),
                ("Read the action awaiting completion", receipts),
            ];
        }
        if let Some(next) = self.state["next_commands"]
            .as_array()
            .filter(|v| !v.is_empty())
        {
            let mut commands = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for item in next {
                if let (Some(id), Some(endpoint), Some(instance)) = (
                    item["command"].as_str(),
                    self.state["ui"]["endpoint"].as_str(),
                    self.pinned_instance.as_deref(),
                ) {
                    // Older shared observations used the CLI help verb; normalize to the existing registry query.
                    let (command, params) = if item["operation"] == "commands" {
                        ("command.list", json!({"filter":id}))
                    } else {
                        (id, item["params"].clone())
                    };
                    let key = format!("{command}:{params}");
                    if !seen.insert(key) {
                        continue;
                    }
                    let mut literal = format!(
                        "{exe} --bridge {} exec {}",
                        shell_quote(endpoint),
                        shell_quote(command)
                    );
                    for (k, v) in params.as_object().into_iter().flatten() {
                        literal.push(' ');
                        literal.push_str(&shell_quote(&format!(
                            "{k}={}",
                            v.as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| v.to_string())
                        )));
                    }
                    // The private endpoint already pins the instance; local_ui validates the project and adds its nonce guard.
                    let _ = instance;
                    literal.push_str(&format!(
                        " {}",
                        shell_quote(&format!("_expected_project={project}"))
                    ));
                    commands.push((
                        item["label"].as_str().unwrap_or("See command arguments"),
                        literal,
                    ));
                }
            }
            if !commands.is_empty() {
                return commands;
            }
        }
        if let (Some(endpoint), Some(instance)) = (
            self.state["ui"]["endpoint"].as_str(),
            self.pinned_instance.as_deref(),
        ) {
            let design = format!(
                "{exe} --bridge {} exec storyboard.design {} {}",
                shell_quote(endpoint),
                shell_quote(&format!("_expected_project={project}")),
                shell_quote(&format!("_expected_instance={instance}"))
            );
            vec![
                (
                    "Inspect the active saved deck and pending image edits",
                    design,
                ),
                ("Read the last action's completion receipts", receipts),
            ]
        } else {
            vec![("Inspect this project's saved state", inspection)]
        }
    }
    fn copy_command(&mut self) {
        use std::io::Write;
        let commands = self.copy_commands();
        let text = &commands[self.command_selected % commands.len()].1;
        match std::process::Command::new("wl-copy").arg("--").stdin(std::process::Stdio::piped()).spawn(){
   Ok(mut child)=>{let written=child.stdin.take().ok_or_else(||io::Error::other("clipboard pipe unavailable")).and_then(|mut input|input.write_all(text.as_bytes()));let status=child.wait();self.message=if written.is_ok()&&status.is_ok_and(|s|s.success()){"COPIED command text only. Nothing executed.".into()}else{"Clipboard failed. Select terminal text and use Ctrl+Shift+C in Foot; nothing executed.".into()};},
   Err(_)=>self.message="Clipboard unavailable. Select terminal text and use Ctrl+Shift+C in Foot; nothing executed.".into()
  }
    }
}

/// Abbreviate a literal command for the panel: a long absolute path keeps its last
/// segments so the file stays identifiable, and the tail is marked as abbreviated.
/// The unabbreviated literal is what C copies and what the CLI prints.
fn abbreviate_literal(text: &str, width: usize) -> String {
    let mut shortened = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '/' {
            // A path can sit inside a quoted JSON argument, so scan to the path's own end
            // rather than to a space: quotes and whitespace close it.
            let start = index;
            while index < chars.len()
                && !chars[index].is_whitespace()
                && chars[index] != '\''
                && chars[index] != '"'
            {
                index += 1;
            }
            let path: String = chars[start..index].iter().collect();
            shortened.push_str(&compact_path_token(&path));
        } else {
            shortened.push(chars[index]);
            index += 1;
        }
    }
    fit_text(&shortened, width)
}

/// Keep a path identifiable when it must be shortened: the last two segments survive.
fn compact_path_token(path: &str) -> String {
    if Line::raw(path).width() <= 28 {
        return path.into();
    }
    let trailing: String = path
        .chars()
        .rev()
        .take_while(|c| !c.is_alphanumeric() && *c != '_' && *c != '-' && *c != '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let core = &path[..path.len() - trailing.len()];
    let segments: Vec<&str> = core.split('/').filter(|s| !s.is_empty()).collect();
    let mut last: Vec<&str> = segments.iter().rev().take(2).copied().collect();
    last.reverse();
    format!("…/{}{}", last.join("/"), trailing)
}
fn wrapped_rows(lines: &[Line<'_>], width: u16) -> u16 {
    lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(usize::from(width.max(1))) as u16)
        .sum()
}
fn film_holes(width: usize) -> String {
    (0..width)
        .map(|i| if i % 4 == 3 { ' ' } else { '▪' })
        .collect()
}
fn bounded_line(line: Line<'static>, width: usize) -> Line<'static> {
    let mut remaining = width;
    let mut spans = Vec::new();
    for span in line.spans {
        if remaining == 0 {
            break;
        }
        let len = Line::raw(span.content.as_ref()).width();
        spans.push(Span::styled(fit_text(&span.content, remaining), span.style));
        if len > remaining {
            break;
        }
        remaining -= len;
    }
    Line::from(spans)
}
fn film_status(width: usize, line: Line<'static>) -> Line<'static> {
    let line = bounded_line(line, width.saturating_sub(8));
    let spare = width.saturating_sub(line.width());
    let left = spare / 2;
    let mut spans = vec![Span::styled(film_holes(left), FILM)];
    spans.extend(line.spans);
    spans.push(Span::styled(film_holes(spare - left), FILM));
    Line::from(spans)
}
fn top_bar(c: &Center, width: usize, control: bool) -> Line<'static> {
    let view = if c.telemetry_view {
        "MAYHEM"
    } else if c.muppets_view {
        "MOPS"
    } else if control {
        "WORKSHOP"
    } else {
        "BOOTH"
    };
    let title = fit_text(&format!("MAGIC STUDIO · {view}"), width.saturating_sub(8));
    let title_width = Line::raw(&title).width();
    let start = width.saturating_sub(title_width) / 2;
    let left_room = start.saturating_sub(2);
    let right_room = width.saturating_sub(start + title_width + 2);
    let ui = &c.state["ui"];
    let snap = &c.state["snapshot"];
    let matched = ui["connection"] == "connected"
        && c.state["project"].is_string()
        && ui["active_project"] == c.state["project"];
    let project = c.state["project"]
        .as_str()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .and_then(|p| p.to_str())
        .unwrap_or("No project");
    let studio = if matched { "● Studio" } else { "○ Studio" };
    let saved = if matched {
        match ui["engine_unsaved"].as_bool() {
            Some(false) => "✓ Saved",
            Some(true) => "! Edits",
            None => "? Save",
        }
    } else {
        "? Save"
    };
    let mut details = Vec::new();
    if let Some(clip) = snap["clip"].as_str().filter(|v| *v != "?") {
        details.push(format!("@{}", clip.trim_start_matches('@')));
    }
    if let Some(time) = snap["playhead_sec"].as_f64() {
        details.push(format!("▶{time:.2}s"));
    }
    if let Some(part) = snap["selected_part"].as_str().filter(|v| *v != "?") {
        details.push(part.to_owned());
    }
    let base = format!("{project} · {studio} · {saved}");
    let full = if details.is_empty() {
        base.clone()
    } else {
        format!("{base} · {}", details.join(" · "))
    };
    let candidates = [
        full,
        base,
        format!("{studio} · {saved}"),
        studio.into(),
        String::new(),
    ];
    let left = candidates
        .into_iter()
        .find(|v| Line::raw(v).width() <= left_room)
        .unwrap_or_default();
    let metrics = match c.studio_sample() {
        Some(sample) => format!(
            "CPU {} · RAM {:.0}M",
            sample
                .cpu_pct
                .map(|v| format!("{v:.0}%"))
                .unwrap_or("?".into()),
            crate::telemetry::rss_mb(&sample.procs)
        ),
        None => "CPU ? · RAM ?".into(),
    };
    let (status, short, colour) =
        if let Some(worker) = worker_observation(c).filter(|v| v["state"] != "not_started") {
            (
                crate::worker::headline(worker),
                crate::worker::short_headline(worker),
                worker_colour(worker),
            )
        } else if snap["last_error"].is_object() {
            ("! Error · E inspect".into(), "! Error".into(), RED)
        } else if matched {
            ("● Live".into(), "● Live".into(), GREEN)
        } else {
            ("○ Offline".into(), "○ Offline".into(), AMBER)
        };
    let fleet = control && !c.muppets_view && !c.telemetry_view;
    let left = if fleet {
        fit_text(" Master · Jesse + Gonzo", left_room)
    } else {
        left
    };
    let (status, short) = if fleet {
        let (active, inactive) = workshop::counts(c);
        if c.state["catalog"]["projects"].is_array() {
            (
                format!(
                    "{} · {} Projects",
                    workshop::activity_label(c),
                    active + inactive
                ),
                format!("{} Projects", active + inactive),
            )
        } else {
            ("Catalog unavailable".into(), "? Projects".into())
        }
    } else {
        (status, short)
    };
    let colour = if fleet { TEAL } else { colour };
    let full = if fleet {
        status.clone()
    } else {
        format!("{status} · {metrics}")
    };
    let right = [full, status, short, String::new()]
        .into_iter()
        .find(|v| Line::raw(v).width() <= right_room)
        .unwrap_or_default();
    let mut spans = vec![
        Span::styled(left.clone(), MUTED),
        Span::styled(
            film_holes(start.saturating_sub(Line::raw(&left).width())),
            FILM,
        ),
    ];
    spans.extend(crate::logo::compact_title(&title).spans);
    spans.push(Span::styled(
        film_holes(width.saturating_sub(start + title_width + Line::raw(&right).width())),
        FILM,
    ));
    spans.push(Span::styled(right, colour));
    Line::from(spans)
}
/// SMPTE-style timecode from a playhead position.
fn timecode(seconds: f64, fps: f64) -> String {
    let fps = if fps.is_finite() && fps >= 1.0 {
        fps.round()
    } else {
        24.0
    };
    let total = seconds.max(0.0);
    let whole = total.floor();
    let secs = whole as u64;
    let frames = (((total - whole) * fps).floor().max(0.0)).min(fps - 1.0) as u64;
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60,
        frames
    )
}
/// Broadcast tally: the word always travels with the colour.
fn tally(status: &str) -> (Color, &'static str) {
    match status {
        "working" | "busy" => (AMBER, "WORKING"),
        "stalled" | "blocked" | "error" => (RED, "STALLED"),
        "idle" | "done" => (GREEN, "IDLE"),
        _ => (MUTED, "UNKNOWN"),
    }
}
fn root_actions(c: &Center) -> Vec<&Value> {
    c.state["activity"]["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .filter(|a| {
            stages(a)
                .iter()
                .any(|e| matches!(e["stage"].as_str(), Some("dispatch_running" | "ui_click")))
        })
        .take(3)
        .collect()
}
fn stages(action: &Value) -> Vec<&Value> {
    action["events"].as_array().into_iter().flatten().collect()
}
fn target(event: &Value) -> &str {
    event["command"]
        .as_str()
        .or(event["control"].as_str())
        .unwrap_or("unmapped interaction")
}
fn root_start(action: &Value) -> Option<&Value> {
    let events = action["events"].as_array()?;
    events
        .iter()
        .find(|e| e["stage"] == "dispatch_running" && e["request_id"] == action["root_request_id"])
        .or_else(|| events.iter().find(|e| e["stage"] == "dispatch_running"))
        .or_else(|| events.first())
}
fn root_finish(action: &Value) -> Option<&Value> {
    let start = root_start(action)?;
    action["events"].as_array()?.iter().rev().find(|e| {
        matches!(
            e["stage"].as_str(),
            Some("engine_success" | "engine_error" | "engine_needs_work" | "handler_error")
        ) && (e["command"] == start["command"] || !start["command"].is_string())
            && (!start["request_id"].is_string() || e["request_id"] == start["request_id"])
    })
}
fn engine_badge(action: &Value) -> (&'static str, Color) {
    match root_finish(action).and_then(|e| e["stage"].as_str()) {
        Some("engine_success") => {
            let status = root_finish(action)
                .and_then(|e| e["status"].as_str())
                .or_else(|| {
                    stages(action)
                        .into_iter()
                        .rev()
                        .find_map(|e| e["outputs"]["status"].as_str())
                });
            match status {
                Some("running") => ("● WORKING", GREEN),
                Some("stalled") => ("● STALLED", AMBER),
                Some("error") => ("● FAILED", RED),
                _ => ("[OK] SUCCEEDED", DESK_FG),
            }
        }
        Some("engine_error" | "handler_error") => ("✗ FAILED", RED),
        Some("engine_needs_work") => ("! NEEDS WORK", AMBER),
        _ if stages(action)
            .iter()
            .any(|e| e["stage"] == "dispatch_running") =>
        {
            ("[>] RUNNING", GREEN)
        }
        _ if root_start(action).is_some_and(|e| e["interaction"] == "navigation") => {
            ("— UI NAVIGATION", MUTED)
        }
        _ if stages(action).iter().any(|e| e["stage"] == "ui_click") => ("— UI INTERACTION", MUTED),
        _ => ("! COMPLETION UNKNOWN", AMBER),
    }
}
fn action_ack(action: &Value) -> bool {
    stages(action).iter().any(|e| {
        matches!(
            e["stage"].as_str(),
            Some("frontend_ack" | "ui_navigation_applied")
        ) && (e["request_id"].as_str().is_some_and(|id| {
            Some(id) == root_start(action).and_then(|s| s["request_id"].as_str())
        }) || root_finish(action).is_some_and(|finish| {
            finish["engine_revision"].is_u64() && finish["engine_revision"] == e["engine_revision"]
        }))
    })
}
fn stamp(event: &Value) -> String {
    event["unix_micros"]
        .as_u64()
        .map(|us| {
            let s = us / 1_000_000;
            format!("{:02}:{:02}:{:02}", s / 3600 % 24, s / 60 % 60, s % 60)
        })
        .unwrap_or_else(|| "--:--:--".into())
}
fn revision(value: &Value) -> String {
    value
        .as_u64()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unknown".into())
}
// Actual receipt counts in eight fixed one-second buckets. No audio/progress claim.
fn activity_wave(c: &Center) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut bins = [0usize; 8];
    let mut seen = std::collections::HashSet::new();
    for action in root_actions(c) {
        for event in stages(action) {
            if let (Some(us), Some(sequence)) =
                (event["unix_micros"].as_u64(), event["sequence"].as_u64())
            {
                if seen.insert(sequence) {
                    let second = us / 1_000_000;
                    if second <= now && now - second < 8 {
                        bins[7 - (now - second) as usize] += 1;
                    }
                }
            }
        }
    }
    let levels = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let wave: String = bins.iter().map(|n| levels[(*n).min(7)]).collect();
    let count = bins.iter().sum::<usize>();
    let running = root_actions(c)
        .into_iter()
        .find(|a| matches!(engine_badge(a).0, "● WORKING" | "[>] RUNNING"));
    let state = running
        .and_then(|a| root_start(a))
        .map(|e| {
            format!(
                "[>] {} · {}s elapsed",
                target(e),
                now.saturating_sub(
                    e["unix_micros"].as_u64().unwrap_or(now * 1_000_000) / 1_000_000
                )
            )
        })
        .unwrap_or_else(|| "idle".into());
    format!("activity {wave} · {count} receipts/8s · {state}")
}
fn action_lines(c: &Center, action: &Value, compact: bool) -> Vec<Line<'static>> {
    let Some(start) = root_start(action) else {
        return vec![styled("— action details unavailable", MUTED)];
    };
    let first = action["first_sequence"].as_u64();
    let last = action["last_sequence"].as_u64();
    let sequence = match (first, last) {
        (Some(a), Some(b)) if a != b => format!("#{a}–{b}"),
        (Some(a), _) => format!("#{a}"),
        _ => "#?".into(),
    };
    let (engine, color) = engine_badge(action);
    let mut heading = styled(
        format!(
            "{}{}  ·  {} UTC",
            if c.details_open {
                format!("{sequence}  ")
            } else {
                String::new()
            },
            target(start),
            stamp(start)
        ),
        if color == RED { RED } else { BLUE },
    );
    if c.flash_started
        .is_some_and(|at| at.elapsed() < Duration::from_millis(800))
        && last.is_some_and(|seq| seq > c.flash_after)
    {
        heading = heading.style(
            Style::default()
                .fg(if color == RED { RED } else { BLUE })
                .bg(DESK_SELECTED),
        );
    }
    let ack = action_ack(action);
    let navigation = root_start(action).is_some_and(|e| e["interaction"] == "navigation");
    let ui_result = if ack {
        if navigation {
            "NAVIGATION APPLIED"
        } else {
            "RESULT RENDERED"
        }
    } else if navigation {
        "navigation receipt pending"
    } else if engine_badge(action).0 == "[>] RUNNING" {
        "waiting for result/render"
    } else {
        "no correlated Studio receipt"
    };
    let mut lines = vec![heading];
    if compact {
        lines.push(Line::from(vec![
            Span::styled(format!("  {engine}  "), Style::default().fg(color)),
            Span::styled(
                format!("{} {ui_result}", if ack { "▣" } else { "—" }),
                Style::default().fg(if ack { DESK_FG } else { AMBER }),
            ),
        ]));
    } else {
        lines.push(styled(format!(" ├─ ENGINE: {engine}"), color));
        lines.push(styled(
            format!(
                " ├─ STUDIO: {} {ui_result}{}",
                if ack { "▣" } else { "—" },
                if ack && !navigation {
                    " · visual review separate"
                } else {
                    ""
                }
            ),
            if ack { DESK_FG } else { AMBER },
        ));
        lines.push(styled(
            format!(
                " └─ AGENT: {} · {}",
                start["origin"].as_str().unwrap_or("unknown"),
                start["client_label"].as_str().unwrap_or("unverified")
            ),
            MUTED,
        ));
    }
    if let Some(error) = root_finish(action)
        .and_then(|e| e["error"].as_str())
        .filter(|_| !compact)
    {
        lines.push(styled(
            format!(
                "    {}",
                error.strip_prefix("invalid parameters: ").unwrap_or(error)
            ),
            RED,
        ));
    }
    lines
}
fn render_actions(f: &mut Frame, c: &Center, area: ratatui::layout::Rect, compact: bool) {
    let actions = root_actions(c);
    let mut lines = Vec::new();
    if actions.is_empty() {
        lines.push(styled(
            "— No root actions recorded for this project.",
            MUTED,
        ));
    }
    for (i, action) in actions.iter().enumerate() {
        if i > 0 && !compact {
            lines.push(Line::raw(""));
        }
        lines.extend(action_lines(c, action, compact));
    }
    let block = round_panel("Last actions · newest first · Pg scroll", BLUE, false);
    let inner = block.inner(area);
    let max_scroll = wrapped_rows(&lines, inner.width).saturating_sub(inner.height);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((c.scroll.min(max_scroll), 0))
            .block(block),
        area,
    );
}

#[allow(dead_code)]
fn saved_observation(ui: &Value) -> (&'static str, Color) {
    if ui["connection"] != "connected" {
        return ("? Saved state unobserved", AMBER);
    }
    if ui["draft_fields"].as_array().is_some_and(|v| !v.is_empty()) {
        return ("! Unsaved form edits", AMBER);
    }
    match ui["engine_unsaved"].as_bool() {
        Some(true) => ("! Engine differs from saved project", AMBER),
        Some(false) => ("[OK] Saved project matches", DESK_FG),
        None => ("? Saved state unknown; engine busy/unreadable", AMBER),
    }
}
// These are views of CLI-owned evidence, never an independent readiness evaluator.
// The engine reports lyrics and notes as one "lyrics" item; official artist lyrics are not assessed.
const PROJECT_STAGES: [(&str, &str); 7] = [
    ("Audio", "song"),
    ("Transcript", "transcript"),
    ("Lyrics & notes", "lyrics"),
    ("Reference art", "assets"),
    ("Storyboard", "storyboard"),
    ("Animation", "animate"),
    ("Video / captions", "hyperframes"),
];
const STORYBOARD_STAGE: usize = 4;
fn stage_evidence(c: &Center, index: usize) -> Option<&Value> {
    let id = PROJECT_STAGES[index % PROJECT_STAGES.len()].1;
    c.state["prerequisites"]
        .as_array()?
        .iter()
        .find(|v| v["id"] == id)
}
fn state_badge(state: Option<&str>) -> (&'static str, Color) {
    match state {
        Some("ready" | "replied") => ("[OK]", DESK_FG),
        Some("running" | "working") => ("● working", GREEN),
        Some("blocked" | "error" | "failed") => ("● failed", RED),
        Some("stalled") => ("● stalled", AMBER),
        Some("missing") => ("● missing", BLUE),
        _ => ("? unknown", MUTED),
    }
}
fn chat_outcome(state: &Value) -> String {
    let r = &state["chat"]["request"];
    let (mark, _) = state_badge(r["status"].as_str());
    format!(
        "Chat {mark} · {}",
        r["request_id"].as_str().unwrap_or("not started")
    )
}
fn selected_reason(c: &Center) -> String {
    stage_evidence(c,c.stage_selected).and_then(|v|v["reason"].as_str()).unwrap_or("No separate CLI observation for this input. Transcript text does not establish official artist lyrics or liner notes.").into()
}
fn stage_lines(c: &Center, stacked: bool) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, (label, _)) in PROJECT_STAGES.iter().enumerate() {
        let (glyph, word, color) = match stage_evidence(c, i).and_then(|v| v["state"].as_str()) {
            Some("ready") => ("●", "Ready", GREEN),
            Some("running" | "working") => ("●", "Working", GREEN),
            Some("blocked") => ("●", "Blocked", AMBER),
            Some("failed" | "error") => ("●", "Failed", RED),
            Some("stalled") => ("●", "Stalled", AMBER),
            Some("missing") => ("○", "Missing", BLUE),
            _ => ("?", "Unknown", MUTED),
        };
        let line = Line::from(vec![
            Span::styled(if i == c.stage_selected { "› " } else { "  " }, DESK_FG),
            Span::styled(format!("{label:<16} "), MUTED),
            Span::styled(glyph, color),
            Span::styled(format!(" {word:<7}"), DESK_FG),
        ]);
        if stacked && i % 2 == 1 {
            let mut previous = lines.pop().unwrap_or_default();
            previous.spans.push(Span::styled("  │  ", DIM));
            previous.spans.extend(line.spans);
            lines.push(previous);
        } else {
            lines.push(line);
        }
    }
    lines
}

fn doctor_badge(c: &Center) -> (&'static str, Color) {
    let d = &c.state["storyboard_doctor"];
    if c.doctor_receiver.is_some() {
        ("● Checking", GREEN)
    } else if c.doctor_error.is_some() {
        ("● Check failed", RED)
    } else if d.is_null() {
        ("● Not checked", BLUE)
    } else if d["stale"] == true {
        ("● Needs review · stale", AMBER)
    } else if d["healthy"] == true {
        ("✓ Checked · section boundary only", DESK_FG)
    } else {
        ("● Needs review", AMBER)
    }
}
fn render_stage_activity(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    if let Some(worker) = worker_observation(c) {
        render_worker(f, worker, area);
        return;
    }
    let mut lines = if area.height < 4 {
        vec![styled(activity_wave(c), MUTED)]
    } else {
        vec![
            styled(selected_reason(c), DESK_FG),
            styled(activity_wave(c), MUTED),
        ]
    };
    if c.stage_selected == STORYBOARD_STAGE {
        let (mark, color) = doctor_badge(c);
        lines.insert(
            0,
            styled(format!("F10 🩺 [+] Storyboard Doctor · {mark}"), color),
        );
        let d = &c.state["storyboard_doctor"];
        if let Some(error) = &c.doctor_error {
            lines.push(styled(error, RED));
        }
        if !d.is_null() {
            let time = d["assessed_at_unix_ms"].as_u64().unwrap_or(0) / 1000;
            let rev = d["storyboard_revision"].as_str().unwrap_or("unknown");
            lines.push(styled(
                format!(
                    "Checked {:02}:{:02}:{:02} UTC · input {}",
                    time / 3600 % 24,
                    time / 60 % 60,
                    time % 60,
                    rev.get(..rev.len().min(8)).unwrap_or(rev)
                ),
                MUTED,
            ));
            if d["tasks"].as_array().is_some_and(|v| v.is_empty()) {
                lines.push(styled(
                    "No supported spillover found. Other quality checks unassessed.",
                    MUTED,
                ));
            }
            for task in d["tasks"].as_array().into_iter().flatten() {
                lines.push(styled(
                    format!(
                        "{} / page {}: {}",
                        task["scene_id"].as_str().unwrap_or("?"),
                        task["page_id"],
                        task["suspected_flaw"].as_str().unwrap_or("Review")
                    ),
                    AMBER,
                ));
                lines.push(styled(
                    format!(
                        "Words {} · {}",
                        task["source_word_ids"],
                        task["affected_lyrics"].as_str().unwrap_or("")
                    ),
                    DESK_FG,
                ));
                lines.push(styled(
                    task["confidence"].as_str().unwrap_or("Evidence unknown"),
                    MUTED,
                ));
                lines.push(styled(
                    task["proposed_fix"].as_str().unwrap_or("Inspect source"),
                    DESK_FG,
                ));
            }
            for item in d["unassessed"].as_array().into_iter().flatten().take(1) {
                lines.push(styled(
                    format!(
                        "Unassessed: {}",
                        item["reason"]
                            .as_str()
                            .unwrap_or("source evidence unavailable")
                    ),
                    AMBER,
                ));
            }
        }
    }
    let m = &c.state["metrics"];
    let observed = |key: &str| {
        m[key]
            .as_u64()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into())
    };
    lines.push(styled(
        format!(
            "{} words · {} groups · {}s audio · {}/{} assets",
            observed("timed_words"),
            observed("lyric_groups"),
            m["recording_duration_sec"]
                .as_f64()
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "?".into()),
            observed("available_assets"),
            observed("registered_assets")
        ),
        MUTED,
    ));
    if let Some(r) = c.state["chat"]["request"]
        .as_object()
        .filter(|r| r.get("status").is_some_and(|v| v == "running"))
    {
        let elapsed = r.get("sent_at").and_then(Value::as_u64).map(|ms| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|v| v.as_secs().saturating_sub(ms / 1000))
                .unwrap_or(0)
        });
        lines.push(styled(
            format!(
                "● agent.send working · {} · {}s elapsed",
                c.state["chat"]["selection"]["model"]
                    .as_str()
                    .unwrap_or("model unknown"),
                elapsed.map(|v| v.to_string()).unwrap_or_else(|| "?".into())
            ),
            GREEN,
        ));
    }
    if c.details_open {
        lines.push(styled(
            format!(
                "Project: {}",
                c.state["project"].as_str().unwrap_or("unknown")
            ),
            MUTED,
        ));
        lines.push(styled(
            format!(
                "Session: {} · request {} · engine/ack {}/{} · DOM {}ms",
                c.state["chat"]["session"].as_str().unwrap_or("none"),
                c.state["chat"]["request"]["request_id"]
                    .as_str()
                    .unwrap_or("none"),
                revision(&c.state["ui"]["engine_revision"]),
                revision(&c.state["ui"]["ack_engine_revision"]),
                c.state["ui"]["frontend_report_age_ms"]
            ),
            MUTED,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(round_panel(
                &format!(
                    "{} · ACTIVITY{}",
                    if c.stage_selected == STORYBOARD_STAGE {
                        "▰ ▰  STORYBOARD REVIEW  ▰ ▰"
                    } else {
                        PROJECT_STAGES[c.stage_selected].0
                    },
                    if c.details_open { " / DETAILS" } else { "" }
                ),
                BLUE,
                false,
            )),
        area,
    );
}
fn worker_observation(c: &Center) -> Option<&Value> {
    let value = &c.state["animation_worker"];
    (c.state["project"].is_string() && value["project"] == c.state["project"]).then_some(value)
}
fn worker_colour(value: &Value) -> Color {
    if value["observation_error"].is_string() || value["state"] == "failed" {
        RED
    } else if crate::worker::stale(value)
        || matches!(value["state"].as_str(), Some("running" | "cancelling"))
    {
        AMBER
    } else if value["state"] == "completed" {
        GREEN
    } else {
        MUTED
    }
}
/// A btop-style meter: filled cells with a severity colour. Unknown is drawn as
/// middle dots, never as an empty bar — missing is not zero.
fn meter(done: Option<u64>, total: Option<u64>, width: usize) -> (String, Color) {
    if width == 0 {
        return (String::new(), MUTED);
    }
    let (Some(done), Some(total)) = (done, total) else {
        return ("·".repeat(width), MUTED);
    };
    if total == 0 {
        return ("·".repeat(width), MUTED);
    }
    let ratio = (done.min(total) as f64 / total as f64).clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let bar = format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    );
    let colour = if ratio < 0.5 {
        GREEN
    } else if ratio < 0.8 {
        AMBER
    } else {
        RED
    };
    (bar, colour)
}

/// A render's progress as a timeline: what is finished, the playhead at the
/// boundary, and what is still to come. Borrowed from an animation editor's
/// frame ruler rather than a plain meter, because a render is a sequence.
fn frame_track(done: Option<u64>, total: Option<u64>, width: usize) -> Vec<Span<'static>> {
    let (Some(done), Some(total)) = (done, total) else {
        return vec![Span::styled("·".repeat(width), Style::default().fg(MUTED))];
    };
    if total == 0 {
        return vec![Span::styled("·".repeat(width), Style::default().fg(MUTED))];
    }
    let cells = width.max(6);
    let filled = ((done.min(total) as f64 / total as f64) * cells as f64).round() as usize;
    vec![
        Span::styled("█".repeat(filled), Style::default().fg(BLUE)),
        Span::styled("▏", Style::default().fg(TEAL)),
        Span::styled(
            "░".repeat(cells.saturating_sub(filled)),
            Style::default().fg(MUTED),
        ),
    ]
}
/// One aligned label/value row. The label column is shared, so values line up.
fn metric_row(label: &str, bar: &str, colour: Color, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<9}"), Style::default().fg(MUTED)),
        Span::styled(bar.to_string(), Style::default().fg(colour)),
        Span::styled(format!("  {value}"), Style::default().fg(DESK_FG)),
    ])
}
/// The same row, with a timeline instead of a bar.
fn track_row(label: &str, track: Vec<Span<'static>>, value: &str) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{label:<9}"),
        Style::default().fg(MUTED),
    )];
    spans.extend(track);
    spans.push(Span::styled(
        format!("  {value}"),
        Style::default().fg(DESK_FG),
    ));
    Line::from(spans)
}
fn render_worker(f: &mut Frame, value: &Value, area: ratatui::layout::Rect) {
    let width = area.width.saturating_sub(2) as usize;
    let inner = area.height.saturating_sub(2) as usize;
    if value["state"] == "not_started" {
        let mut lines = vec![styled(crate::worker::state_line(value), MUTED)];
        lines.push(styled(
            fit_text(
                "Waiting for a render submission · Backstage never starts one by polling",
                width,
            ),
            MUTED,
        ));
        if let Some(owner) = value["worker_owner"]["job"].as_str() {
            lines.push(styled(
                fit_text(&format!("Global worker busy: {owner}"), width),
                AMBER,
            ));
        }
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .block(booth::panel("ARTWORK IN MOTION", false)),
            area,
        );
        return;
    }
    // Rows are ordered by what matters first, and the panel shows as many as fit.
    let telemetry = &value["telemetry"];
    let frames_value = format!(
        "{}/{}",
        value["frames_completed"]
            .as_u64()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into()),
        value["frames_total"]
            .as_u64()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into())
    );
    let frames_track = frame_track(
        value["frames_completed"].as_u64(),
        value["frames_total"].as_u64(),
        20,
    );
    let limit = telemetry["memory_max_bytes"]
        .as_u64()
        .or_else(|| value["limits"]["memory_max_bytes"].as_u64());
    let (ram_bar, ram_colour) = meter(telemetry["memory_current_bytes"].as_u64(), limit, 16);
    let mib = |v: &Value| {
        v.as_u64()
            .map(|n| format!("{:.0}MiB", n as f64 / 1048576.0))
            .unwrap_or_else(|| "?".into())
    };
    let ram_value = format!(
        "{} / {} · peak {} · swap {} / {}",
        mib(&telemetry["memory_current_bytes"]),
        mib(&telemetry["memory_max_bytes"]),
        mib(&telemetry["memory_peak_bytes"]),
        mib(&telemetry["swap_current_bytes"]),
        mib(&telemetry["swap_max_bytes"])
    );
    let selection = &value["selection"];
    let identity = format!(
        "{} · {} · {}",
        selection["character"].as_str().unwrap_or("character ?"),
        selection["track"].as_str().unwrap_or("action ?"),
        value["job"].as_str().unwrap_or("job ?")
    );
    let source = if value["telemetry_source"] == "terminal_receipt"
        || matches!(
            value["state"].as_str(),
            Some("completed" | "cancelled" | "failed")
        ) {
        "saved receipt"
    } else {
        "worker service"
    };
    let age = crate::worker::age_ms(value, "updated_at")
        .map(booth::age_label)
        .unwrap_or_else(|| "?".into());
    let source_row = format!(
        "{source} · recorded {age} ago{}",
        if crate::worker::stale(value) {
            " · STALE"
        } else {
            ""
        }
    );
    let owner = &value["worker_owner"];
    let owner_line = if owner.is_object() {
        format!(
            "{} · {} · {}",
            owner["job"].as_str().unwrap_or("?"),
            owner["project"].as_str().unwrap_or("project unknown"),
            owner["state"].as_str().unwrap_or("state ?")
        )
    } else {
        format!(
            "Global worker owner: {}",
            value["worker_owner_status"]
                .as_str()
                .unwrap_or("unavailable")
        )
    };
    let mut rows: Vec<Line<'static>> = Vec::new();
    rows.push(Line::from(vec![
        Span::styled("● ", worker_colour(value)),
        Span::styled(
            if inner >= 3 {
                crate::worker::state_line(value)
            } else {
                crate::worker::headline(value)
            },
            DESK_FG,
        ),
    ]));
    rows.push(track_row("FRAMES", frames_track, &frames_value));
    rows.push(metric_row(
        "RAM",
        &ram_bar,
        if ram_colour == GREEN {
            BLUE
        } else {
            ram_colour
        },
        &ram_value,
    ));
    rows.push(styled(
        fit_text(&format!("{:<9}{identity}", "JOB"), width),
        MUTED,
    ));
    rows.push(styled(
        fit_text(&format!("{:<9}{source_row}", "SOURCE"), width),
        MUTED,
    ));
    rows.push(styled(
        fit_text(&format!("{:<9}{owner_line}", "SLOT"), width),
        MUTED,
    ));
    if let Some(error) = value["observation_error"]
        .as_str()
        .or(value["error"].as_str())
        .or(value["detail"].as_str())
    {
        rows.push(styled(fit_text(error, width), RED));
    }
    let lines: Vec<Line<'static>> = rows.into_iter().take(inner.max(1)).collect();
    f.render_widget(
        Paragraph::new(lines).block(booth::panel("ARTWORK IN MOTION", false)),
        area,
    );
    booth::motion_border(f, value, area);
}
fn conversation_text(text: &str) -> &str {
    let text = text
        .split("\n\n[Magic Studio context at send time:")
        .next()
        .unwrap_or("");
    // Migration-only: hide injected context in old stored turns; never emit the old branding.
    text.split("\n\n[AnimationCraft context at send time:")
        .next()
        .unwrap_or("")
}
fn render_conversation(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    booth::render_chat(f, c, area);
}
/// The booth keeps typed events past the journal's three-action retention while it runs.
fn merge_tail(old: &Value, new: Vec<Value>) -> Value {
    let mut events = old.as_array().cloned().unwrap_or_default();
    let seen = events
        .iter()
        .filter_map(|e| e["sequence"].as_u64())
        .max()
        .unwrap_or(0);
    events.extend(
        new.into_iter()
            .filter(|e| e["sequence"].as_u64().is_some_and(|n| n > seen)),
    );
    let excess = events.len().saturating_sub(50);
    events.drain(..excess);
    json!(events)
}
fn selected_event(c: &Center) -> Option<&Value> {
    if c.event_open {
        return c.event_detail.as_ref();
    }
    c.state["tail"].as_array()?.iter().rev().nth(c.event_index)
}
fn event_agent(c: &Center, event: &Value) -> Option<usize> {
    let agents = c.state["agent_gang"].as_array()?;
    if let Some(pane) = event["pane_id"].as_str() {
        return agents
            .iter()
            .position(|g| g["pane_id"].as_str() == Some(pane));
    }
    let label = event["client"].as_str().or(event["origin"].as_str())?;
    let mut found = agents.iter().enumerate().filter(|(_, g)| {
        g["name"]
            .as_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(label))
    });
    let (index, _) = found.next()?;
    if found.next().is_some() {
        None
    } else {
        Some(index)
    }
}
fn render_tail(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let block = round_panel(
        "EVENTS · E select · Enter inspect",
        if c.event_focus { TEAL } else { DESK_LINE },
        c.event_focus,
    );
    let inner = block.inner(area);
    let rows = inner.height as usize;
    let width = inner.width as usize;
    let tiles = gang_tiles(c);
    let start = if c.event_focus {
        c.event_index.saturating_sub(rows.saturating_sub(1))
    } else {
        0
    };
    let mut lines: Vec<Line<'static>> = c.state["tail"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .enumerate()
        .skip(start)
        .take(rows)
        .map(|(i, e)| {
            let owner = event_agent(c, e).and_then(|index| tiles.get(index));
            let identity = owner.map(|g| g.1).unwrap_or(MUTED);
            let label = owner
                .map(|g| format!("{} {}", moppet_icon(&g.0), g.0))
                .unwrap_or_else(|| "?".into());
            let mark = if e["ok"] == false { "✗" } else { "·" };
            let text = format!(
                "{} {} {} {} {}",
                if c.event_focus && i == c.event_index {
                    "›"
                } else {
                    " "
                },
                stamp(e),
                label,
                e["command"]
                    .as_str()
                    .unwrap_or_else(|| e["type"].as_str().unwrap_or("?")),
                e["error"].as_str().unwrap_or("")
            );
            let mut line = bounded_line(
                Line::from(vec![
                    Span::styled(mark, if e["ok"] == false { RED } else { identity }),
                    Span::styled(text, identity),
                ]),
                width,
            );
            if c.event_focus && i == c.event_index {
                line = line.style(Style::default().bg(DESK_SELECTED));
            }
            line
        })
        .collect();
    if lines.is_empty() {
        lines.push(styled("No events recorded", MUTED));
    }
    f.render_widget(Paragraph::new(lines).block(block), area);
}
fn render_event(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let Some(event) = selected_event(c) else {
        return;
    };
    let tiles = gang_tiles(c);
    let owner = event_agent(c, event).and_then(|index| tiles.get(index));
    let heading = owner
        .map(|g| format!("{} {}", moppet_icon(&g.0), g.0))
        .unwrap_or_else(|| "Unknown agent".into());
    let colour = owner.map(|g| g.1).unwrap_or(MUTED);
    let mut lines = vec![styled(heading, colour)];
    for (label, value) in [
        ("Time", Some(stamp(event))),
        ("Command", event["command"].as_str().map(str::to_owned)),
        ("Error", event["error"].as_str().map(str::to_owned)),
        ("Source", event["origin"].as_str().map(str::to_owned)),
        ("Client", event["client"].as_str().map(str::to_owned)),
        ("Request", event["request_id"].as_str().map(str::to_owned)),
        ("Project", event["project"].as_str().map(str::to_owned)),
    ] {
        if let Some(value) = value {
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<9}"), MUTED),
                Span::styled(value, if label == "Error" { RED } else { DESK_FG }),
            ]));
        }
    }
    let selected = tiles.get(c.gang_selected);
    lines.push(Line::raw(""));
    lines.push(styled(
        format!(
            "Fix note → {} · , / . choose recipient · p compose",
            selected
                .map(|g| format!("{} {}", moppet_icon(&g.0), g.0))
                .unwrap_or_else(|| "Choose an agent".into())
        ),
        TEAL,
    ));
    if owner.is_none() {
        lines.push(styled(
            "No agent ownership recorded. Choose the recipient explicitly.",
            MUTED,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((c.scroll, 0))
            .block(round_panel(
                "EVENT · C inspect command · Esc back · Pg scroll",
                colour,
                true,
            )),
        area,
    );
}
fn render_next(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let commands = c.copy_commands();
    let Some((effect, literal)) = commands.get(c.command_selected % commands.len().max(1)) else {
        return;
    };
    let reason = c.state["next_commands"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| v["label"].as_str() == Some(*effect))
        .and_then(|v| v["reason"].as_str())
        .unwrap_or(effect);
    let mut lines = vec![styled(*effect, DESK_FG)];
    if reason != *effect {
        lines.push(styled(reason, MUTED));
    }
    lines.push(styled(
        abbreviate_literal(literal, area.width.saturating_sub(4) as usize),
        BLUE,
    ));
    let title = format!("{} · C Copy · Pg Scroll", booth::action_title(c, literal));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((c.scroll, 0))
            .block(booth::panel(title, true)),
        area,
    );
}
/// Shared names and status colours for cards and the selected-agent tabs.
fn gang_tiles(c: &Center) -> Vec<(String, Color, Color, &'static str)> {
    let mut tiles: Vec<_> = c.state["agent_gang"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|g| {
            let name = g["name"]
                .as_str()
                .or(g["agent"].as_str())
                .unwrap_or("?")
                .to_owned();
            let (colour, word) = tally(g["agent_status"].as_str().unwrap_or(""));
            let identity = moppet_identity(&name);
            (name, identity, colour, word)
        })
        .collect();
    let (colour, word) = match c.state["chat"]["request"]["status"].as_str().unwrap_or("") {
        "replied" => (GREEN, "REPLIED"),
        "running" | "queued" => (AMBER, "WORKING"),
        "error" => (RED, "ERROR"),
        _ => (MUTED, "IDLE"),
    };
    tiles.push(("Sprocket".into(), moppet_identity("Sprocket"), colour, word));
    let mut used = Vec::new();
    for tile in &mut tiles {
        if used.contains(&tile.1) {
            tile.1 = MOPPET_CAST
                .iter()
                .map(|(_, _, colour)| *colour)
                .chain([Color::Rgb(236, 149, 149), Color::Rgb(132, 202, 191)])
                .find(|colour| !used.contains(colour))
                .unwrap_or(Color::Indexed((16 + used.len() % 216) as u8));
        }
        used.push(tile.1);
    }
    tiles
}
const MOPPET_CAST: [(&str, &str, Color); 10] = [
    ("sam", "🦅", Color::Rgb(229, 196, 109)),
    ("kermit", "🐸", Color::Rgb(171, 214, 111)),
    ("gonzo", "🚀", Color::Rgb(182, 154, 248)),
    ("scooter", "🛵", Color::Rgb(239, 145, 181)),
    ("beaker", "🧪", Color::Rgb(137, 215, 182)),
    ("bunsen", "🔬", Color::Rgb(112, 201, 212)),
    ("fozzie", "🐻", Color::Rgb(237, 170, 114)),
    ("statler", "🎭", Color::Rgb(197, 167, 220)),
    ("waldorf", "🧐", Color::Rgb(137, 175, 240)),
    ("sprocket", "🐶", Color::Rgb(215, 196, 162)),
];
fn moppet_cast(name: &str) -> Option<&'static (&'static str, &'static str, Color)> {
    let name = name
        .split_whitespace()
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase();
    MOPPET_CAST.iter().find(|(key, _, _)| *key == name)
}
fn moppet_icon(name: &str) -> &'static str {
    moppet_cast(name).map(|(_, icon, _)| *icon).unwrap_or("🎬")
}
fn moppet_identity(name: &str) -> Color {
    moppet_cast(name)
        .map(|(_, _, colour)| *colour)
        .unwrap_or(Color::Rgb(236, 149, 149))
}
fn agent_glyph(word: &str) -> &'static str {
    match word {
        "WORKING" => {
            let frame = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|t| (t.as_millis() / 350) % 4)
                .unwrap_or(0);
            ["◐", "◓", "◑", "◒"][frame as usize]
        }
        "STALLED" => "◐",
        "ERROR" => "●",
        "REPLIED" => "◆",
        _ => "○",
    }
}
fn render_gang(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let tiles = gang_tiles(c);
    let selected = c.gang_selected.min(tiles.len() - 1);
    let count = (area.width as usize / 16).max(1).min(tiles.len());
    let start = selected.saturating_sub(count - 1).min(tiles.len() - count);
    let columns =
        Layout::horizontal((0..count).map(|_| Constraint::Ratio(1, count as u32))).split(area);
    for (offset, (name, identity, colour, word)) in tiles[start..start + count].iter().enumerate() {
        let active = start + offset == selected;
        let index = start + offset;
        agent_button(c, index, columns[offset], false);
        let title = fit_text(
            &format!(
                "{}{} {} {name}",
                if active { "› " } else { "" },
                agent_number(index),
                moppet_icon(name)
            ),
            columns[offset].width.saturating_sub(4) as usize,
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(Style::default().bg(if active { DESK_SELECTED } else { DESK_BG }))
            .border_style(Style::default().fg(*identity).add_modifier(if active {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }))
            .title(Span::styled(
                format!(" {title} "),
                if active {
                    Style::default()
                        .fg(DESK_BG)
                        .bg(*identity)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(*identity)
                },
            ));
        let line = styled(format!("{} {word}", agent_glyph(word)), *colour);
        f.render_widget(Paragraph::new(line).block(block), columns[offset]);
    }
}
fn agent_number(index: usize) -> String {
    match index {
        0..=8 => (index + 1).to_string(),
        9 => "0".into(),
        _ => "·".into(),
    }
}
fn agent_button(c: &Center, index: usize, area: ratatui::layout::Rect, tab: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let pane = c.state["agent_gang"][index]["pane_id"].as_str();
    let identity = pane.map(str::to_owned).unwrap_or_else(|| {
        if index + 1 == gang_tiles(c).len() {
            "sprocket".into()
        } else {
            index.to_string()
        }
    });
    let params = if let Some(pane) = pane {
        json!({"pane":pane})
    } else {
        json!({"index":index})
    };
    let label = gang_tiles(c)
        .get(index)
        .map(|g| g.0.clone())
        .unwrap_or_default();
    c.hits.borrow_mut().push(Hit {
        id: format!("agent-{}-{identity}", if tab { "tab" } else { "card" }),
        label,
        area,
        action: "agent".into(),
        params,
        enabled: c.poke.is_none() && !c.chat_editing && !c.show_form,
    });
}
fn render_tabs(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let line = tab_strip(c, area.width as usize);
    let names: Vec<_> = if c.scope.is_some() {
        vec!["booth", "mops", "animal"]
    } else {
        vec!["workshop", "booth", "mops", "animal"]
    };
    let mut offset = 0usize;
    for (i, span) in line.spans.iter().enumerate() {
        let width = Line::from(span.clone()).width();
        if i % 2 == 0 && i / 2 < names.len() {
            let name = names[i / 2];
            let available = (area.width as usize).saturating_sub(offset).min(width);
            if available > 0 {
                c.hits.borrow_mut().push(Hit {
                    id: format!("view-{name}"),
                    label: name.into(),
                    area: ratatui::layout::Rect::new(
                        area.x + offset as u16,
                        area.y,
                        available as u16,
                        1,
                    ),
                    action: "view".into(),
                    params: json!({"name":name}),
                    enabled: c.poke.is_none() && !c.chat_editing && !c.show_form,
                });
            }
        }
        offset += width;
    }
    f.render_widget(Paragraph::new(line), area);
}
/// A sliding strip keeps the selected agent visible even in a narrow terminal.
/// The agent band drawn vertically for tall, narrow panes: one row per agent, working ones
/// animated, with the task each is on. The selected agent stays in view when rows run out.
fn render_agent_list(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let block = round_panel("AGENTS · 1–9 select", TEAL, false);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let tiles = gang_tiles(c);
    let selected = c.gang_selected.min(tiles.len() - 1);
    let rows = (inner.height as usize).min(tiles.len());
    let start = selected.saturating_sub(rows - 1).min(tiles.len() - rows);
    for (offset, (name, identity, colour, word)) in tiles[start..start + rows].iter().enumerate() {
        let index = start + offset;
        let active = index == selected;
        let row = ratatui::layout::Rect::new(inner.x, inner.y + offset as u16, inner.width, 1);
        agent_button(c, index, row, false);
        let task = c.state["agent_gang"]
            .as_array()
            .and_then(|a| a.get(index))
            .and_then(|g| g["terminal_title_stripped"].as_str())
            .unwrap_or("");
        let head = format!(
            "{}{} {} {} ",
            if active { "›" } else { " " },
            agent_number(index),
            moppet_icon(name),
            name
        );
        let word_cell = format!("{} {word:<7}", agent_glyph(word));
        let room = (row.width as usize)
            .saturating_sub(Line::raw(&head).width() + Line::raw(&word_cell).width() + 1);
        let bg = if active { DESK_SELECTED } else { DESK_BG };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    head,
                    Style::default()
                        .fg(*identity)
                        .bg(bg)
                        .add_modifier(if active {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(word_cell, Style::default().fg(*colour).bg(bg)),
                Span::styled(
                    format!(" {}", fit_text(task, room)),
                    Style::default().fg(MUTED).bg(bg),
                ),
            ])),
            row,
        );
    }
}
fn agent_strip(c: &Center, width: usize) -> Line<'static> {
    agent_strip_at(c, width, None)
}
fn agent_strip_at(c: &Center, width: usize, area: Option<ratatui::layout::Rect>) -> Line<'static> {
    let tiles = gang_tiles(c);
    let selected = c.gang_selected.min(tiles.len() - 1);
    let label = |i: usize| {
        format!(
            " {} {} {} {} ",
            agent_number(i),
            moppet_icon(&tiles[i].0),
            agent_glyph(tiles[i].3),
            tiles[i].0
        )
    };
    let budget = width.saturating_sub(4);
    let mut start = selected;
    let mut end = selected + 1;
    let mut used = Line::raw(label(selected)).width();
    while start > 0 && used + Line::raw(label(start - 1)).width() <= budget {
        start -= 1;
        used += Line::raw(label(start)).width();
    }
    while end < tiles.len() && used + Line::raw(label(end)).width() <= budget {
        used += Line::raw(label(end)).width();
        end += 1;
    }
    let mut spans = vec![Span::styled(if start > 0 { "‹ " } else { "  " }, MUTED)];
    let mut offset = 2usize;
    for i in start..end {
        let text = fit_text(&label(i), budget);
        let cells = Line::raw(&text).width();
        if let Some(area) = area {
            agent_button(
                c,
                i,
                ratatui::layout::Rect::new(
                    area.x.saturating_add(offset as u16),
                    area.y,
                    cells as u16,
                    1,
                ),
                true,
            );
        }
        offset += cells;
        let style = if i == selected {
            Style::default()
                .fg(DESK_BG)
                .bg(tiles[i].1)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(tiles[i].1)
        };
        spans.push(Span::styled(text, style));
    }
    spans.push(Span::styled(
        if end < tiles.len() { " ›" } else { "  " },
        MUTED,
    ));
    Line::from(spans)
}

fn render_overview(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    if area.height >= 12 {
        let parts = Layout::vertical([Constraint::Length(9), Constraint::Min(3)]).split(area);
        booth::render_project(f, c, parts[0]);
        panels::render(f, c, parts[1]);
    } else {
        booth::render_project(f, c, area);
    }
}
fn render_desk(f: &mut Frame, c: &Center, control: bool) {
    let area = f.area();
    let narrow = area.width < 110;
    f.render_widget(
        Block::default().style(Style::default().bg(DESK_BG).fg(DESK_FG)),
        area,
    );
    // Narrow panes keep one status line (glyph per agent) rather than losing the band; MOPS draws its own.
    let agents = c.state["agent_gang"].as_array().map_or(0, Vec::len) + 1;
    let tall_narrow = narrow && !c.muppets_view && area.height >= 34;
    let gang_rows = if c.muppets_view {
        0
    } else if tall_narrow {
        ((agents + 2) as u16).min(area.height / 3).max(4)
    } else if narrow {
        1
    } else {
        3
    };
    let usage_lines = usage_readout_lines(c);
    let usage_rows = wrapped_rows(&usage_lines, area.width);
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(usage_rows),
        Constraint::Length(gang_rows),
        Constraint::Length(1),
    ])
    .split(area);
    f.render_widget(
        Paragraph::new(top_bar(c, area.width as usize, control)),
        chunks[0],
    );
    render_tabs(f, c, chunks[1]);
    if let Some(poke) = &c.poke {
        let lines = vec![
            styled(
                format!("{} {}", moppet_icon(&poke.name), poke.name),
                moppet_identity(&poke.name),
            ),
            styled(c.message.clone(), AMBER),
            styled(format!("{}▏", poke.text), DESK_FG),
        ];
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .block(round_panel(
                    "FIX NOTE · Enter send · Esc cancel",
                    moppet_identity(&poke.name),
                    true,
                )),
            chunks[2],
        );
    } else if c.onboarding.is_some() {
        roles::render_onboarding(f, c, chunks[2]);
    } else if c.chat_editing {
        render_conversation(f, c, chunks[2]);
    } else if c.event_open {
        render_event(f, c, chunks[2]);
    } else if c.config_view {
        render_config(f, c, chunks[2]);
    } else if c.telemetry_view {
        if c.scope.is_some() {
            roles::render_animal(f, c, chunks[2]);
        } else {
            render_telemetry(f, c, chunks[2]);
        }
    } else if control && !c.muppets_view {
        workshop::render(f, c, chunks[2]);
    } else if c.details_open {
        render_pulse(f, c, chunks[2]);
    } else if c.muppets_view {
        render_muppets(f, c, chunks[2]);
    } else {
        let compact_actions = (root_actions(c).len() as u16 * 2 + 2).clamp(4, 8);
        let bottom_height = if narrow { compact_actions + 12 } else { 15 };
        let body = Layout::vertical([Constraint::Min(8), Constraint::Length(bottom_height)])
            .split(chunks[2]);
        if narrow {
            let stacked = area.width >= 68;
            let p = Layout::vertical([
                Constraint::Length(if stacked { 6 } else { 9 }),
                Constraint::Length(if c.details_open {
                    9
                } else if body[0].height >= 14 {
                    4
                } else {
                    3
                }),
                Constraint::Min(3),
            ])
            .split(body[0]);
            f.render_widget(
                Paragraph::new(stage_lines(c, stacked)).block(round_panel(
                    "STAGES · [ / ] select",
                    TEAL,
                    true,
                )),
                p[0],
            );
            render_stage_activity(f, c, p[1]);
            if c.details_open {
                render_pulse(f, c, p[2]);
            } else {
                render_overview(f, c, p[2]);
            }
        } else {
            let p = Layout::horizontal([Constraint::Length(31), Constraint::Min(30)])
                .spacing(1)
                .split(body[0]);
            let stages = stage_lines(c, false);
            let mut rail = p[0];
            rail.height = wrapped_rows(&stages, rail.width.saturating_sub(2))
                .saturating_add(2)
                .min(rail.height);
            f.render_widget(
                Paragraph::new(stages)
                    .wrap(Wrap { trim: false })
                    .block(round_panel("STAGES · [ / ] select", TEAL, true)),
                rail,
            );
            let right = Layout::vertical([
                Constraint::Length(if c.details_open { 11 } else { 7 }),
                Constraint::Min(5),
            ])
            .split(p[1]);
            render_stage_activity(f, c, right[0]);
            render_overview(f, c, right[1]);
        }
        if narrow {
            let p = Layout::vertical([Constraint::Length(compact_actions), Constraint::Min(10)])
                .split(body[1]);
            let top = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(p[0]);
            render_actions(f, c, top[0], true);
            render_tail(f, c, top[1]);
            render_next(f, c, p[1]);
        } else {
            let p = Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
                .spacing(1)
                .split(body[1]);
            let left = Layout::vertical([Constraint::Length(compact_actions), Constraint::Min(3)])
                .split(p[0]);
            render_actions(f, c, left[0], true);
            render_tail(f, c, left[1]);
            render_next(f, c, p[1]);
        }
    }
    // Reserve actual rows; wrapping alone cannot make a clipped panel visible.
    // Ratatui Paragraph wrapping: https://docs.rs/ratatui/latest/ratatui/widgets/struct.Paragraph.html#method.wrap
    f.render_widget(Paragraph::new(usage_lines).wrap(Wrap { trim: false }), chunks[3]);
    if tall_narrow {
        render_agent_list(f, c, chunks[4]);
    } else if gang_rows >= 3 {
        render_gang(f, c, chunks[4]);
    } else if gang_rows == 1 {
        f.render_widget(
            Paragraph::new(agent_strip_at(c, chunks[4].width as usize, Some(chunks[4]))),
            chunks[4],
        );
    }
    let booth_open = c.state["ui"]["connection"] == "connected";
    let width = area.width as usize;
    let line = if let Some(poke) = &c.poke {
        styled(
            format!(
                " POKE {} › {}█ · Enter send · Esc cancel ",
                poke.name, poke.text
            ),
            TEAL,
        )
    } else if c.chat_editing {
        styled(" SPROKET · Enter Send · F9/Esc Close · Pg History ", MUTED)
    } else if c.message.starts_with("Restricted:")
        || c.message.starts_with("COPIED")
        || c.message.starts_with("Chat blocked:")
        || c.message.contains("error")
    {
        styled(format!(" {} ", c.message), AMBER)
    } else {
        let mut spans = vec![Span::styled(
            format!(" ◉ BACKSTAGE {} ", if booth_open { "●" } else { "○" }),
            if booth_open { GREEN } else { AMBER },
        )];
        if narrow && !c.muppets_view && !control {
            let room = width.saturating_sub(8 + Line::from(spans.clone()).width());
            spans.extend(agent_strip(c, room).spans);
        } else {
            let hints = if control && !c.muppets_view && !c.telemetry_view {
                "Arrows Select · Enter Booth · I Inactive · B Agent Booth · A Check in all · D Details · C Copy · G MOPS · ^Q Exit"
            } else if c.muppets_view {
                "1–9/0 Select · ← → Agent · Pg Scroll · F9 Sproket · ^Q Exit"
            } else {
                "G MOPS · E Events · [ ] Stage · , . Agent · A Check in all · F9 Sproket · D Details · C Copy · ^Q Exit"
            };
            for hint in hints.split(" · ") {
                let span = Span::styled(format!(" · {hint}"), MUTED);
                if Line::from(spans.clone()).width() + Line::from(span.clone()).width() + 8 <= width
                {
                    spans.push(span);
                }
            }
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(film_status(width, line)), chunks[5]);
}

/// The tab strip: four views, the active one filled, like a browser's tabs.
fn tab_strip(c: &Center, width: usize) -> Line<'static> {
    let no_project = !c.state["project"].is_string();
    let tabs = vec![
        (
            "WORKSHOP",
            "F1",
            c.control_view || (no_project && !c.muppets_view && !c.telemetry_view),
        ),
        (
            "BOOTH",
            "F2",
            !c.control_view && !no_project && !c.muppets_view && !c.telemetry_view,
        ),
        ("MOPS", "F3", c.muppets_view),
        ("MAYHEM", "F4", c.telemetry_view),
    ];
    let mut spans: Vec<Span<'static>> = Vec::new();
    let tabs: Vec<_> = tabs
        .into_iter()
        .filter(|(label, _, _)| c.scope.is_none() || *label != "WORKSHOP")
        .collect();
    let required: usize = tabs
        .iter()
        .map(|(label, key, _)| label.len() + key.len() + 3)
        .sum::<usize>()
        + 9;
    for (index, (label, key, active)) in tabs.iter().enumerate() {
        let label = if required <= width {
            *label
        } else if width >= 50 {
            match *label {
                "WORKSHOP" => "Work",
                "BOOTH" => "Booth",
                "MOPS" => "MOPS",
                "MAYHEM" => "Mayhem",
                "CONFIG" => "Config",
                _ => "?",
            }
        } else {
            ""
        };
        if index > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(DIM)));
        }
        if *active {
            spans.push(Span::styled(
                format!(" {key} {label} "),
                Style::default()
                    .fg(DESK_BG)
                    .bg(TEAL)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {key} {label} "),
                Style::default().fg(MUTED),
            ));
        }
    }
    // Right side: what the desk can see right now, so the strip is also a status line.
    let owner = &c.state["animation_worker"]["worker_owner"];
    let slot = if owner.is_object() {
        format!(
            "slot {}",
            owner["job"]
                .as_str()
                .unwrap_or("?")
                .rsplit('-')
                .next()
                .unwrap_or("?")
        )
    } else {
        format!(
            "slot {}",
            c.state["animation_worker"]["worker_owner_status"]
                .as_str()
                .unwrap_or("idle")
        )
    };
    let projects = c.state["catalog"]["projects"]
        .as_array()
        .map(|p| p.len())
        .unwrap_or(0);
    let right = if c.scope.is_some() {
        "  Project Booth · locked ".into()
    } else if c.view_name() == "workshop" {
        "  Arrows select · Enter Booth ".into()
    } else {
        format!("  {projects} projects · {slot} ")
    };
    let used: usize = spans
        .iter()
        .map(|s| s.content.chars().count())
        .sum::<usize>();
    if used + right.chars().count() < width {
        spans.push(Span::styled(right, Style::default().fg(MUTED)));
    }
    bounded_line(Line::from(spans), width)
}

fn render(f: &mut Frame, c: &Center) {
    c.hits.borrow_mut().clear();
    if c.show_form {
        render_form(f, c);
        return;
    }
    render_desk(
        f,
        c,
        c.scope.is_none() && (c.control_view || !c.state["project"].is_string()),
    );
}

pub fn start(p: Value, print: bool) -> Result<(), String> {
    let mut c = Center::new(&p)?;
    if !c.fields[0].is_empty() {
        c.session
            .execute("project.attach", json!({"path":c.fields[0]}))
            .map_err(|e| e.to_string())?;
        c.refresh();
    } else {
        c.refresh();
    }
    if print {
        let text = c.cli_text();
        println!(
            "MAGIC STUDIO / COMMAND CENTER\nProject: {}\nStatus: {}\nTranscript valid: {}\n{}\n{}\n{}",
            c.state["project"],
            crate::snapshot::status_line(&c.state["snapshot"]),
            c.state["transcript"]["valid"],
            c.message,
            text,
            c.details()
        );
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("Command Center needs an interactive terminal. Use center --print for inspection or run - for JSONL commands.".into());
    }
    // A restart waits for the terminal to be released by the instance it replaces.
    if let Some(delay) = std::env::var("MAGICSTUDIO_RESTART_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }
    match std::env::var("MAGICSTUDIO_VIEW").as_deref() {
        Ok("animal" | "telemetry") => c.telemetry_view = true,
        Ok("mops" | "muppets" | "moppets") => c.muppets_view = true,
        Ok("workshop") => c.control_view = true,
        _ => {}
    }
    let server = crate::tui::Server::bind()?;
    let mut reload = false;
    let mut frame_number = 0u64;
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(io::stdout(), crossterm::event::EnableMouseCapture);
    let result = (|| -> io::Result<()> {
        let mut refreshed = std::time::Instant::now();
        let mut worker = crate::worker::Poller::default();
        loop {
            c.poll_studios();
            c.poll_chat();
            c.poll_doctor();
            if refreshed.elapsed() > Duration::from_secs(1) {
                c.refresh();
                refreshed = std::time::Instant::now();
            }
            if let Some(observation) = worker.poll(c.state["project"].as_str()) {
                c.state["animation_worker"] = if let Some(scope) = &c.scope {
                    scope.redact(observation)
                } else {
                    observation
                };
            }
            let mut replies = Vec::new();
            let mut flow = control::Flow::Continue;
            for pending in server.receive() {
                let action = pending.request["action"].as_str().unwrap_or("look");
                let result = control::action(&mut c, action, &pending.request["params"]);
                if let Ok(next) = result.as_ref() {
                    if *next != control::Flow::Continue {
                        flow = *next;
                    }
                }
                replies.push((pending, result));
            }
            frame_number += 1;
            let frame = terminal.draw(|f| render(f, &c))?;
            for (pending, result) in replies {
                let mut look = control::look(&c, frame.buffer, frame_number, &server.endpoint());
                look["generation"] = json!(server.generation);
                look["action"] = pending.request["action"].clone();
                match result {
                    Ok(next) => {
                        look["ok"] = json!(true);
                        if next == control::Flow::Reload {
                            look["reload_requested"] = json!(true);
                        }
                    }
                    Err(error) => {
                        look["ok"] = json!(false);
                        look["error"] = json!(error);
                    }
                }
                pending.reply(&look);
            }
            if flow == control::Flow::Quit {
                break;
            }
            if flow == control::Flow::Reload {
                reload = true;
                break;
            }
            if !event::poll(Duration::from_millis(120))? {
                continue;
            }
            let key = match event::read()? {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => key,
                Event::Mouse(mouse)
                    if mouse.kind == event::MouseEventKind::Down(event::MouseButton::Left) =>
                {
                    match control::mouse(&mut c, mouse.column, mouse.row) {
                        Ok(control::Flow::Quit) => break,
                        Ok(control::Flow::Reload) => {
                            reload = true;
                            break;
                        }
                        Ok(control::Flow::Continue) => {}
                        Err(error) => c.message = error,
                    }
                    continue;
                }
                _ => continue,
            };
            match control::key(&mut c, key) {
                control::Flow::Continue => {}
                control::Flow::Quit => break,
                control::Flow::Reload => {
                    reload = true;
                    break;
                }
            }
        }
        Ok(())
    })();
    let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    drop(server);
    result.map_err(|e| e.to_string())?;
    if reload {
        control::reload(&c)?;
    }
    Ok(())
}
