//! Terminal presentation only. All project operations execute the shared engine registry.
use magicstudio_engine::Session;
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
    "Existing project (.magic)",
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
    agent_gang: Value,
    agent_gang_at: Option<std::time::Instant>,
    muppets_view: bool,
    catalog_selected: usize,
    spy: Option<String>,
    spy_at: Option<std::time::Instant>,
    pulse: Option<crate::telemetry::Sample>,
    pulse_at: Option<std::time::Instant>,
    scroll: u16,
    show_form: bool,
    control_view: bool,
    command_selected:usize,
    seen_cursor:u64,
    flash_after:u64,
    flash_started:Option<std::time::Instant>,
    pinned_instance:Option<String>,
    stage_selected:usize,
    details_open:bool,
    chat_draft:String,
    chat_editing:bool,
    chat_sent:String,
    chat_sent_request:Option<String>,
    doctor_receiver:Option<std::sync::mpsc::Receiver<Result<Value,String>>>,
    doctor_error:Option<String>,
    chat_receiver:Option<std::sync::mpsc::Receiver<Result<Value,String>>>,
}
impl Center {
    fn new(p: &Value) -> Result<Self, String> {
        let value = |k: &str| p[k].as_str().unwrap_or_default().to_string();
        let root = magicstudio_engine::project_settings::root().map_err(|e|e.to_string())?;
        Ok(Self{session:Session::user_session(),fields:[value("project"),p["root"].as_str().map(str::to_owned).unwrap_or_else(||root.to_string_lossy().into_owned()),value("name"),value("audio"),value("transcript"),value("captions"),value("audiomap"),value("materials")],selected:0,editing:false,show_form:false,control_view:false,command_selected:0,seen_cursor:0,flash_after:0,flash_started:None,pinned_instance:None,stage_selected:0,details_open:false,chat_draft:String::new(),chat_editing:false,chat_sent:String::new(),chat_sent_request:None,chat_receiver:None,doctor_receiver:None,doctor_error:None,state:json!({}),agent_gang:json!([]),agent_gang_at:None,muppets_view:false,catalog_selected:0,pulse:None,pulse_at:None,spy:None,spy_at:None,message:format!("Blue not started / red failed / yellow stalled / green working / [OK] completed. Production root: {}.",root.display()),scroll:0})
    }
    fn refresh(&mut self) {
        // No project is attached: the booth waits in the workshop for you to choose one.
        if self.session.project_path.is_none() { self.control_view=true; }
        let project=self.session.project_path.as_ref().map(|p|p.to_string_lossy().into_owned());
        let observe=|pin:Option<&str>|crate::local_ui::call("auto","ui.feedback",json!({"project":project,"_expected_instance":pin.map(str::to_owned)})).unwrap_or_else(|e|json!({"connection":"error","error":e}));
        let mut ui=observe(self.pinned_instance.as_deref());
        // Observation is read-only and project-matched, so a Studio that restarted under us is
        // re-discovered rather than pinned out. Commands still refuse to switch a live Studio.
        if ui["connection"]!="connected" && self.pinned_instance.is_some(){
            self.pinned_instance=None;
            ui=observe(None);
        }
        // Keep the last project inspection while the engine is busy; read its real journal directly.
        let inspection=if ui["connection"]=="connected" && ui["engine_unsaved"].is_null() && self.state["project"].is_string(){
            let mut value=self.state.clone();
            value["activity"]=self.session.execute("backstage.events",json!({"project":project,"limit":192})).unwrap_or_else(|e|json!({"error":e.to_string()}));
            Ok(value)
        }else{crate::dispatch(&mut self.session,"backstage.status",json!({"_origin":"Booth inspection","_expected_instance":self.pinned_instance}))};
        match inspection {
            Ok(mut value)=>{
                if self.session.project_path.is_some() && ui["engine_unsaved"].is_boolean(){
                    let params=json!({"_expected_instance":self.pinned_instance});
                    match crate::dispatch(&mut self.session,"agent.log",json!({"refresh":true,"_expected_instance":self.pinned_instance})){Ok(turns)=>{value["chat_turns"]=turns;},Err(error)=>{value["chat_error"]=json!(error);}}
                    value["chat"]=crate::dispatch(&mut self.session,"agent.session",params).unwrap_or_else(|error|json!({"error":error}));
                }else{value["chat"]=self.state["chat"].clone();value["chat_turns"]=self.state["chat_turns"].clone();}
                value["ui"]=ui;
                if self.pinned_instance.is_none(){self.pinned_instance=value["ui"]["instance"].as_str().map(str::to_owned);}
                value["catalog"]=self.session.execute("project.catalog",json!({})).unwrap_or_else(|e|json!({"error":e.to_string()}));
                value["animation_worker"]=self.state["animation_worker"].clone();
                let live=crate::snapshot::live(&value["ui"],&self.state["snapshot"]);
                value["snapshot"]=crate::snapshot::build(value["project"].as_str(),&value["ui"],&live,&value["activity"]);
                value["tail"]=merge_tail(&self.state["tail"],crate::snapshot::tail(&value["activity"],value["snapshot"]["project"].as_str()));
                let cursor=value["activity"]["next_cursor"].as_u64().unwrap_or(0);
                if self.seen_cursor>0&&cursor>self.seen_cursor{self.flash_after=self.seen_cursor;self.flash_started=Some(std::time::Instant::now());}
                self.seen_cursor=cursor;self.state=value;
                if self.agent_gang_at.map(|t|t.elapsed().as_secs()>=3).unwrap_or(true) {
                    if let Ok(out)=std::process::Command::new("herdr").args(["agent","list"]).output() {
                        if let Ok(v)=serde_json::from_slice::<Value>(&out.stdout) {
                            let mut agents=v["result"]["agents"].as_array().cloned().unwrap_or_default();
                            if agents.iter().any(|a|!a["name"].is_string()) {
                                if let Ok(pout)=std::process::Command::new("herdr").args(["pane","list"]).output()
                                    && let Ok(pv)=serde_json::from_slice::<Value>(&pout.stdout)
                                    && let Some(panes)=pv["result"]["panes"].as_array()
                                {
                                    for a in &mut agents {
                                        if !a["name"].is_string()
                                            && let Some(pid)=a["pane_id"].as_str()
                                            && let Some(lbl)=panes.iter().find(|p|p["pane_id"].as_str()==Some(pid)).and_then(|p|p["label"].as_str())
                                        {
                                            a["name"]=json!(lbl);
                                        }
                                    }
                                }
                            }
                            agents.sort_by_key(|a|(!a["name"].is_string(),a["workspace_id"].as_str()!=Some("wG")));
                            self.agent_gang=json!(agents);
                            self.agent_gang_at=Some(std::time::Instant::now());
                        }
                    }
                }
                self.state["agent_gang"]=self.agent_gang.clone();
                // The backstage watch: read what an agent last said, straight from its pane.
                if self.spy_at.map(|t|t.elapsed().as_secs()>=5).unwrap_or(true){
                    if let Some(name)=self.state["agent_gang"].as_array().and_then(|a|a.first()).and_then(|g|g["name"].as_str().map(str::to_owned)){
                        if let Ok(out)=std::process::Command::new("herdr").args(["agent","read",&name,"--source","recent-unwrapped","--lines","40"]).output(){
                            if let Ok(text)=String::from_utf8(out.stdout){self.spy=Some(text.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"));}
                        }
                    }
                    self.spy_at=Some(std::time::Instant::now());
                }
                self.sample_studio();
                if self.message.starts_with("Chat ") && !self.message.starts_with("Chat error:") && self.chat_receiver.is_none(){self.message=chat_outcome(&self.state);}
                self.observe_chat_delivery();

            },Err(error)=>{self.message=error.clone();self.state["ui"]=json!({"connection":"stale_or_unavailable","ui_ready":false,"error":error,"recovery":"F8 explicitly reattaches the same project; Studio is never switched."});},
        }
    }
    fn studio_pid(&self)->Option<u32>{
        let ui=&self.state["ui"];
        if ui["connection"]!="connected" || !self.state["project"].is_string() || ui["active_project"]!=self.state["project"] {return None;}
        let instance=ui["instance"].as_str()?;
        if self.pinned_instance.as_deref().is_some_and(|pin|pin!=instance){return None;}
        crate::telemetry::pid_from_socket(std::path::Path::new(instance))
    }
    fn sample_studio(&mut self){
        let Some(pid)=self.studio_pid() else {self.pulse=None;self.pulse_at=None;return;};
        let instance=self.state["ui"]["instance"].as_str().unwrap_or_default();
        let changed=self.pulse.as_ref().is_none_or(|p|p.pid!=pid || p.instance!=instance);
        if changed || self.pulse_at.is_none_or(|t|t.elapsed()>=Duration::from_secs(2)){
            self.pulse=crate::telemetry::Sample::capture(instance,self.pulse.as_ref());
            self.pulse_at=Some(std::time::Instant::now());
        }
    }
    fn studio_sample(&self)->Option<&crate::telemetry::Sample>{
        self.pulse.as_ref().filter(|p|Some(p.pid)==self.studio_pid() && self.state["ui"]["instance"].as_str()==Some(p.instance.as_str()) && p.at.elapsed()<Duration::from_secs(6))
    }
    /// Attach the project chosen in the workshop and watch it from the booth.
    fn observe_selected(&mut self){
        let Some(path)=self.state["catalog"]["projects"].as_array().and_then(|p|p.get(self.catalog_selected)).and_then(|p|p["path"].as_str().map(str::to_owned)) else {return;};
        if self.session.execute("project.attach",json!({"path":path})).is_err(){return;}
        self.fields[0]=path;
        self.control_view=false;
        self.muppets_view=false;
        self.pinned_instance=None;
        self.refresh();
    }
    fn observe_chat_delivery(&mut self){
        let receipt=&self.state["chat"]["request"];
        if self.chat_sent_request.as_deref()==receipt["request_id"].as_str() && self.chat_sent_request.is_some() && matches!(receipt["status"].as_str(),Some("running"|"replied"|"stalled")){
            if self.chat_draft==self.chat_sent{self.chat_draft.clear();self.chat_editing=false;}
            self.message=chat_outcome(&self.state);
        }
    }
    fn poll_chat(&mut self){
        if let Some(result)=self.chat_receiver.as_ref().and_then(|r|r.try_recv().ok()){
            self.chat_receiver=None;
            self.message=match result {Ok(_r)=>{if self.chat_draft==self.chat_sent{self.chat_draft.clear();}chat_outcome(&self.state)},Err(e)=>format!("Chat error: {e}. Draft retained; inspect existing receipt before retry.")};
            self.refresh();
        }
    }
    fn send_chat(&mut self){
        if self.chat_receiver.is_some() || self.chat_draft.trim().is_empty(){return;}
        let (Some(endpoint),Some(project),Some(instance))=(self.state["ui"]["endpoint"].as_str(),self.state["project"].as_str(),self.pinned_instance.as_deref()) else {self.message="Chat blocked: no matched pinned Studio/project; no message sent".into();return;};
        if self.state["ui"]["active_project"].as_str()!=Some(project){self.message="Chat blocked: Studio project differs; no message sent".into();return;}
        let Ok(exe)=std::env::current_exe()else{self.message="Chat blocked: current CLI executable unavailable".into();return;};
        let request_id=format!("booth-chat-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_nanos()).unwrap_or(0));
        self.chat_sent_request=Some(request_id.clone());
        let params=json!({"message":self.chat_draft.trim(),"new_chat":!self.state["chat"]["session"].is_string(),"session":self.state["chat"]["session"],"wait":0,"_expected_project":project,"_expected_instance":instance,"_request_id":request_id,"_origin":"Booth chat","_client":"Booth user"});
        let endpoint=endpoint.to_owned();let (tx,rx)=std::sync::mpsc::channel();self.chat_receiver=Some(rx);self.chat_sent=self.chat_draft.clone();self.message="Chat queued locally; CLI acceptance and reply not yet confirmed".into();
        std::thread::spawn(move||{
            let mut command=std::process::Command::new(exe);command.args(["--bridge",&endpoint,"exec","agent.send"]);
            if let Some(fields)=params.as_object(){for(k,v)in fields{command.arg(format!("{k}={v}"));}}
            let result=command.output().map_err(|e|e.to_string()).and_then(|o|{if !o.status.success(){return Err(String::from_utf8_lossy(&o.stderr).into_owned());}serde_json::from_slice::<Value>(&o.stdout).map_err(|e|e.to_string()).and_then(|r|if r["ok"]==false{Err(r.to_string())}else{Ok(r)})});let _=tx.send(result);
        });
    }
    fn poll_doctor(&mut self){
        if let Some(result)=self.doctor_receiver.as_ref().and_then(|r|r.try_recv().ok()){
            self.doctor_receiver=None;
            match result{Ok(value)=>{self.state["storyboard_doctor"]=value;self.doctor_error=None;},Err(e)=>{self.doctor_error=Some(e.clone());self.message=format!("Doctor check failed: {e}");}}
            self.refresh();
        }
    }
    fn check_storyboard(&mut self){
        if self.doctor_receiver.is_some(){return;}
        self.stage_selected=STORYBOARD_STAGE;self.details_open=true;self.doctor_error=None;
        let (Some(endpoint),Some(project),Some(instance))=(self.state["ui"]["endpoint"].as_str(),self.state["project"].as_str(),self.pinned_instance.as_deref())else{self.doctor_error=Some("No matched Studio project; F8 reattach before checking".into());return;};
        if self.state["ui"]["active_project"].as_str()!=Some(project){self.doctor_error=Some("Studio project differs; no check launched".into());return;}
        let Ok(exe)=std::env::current_exe()else{self.doctor_error=Some("Current CLI executable unavailable".into());return;};
        let endpoint=endpoint.to_owned();let project=project.to_owned();let instance=instance.to_owned();let(tx,rx)=std::sync::mpsc::channel();self.doctor_receiver=Some(rx);
        std::thread::spawn(move||{
            let result=std::process::Command::new(exe).args(["--bridge",&endpoint,"storyboard","--doctor",&format!("_expected_project={project}"),&format!("_expected_instance={instance}")]).output().map_err(|e|e.to_string()).and_then(|o|serde_json::from_slice::<Value>(&o.stdout).map_err(|_|String::from_utf8_lossy(&o.stderr).into_owned()));let _=tx.send(result);
        });
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
                    if id=="project.intake" {format!("INTAKE {}. Native storyboard: {}. Studio verification: pending.",if v["intake_ready"]==true{"READY"}else{"NEEDS WORK"},if v["storyboard_ready"]==true{"created or reopened"}else{"not ready"})} else {format!("COMMAND SUCCEEDED: {id}. This is not storyboard or Studio readiness.")}
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
        text.push_str("\nREAD THE MANUAL\nmc commands workshop.import_transcript\ncrates/engine/src/workshop.rs\n~/.agents/skills/media-use/audio/references/transcribe.md\n\nF7 invokes the existing native DeckCraft storyboard.init command.\n\nThis monitor does not initiate production. Explicit project.intake can run configured audio analysis when valid timing is missing; supplied valid timing is reused. Missing prerequisites remain visible.\n");
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
                "  MAGIC STUDIO ",
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
const DIM: Color=Color::Rgb(120,132,155);
const FILM: Color=Color::Rgb(92,74,42);
fn shell_quote(value:&str)->String{format!("'{}'",value.replace('\'',"'\\''"))}
fn styled(value:impl Into<String>,color:Color)->Line<'static>{Line::styled(value.into(),Style::default().fg(color))}
impl Center{
 fn copy_commands(&self)->Vec<(&str,String)>{
  let exe=shell_quote(&std::env::current_exe().map(|p|p.to_string_lossy().into_owned()).unwrap_or_else(|_|"mc".into()));
   let project=self.state["project"].as_str().unwrap_or("");
   if project.is_empty(){return vec![("List projects to observe in WORKSHOP",format!("{exe} exec project.catalog"))];}
   if let Some(worker)=worker_observation(self).filter(|v|v["state"]=="failed" || v["state"]=="cancelling" || v["observation_error"].is_string())
       && let Some(job)=worker["job"].as_str(){
       return vec![("Inspect this render's authoritative status and receipt",format!("{exe} exec blender.status {} {}",shell_quote(&format!("project={project}")),shell_quote(&format!("job={job}"))))];
   }
  let cursor=self.state["activity"]["next_cursor"].as_u64().unwrap_or(0);
  let inspection=if let (Some(endpoint),Some(instance))=(self.state["ui"]["endpoint"].as_str(),self.pinned_instance.as_deref()){
   format!("{exe} --bridge {} exec workshop.status {} {}",shell_quote(endpoint),shell_quote(&format!("_expected_project={project}")),shell_quote(&format!("_expected_instance={instance}")))
  }else if !project.is_empty(){
   let attach=json!({"id":"project.attach","params":{"path":project}}).to_string();
   let status=json!({"id":"center.status","params":{}}).to_string();
   format!("printf '%s\\n' {} {} | {exe} run -",shell_quote(&attach),shell_quote(&status))
  }else{format!("{exe} exec project.catalog")};
  let feedback=format!("{exe} exec ui.feedback {}",shell_quote(&format!("project={project}")));
  let receipts=format!("{exe} exec center.events after={cursor} limit=192 {}",shell_quote(&format!("project={project}")));
  let actions=root_actions(self);
  if let Some(failed)=actions.iter().find(|a|root_finish(a).is_some_and(|e|matches!(e["stage"].as_str(),Some("engine_error"|"handler_error"|"engine_needs_work")))){
   let command=root_start(failed).map(target).unwrap_or("");
   return vec![("Read the failed action's actual receipts",receipts),("Inspect the failed command's supported parameters",format!("{exe} commands {}",shell_quote(command)))];
  }
  if actions.iter().any(|a|stages(a).iter().any(|e|e["stage"]=="dispatch_running")&&root_finish(a).is_none()){
   return vec![("Read completion of the running action",receipts)];
  }
  if self.state["ui"]["connection"]!="connected" || self.state["ui"]["ui_ready"]!=true{
   return vec![("Check this project's missing Studio observation",feedback),("Read the action awaiting completion",receipts)];
  }
  if let Some(next)=self.state["next_commands"].as_array().filter(|v|!v.is_empty()){
   let mut commands=Vec::new();let mut seen=std::collections::HashSet::new();
   for item in next {
    if let (Some(id),Some(endpoint),Some(instance))=(item["command"].as_str(),self.state["ui"]["endpoint"].as_str(),self.pinned_instance.as_deref()){
     // Older shared observations used the CLI help verb; normalize to the existing registry query.
     let (command,params)=if item["operation"]=="commands"{("command.list",json!({"filter":id}))}else{(id,item["params"].clone())};
     let key=format!("{command}:{params}");if !seen.insert(key){continue;}
     let mut literal=format!("{exe} --bridge {} exec {}",shell_quote(endpoint),shell_quote(command));
     for(k,v)in params.as_object().into_iter().flatten(){literal.push(' ');literal.push_str(&shell_quote(&format!("{k}={}",v.as_str().map(str::to_owned).unwrap_or_else(||v.to_string()))));}
     // The private endpoint already pins the instance; local_ui validates the project and adds its nonce guard.
     let _=instance;
     literal.push_str(&format!(" {}",shell_quote(&format!("_expected_project={project}"))));
     commands.push((item["label"].as_str().unwrap_or("See command arguments"),literal));
    }
   }
   if !commands.is_empty(){return commands;}
  }
  if let (Some(endpoint),Some(instance))=(self.state["ui"]["endpoint"].as_str(),self.pinned_instance.as_deref()){
   let design=format!("{exe} --bridge {} exec storyboard.design {} {}",shell_quote(endpoint),shell_quote(&format!("_expected_project={project}")),shell_quote(&format!("_expected_instance={instance}")));
   vec![("Inspect the active saved deck and pending image edits",design),("Read the last action's completion receipts",receipts)]
  }else{vec![("Inspect this project's saved state",inspection)]}
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
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(accent).add_modifier(if active{Modifier::BOLD}else{Modifier::empty()}))
        .title(Span::styled(format!(" {} ", title.into()), Style::default().fg(accent).add_modifier(if active{Modifier::BOLD}else{Modifier::empty()})))
}
/// A quiet letterbox bar: dim sprocket holes, a dim label and an optional dim corner caption.
fn letterbox(width: usize, label: &str, caption: &str) -> Line<'static> {
    let holes=|count:usize|{let mut out=String::new();while out.chars().count()<count{out.push_str("▪▪▪ ");}out.chars().take(count).collect::<String>()};
    let used=label.chars().count()+caption.chars().count()+2;
    if width<=used{return styled(format!("{label} {caption}").trim().to_string(),MUTED);}
    let spare=width-used;let mut spans=vec![Span::styled(holes(spare/3),FILM),Span::styled(format!(" {label} "),MUTED),Span::styled(holes(spare-spare/3),FILM)];
    if !caption.is_empty(){spans.push(Span::styled(format!(" {caption} "),DIM));}
    Line::from(spans)
}
fn top_bar(c:&Center,width:usize,control:bool)->Line<'static>{
    let name=c.state["project"].as_str().and_then(|p|std::path::Path::new(p).file_stem()).and_then(|p|p.to_str()).unwrap_or("BACKSTAGE");
    let view=if c.muppets_view{"MOPPETS"}else if control{"WORKSHOP"}else{"BOOTH"};
    let side=(width/3).min(36);
    let title=fit_text(&format!("{name} · {view}"),width.saturating_sub(side*2));
    let title_width=Line::raw(&title).width();
    let start=width.saturating_sub(title_width)/2;
    let room=start.saturating_sub(1);
    let metrics=match c.studio_sample(){
        Some(sample)=>format!("Studio CPU {} · RSS {:.0}MiB",sample.cpu_pct.map(|v|format!("{v:.0}%")).unwrap_or("?".into()),crate::telemetry::rss_mb(&sample.procs)),
        None=>"Studio CPU ? · RSS ?".into(),
    };
    let left=fit_text(&metrics,room);
    let ui=&c.state["ui"];
    let matched=ui["connection"]=="connected" && c.state["project"].is_string() && ui["active_project"]==c.state["project"];
    let (status,colour)=if !control && let Some(worker)=worker_observation(c).filter(|v|v["state"]!="not_started"){
        (crate::worker::headline(worker),worker_colour(worker))
    }else if c.state["snapshot"]["last_error"].as_str().is_some_and(|s|!s.is_empty()){
        ("! ERROR · D PULSE".into(),RED)
    }else if matched{
        let snap=&c.state["snapshot"];
        let position=match snap["playhead_sec"].as_f64(){Some(seconds)=>timecode(seconds,snap["fps"].as_f64().unwrap_or(24.0)),None=>"position ?".into()};
        (format!("LIVE · {position}"),GREEN)
    }else{("NOT CONNECTED".into(),AMBER)};
    let right_room=width.saturating_sub(start+title_width+1);
    let right=fit_text(&status,right_room);
    let mut spans=vec![Span::styled(left.clone(),MUTED),Span::raw(" ".repeat(start.saturating_sub(Line::raw(&left).width())))];
    spans.extend(crate::logo::compact_title(&title).spans);
    spans.push(Span::raw(" ".repeat(width.saturating_sub(start+title_width+Line::raw(&right).width()))));
    spans.push(Span::styled(right,colour));
    Line::from(spans)
}
/// SMPTE-style timecode from a playhead position.
fn timecode(seconds: f64, fps: f64) -> String {
    let fps=if fps.is_finite()&&fps>=1.0{fps.round()}else{24.0};
    let total=seconds.max(0.0);let whole=total.floor();let secs=whole as u64;
    let frames=(((total-whole)*fps).floor().max(0.0)).min(fps-1.0) as u64;
    format!("{:02}:{:02}:{:02}:{:02}",secs/3600,(secs/60)%60,secs%60,frames)
}
/// Broadcast tally: the word always travels with the colour.
fn tally(status:&str)->(Color,&'static str){
    match status{
        "working"|"busy"=> (AMBER,"WORKING"),
        "stalled"|"blocked"|"error"=> (RED,"STALLED"),
        "idle"|"done"=> (GREEN,"IDLE"),
        _=> (MUTED,"UNKNOWN"),
    }
}
fn root_actions(c: &Center) -> Vec<&Value> {
    c.state["activity"]["actions"].as_array().into_iter().flatten().rev().filter(|a|stages(a).iter().any(|e|matches!(e["stage"].as_str(),Some("dispatch_running"|"ui_click")))).take(3).collect()
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
        Some("engine_success") => {
            let status=root_finish(action).and_then(|e|e["status"].as_str()).or_else(||stages(action).into_iter().rev().find_map(|e|e["outputs"]["status"].as_str()));
            match status{Some("running")=>("● WORKING",GREEN),Some("stalled")=>("● STALLED",AMBER),Some("error")=>("● FAILED",RED),_=>("[OK] SUCCEEDED",DESK_FG)}
        },
        Some("engine_error" | "handler_error") => ("✗ FAILED", RED),
        Some("engine_needs_work") => ("! NEEDS WORK", AMBER),
        _ if stages(action).iter().any(|e| e["stage"] == "dispatch_running") => ("[>] RUNNING", GREEN),
        _ if root_start(action).is_some_and(|e| e["interaction"] == "navigation") => ("— UI NAVIGATION", MUTED),
        _ if stages(action).iter().any(|e| e["stage"] == "ui_click") => ("— UI INTERACTION", MUTED),
        _ => ("! COMPLETION UNKNOWN", AMBER),
    }
}
fn action_ack(action: &Value) -> bool {
    stages(action).iter().any(|e| matches!(e["stage"].as_str(), Some("frontend_ack" | "ui_navigation_applied")) && (
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
    let count=bins.iter().sum::<usize>();
    let running=root_actions(c).into_iter().find(|a|matches!(engine_badge(a).0,"● WORKING"|"[>] RUNNING"));
    let state=running.and_then(|a|root_start(a)).map(|e|format!("[>] {} · {}s elapsed",target(e),now.saturating_sub(e["unix_micros"].as_u64().unwrap_or(now*1_000_000)/1_000_000))).unwrap_or_else(||"idle".into());
    format!("activity {wave} · {count} receipts/8s · {state}")
}
fn action_lines(c: &Center, action: &Value, compact: bool) -> Vec<Line<'static>> {
    let Some(start) = root_start(action) else { return vec![styled("— action details unavailable", MUTED)]; };
    let first = action["first_sequence"].as_u64(); let last = action["last_sequence"].as_u64();
    let sequence = match (first,last) { (Some(a),Some(b)) if a != b => format!("#{a}–{b}"), (Some(a),_) => format!("#{a}"), _ => "#?".into() };
    let (engine, color) = engine_badge(action);
    let mut heading = styled(format!("{}{}  ·  {} UTC",if c.details_open{format!("{sequence}  ")}else{String::new()},target(start),stamp(start)), if color == RED { RED } else { BLUE });
    if c.flash_started.is_some_and(|at| at.elapsed() < Duration::from_millis(800)) && last.is_some_and(|seq| seq > c.flash_after) {
        heading = heading.style(Style::default().fg(if color == RED { RED } else { BLUE }).bg(DESK_SELECTED));
    }
    let ack = action_ack(action);
    let navigation=root_start(action).is_some_and(|e| e["interaction"]=="navigation");
    let ui_result=if ack {if navigation {"NAVIGATION APPLIED"}else{"RESULT RENDERED"}}else if navigation {"navigation receipt pending"}else if engine_badge(action).0=="[>] RUNNING" {"waiting for result/render"}else {"no correlated Studio receipt"};
    let mut lines = vec![heading];
    if compact {
        lines.push(Line::from(vec![Span::styled(format!("  {engine}  "), Style::default().fg(color)),
            Span::styled(format!("{} {ui_result}",if ack {"▣"}else{"—"}), Style::default().fg(if ack { DESK_FG } else { AMBER }))]));
    } else {
        lines.push(styled(format!(" ├─ ENGINE: {engine}"), color));
        lines.push(styled(format!(" ├─ STUDIO: {} {ui_result}{}",if ack {"▣"}else{"—"},if ack && !navigation {" · visual review separate"}else{""}), if ack { DESK_FG } else { AMBER }));
        lines.push(styled(format!(" └─ AGENT: {} · {}", start["origin"].as_str().unwrap_or("unknown"), start["client_label"].as_str().unwrap_or("unverified")), MUTED));
    }
    if let Some(error) = root_finish(action).and_then(|e| e["error"].as_str()).filter(|_| !compact) { lines.push(styled(format!("    {}", error.strip_prefix("invalid parameters: ").unwrap_or(error)), RED)); }
    lines
}
fn command_preview(command: &str) -> String {
    let exe = std::env::current_exe().ok().map(|p| shell_quote(&p.to_string_lossy()));
    exe.as_deref().and_then(|exe| command.strip_prefix(exe)).map(|rest| format!("<mc>{rest}")).unwrap_or_else(|| command.into())
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
        lines.push(styled(format!("{}  ·  ↑↓ choose", (0..commands.len()).map(|i|format!("{}[{}]", if i == selected { ">" } else { " " }, i + 1)).collect::<Vec<_>>().join(" ")), BLUE));
        lines.push(styled(format!("> [{}] {effect}", selected + 1), BLUE));
        lines.push(styled(format!("    {}", command_preview(literal)), MUTED));
    } else {
        for (i,(effect,command)) in commands.iter().enumerate() {
            if i == selected { selected_row = wrapped_rows(&lines, parts[0].width); }
            lines.push(styled(format!("{} [{}] {}", if i == selected { ">" } else { " " }, i + 1, effect), if i == selected { TEAL } else { DESK_FG }).style(Style::default().fg(if i==selected {TEAL}else{DESK_FG}).add_modifier(if i==selected {Modifier::BOLD}else{Modifier::empty()})));
            lines.push(styled(format!("       {}", command_preview(command)), BLUE));
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
    let block = round_panel("Last actions · newest first · Pg scroll", BLUE, false);
    let inner = block.inner(area);
    let max_scroll = wrapped_rows(&lines, inner.width).saturating_sub(inner.height);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((c.scroll.min(max_scroll), 0)).block(block), area);
}
fn render_catalog(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let projects: Vec<&Value> = c.state["catalog"]["projects"].as_array().into_iter().flatten().collect();
    let block = round_panel("ALL PROJECTS · shared Home recent catalog", TEAL, false);
    let inner = block.inner(area); let width = inner.width as usize;
    let mut lines = vec![styled("Existing Home catalog · j/k choose · Enter observes from the booth · readiness is not approval", MUTED)];
    let widths = if width >= 100 { [width * 24 / 100, width * 20 / 100, width * 25 / 100, width * 12 / 100, width * 19 / 100] } else { [width,0,0,0,0] };
    if width >= 100 { lines.push(styled(format!("{}{}{}{}{}", padded("PROJECT",widths[0]), padded("ENGINE",widths[1]), padded("STUDIO UI",widths[2]), padded("REV LAG",widths[3]), padded("LAST ACTION",widths[4])), MUTED)); }
    let actions = root_actions(c);
    for (index, project) in projects.iter().enumerate() {
        let current = project["path"] == c.state["project"] && c.state["project"].is_string();
        let ui = &c.state["ui"];
        let matched = current && ui["active_project"] == c.state["project"] && ui["connection"] == "connected";
        let engine = if current { actions.first().map(|a|engine_badge(a)).unwrap_or(("— UNKNOWN", DESK_FG)) } else { ("— UNKNOWN", DESK_FG) };
        let ack = if matched && ui["ui_ready"] == true { ("▣ UI UPDATED",GREEN) } else { ("! ACK UNKNOWN / MISSING",AMBER) };
        let picked = index == c.catalog_selected;
        let name = format!("{}{} {}", if current { ">" } else { " " }, if picked { "›" } else { " " }, project["name"].as_str().unwrap_or("unnamed"));
        if width >= 100 {
            lines.push(Line::from(vec![Span::styled(padded(&name,widths[0]),Style::default().fg(if current { BLUE } else { DESK_FG })), Span::styled(padded(engine.0,widths[1]),Style::default().fg(engine.1)), Span::styled(padded(ack.0,widths[2]),Style::default().fg(ack.1)), Span::styled(padded(&lag(ui,matched),widths[3]),Style::default().fg(if matched && ui["ui_ready"] == true { GREEN } else { AMBER })), Span::styled(padded(if current { actions.first().and_then(|a|root_start(a)).map(target).unwrap_or("—") } else { "— unobserved" },widths[4]),Style::default().fg(MUTED))]));
        } else {
            lines.push(styled(name, if picked { TEAL } else { BLUE }));
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
#[allow(dead_code)]
fn saved_observation(ui:&Value)->(&'static str,Color){
 if ui["connection"]!="connected" {return ("? Saved state unobserved",AMBER);}
 if ui["draft_fields"].as_array().is_some_and(|v|!v.is_empty()){return ("! Unsaved form edits",AMBER);}
 match ui["engine_unsaved"].as_bool(){Some(true)=>("! Engine differs from saved project",AMBER),Some(false)=>("[OK] Saved project matches",DESK_FG),None=>("? Saved state unknown; engine busy/unreadable",AMBER)}
}
// These are views of CLI-owned evidence, never an independent readiness evaluator.
// The engine reports lyrics and notes as one "lyrics" item; official artist lyrics are not assessed.
const PROJECT_STAGES:[(&str,&str);7]=[("Audio","song"),("Transcript","transcript"),("Lyrics & notes","lyrics"),("Reference art","assets"),("Storyboard","deckcraft"),("Animation","animate"),("Video / captions","hyperframes")];
const STORYBOARD_STAGE:usize=4;
fn stage_evidence(c:&Center,index:usize)->Option<&Value>{let id=PROJECT_STAGES[index%PROJECT_STAGES.len()].1;c.state["prerequisites"].as_array()?.iter().find(|v|v["id"]==id)}
fn state_badge(state:Option<&str>)->(&'static str,Color){match state{Some("ready"|"replied")=>("[OK]",DESK_FG),Some("running"|"working")=>("● working",GREEN),Some("blocked"|"error"|"failed")=>("● failed",RED),Some("stalled")=>("● stalled",AMBER),Some("missing")=>("● missing",BLUE),_=>("? unknown",MUTED)}}
fn chat_outcome(state:&Value)->String{let r=&state["chat"]["request"];let (mark,_)=state_badge(r["status"].as_str());format!("Chat {mark} · {}",r["request_id"].as_str().unwrap_or("not started"))}
fn selected_reason(c:&Center)->String{stage_evidence(c,c.stage_selected).and_then(|v|v["reason"].as_str()).unwrap_or("No separate CLI observation for this input. Transcript text does not establish official artist lyrics or liner notes.").into()}
fn stage_lines(c:&Center,stacked:bool)->Vec<Line<'static>>{
 let mut lines:Vec<Line<'static>>=Vec::new();for(i,(label,_))in PROJECT_STAGES.iter().enumerate(){let (mark,color)=state_badge(stage_evidence(c,i).and_then(|v|v["state"].as_str()));let line=Line::from(vec![Span::styled(if i==c.stage_selected{"> "}else{"  "},Style::default().fg(DESK_FG)),Span::styled(format!("{label}: {mark}"),Style::default().fg(color))]);
 if stacked&&i%2==1{let mut previous=lines.pop().unwrap_or_default();previous.spans.push(Span::raw("   "));previous.spans.extend(line.spans);lines.push(previous);}else{lines.push(line);}}
 lines
}
fn doctor_badge(c:&Center)->(&'static str,Color){
 let d=&c.state["storyboard_doctor"];
 if c.doctor_receiver.is_some(){("● Checking",GREEN)}else if c.doctor_error.is_some(){("● Check failed",RED)}else if d.is_null(){("● Not checked",BLUE)}else if d["stale"]==true{("● Needs review · stale",AMBER)}else if d["healthy"]==true{("✓ Checked · section boundary only",DESK_FG)}else{("● Needs review",AMBER)}
}
fn render_stage_activity(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
  if let Some(worker)=worker_observation(c){render_worker(f,worker,area);return;}
 let mut lines=if area.height<4{vec![styled(activity_wave(c),MUTED)]}else{vec![styled(selected_reason(c),DESK_FG),styled(activity_wave(c),MUTED)]};
 if c.stage_selected==STORYBOARD_STAGE{
    let(mark,color)=doctor_badge(c);lines.insert(0,styled(format!("F10 🩺 [+] Storyboard Doctor · {mark}"),color));
    let d=&c.state["storyboard_doctor"];
    if let Some(error)=&c.doctor_error{lines.push(styled(error,RED));}
    if !d.is_null(){
        let time=d["assessed_at_unix_ms"].as_u64().unwrap_or(0)/1000;let rev=d["storyboard_revision"].as_str().unwrap_or("unknown");
        lines.push(styled(format!("Checked {:02}:{:02}:{:02} UTC · input {}",time/3600%24,time/60%60,time%60,rev.get(..rev.len().min(8)).unwrap_or(rev)),MUTED));
        if d["tasks"].as_array().is_some_and(|v|v.is_empty()){lines.push(styled("No supported spillover found. Other quality checks unassessed.",MUTED));}
        for task in d["tasks"].as_array().into_iter().flatten(){
            lines.push(styled(format!("{} / page {}: {}",task["scene_id"].as_str().unwrap_or("?"),task["page_id"],task["suspected_flaw"].as_str().unwrap_or("Review")),AMBER));
            lines.push(styled(format!("Words {} · {}",task["source_word_ids"],task["affected_lyrics"].as_str().unwrap_or("")),DESK_FG));
            lines.push(styled(task["confidence"].as_str().unwrap_or("Evidence unknown"),MUTED));
            lines.push(styled(task["proposed_fix"].as_str().unwrap_or("Inspect source"),DESK_FG));
        }
        for item in d["unassessed"].as_array().into_iter().flatten().take(1){lines.push(styled(format!("Unassessed: {}",item["reason"].as_str().unwrap_or("source evidence unavailable")),AMBER));}
    }
 }
 let m=&c.state["metrics"];let observed=|key:&str|m[key].as_u64().map(|v|v.to_string()).unwrap_or_else(||"?".into());
 lines.push(styled(format!("{} words · {} groups · {}s audio · {}/{} assets",observed("timed_words"),observed("lyric_groups"),m["recording_duration_sec"].as_f64().map(|v|format!("{v:.2}")).unwrap_or_else(||"?".into()),observed("available_assets"),observed("registered_assets")),MUTED));
 if let Some(r)=c.state["chat"]["request"].as_object().filter(|r|r.get("status").is_some_and(|v|v=="running")){let elapsed=r.get("sent_at").and_then(Value::as_u64).map(|ms|std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_secs().saturating_sub(ms/1000)).unwrap_or(0));lines.push(styled(format!("● agent.send working · {} · {}s elapsed",c.state["chat"]["selection"]["model"].as_str().unwrap_or("model unknown"),elapsed.map(|v|v.to_string()).unwrap_or_else(||"?".into())),GREEN));}
 if c.details_open{lines.push(styled(format!("Project: {}",c.state["project"].as_str().unwrap_or("unknown")),MUTED));lines.push(styled(format!("Session: {} · request {} · engine/ack {}/{} · DOM {}ms",c.state["chat"]["session"].as_str().unwrap_or("none"),c.state["chat"]["request"]["request_id"].as_str().unwrap_or("none"),revision(&c.state["ui"]["engine_revision"]),revision(&c.state["ui"]["ack_engine_revision"]),c.state["ui"]["frontend_report_age_ms"]),MUTED));}
 f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).block(round_panel(&format!("{} · ACTIVITY{}",if c.stage_selected==STORYBOARD_STAGE{"▰ ▰  STORYBOARD REVIEW  ▰ ▰"}else{PROJECT_STAGES[c.stage_selected].0},if c.details_open{" / DETAILS"}else{""}),BLUE,false)),area);
}
fn worker_observation(c:&Center)->Option<&Value>{
    let value=&c.state["animation_worker"];
    (c.state["project"].is_string() && value["project"]==c.state["project"]).then_some(value)
}
fn worker_colour(value:&Value)->Color{
    if value["observation_error"].is_string() || value["state"]=="failed" {RED}
    else if crate::worker::stale(value) || matches!(value["state"].as_str(),Some("running"|"cancelling")){AMBER}
    else if value["state"]=="completed"{GREEN}else{MUTED}
}
fn render_worker(f:&mut Frame,value:&Value,area:ratatui::layout::Rect){
    let mut lines=vec![styled(crate::worker::headline(value),worker_colour(value))];
    if value["state"]=="not_started"{
        lines.push(styled("Waiting for a render submission · Backstage never starts one by polling",MUTED));
        if let Some(owner)=value["worker_owner"]["job"].as_str(){lines.push(styled(format!("Global worker busy: {owner}"),AMBER));}
        f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).block(round_panel("ARTWORK IN MOTION",TEAL,false)),area);
        return;
    }
    let selection=&value["selection"];
    let identity=format!("{} · {} · {}",selection["character"].as_str().unwrap_or("character ?"),selection["track"].as_str().unwrap_or("action ?"),value["job"].as_str().unwrap_or("job ?"));
    lines.push(styled(fit_text(&identity,area.width.saturating_sub(2) as usize),DESK_FG));
    if let Some(error)=value["observation_error"].as_str().or(value["error"].as_str()).or(value["detail"].as_str()){
        lines.push(styled(fit_text(error,area.width.saturating_sub(2) as usize),RED));
    }else{lines.push(styled(fit_text(&crate::worker::resource_line(value),area.width.saturating_sub(2) as usize),MUTED));}
    if area.height>=6{
        let source=if value["telemetry_source"]=="terminal_receipt" || matches!(value["state"].as_str(),Some("completed"|"cancelled"|"failed")){"saved receipt"}else{"worker service"};
        let age=crate::worker::age_ms(value,"updated_at").map(|ms|format!("{:.1}s",ms as f64/1000.0)).unwrap_or_else(||"?".into());
        lines.push(styled(format!("{source} · recorded {age} ago{} · D PULSE details",if crate::worker::stale(value){" · STALE"}else{""}),MUTED));
    }
    if area.height>=7{
        let owner=&value["worker_owner"];
        let owner_line=if owner.is_object(){format!("Global worker: {} · {}",owner["job"].as_str().unwrap_or("?"),owner["project"].as_str().unwrap_or("project unknown"))}else{format!("Global worker owner: {}",value["worker_owner_status"].as_str().unwrap_or("unavailable"))};
        lines.push(styled(fit_text(&owner_line,area.width.saturating_sub(2) as usize),MUTED));
    }
    f.render_widget(Paragraph::new(lines).block(round_panel("ARTWORK IN MOTION",worker_colour(value),false)),area);
}
fn conversation_text(text:&str)->&str{
    let text=text.split("\n\n[Magic Studio context at send time:").next().unwrap_or("");
    // Migration-only: hide injected context in old stored turns; never emit the old branding.
    text.split("\n\n[AnimationCraft context at send time:").next().unwrap_or("")
}
fn render_conversation(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let r=&c.state["chat"]["request"];let (mark,color)=state_badge(r["status"].as_str());let mut lines=vec![styled(format!("{mark} · {}",r["error"].as_str().or(r["reason"].as_str()).unwrap_or(if r["status"]=="replied"{"reply received"}else{"no completed reply yet"})),color)];
  if let Some(turns)=c.state["chat_turns"].as_array(){for t in turns.iter().rev().take(6).collect::<Vec<_>>().into_iter().rev(){if area.height<8 && t["role"]=="user"{continue;}let text=conversation_text(t["text"].as_str().unwrap_or(""));if lines.len()>1{lines.push(Line::raw(""));}lines.push(styled(format!("{}: {text}",if t["role"]=="user"{"You"}else{"Agent"}),DESK_FG));}}
 let block=round_panel("SPROCKET · the app's own agent",GREEN,c.chat_editing);let inner=block.inner(area);f.render_widget(block,area);
  if inner.height<5{let reply=c.state["chat_turns"].as_array().into_iter().flatten().rev().find(|t|t["role"]=="assistant").and_then(|t|t["text"].as_str()).map(conversation_text).unwrap_or("No reply received");f.render_widget(Paragraph::new(vec![styled(reply,DESK_FG),styled(if c.chat_editing{format!("> {}_",c.chat_draft)}else{"F9 compose".into()},MUTED)]).wrap(Wrap{trim:false}),inner);return;}
 let parts=Layout::vertical([Constraint::Min(1),Constraint::Length(2)]).split(inner);
 let skip=wrapped_rows(&lines,parts[0].width).saturating_sub(parts[0].height);f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).scroll((skip,0)),parts[0]);
 f.render_widget(Paragraph::new(vec![styled(if c.chat_editing{"F9/Esc leave draft · Enter send"}else{"F9 compose · Enter sends only while composing"},MUTED),styled(format!("> {}{}",c.chat_draft,if c.chat_editing{"_"}else{""}),DESK_FG)]).wrap(Wrap{trim:false}),parts[1]);
}
/// The booth keeps typed events past the journal's three-action retention while it runs.
fn merge_tail(old:&Value,new:Vec<Value>)->Value{
 let mut events=old.as_array().cloned().unwrap_or_default();let seen=events.iter().filter_map(|e|e["sequence"].as_u64()).max().unwrap_or(0);
 events.extend(new.into_iter().filter(|e|e["sequence"].as_u64().is_some_and(|n|n>seen)));let excess=events.len().saturating_sub(50);events.drain(..excess);json!(events)
}
fn render_tail(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let block=round_panel("EVENTS · live · backstage.tail",AMBER,false);let rows=block.inner(area).height as usize;
 let mut lines:Vec<Line<'static>>=c.state["tail"].as_array().into_iter().flatten().rev().take(rows).map(|e|{let error=e["error"].as_str().map(|m|format!(" · {m}")).unwrap_or_default();let clip=e["sheet"].as_str().or(e["clip"].as_str()).map(|v|format!(" {v}")).unwrap_or_default();styled(format!("{} {} {}{clip}{error}",stamp(e),e["type"].as_str().unwrap_or("?"),e["command"].as_str().unwrap_or("")),if e["ok"]==false{RED}else{DESK_FG})}).collect();
 if lines.is_empty(){lines.push(styled("— No saved, generation, keyframe or error events yet.",MUTED));}
 f.render_widget(Paragraph::new(lines).block(block),area);
}
fn render_next(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let commands=c.copy_commands();let Some((effect,literal))=commands.get(c.command_selected%commands.len().max(1))else{return;};
 let reason=c.state["next_commands"].as_array().into_iter().flatten().find(|v|v["label"].as_str()==Some(*effect)).and_then(|v|v["reason"].as_str()).unwrap_or(effect);
  let blocker=worker_observation(c).filter(|v|v["state"]=="failed" || v["observation_error"].is_string());
  let mut lines=vec![styled(format!("> {effect}"),DESK_FG),styled(reason,MUTED),styled(literal.clone(),BLUE)];
  if let Some(worker)=blocker{lines.insert(0,styled(format!("{} · D opens PULSE",crate::worker::headline(worker)),RED));}
  f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).scroll((c.scroll,0)).block(round_panel(if blocker.is_some(){"ATTENTION · D PULSE · C copies inspection"}else{"NEXT STEP · C copies literal · Pg scroll"},if blocker.is_some(){RED}else{TEAL},true)),area);
}
/// The multiview: one tally tile per member of the gang, plus Sprocket in the app.
/// Broadcast rule: the colour never travels alone — the word is always on the tile.
fn render_gang(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let mut tiles:Vec<(String,String,String,Color,&'static str)>=Vec::new();
 for g in c.state["agent_gang"].as_array().into_iter().flatten(){
  let name=g["name"].as_str().or(g["agent"].as_str()).unwrap_or("?").to_string();
  let kind=g["agent"].as_str().unwrap_or("?").to_string();
  let cwd=g["cwd"].as_str().or(g["foreground_cwd"].as_str()).unwrap_or("");
  let project=cwd.split('/').last().unwrap_or(cwd).to_string();
  let task=g["terminal_title_stripped"].as_str().unwrap_or("").to_string();
  let (colour,word)=tally(g["agent_status"].as_str().unwrap_or(""));
  tiles.push((name,kind,if task.is_empty(){project.clone()}else{task},colour,word));
 }
 // Sprocket is the app's own agent: always on the wall, even when no agent session is running.
 let sprocket_state=c.state["chat"]["request"]["status"].as_str().unwrap_or("");
 let (sprocket_colour,sprocket_word)=match sprocket_state{"replied"=> (GREEN,"REPLIED"),"running"|"queued"=> (AMBER,"WORKING"),"error"=> (RED,"ERROR"),_=> (MUTED,"IDLE")};
 tiles.push(("Sprocket".into(),"in-app".into(),"the app".into(),sprocket_colour,sprocket_word));
 if tiles.is_empty(){return;}
 let each=(area.width as usize)/tiles.len().max(1);
 let columns=Layout::horizontal(tiles.iter().map(|_|Constraint::Length(each as u16))).split(area);
 for (index,tile) in tiles.iter().enumerate(){
  let (name,kind,project,colour,word)=(&tile.0,&tile.1,&tile.2,&tile.3,&tile.4);
  let block=Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded)
   .border_style(Style::default().fg(*colour))
   .title(Span::styled(format!(" {} ",name),Style::default().fg(*colour).add_modifier(Modifier::BOLD)));
  let inner=block.inner(columns[index]);
  let width=inner.width as usize;
  let lines=vec![styled(format!("{kind}"),MUTED),styled(format!("{} {word}",if *word=="WORKING"||*word=="REPLIED"||*word=="ERROR"{"●"}else{"○"}),*colour),styled(fit_text(project,26),DIM)];
  f.render_widget(Paragraph::new(lines).block(block),columns[index]);
  let _=width;
 }
}

/// Compact tally strip: the glyph shape and the colour both carry the state, so the
/// signal survives a monochrome terminal.
fn gang_tally_spans(c: &Center, compact: bool)->Vec<Span<'static>>{
 let mut spans:Vec<Span<'static>>=Vec::new();
 for g in c.state["agent_gang"].as_array().into_iter().flatten(){
  let name=g["name"].as_str().or(g["agent"].as_str()).unwrap_or("?").to_string();
  let (colour,word)=tally(g["agent_status"].as_str().unwrap_or(""));
  let glyph=match word{"WORKING"=>"●","STALLED"=>"◐","AVAILABLE"=>"○",_=>"·"};
  spans.push(Span::styled(format!("{glyph}{}{name}",if compact{""}else{" "}),colour));
  spans.push(Span::raw(" "));
 }
 let status=c.state["chat"]["request"]["status"].as_str().unwrap_or("");
 let (colour,word)=match status{"replied"=> (GREEN,"REPLIED"),"running"|"queued"=> (RED,"WORKING"),"error"=> (AMBER,"ERROR"),_=> (MUTED,"IDLE")};
 let glyph=match word{"WORKING"=>"●","ERROR"=>"◐","REPLIED"=>"◆",_=>"○"};
 spans.push(Span::styled(format!("{glyph}{}Sprocket",if compact{""}else{" "}),colour));
 spans
}


/// PULSE: the Studio's own health — memory, CPU and the recent error tail, from `/proc`.
fn render_pulse(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
  let mut lines:Vec<Line<'static>>=Vec::new();
  if let Some(worker)=worker_observation(c){
   lines.push(styled(format!("ARTWORK IN MOTION · {}",crate::worker::headline(worker)),worker_colour(worker)));
   for (label,key) in [("Job","job"),("Project","project"),("Unit","unit"),("Invocation","invocation_id")]{lines.push(styled(format!("{label}: {}",worker[key].as_str().unwrap_or("?")),MUTED));}
   lines.push(styled(crate::worker::resource_line(worker),DESK_FG));
   let cpu=worker["telemetry"]["cpu_usage_ns"].as_u64().map(|ns|format!("{:.2}s",ns as f64/1e9)).unwrap_or_else(||"?".into());
   let quota=worker["limits"]["cpu_quota_percent"].as_u64().map(|v|format!("{v}%")).unwrap_or_else(||"?".into());
   let timeout=worker["limits"]["timeout_seconds"].as_u64().map(|v|format!("{v}s")).unwrap_or_else(||"?".into());
   lines.push(styled(format!("CPU time {cpu} · quota {quota} · timeout {timeout}"),DESK_FG));
   let origin=worker["started_at"].as_str().or(worker["submitted_at"].as_str());
   let elapsed=origin.and_then(|s|s.parse::<u64>().ok()).and_then(|ms|crate::worker::now_ms().checked_sub(ms)).map(|ms|format!("{:.1}s since recorded start/submission",ms as f64/1000.0)).unwrap_or_else(||"elapsed ?".into());
   lines.push(styled(elapsed,MUTED));
   let show=|key:&str|worker.get(key).filter(|v|!v.is_null()).map(|v|v.as_str().map(str::to_owned).unwrap_or_else(||v.to_string())).unwrap_or_else(||"?".into());
   lines.push(styled(format!("Service result {} · exec status {} · exit {} / {} · reason {}",show("service_result"),show("exec_main_status"),show("exit_code"),show("exit_status"),show("termination_reason")),MUTED));
   lines.push(styled(format!("Cancellation requested: {} · observed state {}",show("cancellation_requested_at"),show("state")),MUTED));
   let owner=&worker["worker_owner"];
   lines.push(styled(if owner.is_object(){format!("Global slot: {} · {} · {}",owner["job"].as_str().unwrap_or("?"),owner["project"].as_str().unwrap_or("project ?"),owner["state"].as_str().unwrap_or("state ?"))}else{format!("Global slot: {}",worker["worker_owner_status"].as_str().unwrap_or("unavailable"))},MUTED));
   if let Some(error)=worker["worker_owner_error"].as_str(){lines.push(styled(format!("Global owner observation: {error}"),AMBER));}
   let source=worker["telemetry_source"].as_str().unwrap_or(if matches!(worker["state"].as_str(),Some("completed"|"cancelled"|"failed")){"saved terminal receipt"}else{"worker service via blender.status"});
   let age=crate::worker::age_ms(worker,"updated_at").map(|ms|format!("{:.1}s",ms as f64/1000.0)).unwrap_or_else(||"?".into());
   lines.push(styled(format!("Source {source} · recorded {age} ago{}",if crate::worker::stale(worker){" · STALE / not live"}else{""}),AMBER));
   for key in ["observation_error","service_error","error","detail","log","clip"]{if let Some(text)=worker[key].as_str(){lines.push(styled(format!("{key}: {text}"),if key.contains("error"){RED}else{MUTED}));}}
   if let Some(rss)=worker["worker_peak_rss_kib"].as_u64(){lines.push(styled(format!("Blender self-reported peak RSS: {:.0}MiB (not cgroup peak)",rss as f64/1024.0),MUTED));}
   lines.push(Line::raw(""));
  }
  match c.studio_sample(){
   Some(sample)=>{
    let procs=&sample.procs;
    let cpu=sample.cpu_pct.map(|v|format!("{v:.0}%")).unwrap_or_else(||"? (sampling)".into());
    lines.push(styled(format!("Studio PID {} · {} procs · CPU {cpu} · RSS sum {:.0}MiB",sample.pid,procs.len(),crate::telemetry::rss_mb(procs)),DESK_FG));
    lines.push(styled(format!("/proc · {:.1}s ago · one core = 100% · shared RSS may be counted twice",sample.at.elapsed().as_secs_f64()),MUTED));
   let room=area.width.saturating_sub(4) as usize;
   for p in procs.iter().take(6){
    let name=fit_text(&p.name,room.saturating_sub(9).max(6));
    let width=room.saturating_sub(9).max(6);
    let mut row=format!("  {name:<width$}{:>7}",crate::telemetry::mib(p.rss_kb),width=width);
    if room>=34{row.push_str(&format!("  {}",p.pid));}
    lines.push(styled(row,MUTED));
   }
  }
   None=>lines.push(styled("Selected Studio metrics unavailable or stale · no other instance substituted",AMBER)),
 }
 let errors=c.state["activity"]["actions"].as_array().into_iter().flatten().flat_map(|a|a["events"].as_array().into_iter().flatten().filter(|e|e["stage"]=="engine_error"||e["ok"]==false)).collect::<Vec<_>>();
 let errors=errors.iter().rev().take(5).collect::<Vec<_>>();
 lines.push(styled(if errors.is_empty(){"no errors in the journal".to_string()}else{format!("{} error event(s) in the journal",errors.len())},if errors.is_empty(){GREEN}else{RED}));
 for e in errors{lines.push(styled(format!("  {} {}",e["command"].as_str().unwrap_or("?"),e["error"].as_str().unwrap_or("failed")),RED));}
  f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).scroll((c.scroll,0)).block(round_panel("PULSE · worker / Studio · Pg scroll · D closes",BLUE,false)),area);
}

/// MAPPETS: every agent in the gang, what it is building, and whether it is free.
fn render_muppets(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let mut lines:Vec<Line<'static>>=Vec::new();
 let watch=c.spy.clone().unwrap_or_default();
 lines.push(styled("The gang on this project",DESK_FG));
 lines.push(styled("● working · ◐ stalled · ○ idle — ○ is who you can hand the next scene to.",MUTED));
 let mut free=0usize;
 for g in c.state["agent_gang"].as_array().into_iter().flatten(){
  let name=g["name"].as_str().or(g["agent"].as_str()).unwrap_or("?");
  let (colour,word)=tally(g["agent_status"].as_str().unwrap_or(""));
  if word=="IDLE"{free+=1;}
  let task=g["terminal_title_stripped"].as_str().unwrap_or("");
  lines.push(Line::from(vec![
   Span::styled(format!("{} {name}",if word=="WORKING"{"●"}else if word=="STALLED"{"◐"}else{"○"}),colour),
   Span::styled(format!("  {:<9}",g["agent"].as_str().unwrap_or("?")),MUTED),
   Span::styled(format!("{word:<8}"),colour),
   Span::styled(format!(" {task}"),DESK_FG),
  ]));
 }
 lines.push(styled(format!("{free} idle and available"),if free>0{GREEN}else{MUTED}));
 if !watch.is_empty(){
  lines.push(Line::raw(""));
  for line in watch.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev(){lines.push(styled(line.to_string(),MUTED));}
 }
 let sprocket=c.state["chat"]["request"]["status"].as_str().unwrap_or("");
 let (colour,word)=match sprocket{"replied"=>(GREEN,"REPLIED"),"running"|"queued"=>(AMBER,"WORKING"),"error"=>(RED,"ERROR"),_=>(MUTED,"IDLE")};
 lines.push(Line::from(vec![Span::styled(format!("{} Sprocket",if word=="WORKING"{"●"}else{"○"}),colour),Span::styled(format!("  {:<9}","in-app"),MUTED),Span::styled(format!("{word:<8}"),colour),Span::styled(" the app's own agent",DESK_FG)]));
 f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).block(round_panel("MOPPETS · the gang",TEAL,true)),area);
}
fn render_desk(f:&mut Frame,c:&Center,control:bool){
 let area=f.area();let narrow=area.width<110;f.render_widget(Block::default().style(Style::default().bg(DESK_BG).fg(DESK_FG)),area);
 let gang_rows=if narrow{0}else{5};
 let chunks=Layout::vertical([Constraint::Length(1),Constraint::Length(4),Constraint::Min(1),Constraint::Length(gang_rows),Constraint::Length(1),Constraint::Length(1)]).split(area);
  f.render_widget(Paragraph::new(top_bar(c,area.width as usize,control)),chunks[0]);
 f.render_widget(Paragraph::new(letterbox(area.width as usize,"◉ BACKSTAGE","")),chunks[5]);
 let name=c.state["project"].as_str().and_then(|p|std::path::Path::new(p).file_stem()).and_then(|v|v.to_str()).unwrap_or("No project");let ui=&c.state["ui"];let sel=&c.state["chat"]["selection"];
 let matched=ui["connection"]=="connected"&&ui["active_project"]==c.state["project"];
 let chat_status=if c.details_open{chat_outcome(&c.state)}else{let(mark,_)=state_badge(c.state["chat"]["request"]["status"].as_str());format!("Chat {mark}")};let light=|on:bool,label:&str|{if on{Span::styled(format!("● {label}"),GREEN)}else{Span::styled(format!("○ {label}"),AMBER)}};
 let header=vec![
  styled(format!("{name}  ·  {}  ·  {}/{}",ui["view"].as_str().unwrap_or("?"),sel["provider"].as_str().unwrap_or("?"),sel["model"].as_str().unwrap_or("?")),DESK_FG),
  Line::from(vec![light(matched,"studio"),Span::raw("  "),light(ui["engine_unsaved"]!=true,"saved"),Span::raw("  "),light(!chat_status.contains("?"),"chat"),Span::raw("  "),Span::styled(crate::snapshot::status_strip(&c.state["snapshot"]),TEAL)]),
 ];
 f.render_widget(Paragraph::new(header).block(round_panel(if control{"🎬 F1 WORKSHOP | F2 BOOTH | F3 MOPPETS"}else{"🎬 F1 WORKSHOP | F2 BOOTH | F3 MOPPETS · D PULSE"},if control{DESK_LINE}else if matched{GREEN}else{AMBER},true)),chunks[1]);
  if c.details_open{render_pulse(f,c,chunks[2]);}
  else if c.muppets_view{render_muppets(f,c,chunks[2]);}
  else if control{let p=Layout::vertical([Constraint::Min(5),Constraint::Length(11)]).split(chunks[2]);
   render_catalog(f,c,p[0]);render_commands(f,c,p[1],true);}else{
 let compact_actions=(root_actions(c).len() as u16*2+2).clamp(4,8);let bottom_height=if narrow{compact_actions+12}else{15};let body=Layout::vertical([Constraint::Min(8),Constraint::Length(bottom_height)]).split(chunks[2]);
 if narrow{let p=Layout::vertical([Constraint::Length(6),Constraint::Length(if c.details_open{9}else if body[0].height>=14{4}else{3}),Constraint::Min(3)]).split(body[0]);f.render_widget(Paragraph::new(stage_lines(c,true)).block(round_panel("STAGES · [ / ] select",TEAL,true)),p[0]);render_stage_activity(f,c,p[1]);if c.details_open{render_pulse(f,c,p[2]);}else{render_conversation(f,c,p[2]);}}
 else{let p=Layout::horizontal([Constraint::Length(31),Constraint::Min(30)]).spacing(1).split(body[0]);let stages=stage_lines(c,false);let mut rail=p[0];rail.height=wrapped_rows(&stages,rail.width.saturating_sub(2)).saturating_add(2).min(rail.height);f.render_widget(Paragraph::new(stages).wrap(Wrap{trim:false}).block(round_panel("STAGES · [ / ] select",TEAL,true)),rail);let right=Layout::vertical([Constraint::Length(if c.details_open{11}else{7}),Constraint::Min(5)]).split(p[1]);render_stage_activity(f,c,right[0]);let mut conversation=right[1];let turns=c.state["chat_turns"].as_array().map(|a|a.len().min(6)).unwrap_or(0);conversation.height=conversation.height.min((turns as u16*3+7).clamp(10,25));if c.details_open{render_pulse(f,c,conversation);}else{render_conversation(f,c,conversation);}}
 if narrow{let p=Layout::vertical([Constraint::Length(compact_actions),Constraint::Min(10)]).split(body[1]);let top=Layout::horizontal([Constraint::Percentage(50),Constraint::Percentage(50)]).split(p[0]);render_actions(f,c,top[0],true);render_tail(f,c,top[1]);render_next(f,c,p[1]);}else{let p=Layout::horizontal([Constraint::Percentage(48),Constraint::Percentage(52)]).spacing(1).split(body[1]);let left=Layout::vertical([Constraint::Length(compact_actions),Constraint::Min(3)]).split(p[0]);render_actions(f,c,left[0],true);render_tail(f,c,left[1]);render_next(f,c,p[1]);}
 }
 if gang_rows>0{render_gang(f,c,chunks[3]);}
 let booth_open=c.state["ui"]["connection"]=="connected";let booth_light=if booth_open{"BACKSTAGE ● open"}else{"BACKSTAGE ○ off"};
 let keys=if narrow{"F10 F9 D C ^Q"}else{"F10 🩺 · [ / ] stage · F9 chat · D details · C copy · Pg scroll · ^Q exit"};
 let footer:Vec<Line>=if c.message.starts_with("COPIED")||c.message.contains("error"){vec![styled(c.message.clone(),AMBER)]}else{
  let mut spans=vec![Span::styled(if narrow{(if booth_open{"● OPEN"}else{"○ OFF"}).to_string()}else{booth_light.to_string()},if booth_open{GREEN}else{AMBER}),Span::raw(if narrow{"  ·  "}else{"  ·  GANG: "})];
  spans.extend(gang_tally_spans(c,narrow));
  let mut key_span=Span::styled(format!("  ·  {keys}"),MUTED);
  // Fit the strip to the pane: the tally comes first, the key hints are what gets dropped.
  let width=area.width as usize;
  let used=|spans:&Vec<Span>|spans.iter().map(|s|s.content.chars().count()).sum::<usize>();
  if used(&spans)+key_span.content.chars().count()>width{key_span=Span::raw("");}
  while used(&spans)>width&&spans.len()>2{spans.pop();}
  spans.push(key_span);
  vec![Line::from(spans)]
 };
 f.render_widget(Paragraph::new(footer),chunks[4]);
}

fn render(f: &mut Frame,c: &Center) {
    if c.show_form { render_form(f,c); return; }
    render_desk(f,c,c.control_view || !c.state["project"].is_string());
}

pub fn start(p: Value, print: bool) -> Result<(), String> {
    let mut c = Center::new(&p)?;
    if !c.fields[0].is_empty() {
        c.session.execute("project.attach",json!({"path":c.fields[0]})).map_err(|e|e.to_string())?;
        c.refresh();
    } else {
        c.refresh();
    }
    if print {
        println!(
            "MAGIC STUDIO / COMMAND CENTER\nProject: {}\nStatus: {}\nTranscript valid: {}\n{}\n{}",
            c.state["project"],
            crate::snapshot::status_line(&c.state["snapshot"]),
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
        let mut worker=crate::worker::Poller::default();
        loop {
            c.poll_chat();c.poll_doctor();
            if refreshed.elapsed()>Duration::from_secs(1){c.refresh();refreshed=std::time::Instant::now();}
            if let Some(observation)=worker.poll(c.state["project"].as_str()){c.state["animation_worker"]=observation;}
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
            if key.code==KeyCode::F(9){c.chat_editing=!c.chat_editing;continue;}
            if c.chat_editing {match key.code{KeyCode::Esc=>c.chat_editing=false,KeyCode::Enter=>c.send_chat(),KeyCode::Backspace=>{c.chat_draft.pop();},KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL)=>c.chat_draft.push(ch),_=>{}}continue;}
            if key.code==KeyCode::F(1){c.control_view=true;c.show_form=false;c.muppets_view=false;continue;}
            if key.code==KeyCode::F(3) && !c.show_form{c.muppets_view=true;c.control_view=false;continue;}
            if key.code==KeyCode::F(2) && !c.show_form{c.control_view=false;c.muppets_view=false;continue;}
            if key.code==KeyCode::F(10){c.check_storyboard();continue;}
            if key.code==KeyCode::F(4){c.show_form=!c.show_form;continue;}
            if !c.show_form {
                match key.code{
                    KeyCode::Char(n @ '1'..='9') if !key.modifiers.contains(KeyModifiers::CONTROL)=>{let index=(n as usize)-('1' as usize);if index<c.copy_commands().len(){c.command_selected=index;}},
                    KeyCode::Up=>c.command_selected=c.command_selected.saturating_sub(1),
                    KeyCode::Down|KeyCode::Tab=>c.command_selected=(c.command_selected+1)%c.copy_commands().len().max(1),
                    KeyCode::Char('c')=>c.copy_command(),
                    KeyCode::Char('[')=>c.stage_selected=(c.stage_selected+PROJECT_STAGES.len()-1)%PROJECT_STAGES.len(),
                    KeyCode::Char(']')=>c.stage_selected=(c.stage_selected+1)%PROJECT_STAGES.len(),
                    KeyCode::Char('d')=>c.details_open=!c.details_open,
                    KeyCode::Char('j') if c.control_view=>c.catalog_selected+=1,
                    KeyCode::Char('k') if c.control_view=>c.catalog_selected=c.catalog_selected.saturating_sub(1),
                    KeyCode::Enter if c.control_view=>c.observe_selected(),
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

#[cfg(test)]
mod feedback_tests {
    use super::*;
    #[test]
    fn doctor_badges_distinguish_scope_staleness_and_failures(){
        let mut c=Center::new(&json!({})).unwrap();assert_eq!(doctor_badge(&c).0,"● Not checked");
        c.state["storyboard_doctor"]=json!({"healthy":true,"stale":false});assert!(doctor_badge(&c).0.starts_with("✓ Checked"));
        c.state["storyboard_doctor"]["stale"]=json!(true);assert!(doctor_badge(&c).0.contains("stale"));
        c.doctor_error=Some("failure".into());assert_eq!(doctor_badge(&c).0,"● Check failed");
        let(_tx,rx)=std::sync::mpsc::channel();c.doctor_receiver=Some(rx);assert_eq!(doctor_badge(&c).0,"● Checking");
    }

    #[test]
    fn navigation_is_not_engine_work_and_requires_its_own_receipt() {
        let mut a=json!({"root_request_id":"nav-1","events":[{"stage":"ui_click","request_id":"nav-1","interaction":"navigation","control":"sb-next"}]});
        assert_eq!(engine_badge(&a).0,"— UI NAVIGATION");assert!(!action_ack(&a));
        a["events"].as_array_mut().unwrap().push(json!({"stage":"ui_navigation_applied","request_id":"nav-1"}));
        assert!(action_ack(&a));
    }
    #[test]
    fn actual_dispatch_is_running_until_a_matching_finish() {
        let mut a=json!({"root_request_id":"save-1","events":[{"stage":"dispatch_running","request_id":"save-1","command":"storyboard.save_version"}]});
        assert_eq!(engine_badge(&a).0,"[>] RUNNING");
        a["events"].as_array_mut().unwrap().push(json!({"stage":"engine_success","request_id":"other","command":"storyboard.save_version"}));
        assert_eq!(engine_badge(&a).0,"[>] RUNNING");
        a["events"].as_array_mut().unwrap().push(json!({"stage":"engine_error","request_id":"save-1","command":"storyboard.save_version"}));
        assert_eq!(engine_badge(&a).0,"✗ FAILED");
    }
    #[test]
    fn unmapped_click_is_not_reported_as_failed_engine_work() {
        assert_eq!(engine_badge(&json!({"events":[{"stage":"ui_click","request_id":"click-1"}]})).0,"— UI INTERACTION");
    }
    fn fixture()->Center{
        Center{session:Session::default(),fields:std::array::from_fn(|_|String::new()),selected:0,editing:false,state:json!({"project":"/fixture.magic","ui":{"connection":"connected","active_project":"/fixture.magic","engine_unsaved":false,"draft_fields":[],"ui_ready":true,"engine_revision":2,"ack_engine_revision":2,"frontend_report_age_ms":10},"activity":{"next_cursor":42,"actions":[{"root_request_id":"save","first_sequence":40,"last_sequence":42,"events":[{"stage":"dispatch_running","request_id":"save","command":"storyboard.save_version"},{"stage":"engine_success","request_id":"save","command":"storyboard.save_version","engine_revision":2},{"stage":"frontend_ack","request_id":"save","engine_revision":2}]}]}}),message:String::new(),scroll:0,show_form:false,control_view:false,command_selected:0,seen_cursor:0,flash_after:0,flash_started:None,pinned_instance:None,stage_selected:0,details_open:false,chat_draft:String::new(),chat_editing:false,chat_sent:String::new(),chat_sent_request:None,chat_receiver:None,doctor_receiver:None,doctor_error:None,agent_gang:json!([]),agent_gang_at:None,muppets_view:false,catalog_selected:0,pulse:None,pulse_at:None,spy:None,spy_at:None}
    }
    #[test]
    fn unknown_saved_state_is_not_reported_as_clean(){
        assert_eq!(saved_observation(&json!({"connection":"connected","engine_unsaved":null})).1,AMBER);
        assert_eq!(saved_observation(&json!({"connection":"connected","engine_unsaved":true})).1,AMBER);
        assert_eq!(saved_observation(&json!({"connection":"connected","engine_unsaved":false})).1,DESK_FG);
    }
    #[test]
    fn failed_action_gets_its_own_contract_not_generic_transcript_or_agent_commands(){
        let mut c=fixture();c.state["activity"]["actions"][0]["events"][1]["stage"]=json!("engine_error");
        let commands=c.copy_commands();assert_eq!(commands.len(),2);assert!(commands[1].1.contains("commands 'storyboard.save_version'"));assert!(!commands.iter().any(|(_,v)|v.contains("agent.config")||v.contains("workshop.import_transcript")));
    }
    #[test]
    fn readiness_lights_do_not_infer_evidence_from_matched_revisions(){
        let c=fixture();assert!(stage_evidence(&c,0).is_none());assert_eq!(state_badge(Some("missing")).1,BLUE);assert_eq!(state_badge(Some("ready")).0,"[OK]");assert_eq!(state_badge(Some("working")).1,GREEN);assert_eq!(state_badge(Some("blocked")).1,RED);
    }
    #[test]
    fn last_actions_label_keeps_three_root_actions_without_exposing_sequence_by_default(){
        let mut c=fixture();let a=c.state["activity"]["actions"][0].clone();c.state["activity"]["actions"]=json!([a.clone(),a.clone(),a.clone(),a]);assert_eq!(root_actions(&c).len(),3);let lines=action_lines(&c,root_actions(&c)[0],true);assert!(!lines[0].to_string().contains("#40"));c.details_open=true;assert!(action_lines(&c,root_actions(&c)[0],true)[0].to_string().contains("#40"));
    }
    #[test]
    fn accepted_but_pending_agent_reply_still_counts_as_activity(){
      let mut c=fixture();c.state["activity"]["actions"][0]["events"][0]["command"]=json!("agent.send");c.state["activity"]["actions"][0]["events"][1]=json!({"stage":"engine_success","request_id":"save","command":"agent.send","status":"running"});
      assert!(activity_wave(&c).contains("[>] agent.send"));assert!(!activity_wave(&c).contains("idle"));c.state["activity"]["actions"][0]["events"][1]["status"]=json!("replied");assert!(activity_wave(&c).contains("idle"));
    }
    #[test]
    fn next_command_is_literal_pinned_and_comes_from_shared_recommendation(){
      let mut c=fixture();c.pinned_instance=Some("studioA.sock".into());c.state["ui"]["endpoint"]=json!("unix:/tmp/studioA.sock");c.state["next_commands"]=json!([{"label":"See asset import arguments","command":"command.list","operation":"exec","params":{"filter":"project.add_asset"},"reason":"No reference art is registered; inspect the import command before choosing an asset."}]);
      let commands=c.copy_commands();assert_eq!(commands[0].0,"See asset import arguments");assert!(commands[0].1.contains("--bridge 'unix:/tmp/studioA.sock' exec 'command.list' 'filter=project.add_asset'"));assert!(commands[0].1.contains("_expected_project=/fixture.magic"));assert!(commands[0].1.contains("unix:/tmp/studioA.sock"));assert!(!commands[0].1.contains("<mc>"));
    }
    #[test]
    fn matching_shared_chat_receipt_clears_only_the_submitted_draft(){
        let mut c=fixture();c.chat_sent_request=Some("sentA".into());c.chat_sent="hello".into();c.chat_draft="hello".into();c.chat_editing=true;c.state["chat"]=json!({"request":{"request_id":"other","status":"replied"}});c.observe_chat_delivery();assert_eq!(c.chat_draft,"hello");
        c.state["chat"]["request"]["request_id"]=json!("sentA");c.chat_draft="new edit".into();c.observe_chat_delivery();assert_eq!(c.chat_draft,"new edit");
        c.chat_draft="hello".into();c.observe_chat_delivery();assert!(c.chat_draft.is_empty());assert!(!c.chat_editing);assert!(c.message.contains("[OK]"));
    }
    #[test]
    fn approved_booth_uses_shared_stage_evidence_and_replied_state_at_both_sizes(){
        let mut c=fixture();c.message="Chat running · old request".into();c.state["chat"]=json!({"session":"sessionA","selection":{"provider":"opencode","model":"free"},"request":{"status":"replied","request_id":"replyA"}});c.state["chat_turns"]=json!([{"role":"assistant","text":"ACTUAL_REPLY"}]);
        assert!(chat_outcome(&c.state).contains("[OK]"));assert!(!chat_outcome(&c.state).contains("running"));assert!(stage_evidence(&c,2).is_none(),"no prerequisites in the fixture");
        let mut lyrics=c.state.clone();lyrics["prerequisites"]=json!([{"id":"lyrics","state":"ready"}]);let mut l=fixture();l.state=lyrics;assert_eq!(stage_evidence(&l,2).and_then(|v|v["state"].as_str()),Some("ready"),"the lyrics light reads the engine's lyrics item");assert_eq!(PROJECT_STAGES[STORYBOARD_STAGE].0,"Storyboard");
        for(w,h)in [(90,38),(140,44)]{let backend=ratatui::backend::TestBackend::new(w,h);let mut terminal=ratatui::Terminal::new(backend).unwrap();terminal.draw(|f|render_desk(f,&c,false)).unwrap();let b=terminal.backend().buffer();let screen=(0..h).map(|y|(0..w).map(|x|b[(x,y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");for label in ["Lyrics & notes","Animation","ACTUAL_REPLY","Last actions","NEXT STEP"]{assert!(screen.contains(label),"{w}: missing {label}");}assert!(!screen.contains("Chat running"));}
    }
    #[test]
    fn conversation_hides_injected_context_without_rewriting_stored_text(){
        for brand in ["Magic Studio","AnimationCraft"]{
            let stored=format!("Artwork is ready\n\n[{brand} context at send time: private context]");
            assert_eq!(conversation_text(&stored),"Artwork is ready");
            assert!(stored.contains("private context"),"rendering must not mutate stored history");
        }
        assert_eq!(conversation_text("A historical AnimationCraft filename"),"A historical AnimationCraft filename");
    }
    #[test]
    fn pulse_retains_terminal_failure_facts_when_service_is_unavailable(){
        let mut c=fixture();c.details_open=true;
        c.state["animation_worker"]=json!({"project":"/fixture.magic","job":"magicstudio-blender-test","state":"failed","phase":"rendering","termination_reason":"timeout","telemetry_stale":true,"telemetry_source":"terminal_receipt","updated_at":"1","observed_at_ms":crate::worker::now_ms().to_string(),"service_error":"user bus disconnected","service_state":null,"exit_status":15,"telemetry":{"memory_peak_bytes":269307904,"swap_current_bytes":0}});
        for (w,h) in [(90,38),(140,44)]{
            let backend=ratatui::backend::TestBackend::new(w,h);
            let mut terminal=ratatui::Terminal::new(backend).unwrap();
            terminal.draw(|f|render_desk(f,&c,false)).unwrap();
            let b=terminal.backend().buffer();
            let screen=(0..h).map(|y|(0..w).map(|x|b[(x,y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
            for label in ["BLOCKED · timeout","terminal_receipt","STALE / not live","user bus disconnected","CPU time ?","Selected Studio metrics unavailable"]{
                assert!(screen.contains(label),"{w}: missing {label}\n{screen}");
            }
        }
    }
    #[test]
    fn failed_worker_attention_has_safe_project_and_job_scoped_inspection(){
        let mut c=fixture();
        c.state["animation_worker"]=json!({"project":"/fixture.magic","job":"magicstudio-blender-test","state":"failed","termination_reason":"timeout"});
        let commands=c.copy_commands();
        assert_eq!(commands.len(),1);
        assert!(commands[0].1.contains("blender.status"));
        assert!(commands[0].1.contains("project=/fixture.magic"));
        assert!(commands[0].1.contains("job=magicstudio-blender-test"));
        assert!(!commands[0].1.contains("blender.start"));
        c.state["project"]=Value::Null;
        assert!(c.copy_commands()[0].1.ends_with("project.catalog"));
        assert!(!c.copy_commands()[0].1.contains("project="));
    }
    #[test]
    fn booth_leads_with_actual_worker_progress_not_a_fake_idle_reel(){
        let mut c=fixture();
        c.state["animation_worker"]=json!({"job":"magicstudio-blender-test","project":"/fixture.magic","state":"running","phase":"rendering","frames_completed":42,"frames_total":180,"telemetry":{"memory_current_bytes":419430400,"memory_max_bytes":4294967296u64}});
        for (w,h) in [(90,38),(140,44)]{
            let backend=ratatui::backend::TestBackend::new(w,h);
            let mut terminal=ratatui::Terminal::new(backend).unwrap();
            terminal.draw(|f|render_desk(f,&c,false)).unwrap();
            let b=terminal.backend().buffer();
            let screen=(0..h).map(|y|(0..w).map(|x|b[(x,y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
            assert!(screen.contains("42/180"),"{w}: actual frame progress must be visible");
            assert!(screen.contains("ARTWORK IN MOTION"),"{w}: creation should lead the panel");
        }
    }
    #[test]
    fn top_bar_centres_project_and_keeps_unknown_metrics_explicit(){
        let c=fixture();
        for (w,h) in [(60,32),(90,38),(140,44),(240,50)] {
            let backend=ratatui::backend::TestBackend::new(w,h);
            let mut terminal=ratatui::Terminal::new(backend).unwrap();
            terminal.draw(|f|render_desk(f,&c,false)).unwrap();
            let b=terminal.backend().buffer();
            let row=(0..w).map(|x|b[(x,0)].symbol()).collect::<String>();
            assert!(row.contains("CPU ?"),"{w}: {row}");
            assert!(row.contains("RSS ?"),"{w}: {row}");
            let byte=row.find("fixture").expect("project title must survive narrowing");
            let start=row[..byte].chars().count();
            assert!((start as isize-((w as usize-"fixture · BOOTH".chars().count())/2) as isize).abs()<=1,"{w}: title not centred: {row}");
            assert!(!row.contains("REEL IDLE"),"unobserved production is not idle evidence");
        }
    }
    #[test]
    fn metrics_are_selected_instance_only_fresh_and_preserve_zero(){
        let mut c=fixture();
        let pid=std::process::id();
        let instance=format!("{pid}-test.sock");
        c.state["ui"]["instance"]=json!(instance);
        c.pinned_instance=Some(instance);
        c.pulse=crate::telemetry::Sample::capture(c.pinned_instance.as_deref().unwrap(),None);
        c.pulse.as_mut().unwrap().cpu_pct=Some(0.0);
        assert!(top_bar(&c,140,false).to_string().contains("CPU 0%"));
        c.pulse.as_mut().unwrap().at=std::time::Instant::now()-Duration::from_secs(7);
        assert!(top_bar(&c,140,false).to_string().contains("CPU ? · RSS ?"));
        c.pulse.as_mut().unwrap().at=std::time::Instant::now();
        c.state["ui"]["instance"]=json!(format!("{}-other.sock",pid+1));
        assert!(c.studio_sample().is_none(),"replacement instance must not inherit metrics");
        let replacement=format!("{pid}-replacement.sock");
        c.state["ui"]["instance"]=json!(replacement);
        c.pinned_instance=Some(replacement);
        assert!(c.studio_sample().is_none(),"same PID with a new instance nonce must not inherit metrics");
        c.state["ui"]["instance"]=json!(format!("{pid}-test.sock"));
        c.pinned_instance=Some(format!("{pid}-test.sock"));
        c.state["ui"]["active_project"]=json!("/other.magic");
        assert!(c.studio_sample().is_none(),"different project must not inherit metrics");
        c.sample_studio();
        assert!(c.pulse.is_none(),"disconnect clears the previous sample");
    }
    #[test]
    fn long_unicode_titles_fit_without_overwriting_side_metrics(){
        let mut c=fixture();
        c.state["project"]=json!("/映画映画映画映画映画映画映画映画映画映画.magic");
        for width in [0,1,20,60,90,140,240]{
            let line=top_bar(&c,width,false);
            assert!(line.width()<=width,"{width}: {}",line.to_string());
            if width>=60{assert!(line.to_string().contains("CPU ?"));}
        }
    }
    #[test]
    fn workshop_reclaims_the_oversized_logo_rows(){
        let c=fixture();
        let backend=ratatui::backend::TestBackend::new(90,38);
        let mut terminal=ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f|render_desk(f,&c,true)).unwrap();
        let b=terminal.backend().buffer();
        let first_body=(0..90).map(|x|b[(x,5)].symbol()).collect::<String>();
        assert!(first_body.contains("ALL PROJECTS"),"catalog must start immediately below header: {first_body}");
    }
    #[test]
    fn both_views_show_the_shared_status_line_and_booth_follows_the_tail(){
        let mut c=fixture();
        c.state["snapshot"]=json!({"project":"/fixture.magic","subject":"bee","clip":"fly","playhead_sec":1.5,"selected_part":"wing","unsaved_edits":1,"hyperframes":false,"fps":24,"last_error":null});
        c.state["tail"]=merge_tail(&json!([]),crate::snapshot::tail(&json!({"events":[{"sequence":9,"stage":"saved_state","command":"project.save"}]}),None));
        assert_eq!(merge_tail(&c.state["tail"],crate::snapshot::tail(&json!({"events":[{"sequence":9,"stage":"saved_state","command":"project.save"}]}),None)).as_array().unwrap().len(),1,"polling the same journal row twice adds it once");
        for control in [false,true]{for(w,h)in [(90,38),(140,44)]{
            let backend=ratatui::backend::TestBackend::new(w,h);let mut terminal=ratatui::Terminal::new(backend).unwrap();terminal.draw(|f|render_desk(f,&c,control)).unwrap();let b=terminal.backend().buffer();let screen=(0..h).map(|y|(0..w).map(|x|b[(x,y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
            assert!(screen.contains("@fly · ▶1.50s · part wing · unsaved 1"),"{w} control={control}: missing status line\n{screen}");
            if !control{assert!(screen.contains("EVENTS")&&screen.contains("saved project.save"),"{w}: missing event tail\n{screen}");}
        }}
    }

}
