//! Terminal presentation only. All project operations execute the shared engine registry.
mod control;
mod workshop;
mod booth;
mod roles;
mod panels;
use crate::mcp_presence;
pub(crate) use crate::theme::*;
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
struct Hit{ id:String,label:String,area:ratatui::layout::Rect,action:String,params:Value,enabled:bool }
struct Center {
    session: Session,
    scope:Option<crate::scope::Scope>,
    onboarding:Option<Value>,
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
    command_selected:usize,
    seen_cursor:u64,
    flash_after:u64,
    flash_started:Option<std::time::Instant>,
    pinned_instance:Option<String>,
    stage_selected:usize,
    details_open:bool,
    chat_draft:String,
    chat_draft_project:Option<String>,
    chat_editing:bool,
    chat_sent:String,
    chat_sent_request:Option<String>,
    doctor_receiver:Option<std::sync::mpsc::Receiver<Result<Value,String>>>,
    doctor_error:Option<String>,
    chat_receiver:Option<std::sync::mpsc::Receiver<Result<Value,String>>>,
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
        let root = magicstudio_engine::project_settings::root().map_err(|e|e.to_string())?;
        let mut center=Self{session:Session::user_session(),scope:None,onboarding:None,fields:[value("project"),p["root"].as_str().map(str::to_owned).unwrap_or_else(||root.to_string_lossy().into_owned()),value("name"),value("audio"),value("transcript"),value("captions"),value("audiomap"),value("materials")],selected:0,editing:false,show_form:false,control_view:false,command_selected:0,seen_cursor:0,flash_after:0,flash_started:None,pinned_instance:None,stage_selected:0,details_open:false,chat_draft:String::new(),chat_draft_project:None,chat_editing:false,chat_sent:String::new(),chat_sent_request:None,chat_receiver:None,doctor_receiver:None,doctor_error:None,state:json!({}),github_health:crate::github_health::empty(),github_health_receiver:None,agent_gang:json!([]),agent_gang_at:None,studios:json!({"sessions":[],"errors":[]}),studios_at:None,studios_receiver:None,telemetry_view:false,config_view:false,dials:None,dials_at:None,metrics_at:None,proc_snapshot:Vec::new(),mcp_servers:Vec::new(),mcp_sessions:Vec::new(),touches:Vec::new(),skills:Vec::new(),settings:crate::settings::TuiSettings::load(),procs:Vec::new(),history:Vec::new(),logs:json!({"blender":[],"studio":[],"omarchy":[],"commits":[]}),logs_at:None,usage:Vec::new(),gang_selected:0,event_index:0,event_focus:false,event_open:false,event_detail:None,hits:Default::default(),poke:None,muppets_view:false,catalog_selected:0,catalog_show_inactive:None,catalog_columns:std::cell::Cell::new(1),pulse:None,pulse_at:None,spy:None,spy_at:None,message:format!("Blue not started / red failed / yellow stalled / green working / [OK] completed. Production root: {}.",root.display()),scroll:0,panel_config:magicstudio_engine::panels::Config::default(),panel_config_error:None,panel_report:magicstudio_engine::panels::PanelReport::default(),last_panels_revision:None,panel_rows:Vec::new(),panel_detail:Vec::new()};
        match panels::load_config(){Ok(config)=>center.panel_config=config,Err(error)=>center.panel_config_error=Some(error),}
        center.scope=crate::scope::current()?.cloned();
        if let Some(scope)=&center.scope{
            if !center.fields[0].is_empty()&&std::fs::canonicalize(&center.fields[0]).map_err(|e|e.to_string())?!=scope.project{return Err("Restricted: Booth project differs from assignment".into());}
            center.fields[0]=scope.project.to_string_lossy().into_owned();center.fields[1]=scope.project.parent().map(|p|p.to_string_lossy().into_owned()).unwrap_or_default();center.control_view=false;center.message="Assigned project Booth · BOOTH / MOPS / MAYHEM".into();
        }else{center.control_view=true;}
        Ok(center)
    }
    fn catalog(&mut self)->Value{if let Some(scope)=&self.scope{scope.catalog()}else{self.session.execute("project.catalog",json!({})).unwrap_or_else(|e|json!({"error":e.to_string()}))}}
    fn refresh(&mut self) {
        self.poll_github_health();
        // No project is attached: the booth waits in the workshop for you to choose one.
        if self.session.project_path.is_none() { self.control_view=true; }
        let project=self.session.project_path.as_ref().map(|p|p.to_string_lossy().into_owned());
        if self.state["project"].as_str()!=project.as_deref(){
            let catalog=self.state["catalog"].clone();self.state=json!({"project":project,"catalog":catalog,"agent_gang":self.agent_gang});self.pinned_instance=None;
        }
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
                value["catalog"]=self.catalog();
                workshop::reconcile(self,&value["catalog"]);
                value["animation_worker"]=self.state["animation_worker"].clone();
                let live=crate::snapshot::live(&value["ui"],&self.state["snapshot"]);
                value["snapshot"]=crate::snapshot::build(value["project"].as_str(),&value["ui"],&live,&value["activity"]);
                value["tail"]=merge_tail(&self.state["tail"],crate::snapshot::tail(&value["activity"],value["snapshot"]["project"].as_str()));
                let cursor=value["activity"]["next_cursor"].as_u64().unwrap_or(0);
                if self.seen_cursor>0&&cursor>self.seen_cursor{self.flash_after=self.seen_cursor;self.flash_started=Some(std::time::Instant::now());}
                self.seen_cursor=cursor;self.state=value;
                let visible=workshop::order(self);if !visible.contains(&self.catalog_selected)&&let Some(index)=visible.first(){self.catalog_selected=*index;}
                // Which Studio instances exist, and what each one has open. The roster
                // needs real evidence per project instead of a placeholder, so this is
                // read off the instances themselves and refreshed on its own cadence.
                // The telemetry tab is on demand: nothing is sampled while it is closed,
                // so the dials can be lavish without costing anything the rest of the time.
                if self.telemetry_view&&self.scope.is_none(){
                    if self.dials_at.map(|t|t.elapsed().as_secs()>=1).unwrap_or(true){
                        let current=crate::telemetry::all_procs();
                        self.procs=crate::dials::top_processes(&self.proc_snapshot,&current,18);
                        self.proc_snapshot=current;
                        self.mcp_servers=crate::mcp_watch::scan();
                        if let Some(host)=crate::dials::sample_host(self.dials.as_ref()){
                            if let Some(cpu)=host.cpu_pct{self.history.push(cpu);if self.history.len()>240{self.history.remove(0);}}
                            self.dials=Some(host);
                        }
                        self.dials_at=Some(std::time::Instant::now());
                    }
                    // One measured sample per minute: enough to see a trend, cheap to keep.
                    if self.metrics_at.map(|t|t.elapsed().as_secs()>=60).unwrap_or(true){
                        let studios=self.studios["sessions"].as_array().map(|v|v.len()).unwrap_or(0);
                        let stale=self.studios["errors"].as_array().map(|v|v.len()).unwrap_or(0);
                        crate::mayhem::record(&crate::mayhem::Sample::capture(&self.mcp_servers,studios,stale));
                        self.metrics_at=Some(std::time::Instant::now());
                    }
                    if self.logs_at.map(|t|t.elapsed().as_secs()>=5).unwrap_or(true){
                        let blender=self.state["animation_worker"]["log"].as_str().unwrap_or("").to_string();
                        let studio:Vec<String>=self.state["tail"].as_array().into_iter().flatten().rev().take(8).map(|e|format!("#{} {} {}",e["sequence"],e["type"].as_str().unwrap_or("?"),e["command"].as_str().unwrap_or(""))).collect();
                        self.logs=json!({
                            "blender":crate::dials::file_tail(&blender,10),
                            "studio":studio,
                            "omarchy":crate::dials::journal_tail(10),
                            "commits":crate::dials::commits(&std::env::current_dir().unwrap_or_default(),8),
                        });
                        self.usage=crate::dials::agent_usage();
                        self.logs_at=Some(std::time::Instant::now());
                    }
                }
                self.poll_studios();
                if self.chat_sent_request.is_some() && self.message.starts_with("Chat ") && !self.message.starts_with("Chat error:") && !self.message.starts_with("Chat blocked:") && self.chat_receiver.is_none(){self.message=chat_outcome(&self.state);}
                self.observe_chat_delivery();

            },Err(error)=>{let catalog=self.catalog();workshop::reconcile(self,&catalog);self.state["catalog"]=catalog;self.message=error.clone();self.state["ui"]=json!({"connection":"stale_or_unavailable","ui_ready":false,"error":error,"recovery":"F8 explicitly reattaches the same project; Studio is never switched."});},
        }
                if self.agent_gang_at.map(|t|t.elapsed().as_secs()>=self.settings.agent_refresh_secs).unwrap_or(true) {
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
                            if let Some(scope)=&self.scope{agents=scope.roster(agents);}
                            agents.sort_by_key(|a|(!a["name"].is_string(),a["workspace_id"].as_str()!=Some("wG")));
                            let sprocket_selected=self.agent_gang.as_array().is_some_and(|a|!a.is_empty()&&self.gang_selected==a.len());
                            let previous=self.selected_agent().and_then(|g|g["pane_id"].as_str()).map(str::to_owned);
                            if let Some(previous)=previous{
                                if let Some(index)=agents.iter().position(|g|g["pane_id"].as_str()==Some(previous.as_str())){self.gang_selected=index;}
                                else{self.gang_selected=self.gang_selected.min(agents.len());self.spy=None;self.spy_at=None;}
                            }
                            if sprocket_selected{self.gang_selected=agents.len();}
                            self.agent_gang=json!(agents);
                            self.agent_gang_at=Some(std::time::Instant::now());
                            // Read /proc only while MOPS is the open tab.
                            if self.muppets_view{self.mcp_sessions=mcp_presence::sessions();self.touches=crate::touch::read_all();}
                            match self.session.execute("skills.list", json!({})) {
                                Ok(v)=>{if let Some(arr)=v["skills"].as_array(){self.skills=arr.clone();}}
                                Err(_)=>{self.skills=Vec::new();}
                            }
                        }
                    }
                }
                self.state["agent_gang"]=self.agent_gang.clone();
                // The backstage watch: read what an agent last said, straight from its pane.
                if self.scope.is_some(){self.spy=self.selected_agent().and_then(|g|g["pane_id"].as_str()).map(|pane|roles::activity(self,pane));}
                else if self.spy_at.map(|t|t.elapsed().as_secs()>=5).unwrap_or(true){
                    self.spy=None;
                    if let Some(name)=self.selected_agent().and_then(|g|g["pane_id"].as_str().or(g["name"].as_str())).map(str::to_owned){
                        if let Ok(out)=std::process::Command::new("herdr").args(["agent","read",&name,"--source","recent-unwrapped","--lines","40"]).output(){
                            if out.status.success() && let Ok(text)=String::from_utf8(out.stdout){self.spy=Some(text.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"));}
                        }
                    }
                    self.spy_at=Some(std::time::Instant::now());
                }
                self.sample_studio();
        panels::sync(self);
        if let Some(scope)=&self.scope{self.state=scope.redact(self.state.clone());}

    }
    fn refresh_github_health(&mut self) {
        if self.scope.is_some() || self.github_health_receiver.is_some() { return; }
        self.github_health["refreshing"] = json!(true);
        let repo = crate::github_health::repo_root(&std::env::current_dir().unwrap_or_default());
        let (sender, receiver) = std::sync::mpsc::channel();
        self.github_health_receiver = Some(receiver);
        std::thread::spawn(move || { let _ = sender.send(crate::github_health::inspect(&repo)); });
    }
    fn poll_github_health(&mut self) {
        let Some(receiver) = &self.github_health_receiver else { return; };
        match receiver.try_recv() {
            Ok(value) => { self.github_health = value; self.github_health_receiver = None; }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.github_health = json!({"schema":"mc.github-health/1","ok":false,"observed_at_ms":null,"errors":["GitHub observation worker disconnected"],"pull_requests":null});
                self.github_health_receiver = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    fn poll_studios(&mut self){
        if self.scope.is_some(){return;}
        if let Some(receiver)=&self.studios_receiver{
            match receiver.try_recv(){
                Ok(discovery)=>{self.studios=discovery;self.studios_receiver=None;},
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{self.studios["observation_error"]=json!("Studio discovery unavailable");self.studios_receiver=None;},
                Err(std::sync::mpsc::TryRecvError::Empty)=>{if self.studios_at.is_some_and(|at|at.elapsed().as_secs()>=6){self.studios["observation_error"]=json!("Studio discovery pending");}},
            }
        }
        if self.studios_receiver.is_none()&&self.studios_at.is_none_or(|at|at.elapsed().as_secs()>=self.settings.studio_discovery_secs){
            let (sender,receiver)=std::sync::mpsc::channel();self.studios_receiver=Some(receiver);self.studios_at=Some(std::time::Instant::now());
            // Retain one slow discovery instead of discarding its result or accumulating threads.
            std::thread::spawn(move||{let mut value=magicstudio_engine::studio_sessions::discover();value["observed_at_ms"]=json!(crate::worker::now_ms());let _=sender.send(value);});
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
    /// Which view the desk is showing, so a restart can come back to it.
    fn view_name(&self)->&'static str{
        if self.telemetry_view{"animal"}else if self.muppets_view{"mops"}else if self.scope.is_none()&&(self.control_view||!self.state["project"].is_string()){"workshop"}else{"booth"}
    }
    /// The agent the gang strip has selected, with its pane and its live state.
    fn selected_agent(&self)->Option<&Value>{
        self.agent_gang.as_array().and_then(|agents|agents.get(self.gang_selected)).filter(|agent|agent["name"].is_string())
    }
    fn cycle_agent(&mut self,step:isize){
        let count=self.agent_gang.as_array().map_or(0,Vec::len)+1;
        self.gang_selected=(self.gang_selected as isize+step).rem_euclid(count as isize) as usize;
        self.spy=None;self.spy_at=None;self.scroll=0;
    }
    /// An agent that is idle or stalled can be poked; a working one would be interrupted.
    fn pokeable(status:&str)->bool{
        matches!(status,"idle"|"done"|"stalled"|"blocked")
    }
    /// The default nudge: one line, asking for the state of the work.
    fn poke_prompt()->&'static str{
        "Status check: what are you working on right now, and is anything blocking you? Reply with one short line."
    }
    /// Open the composer for the selected agent.
    fn begin_poke(&mut self){
        let Some(agent)=self.selected_agent() else {self.message="No agent selected in the gang strip.".into();return;};
        let name=agent["name"].as_str().unwrap_or("agent").to_string();
        let target=agent["pane_id"].as_str().unwrap_or("").to_string();
        if target.is_empty(){self.message="That agent has no pane to send to.".into();return;}
        let text=if self.event_open{
            let event=selected_event(self).cloned().unwrap_or(Value::Null);
            format!("Please investigate this Magic Studio event and propose a fix.\nProject: {}\nCommand: {}\nError: {}\nRequest: {}\nAdditional details: ",event["project"].as_str().unwrap_or("Unknown"),event["command"].as_str().unwrap_or("Unknown"),event["error"].as_str().unwrap_or("No error text"),event["request_id"].as_str().unwrap_or("Unknown"))
        }else{Self::poke_prompt().to_string()};
        self.poke=Some(Poke{target,name,text});
    }
    /// The team check-in: work counts only once it is committed, so that is all it asks for.
    fn check_in_prompt()->&'static str{
        "Check in: commit what you have through the pre-commit hook (no manual test runs), then reply with one line: the SHA and what changed, or 'nothing to commit'."
    }
    /// Send the check-in to every idle agent at once; busy agents are named, never interrupted.
    fn check_in_all(&mut self){
        let agents:Vec<(String,String,String)>=self.agent_gang.as_array().into_iter().flatten()
            .filter_map(|g|Some((g["name"].as_str()?.to_string(),g["pane_id"].as_str()?.to_string(),g["agent_status"].as_str().unwrap_or("").to_string())))
            .collect();
        let (mut sent,mut busy,mut failed)=(Vec::new(),Vec::new(),Vec::new());
        for (name,target,status) in agents {
            if !Self::pokeable(&status){busy.push(name);}
            else if self.send_poke(&target,Self::check_in_prompt()){sent.push(name);}
            else {failed.push(name);}
        }
        let list=|names:&Vec<String>|if names.is_empty(){"nobody".to_string()}else{names.join(", ")};
        self.message=format!("Check-in sent to {}{}{}",list(&sent),
            if busy.is_empty(){String::new()}else{format!(" · busy, not interrupted: {}",list(&busy))},
            if failed.is_empty(){String::new()}else{format!(" · not delivered: {}",list(&failed))});
    }
    /// Send the default nudge immediately, without the composer.
    fn quick_poke(&mut self){
        let Some(agent)=self.selected_agent() else {self.message="No agent selected in the gang strip.".into();return;};
        let status=agent["agent_status"].as_str().unwrap_or("");
        if !Self::pokeable(status){
            self.message=format!("{} is {status}; a poke would interrupt work in progress.",agent["name"].as_str().unwrap_or("agent"));
            return;
        }
        let name=agent["name"].as_str().unwrap_or("agent").to_string();
        let target=agent["pane_id"].as_str().unwrap_or("").to_string();
        if target.is_empty(){self.message="That agent has no pane to send to.".into();return;}
        let text=Self::poke_prompt().to_string();
        self.send_poke(&target,&text);
        let _=name;
    }
    /// Deliver a message to an agent pane. The multiplexer decides whether it is
    /// accepted; a refusal is reported as it happened, never as a delivery.
    fn send_poke(&mut self,target:&str,text:&str)->bool{
        if self.scope.as_ref().is_some_and(|scope|scope.member(target).is_none()){self.message="Restricted: recipient is outside the project team".into();return false;}
        match std::process::Command::new("herdr").args(["agent","prompt",target,text]).output(){
            Ok(output)=>{
                let said=String::from_utf8_lossy(&output.stdout).trim().to_string();
                let parsed=serde_json::from_str::<Value>(&said).ok();
                let error=parsed.as_ref().and_then(|v|v.get("error").or_else(||v["result"].get("error"))).filter(|v|!v.is_null());
                if let Some(error)=error{self.message=format!("Not sent: {error}. Draft retained.");false}
                else if !output.status.success(){self.message=format!("Not sent: {}. Draft retained.",String::from_utf8_lossy(&output.stderr).trim());false}
                else{self.message=format!("Submitted to {target}; acknowledgement not yet confirmed.");true}
            }
            Err(error)=>{self.message=format!("Not sent: {error}. Draft retained.");false}
        }
    }
    /// Attach the project chosen in the workshop and watch it from the booth.
    fn observe_selected(&mut self){
        let Some(path)=self.state["catalog"]["projects"].as_array().and_then(|p|p.get(self.catalog_selected)).and_then(|p|p["path"].as_str().map(str::to_owned)) else {return;};
        if let Err(error)=self.session.execute("project.attach",json!({"path":path})){self.message=format!("Project observation failed: {error}");return;}
        self.fields[0]=path;self.message.clear();
        let catalog=self.state["catalog"].clone();
        self.state=json!({"project":self.fields[0],"catalog":catalog,"agent_gang":self.agent_gang});
        self.control_view=false;self.details_open=false;self.command_selected=0;
        self.muppets_view=false;
        self.telemetry_view=false;
        self.event_open=false;self.event_focus=false;self.event_detail=None;
        self.state["tail"]=json!([]);self.state["snapshot"]=Value::Null;self.scroll=0;
        self.pinned_instance=None;
        self.refresh();
    }
    fn observe_chat_delivery(&mut self){
        let receipt=&self.state["chat"]["request"];
        if self.chat_sent_request.as_deref()==receipt["request_id"].as_str() && self.chat_sent_request.is_some() && matches!(receipt["status"].as_str(),Some("running"|"replied"|"stalled")){
            if self.chat_draft==self.chat_sent{self.chat_draft.clear();}
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
        if self.scope.as_ref().is_some_and(|scope|!scope.permits("agent.send")){self.message="Restricted: Sproket send is not granted; draft retained.".into();return;}
        if self.chat_draft_project.as_deref().is_some_and(|p|Some(p)!=self.state["project"].as_str()){self.message="Chat blocked: this draft belongs to another project. Reopen that project to send it.".into();return;}
        let (Some(endpoint),Some(project),Some(instance))=(self.state["ui"]["endpoint"].as_str(),self.state["project"].as_str(),self.pinned_instance.as_deref()) else {self.message="Chat blocked: no matched pinned Studio/project; no message sent".into();return;};
        if self.state["ui"]["active_project"].as_str()!=Some(project){self.message="Chat blocked: Studio project differs; no message sent".into();return;}
        let Ok(exe)=std::env::current_exe()else{self.message="Chat blocked: current CLI executable unavailable".into();return;};
        let request_id=format!("booth-chat-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_nanos()).unwrap_or(0));
        self.chat_sent_request=Some(request_id.clone());
        let params=json!({"message":self.chat_draft.trim(),"new_chat":!self.state["chat"]["session"].is_string(),"session":self.state["chat"]["session"],"wait":0,"_expected_project":project,"_expected_instance":instance,"_request_id":request_id,"_origin":"Booth chat","_client":"Booth user"});
        let assignment=self.scope.clone();let endpoint=endpoint.to_owned();let (tx,rx)=std::sync::mpsc::channel();self.chat_receiver=Some(rx);self.chat_sent=self.chat_draft.clone();self.message="Chat queued locally; CLI acceptance and reply not yet confirmed".into();
        std::thread::spawn(move||{
            let mut command=std::process::Command::new(exe);if let Some(scope)=assignment{scope.inherit(&mut command);}command.args(["--bridge",&endpoint,"exec","agent.send"]);
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
        let assignment=self.scope.clone();let endpoint=endpoint.to_owned();let project=project.to_owned();let instance=instance.to_owned();let(tx,rx)=std::sync::mpsc::channel();self.doctor_receiver=Some(rx);
        std::thread::spawn(move||{
            let mut command=std::process::Command::new(exe);if let Some(scope)=assignment{scope.inherit(&mut command);}let result=command.args(["--bridge",&endpoint,"exec","storyboard.doctor",&format!("_expected_project={project}"),&format!("_expected_instance={instance}")]).output().map_err(|e|e.to_string()).and_then(|o|serde_json::from_slice::<Value>(&o.stdout).map_err(|_|String::from_utf8_lossy(&o.stderr).into_owned()));let _=tx.send(result);
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
                    out.push_str(&format!("  owner limits: {}\n", limits_text(&owner["limits"])));
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
                    value["worker_owner_status"].as_str().unwrap_or("not reported by this engine build")
                ));
                if let Some(error) = value["worker_owner_error"].as_str() {
                    out.push_str(&format!("  slot error: {error}\n"));
                }
            }
            Err(error) => out.push_str(&format!("  slot: not reported by this engine build ({error})\n")),
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
                observation["frames_completed"].as_u64().map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                observation["frames_total"].as_u64().map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                stamp_text(&observation["updated_at"])
            ));
            out.push_str(&format!("  limits: {}\n", limits_text(&observation["limits"])));
            out.push_str(&format!(
                "  submitted_at: {} · started_at: {} · cancellation_requested_at: {}\n",
                stamp_text(&observation["submitted_at"]),
                stamp_text(&observation["started_at"]),
                stamp_text(&observation["cancellation_requested_at"])
            ));
            out.push_str(&format!("  telemetry: {}\n", crate::worker::resource_line(&observation)));
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
                    for problem in project["readiness"]["problems"].as_array().into_iter().flatten() {
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
    let mib = |value: &Value| value.as_u64().map(|v| format!("{:.0}MiB", v as f64 / 1048576.0)).unwrap_or_else(|| "?".into());
    format!(
        "memory {} · swap {} · cpu {}% · timeout {}",
        mib(&limits["memory_max_bytes"]),
        mib(&limits["swap_max_bytes"]),
        limits["cpu_quota_percent"].as_u64().map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
        limits["timeout_seconds"].as_u64().map(|v| format!("{v}s")).unwrap_or_else(|| "?".into())
    )
}
/// A timestamp field exactly as recorded: decimal epoch-ms string, or null.
fn stamp_text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| "null".into())
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
            Span::styled("/ COMMAND CENTER · THE BOOTH  ·  F6 Monitor",Style::default().fg(BLUE)),
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
fn signal_color(signal:magicstudio_engine::panels::Signal)->Color{
 match signal{
  magicstudio_engine::panels::Signal::Idle=>MUTED,
  magicstudio_engine::panels::Signal::InUse=>GREEN,
  magicstudio_engine::panels::Signal::Attention=>AMBER,
  magicstudio_engine::panels::Signal::Problem=>RED,
 }
}
fn shell_quote(value:&str)->String{format!("'{}'",value.replace('\'',"'\\''"))}
impl Center{
 fn copy_commands(&self)->Vec<(&str,String)>{
  if let Some(value)=&self.onboarding{return vec![("Start the assigned project Booth",value["launch"].as_str().unwrap_or("").into())];}
  if let Some(scope)=&self.scope{
   let prefix=format!("{}={} mc",crate::scope::ENV,shell_quote(&scope.file.to_string_lossy()));
   return vec![("Inspect assigned project",format!("{prefix} exec center.status")),("Read project receipts",format!("{prefix} exec center.events")),("Show granted actions",format!("{prefix} commands"))];
  }
  if self.view_name()=="workshop"&&!self.event_open{return workshop::commands(self);}
  if self.event_open && let Some(command)=selected_event(self).and_then(|e|e["command"].as_str()){
   return vec![("Inspect this event’s command contract",format!("mc commands {}",shell_quote(command)))];
  }
  let exe=shell_quote(&std::env::current_exe().map(|p|p.to_string_lossy().into_owned()).unwrap_or_else(|_|"mc".into()));
  let exe=if let Some(scope)=&self.scope{format!("{}={} {exe}",crate::scope::ENV,shell_quote(&scope.file.to_string_lossy()))}else{exe};
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
            while index < chars.len() && !chars[index].is_whitespace() && chars[index] != '\'' && chars[index] != '"' {
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
    lines.iter().map(|line| line.width().max(1).div_ceil(usize::from(width.max(1))) as u16).sum()
}
fn film_holes(width:usize)->String{(0..width).map(|i|if i%4==3{' '}else{'▪'}).collect()}
fn bounded_line(line:Line<'static>,width:usize)->Line<'static>{
 let mut remaining=width;let mut spans=Vec::new();
 for span in line.spans{
  if remaining==0{break;}
  let len=Line::raw(span.content.as_ref()).width();
  spans.push(Span::styled(fit_text(&span.content,remaining),span.style));
  if len>remaining{break;}
  remaining-=len;
 }
 Line::from(spans)
}
fn film_status(width:usize,line:Line<'static>)->Line<'static>{
 let line=bounded_line(line,width.saturating_sub(8));
 let spare=width.saturating_sub(line.width());let left=spare/2;
 let mut spans=vec![Span::styled(film_holes(left),FILM)];
 spans.extend(line.spans);spans.push(Span::styled(film_holes(spare-left),FILM));Line::from(spans)
}
fn top_bar(c:&Center,width:usize,control:bool)->Line<'static>{
 let view=if c.telemetry_view{"MAYHEM"}else if c.muppets_view{"MOPS"}else if control{"WORKSHOP"}else{"BOOTH"};
 let title=fit_text(&format!("MAGIC STUDIO · {view}"),width.saturating_sub(8));
 let title_width=Line::raw(&title).width();let start=width.saturating_sub(title_width)/2;
 let left_room=start.saturating_sub(2);let right_room=width.saturating_sub(start+title_width+2);
 let ui=&c.state["ui"];let snap=&c.state["snapshot"];
 let matched=ui["connection"]=="connected"&&c.state["project"].is_string()&&ui["active_project"]==c.state["project"];
 let project=c.state["project"].as_str().and_then(|p|std::path::Path::new(p).file_stem()).and_then(|p|p.to_str()).unwrap_or("No project");
 let studio=if matched{"● Studio"}else{"○ Studio"};
 let saved=if matched{match ui["engine_unsaved"].as_bool(){Some(false)=>"✓ Saved",Some(true)=>"! Edits",None=>"? Save"}}else{"? Save"};
 let mut details=Vec::new();
 if let Some(clip)=snap["clip"].as_str().filter(|v|*v!="?"){details.push(format!("@{}",clip.trim_start_matches('@')));}
 if let Some(time)=snap["playhead_sec"].as_f64(){details.push(format!("▶{time:.2}s"));}
 if let Some(part)=snap["selected_part"].as_str().filter(|v|*v!="?"){details.push(part.to_owned());}
 let base=format!("{project} · {studio} · {saved}");
 let full=if details.is_empty(){base.clone()}else{format!("{base} · {}",details.join(" · "))};
 let candidates=[full,base,format!("{studio} · {saved}"),studio.into(),String::new()];
 let left=candidates.into_iter().find(|v|Line::raw(v).width()<=left_room).unwrap_or_default();
 let metrics=match c.studio_sample(){Some(sample)=>format!("CPU {} · RAM {:.0}M",sample.cpu_pct.map(|v|format!("{v:.0}%")).unwrap_or("?".into()),crate::telemetry::rss_mb(&sample.procs)),None=>"CPU ? · RAM ?".into()};
 let(status,short,colour)=if let Some(worker)=worker_observation(c).filter(|v|v["state"]!="not_started"){
  (crate::worker::headline(worker),crate::worker::short_headline(worker),worker_colour(worker))
 }else if snap["last_error"].is_object(){("! Error · E inspect".into(),"! Error".into(),RED)}
 else if matched{("● Live".into(),"● Live".into(),GREEN)}else{("○ Offline".into(),"○ Offline".into(),AMBER)};
 let fleet=control&&!c.muppets_view&&!c.telemetry_view;
 let left=if fleet{fit_text(" Master · Jesse + Gonzo",left_room)}else{left};
 let (status,short)=if fleet{let (active,inactive)=workshop::counts(c);if c.state["catalog"]["projects"].is_array(){(format!("{} · {} Projects",workshop::activity_label(c),active+inactive),format!("{} Projects",active+inactive))}else{("Catalog unavailable".into(),"? Projects".into())}}else{(status,short)};
 let colour=if fleet{TEAL}else{colour};
 let full=if fleet{status.clone()}else{format!("{status} · {metrics}")};
 let right=[full,status,short,String::new()].into_iter().find(|v|Line::raw(v).width()<=right_room).unwrap_or_default();
 let mut spans=vec![Span::styled(left.clone(),MUTED),Span::styled(film_holes(start.saturating_sub(Line::raw(&left).width())),FILM)];
 spans.extend(crate::logo::compact_title(&title).spans);
 spans.push(Span::styled(film_holes(width.saturating_sub(start+title_width+Line::raw(&right).width())),FILM));
 spans.push(Span::styled(right,colour));Line::from(spans)
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
 let mut lines:Vec<Line<'static>>=Vec::new();
 for(i,(label,_))in PROJECT_STAGES.iter().enumerate(){
  let (glyph,word,color)=match stage_evidence(c,i).and_then(|v|v["state"].as_str()){
   Some("ready")=>("●","Ready",GREEN),Some("running"|"working")=>("●","Working",GREEN),
   Some("blocked")=>("●","Blocked",AMBER),Some("failed"|"error")=>("●","Failed",RED),
   Some("stalled")=>("●","Stalled",AMBER),Some("missing")=>("○","Missing",BLUE),_=>("?","Unknown",MUTED)
  };
  let line=Line::from(vec![Span::styled(if i==c.stage_selected{"› "}else{"  "},DESK_FG),Span::styled(format!("{label:<16} "),MUTED),Span::styled(glyph,color),Span::styled(format!(" {word:<7}"),DESK_FG)]);
  if stacked&&i%2==1{let mut previous=lines.pop().unwrap_or_default();previous.spans.push(Span::styled("  │  ",DIM));previous.spans.extend(line.spans);lines.push(previous);}else{lines.push(line);}
 }
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
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(width.saturating_sub(filled)));
    let colour = if ratio < 0.5 { GREEN } else if ratio < 0.8 { AMBER } else { RED };
    (bar, colour)
}

/// A render's progress as a timeline: what is finished, the playhead at the
/// boundary, and what is still to come. Borrowed from an animation editor's
/// frame ruler rather than a plain meter, because a render is a sequence.
fn frame_track(done:Option<u64>,total:Option<u64>,width:usize)->Vec<Span<'static>>{
    let (Some(done),Some(total))=(done,total) else {
        return vec![Span::styled("·".repeat(width),Style::default().fg(MUTED))];
    };
    if total==0{return vec![Span::styled("·".repeat(width),Style::default().fg(MUTED))];}
    let cells=width.max(6);
    let filled=((done.min(total) as f64/total as f64)*cells as f64).round() as usize;
    vec![
        Span::styled("█".repeat(filled),Style::default().fg(BLUE)),
        Span::styled("▏",Style::default().fg(TEAL)),
        Span::styled("░".repeat(cells.saturating_sub(filled)),Style::default().fg(MUTED)),
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
    let mut spans = vec![Span::styled(format!("{label:<9}"), Style::default().fg(MUTED))];
    spans.extend(track);
    spans.push(Span::styled(format!("  {value}"), Style::default().fg(DESK_FG)));
    Line::from(spans)
}
fn render_worker(f:&mut Frame,value:&Value,area:ratatui::layout::Rect){
    let width=area.width.saturating_sub(2) as usize;
    let inner=area.height.saturating_sub(2) as usize;
    if value["state"]=="not_started"{
        let mut lines=vec![styled(crate::worker::state_line(value),MUTED)];
        lines.push(styled(fit_text("Waiting for a render submission · Backstage never starts one by polling",width),MUTED));
        if let Some(owner)=value["worker_owner"]["job"].as_str(){lines.push(styled(fit_text(&format!("Global worker busy: {owner}"),width),AMBER));}
        f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).block(booth::panel("ARTWORK IN MOTION",false)),area);
        return;
    }
    // Rows are ordered by what matters first, and the panel shows as many as fit.
    let telemetry=&value["telemetry"];
    let frames_value=format!("{}/{}",value["frames_completed"].as_u64().map(|v|v.to_string()).unwrap_or_else(||"?".into()),value["frames_total"].as_u64().map(|v|v.to_string()).unwrap_or_else(||"?".into()));
    let frames_track=frame_track(value["frames_completed"].as_u64(),value["frames_total"].as_u64(),20);
    let limit=telemetry["memory_max_bytes"].as_u64().or_else(||value["limits"]["memory_max_bytes"].as_u64());
    let (ram_bar,ram_colour)=meter(telemetry["memory_current_bytes"].as_u64(),limit,16);
    let mib=|v:&Value|v.as_u64().map(|n|format!("{:.0}MiB",n as f64/1048576.0)).unwrap_or_else(||"?".into());
    let ram_value=format!("{} / {} · peak {} · swap {} / {}",mib(&telemetry["memory_current_bytes"]),mib(&telemetry["memory_max_bytes"]),mib(&telemetry["memory_peak_bytes"]),mib(&telemetry["swap_current_bytes"]),mib(&telemetry["swap_max_bytes"]));
    let selection=&value["selection"];
    let identity=format!("{} · {} · {}",selection["character"].as_str().unwrap_or("character ?"),selection["track"].as_str().unwrap_or("action ?"),value["job"].as_str().unwrap_or("job ?"));
    let source=if value["telemetry_source"]=="terminal_receipt" || matches!(value["state"].as_str(),Some("completed"|"cancelled"|"failed")){"saved receipt"}else{"worker service"};
    let age=crate::worker::age_ms(value,"updated_at").map(booth::age_label).unwrap_or_else(||"?".into());
    let source_row=format!("{source} · recorded {age} ago{}",if crate::worker::stale(value){" · STALE"}else{""});
    let owner=&value["worker_owner"];
    let owner_line=if owner.is_object(){format!("{} · {} · {}",owner["job"].as_str().unwrap_or("?"),owner["project"].as_str().unwrap_or("project unknown"),owner["state"].as_str().unwrap_or("state ?"))}else{format!("Global worker owner: {}",value["worker_owner_status"].as_str().unwrap_or("unavailable"))};
    let mut rows:Vec<Line<'static>>=Vec::new();
    rows.push(Line::from(vec![Span::styled("● ",worker_colour(value)),Span::styled(if inner>=3{crate::worker::state_line(value)}else{crate::worker::headline(value)},DESK_FG)]));
    rows.push(track_row("FRAMES",frames_track,&frames_value));
    rows.push(metric_row("RAM",&ram_bar,if ram_colour==GREEN{BLUE}else{ram_colour},&ram_value));
    rows.push(styled(fit_text(&format!("{:<9}{identity}","JOB"),width),MUTED));
    rows.push(styled(fit_text(&format!("{:<9}{source_row}","SOURCE"),width),MUTED));
    rows.push(styled(fit_text(&format!("{:<9}{owner_line}","SLOT"),width),MUTED));
    if let Some(error)=value["observation_error"].as_str().or(value["error"].as_str()).or(value["detail"].as_str()){
        rows.push(styled(fit_text(error,width),RED));
    }
    let lines:Vec<Line<'static>>=rows.into_iter().take(inner.max(1)).collect();
    f.render_widget(Paragraph::new(lines).block(booth::panel("ARTWORK IN MOTION",false)),area);
    booth::motion_border(f,value,area);
}
fn conversation_text(text:&str)->&str{
    let text=text.split("\n\n[Magic Studio context at send time:").next().unwrap_or("");
    // Migration-only: hide injected context in old stored turns; never emit the old branding.
    text.split("\n\n[AnimationCraft context at send time:").next().unwrap_or("")
}
fn render_conversation(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){booth::render_chat(f,c,area);}
/// The booth keeps typed events past the journal's three-action retention while it runs.
fn merge_tail(old:&Value,new:Vec<Value>)->Value{
 let mut events=old.as_array().cloned().unwrap_or_default();let seen=events.iter().filter_map(|e|e["sequence"].as_u64()).max().unwrap_or(0);
 events.extend(new.into_iter().filter(|e|e["sequence"].as_u64().is_some_and(|n|n>seen)));let excess=events.len().saturating_sub(50);events.drain(..excess);json!(events)
}
fn selected_event(c:&Center)->Option<&Value>{if c.event_open{return c.event_detail.as_ref();}c.state["tail"].as_array()?.iter().rev().nth(c.event_index)}
fn event_agent(c:&Center,event:&Value)->Option<usize>{
 let agents=c.state["agent_gang"].as_array()?;
 if let Some(pane)=event["pane_id"].as_str(){return agents.iter().position(|g|g["pane_id"].as_str()==Some(pane));}
 let label=event["client"].as_str().or(event["origin"].as_str())?;
 let mut found=agents.iter().enumerate().filter(|(_,g)|g["name"].as_str().is_some_and(|name|name.eq_ignore_ascii_case(label)));
 let(index,_)=found.next()?;if found.next().is_some(){None}else{Some(index)}
}
fn render_tail(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let block=round_panel("EVENTS · E select · Enter inspect",if c.event_focus{TEAL}else{DESK_LINE},c.event_focus);
 let inner=block.inner(area);let rows=inner.height as usize;let width=inner.width as usize;
 let tiles=gang_tiles(c);
 let start=if c.event_focus{c.event_index.saturating_sub(rows.saturating_sub(1))}else{0};
 let mut lines:Vec<Line<'static>>=c.state["tail"].as_array().into_iter().flatten().rev().enumerate().skip(start).take(rows).map(|(i,e)|{
  let owner=event_agent(c,e).and_then(|index|tiles.get(index));
  let identity=owner.map(|g|g.1).unwrap_or(MUTED);
  let label=owner.map(|g|format!("{} {}",moppet_icon(&g.0),g.0)).unwrap_or_else(||"?".into());
  let mark=if e["ok"]==false{"✗"}else{"·"};
  let text=format!("{} {} {} {} {}",if c.event_focus&&i==c.event_index{"›"}else{" "},stamp(e),label,e["command"].as_str().unwrap_or_else(||e["type"].as_str().unwrap_or("?")),e["error"].as_str().unwrap_or(""));
  let mut line=bounded_line(Line::from(vec![Span::styled(mark,if e["ok"]==false{RED}else{identity}),Span::styled(text,identity)]),width);
  if c.event_focus&&i==c.event_index{line=line.style(Style::default().bg(DESK_SELECTED));}line
 }).collect();
 if lines.is_empty(){lines.push(styled("No events recorded",MUTED));}
 f.render_widget(Paragraph::new(lines).block(block),area);
}
fn render_event(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let Some(event)=selected_event(c)else{return;};
 let tiles=gang_tiles(c);let owner=event_agent(c,event).and_then(|index|tiles.get(index));
 let heading=owner.map(|g|format!("{} {}",moppet_icon(&g.0),g.0)).unwrap_or_else(||"Unknown agent".into());
 let colour=owner.map(|g|g.1).unwrap_or(MUTED);
 let mut lines=vec![styled(heading,colour)];
 for(label,value)in [("Time",Some(stamp(event))),("Command",event["command"].as_str().map(str::to_owned)),("Error",event["error"].as_str().map(str::to_owned)),("Source",event["origin"].as_str().map(str::to_owned)),("Client",event["client"].as_str().map(str::to_owned)),("Request",event["request_id"].as_str().map(str::to_owned)),("Project",event["project"].as_str().map(str::to_owned))]{
  if let Some(value)=value{lines.push(Line::from(vec![Span::styled(format!("{label:<9}"),MUTED),Span::styled(value,if label=="Error"{RED}else{DESK_FG})]));}
 }
 let selected=tiles.get(c.gang_selected);
 lines.push(Line::raw(""));
 lines.push(styled(format!("Fix note → {} · , / . choose recipient · p compose",selected.map(|g|format!("{} {}",moppet_icon(&g.0),g.0)).unwrap_or_else(||"Choose an agent".into())),TEAL));
 if owner.is_none(){lines.push(styled("No agent ownership recorded. Choose the recipient explicitly.",MUTED));}
 f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).scroll((c.scroll,0)).block(round_panel("EVENT · C inspect command · Esc back · Pg scroll",colour,true)),area);
}
fn render_next(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let commands=c.copy_commands();let Some((effect,literal))=commands.get(c.command_selected%commands.len().max(1))else{return;};
 let reason=c.state["next_commands"].as_array().into_iter().flatten().find(|v|v["label"].as_str()==Some(*effect)).and_then(|v|v["reason"].as_str()).unwrap_or(effect);
 let mut lines=vec![styled(*effect,DESK_FG)];if reason!=*effect{lines.push(styled(reason,MUTED));}
 lines.push(styled(abbreviate_literal(literal,area.width.saturating_sub(4) as usize),BLUE));
 let title=format!("{} · C Copy · Pg Scroll",booth::action_title(c,literal));
 f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).scroll((c.scroll,0)).block(booth::panel(title,true)),area);
}
/// Shared names and status colours for cards and the selected-agent tabs.
fn gang_tiles(c:&Center)->Vec<(String,Color,Color,&'static str)>{
 let mut tiles:Vec<_>=c.state["agent_gang"].as_array().into_iter().flatten().map(|g|{
  let name=g["name"].as_str().or(g["agent"].as_str()).unwrap_or("?").to_owned();
  let(colour,word)=tally(g["agent_status"].as_str().unwrap_or(""));let identity=moppet_identity(&name);(name,identity,colour,word)
 }).collect();
 let(colour,word)=match c.state["chat"]["request"]["status"].as_str().unwrap_or(""){
  "replied"=>(GREEN,"REPLIED"),"running"|"queued"=>(AMBER,"WORKING"),"error"=>(RED,"ERROR"),_=>(MUTED,"IDLE")
 };
 tiles.push(("Sprocket".into(),moppet_identity("Sprocket"),colour,word));
 let mut used=Vec::new();
 for tile in &mut tiles{
  if used.contains(&tile.1){tile.1=MOPPET_CAST.iter().map(|(_,_,colour)|*colour).chain([Color::Rgb(236,149,149),Color::Rgb(132,202,191)]).find(|colour|!used.contains(colour)).unwrap_or(Color::Indexed((16+used.len()%216) as u8));}
  used.push(tile.1);
 }
 tiles
}
const MOPPET_CAST:[(&str,&str,Color);10]=[
 ("sam","🦅",Color::Rgb(229,196,109)),("kermit","🐸",Color::Rgb(171,214,111)),
 ("gonzo","🚀",Color::Rgb(182,154,248)),("scooter","🛵",Color::Rgb(239,145,181)),
 ("beaker","🧪",Color::Rgb(137,215,182)),("bunsen","🔬",Color::Rgb(112,201,212)),
 ("fozzie","🐻",Color::Rgb(237,170,114)),("statler","🎭",Color::Rgb(197,167,220)),
 ("waldorf","🧐",Color::Rgb(137,175,240)),("sprocket","🐶",Color::Rgb(215,196,162)),
];
fn moppet_cast(name:&str)->Option<&'static (&'static str,&'static str,Color)>{
 let name=name.split_whitespace().next().unwrap_or(name).to_ascii_lowercase();
 MOPPET_CAST.iter().find(|(key,_,_)|*key==name)
}
fn moppet_icon(name:&str)->&'static str{moppet_cast(name).map(|(_,icon,_)|*icon).unwrap_or("🎬")}
fn moppet_identity(name:&str)->Color{moppet_cast(name).map(|(_,_,colour)|*colour).unwrap_or(Color::Rgb(236,149,149))}
fn agent_glyph(word:&str)->&'static str{match word{
 "WORKING"=>{let frame=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t|(t.as_millis()/350)%4).unwrap_or(0);["◐","◓","◑","◒"][frame as usize]},
 "STALLED"=>"◐","ERROR"=>"●","REPLIED"=>"◆",_=>"○"
}}
fn render_gang(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let tiles=gang_tiles(c);let selected=c.gang_selected.min(tiles.len()-1);
 let count=(area.width as usize/16).max(1).min(tiles.len());
 let start=selected.saturating_sub(count-1).min(tiles.len()-count);
 let columns=Layout::horizontal((0..count).map(|_|Constraint::Ratio(1,count as u32))).split(area);
 for (offset,(name,identity,colour,word)) in tiles[start..start+count].iter().enumerate(){
  let active=start+offset==selected;
  let index=start+offset;
  agent_button(c,index,columns[offset],false);
  let title=fit_text(&format!("{}{} {} {name}",if active{"› "}else{""},agent_number(index),moppet_icon(name)),columns[offset].width.saturating_sub(4) as usize);
  let block=Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded)
   .style(Style::default().bg(if active{DESK_SELECTED}else{DESK_BG}))
   .border_style(Style::default().fg(*identity).add_modifier(if active{Modifier::BOLD}else{Modifier::empty()}))
   .title(Span::styled(format!(" {title} "),if active{Style::default().fg(DESK_BG).bg(*identity).add_modifier(Modifier::BOLD)}else{Style::default().fg(*identity)}));
  let line=styled(format!("{} {word}",agent_glyph(word)),*colour);
  f.render_widget(Paragraph::new(line).block(block),columns[offset]);
 }
}
fn agent_number(index:usize)->String{match index{0..=8=>(index+1).to_string(),9=>"0".into(),_=>"·".into()}}
fn agent_button(c:&Center,index:usize,area:ratatui::layout::Rect,tab:bool){
 if area.width==0||area.height==0{return;}
 let pane=c.state["agent_gang"][index]["pane_id"].as_str();
 let identity=pane.map(str::to_owned).unwrap_or_else(||if index+1==gang_tiles(c).len(){"sprocket".into()}else{index.to_string()});
 let params=if let Some(pane)=pane{json!({"pane":pane})}else{json!({"index":index})};
 let label=gang_tiles(c).get(index).map(|g|g.0.clone()).unwrap_or_default();
 c.hits.borrow_mut().push(Hit{id:format!("agent-{}-{identity}",if tab{"tab"}else{"card"}),label,area,action:"agent".into(),params,enabled:c.poke.is_none()&&!c.chat_editing&&!c.show_form});
}
fn render_tabs(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let line=tab_strip(c,area.width as usize);let names:Vec<_>=if c.scope.is_some(){vec!["booth","mops","animal"]}else{vec!["workshop","booth","mops","animal"]};let mut offset=0usize;
 for(i,span)in line.spans.iter().enumerate(){
  let width=Line::from(span.clone()).width();
  if i%2==0&&i/2<names.len(){
   let name=names[i/2];let available=(area.width as usize).saturating_sub(offset).min(width);
   if available>0{c.hits.borrow_mut().push(Hit{id:format!("view-{name}"),label:name.into(),area:ratatui::layout::Rect::new(area.x+offset as u16,area.y,available as u16,1),action:"view".into(),params:json!({"name":name}),enabled:c.poke.is_none()&&!c.chat_editing&&!c.show_form});}
  }
  offset+=width;
 }
 f.render_widget(Paragraph::new(line),area);
}
/// A sliding strip keeps the selected agent visible even in a narrow terminal.
/// The agent band drawn vertically for tall, narrow panes: one row per agent, working ones
/// animated, with the task each is on. The selected agent stays in view when rows run out.
fn render_agent_list(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let block=round_panel("AGENTS · 1–9 select",TEAL,false);
 let inner=block.inner(area);f.render_widget(block,area);
 if inner.height==0{return;}
 let tiles=gang_tiles(c);let selected=c.gang_selected.min(tiles.len()-1);
 let rows=(inner.height as usize).min(tiles.len());
 let start=selected.saturating_sub(rows-1).min(tiles.len()-rows);
 for(offset,(name,identity,colour,word)) in tiles[start..start+rows].iter().enumerate(){
  let index=start+offset;let active=index==selected;
  let row=ratatui::layout::Rect::new(inner.x,inner.y+offset as u16,inner.width,1);
  agent_button(c,index,row,false);
  let task=c.state["agent_gang"].as_array().and_then(|a|a.get(index)).and_then(|g|g["terminal_title_stripped"].as_str()).unwrap_or("");
  let head=format!("{}{} {} {} ",if active{"›"}else{" "},agent_number(index),moppet_icon(name),name);
  let word_cell=format!("{} {word:<7}",agent_glyph(word));
  let room=(row.width as usize).saturating_sub(Line::raw(&head).width()+Line::raw(&word_cell).width()+1);
  let bg=if active{DESK_SELECTED}else{DESK_BG};
  f.render_widget(Paragraph::new(Line::from(vec![
   Span::styled(head,Style::default().fg(*identity).bg(bg).add_modifier(if active{Modifier::BOLD}else{Modifier::empty()})),
   Span::styled(word_cell,Style::default().fg(*colour).bg(bg)),
   Span::styled(format!(" {}",fit_text(task,room)),Style::default().fg(MUTED).bg(bg)),
  ])),row);
 }
}
fn agent_strip(c:&Center,width:usize)->Line<'static>{agent_strip_at(c,width,None)}
fn agent_strip_at(c:&Center,width:usize,area:Option<ratatui::layout::Rect>)->Line<'static>{
 let tiles=gang_tiles(c);let selected=c.gang_selected.min(tiles.len()-1);
 let label=|i:usize|format!(" {} {} {} {} ",agent_number(i),moppet_icon(&tiles[i].0),agent_glyph(tiles[i].3),tiles[i].0);
 let budget=width.saturating_sub(4);
 let mut start=selected;let mut end=selected+1;let mut used=Line::raw(label(selected)).width();
 while start>0 && used+Line::raw(label(start-1)).width()<=budget{start-=1;used+=Line::raw(label(start)).width();}
 while end<tiles.len() && used+Line::raw(label(end)).width()<=budget{used+=Line::raw(label(end)).width();end+=1;}
 let mut spans=vec![Span::styled(if start>0{"‹ "}else{"  "},MUTED)];
 let mut offset=2usize;
 for i in start..end{
  let text=fit_text(&label(i),budget);let cells=Line::raw(&text).width();
  if let Some(area)=area{agent_button(c,i,ratatui::layout::Rect::new(area.x.saturating_add(offset as u16),area.y,cells as u16,1),true);}
  offset+=cells;
  let style=if i==selected{Style::default().fg(DESK_BG).bg(tiles[i].1).add_modifier(Modifier::BOLD)}else{Style::default().fg(tiles[i].1)};
  spans.push(Span::styled(text,style));
 }
 spans.push(Span::styled(if end<tiles.len(){" ›"}else{"  "},MUTED));
 Line::from(spans)
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
  f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).scroll((c.scroll,0)).block(round_panel("MAYHEM · worker / Studio · Pg scroll · D closes",BLUE,false)),area);
}

/// "5m ago" for a unix-seconds timestamp.
fn ago(at:u64)->String{
 let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0);
 let d=now.saturating_sub(at);
 if d<90{format!("{d}s ago")}else if d<5400{format!("{}m ago",d/60)}else{format!("{}h ago",d/3600)}
}
/// One agent as MOPS shows it: the same facts feed the roster row and the detail panel.
struct MopsAgent{index:usize,pane:Option<String>,name:String,icon:&'static str,identity:Color,provider:String,word:&'static str,state:Color,
 mcp:Option<(String,Color)>,task:String,folder:String,session:Option<String>,harness:Option<String>,tokens:Option<String>,studio:Value}

fn mops_agents(c:&Center)->Vec<MopsAgent>{
 let tiles=gang_tiles(c);
 tiles.iter().enumerate().map(|(index,(name,identity,state,word))|{
  let g=c.state["agent_gang"].as_array().and_then(|a|a.get(index));
  let text=|v:&Value|v.as_str().filter(|s|!s.is_empty()).map(str::to_owned);
  let field=|key:&str|g.and_then(|g|text(&g[key]));
  let provider=field("agent");
  let mcp=g.and_then(|g|g["pane_id"].as_str()).and_then(|pane|c.mcp_sessions.iter().find(|m|m["pane"].as_str()==Some(pane))).map(|m|{
   if m["stale"]==true{("OUT OF DATE, restart its MCP".to_owned(),RED)}
   else{(format!("v{} · {} · {}",m["version"].as_str().unwrap_or("?"),if m["bridge"].is_string(){"window"}else{"headless"},ago(m["loaded_at"].as_u64().unwrap_or(0))),GREEN)}
  });
  let tokens=provider.as_deref().and_then(|p|c.usage.iter().find(|u|u["name"].as_str().is_some_and(|n|n.eq_ignore_ascii_case(p)))).and_then(|u|{
   Some(format!("{:.1}M today · {} sessions · provider total",u["todayTotalTokens"].as_u64()? as f64/1_000_000.0,u["todaySessions"].as_u64().unwrap_or(0)))
  });
  let session=g.map(|g|&g["agent_session"]);
  let studio=field("pane_id").map(|pane|crate::studios::assignment(&pane,&c.studios)).unwrap_or(Value::Null);
  MopsAgent{index,pane:g.and_then(|g|g["pane_id"].as_str()).map(str::to_owned),name:name.clone(),icon:moppet_icon(name),identity:*identity,provider:provider.clone().unwrap_or_else(||if g.is_none(){"in-app".into()}else{"?".into()}),
   word,state:*state,mcp,task:field("terminal_title_stripped").unwrap_or_else(||if g.is_none(){"the app's own agent".into()}else{String::new()}),
   folder:field("cwd").or(field("foreground_cwd")).unwrap_or_default(),
   session:session.and_then(|s|text(&s["value"])).map(|v|v.chars().take(8).collect()),
   harness:session.and_then(|s|text(&s["source"])).map(|v|v.trim_start_matches("herdr:").to_owned()),tokens,studio}
 }).collect()
}

/// The newest thing the agent did: its last tool or thought marker, else the last line that is
/// not a footer or hint. Runs of spaces collapse so a status bar cannot wrap the row.
fn latest_activity(spy:&str)->Option<String>{
 let lines:Vec<String>=spy.lines().map(|l|l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l|!l.is_empty()).collect();
 let marked=lines.iter().rev().find(|l|l.starts_with(['●','▸','•','$','✓','+'])&&!l.starts_with("▸ Thought for"));
 marked.or_else(||lines.iter().rev().find(|l|!l.starts_with(['─','│','>'])&&!l.starts_with("Thought for")&&!l.starts_with("esc ")&&!l.starts_with("Press ")&&!l.contains("ctrl+")&&!l.contains("to interrupt"))).cloned()
}

/// The newest command (and UI control) this agent's pane sent through the CLI.
/// Returns `(what, unix seconds)`.
fn last_touch(c:&Center,a:&MopsAgent)->Option<(String,u64)>{
 let client=format!("herdr:{}",a.pane.as_deref()?);
 let t=c.touches.iter().find(|t|t["client"].as_str()==Some(client.as_str()))?;
 let what=match(t["control"].as_str(),t["command"].as_str()){(Some(c),Some(m))=>format!("{c} · {m}"),(None,Some(m))=>m.to_owned(),(Some(c),None)=>c.to_owned(),_=>"unmapped interaction".to_owned()};
 Some((what,t["at"].as_u64().unwrap_or(0)))
}

/// What is known about one agent. A fact nobody reported reads "not reported", never zero.
fn mops_facts<'a>(c:&'a Center,a:&'a MopsAgent)->Vec<(&'a str,String,Color)>{
 let known=|v:&Option<String>|match v{Some(v)=>(v.clone(),DESK_FG),None=>("not reported".to_owned(),DIM)};
 let (mcp,mcp_colour)=a.mcp.clone().map(|(t,c)|(t,c)).unwrap_or(("not reported".into(),DIM));
 let (harness,hc)=known(&a.harness);let (session,sc)=known(&a.session);let (tokens,tc)=known(&a.tokens);
 let activity=if a.index==c.gang_selected.min(c.state["agent_gang"].as_array().map_or(0,Vec::len)){c.spy.as_deref().and_then(latest_activity)}else{None};
 let facts = vec![
  ("Task",if a.task.is_empty(){"not reported".into()}else{a.task.clone()},if a.task.is_empty(){DIM}else{DESK_FG}),
  ("Studio",a.studio["state"].as_str().unwrap_or("unassigned").to_owned(),if a.studio["state"]=="connected"{TEAL}else{AMBER}),
  ("Port",a.studio["port"].as_u64().map(|v|v.to_string()).unwrap_or_else(||"not assigned".into()),TEAL),
  ("Project",a.studio["project"].as_str().and_then(|p|Path::new(p).file_name()).and_then(|p|p.to_str()).unwrap_or("not assigned").into(),DESK_FG),
  ("Build",a.studio["build"].as_str().map(|v|format!("{} · {v}",a.studio["branch"].as_str().unwrap_or("branch unknown"))).unwrap_or_else(||"not reported".into()),DESK_FG),
  ("Access",match a.studio["writer_owner"].as_bool(){Some(true)=>"project writer",Some(false)=>"review attachment",None=>"ownership unknown"}.into(),MUTED),
  ("Provider",a.provider.clone(),a.identity),("Harness",harness,hc),("Session",session,sc),("MCP",mcp,mcp_colour),
  ("Model","not reported".into(),DIM),("Tokens",tokens,tc),
  ("Folder",if a.folder.is_empty(){"not reported".into()}else{a.folder.clone()},if a.folder.is_empty(){DIM}else{DESK_FG}),
  ("Activity",activity.clone().unwrap_or_else(||"waiting for output".into()),if activity.is_some(){DESK_FG}else{DIM}),
  ("Last touch",match last_touch(c,a){Some((what,at))=>(format!("{what} · {}",ago(at)),TEAL),None=>("nothing seen from this pane".into(),DIM)}.0,
   if last_touch(c,a).is_some(){TEAL}else{DIM}),
 ];

 facts
}

/// Every agent as one row, so the fleet reads at a glance; columns drop as the pane narrows.
fn mops_roster_lines(agents:&[MopsAgent],selected:usize,width:usize,rows:usize)->Vec<Line<'static>>{
 let wide=width>=118;let mid=width>=84;let tasks=width>=70;
 let name_w=13;let state_w=10;let provider_w=if mid{10}else{0};let mcp_w=if wide{28}else{0};
 let fixed=3+name_w+state_w+8+provider_w+mcp_w+(if mid{1}else{0})*3;
 let rest=width.saturating_sub(fixed);
 let folder_w=if wide{rest*2/5}else{0};let task_w=if tasks{rest.saturating_sub(folder_w)}else{0};
 let head=|text:&str,w:usize|Span::styled(cell(text,w),Style::default().fg(DIM));
 let mut lines=vec![Line::from(vec![head("",3),head("AGENT",name_w),head("STATE",state_w),head(if mid{"PROVIDER "}else{""},provider_w+usize::from(mid)),head("PORT",8),head(if wide{"MCP "}else{""},mcp_w+usize::from(wide)),head(if tasks{"TASK"}else{""},task_w),head(if wide{"FOLDER"}else{""},folder_w)])];
 let shown=rows.saturating_sub(1).min(agents.len());
 if shown==0{return lines;}
 let start=selected.saturating_sub(shown-1).min(agents.len()-shown);
 for a in &agents[start..start+shown]{
  let active=a.index==selected;let bg=if active{DESK_SELECTED}else{DESK_BG};
  let on=|fg:Color|Style::default().fg(fg).bg(bg);
  let folder=a.folder.rsplit('/').next().unwrap_or("").to_owned();
  let mut spans=vec![
   Span::styled(format!("{}{} ",if active{"›"}else{" "},agent_number(a.index)),on(a.identity).add_modifier(if active{Modifier::BOLD}else{Modifier::empty()})),
   Span::styled(cell(&format!("{} {}",a.icon,a.name),name_w),on(a.identity).add_modifier(if active{Modifier::BOLD}else{Modifier::empty()})),
   Span::styled(cell(&format!("{} {}",agent_glyph(a.word),a.word.to_lowercase()),state_w),on(a.state)),
  ];
  if mid{spans.push(Span::styled(cell(&a.provider,provider_w+1),on(MUTED)));}
  let port=a.studio["port"].as_u64().map(|v|v.to_string()).unwrap_or_else(||match a.studio["state"].as_str(){Some("closed")=>"closed",Some("ambiguous")=>"multiple",_=>"—"}.into());
  spans.push(Span::styled(cell(&port,8),on(if a.studio["state"]=="connected"{TEAL}else{AMBER})));
  if wide{let (t,col)=a.mcp.clone().unwrap_or(("—".into(),DIM));spans.push(Span::styled(cell(&t,mcp_w+1),on(col)));}
  if tasks{spans.push(Span::styled(cell(&a.task,task_w.saturating_sub(1)),on(DESK_FG)));spans.push(Span::styled(" ",on(DESK_FG)));}
  if wide{spans.push(Span::styled(cell(&folder,folder_w),on(MUTED)));}
  lines.push(Line::from(spans));
 }
 lines
}

/// MOPS: the whole fleet as a roster, and the selected agent's facts formatted beneath it.
fn render_muppets(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let block=round_panel("MOPS · 1–9 / 0 select · ← → agent · Pg scroll",TEAL,true);
 let inner=block.inner(area);f.render_widget(block,area);
 if inner.height==0{return;}
 let agents=mops_agents(c);let selected=c.gang_selected.min(agents.len()-1);
 let roster_h=((agents.len()+1) as u16).min(inner.height/2).max(2);
 let parts=Layout::vertical([Constraint::Length(roster_h),Constraint::Min(0)]).split(inner);
 let roster=mops_roster_lines(&agents,selected,parts[0].width as usize,parts[0].height as usize);
 let first=agents.len().saturating_sub(roster.len().saturating_sub(1)).min(selected.saturating_sub(roster.len().saturating_sub(2)));
 for(offset,_) in roster.iter().enumerate().skip(1){
  if let Some(a)=agents.get(first+offset-1){agent_button(c,a.index,ratatui::layout::Rect::new(parts[0].x,parts[0].y+offset as u16,parts[0].width,1),false);}
 }
 f.render_widget(Paragraph::new(roster),parts[0]);
 if parts[1].height<3{return;}
 let a=&agents[selected];
 let title=format!("{} {} · {} {}",a.icon,a.name,agent_glyph(a.word),a.word);
 let block=quiet_panel(title,true);let body=block.inner(parts[1]);f.render_widget(block,parts[1]);
 if c.state["agent_gang"].as_array().and_then(|g|g.get(selected)).is_none(){
  let mut lines=vec![styled("In-app project agent · F9 chat",MUTED)];
  for t in c.state["chat_turns"].as_array().into_iter().flatten(){lines.push(styled(format!("{}: {}",t["role"].as_str().unwrap_or("agent"),conversation_text(t["text"].as_str().unwrap_or(""))),DESK_FG));}
  if lines.len()==1{lines.push(styled("No conversation yet",MUTED));}
  f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).scroll((c.scroll,0)),body);
  return;
 }
 let facts=mops_facts(c,a);
 // One line per fact, cut to its column: a wrapped value would lose its label.
 let rows=|w:u16,from:usize,to:usize|->Vec<Line<'static>>{facts[from..to].iter().map(|(l,v,col)|fact(l,fit_text(v,(w as usize).saturating_sub(11)),*col)).collect()};
 if body.width>=100{
  let cols=Layout::horizontal([Constraint::Percentage(45),Constraint::Percentage(55)]).spacing(2).split(body);
  f.render_widget(Paragraph::new(rows(cols[0].width,0,6)),cols[0]);
  f.render_widget(Paragraph::new(rows(cols[1].width,6,facts.len())),cols[1]);
 }else{f.render_widget(Paragraph::new(rows(body.width,0,facts.len())).scroll((c.scroll,0)),body);}
}
fn render_overview(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 if area.height>=12{
  let parts=Layout::vertical([Constraint::Length(9),Constraint::Min(3)]).split(area);
  booth::render_project(f,c,parts[0]);panels::render(f,c,parts[1]);
 }else{booth::render_project(f,c,area);}
}
fn render_desk(f:&mut Frame,c:&Center,control:bool){
 let area=f.area();let narrow=area.width<110;f.render_widget(Block::default().style(Style::default().bg(DESK_BG).fg(DESK_FG)),area);
 // Narrow panes keep one status line (glyph per agent) rather than losing the band; MOPS draws its own.
 let agents=c.state["agent_gang"].as_array().map_or(0,Vec::len)+1;
 let tall_narrow=narrow&&!c.muppets_view&&area.height>=34;
 let gang_rows=if c.muppets_view{0}else if tall_narrow{((agents+2) as u16).min(area.height/3).max(4)}else if narrow{1}else{3};
 let chunks=Layout::vertical([Constraint::Length(1),Constraint::Length(1),Constraint::Min(1),Constraint::Length(gang_rows),Constraint::Length(1)]).split(area);
  f.render_widget(Paragraph::new(top_bar(c,area.width as usize,control)),chunks[0]);
  render_tabs(f,c,chunks[1]);
  if let Some(poke)=&c.poke{
   let lines=vec![styled(format!("{} {}",moppet_icon(&poke.name),poke.name),moppet_identity(&poke.name)),styled(c.message.clone(),AMBER),styled(format!("{}▏",poke.text),DESK_FG)];
   f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).block(round_panel("FIX NOTE · Enter send · Esc cancel",moppet_identity(&poke.name),true)),chunks[2]);
  }
  else if c.onboarding.is_some(){roles::render_onboarding(f,c,chunks[2]);}
  else if c.chat_editing{render_conversation(f,c,chunks[2]);}
  else if c.event_open{render_event(f,c,chunks[2]);}
  else if c.config_view{render_config(f,c,chunks[2]);}else if c.telemetry_view{if c.scope.is_some(){roles::render_animal(f,c,chunks[2]);}else{render_telemetry(f,c,chunks[2]);}}
  else if control&&!c.muppets_view{workshop::render(f,c,chunks[2]);}
  else if c.details_open{render_pulse(f,c,chunks[2]);}
  else if c.muppets_view{render_muppets(f,c,chunks[2]);}
  else{
 let compact_actions=(root_actions(c).len() as u16*2+2).clamp(4,8);let bottom_height=if narrow{compact_actions+12}else{15};let body=Layout::vertical([Constraint::Min(8),Constraint::Length(bottom_height)]).split(chunks[2]);
 if narrow{let stacked=area.width>=68;let p=Layout::vertical([Constraint::Length(if stacked{6}else{9}),Constraint::Length(if c.details_open{9}else if body[0].height>=14{4}else{3}),Constraint::Min(3)]).split(body[0]);f.render_widget(Paragraph::new(stage_lines(c,stacked)).block(round_panel("STAGES · [ / ] select",TEAL,true)),p[0]);render_stage_activity(f,c,p[1]);if c.details_open{render_pulse(f,c,p[2]);}else{render_overview(f,c,p[2]);}}
 else{let p=Layout::horizontal([Constraint::Length(31),Constraint::Min(30)]).spacing(1).split(body[0]);let stages=stage_lines(c,false);let mut rail=p[0];rail.height=wrapped_rows(&stages,rail.width.saturating_sub(2)).saturating_add(2).min(rail.height);f.render_widget(Paragraph::new(stages).wrap(Wrap{trim:false}).block(round_panel("STAGES · [ / ] select",TEAL,true)),rail);let right=Layout::vertical([Constraint::Length(if c.details_open{11}else{7}),Constraint::Min(5)]).split(p[1]);render_stage_activity(f,c,right[0]);render_overview(f,c,right[1]);}
 if narrow{let p=Layout::vertical([Constraint::Length(compact_actions),Constraint::Min(10)]).split(body[1]);let top=Layout::horizontal([Constraint::Percentage(50),Constraint::Percentage(50)]).split(p[0]);render_actions(f,c,top[0],true);render_tail(f,c,top[1]);render_next(f,c,p[1]);}else{let p=Layout::horizontal([Constraint::Percentage(48),Constraint::Percentage(52)]).spacing(1).split(body[1]);let left=Layout::vertical([Constraint::Length(compact_actions),Constraint::Min(3)]).split(p[0]);render_actions(f,c,left[0],true);render_tail(f,c,left[1]);render_next(f,c,p[1]);}
 }
 if tall_narrow{render_agent_list(f,c,chunks[3]);}else if gang_rows>=3{render_gang(f,c,chunks[3]);}else if gang_rows==1{f.render_widget(Paragraph::new(agent_strip_at(c,chunks[3].width as usize,Some(chunks[3]))),chunks[3]);}
 let booth_open=c.state["ui"]["connection"]=="connected";
 let width=area.width as usize;
 let line=if let Some(poke)=&c.poke{
  styled(format!(" POKE {} › {}█ · Enter send · Esc cancel ",poke.name,poke.text),TEAL)
 }else if c.chat_editing{
  styled(" SPROKET · Enter Send · F9/Esc Close · Pg History ",MUTED)
 }else if c.message.starts_with("Restricted:")||c.message.starts_with("COPIED")||c.message.starts_with("Chat blocked:")||c.message.contains("error"){
  styled(format!(" {} ",c.message),AMBER)
 }else{
  let mut spans=vec![Span::styled(format!(" ◉ BACKSTAGE {} ",if booth_open{"●"}else{"○"}),if booth_open{GREEN}else{AMBER})];
  if narrow&&!c.muppets_view&&!control{
   let room=width.saturating_sub(8+Line::from(spans.clone()).width());
   spans.extend(agent_strip(c,room).spans);
  }else{
   let hints=if control&&!c.muppets_view&&!c.telemetry_view{"Arrows Select · Enter Booth · I Inactive · B Agent Booth · A Check in all · D Details · C Copy · G MOPS · ^Q Exit"}else if c.muppets_view{"1–9/0 Select · ← → Agent · Pg Scroll · F9 Sproket · ^Q Exit"}else{"G MOPS · E Events · [ ] Stage · , . Agent · A Check in all · F9 Sproket · D Details · C Copy · ^Q Exit"};
   for hint in hints.split(" · "){
    let span=Span::styled(format!(" · {hint}"),MUTED);
    if Line::from(spans.clone()).width()+Line::from(span.clone()).width()+8<=width{spans.push(span);}
   }
  }
  Line::from(spans)
 };
 f.render_widget(Paragraph::new(film_status(width,line)),chunks[4]);
}

/// The tab strip: four views, the active one filled, like a browser's tabs.
fn tab_strip(c:&Center,width:usize)->Line<'static>{
 let no_project=!c.state["project"].is_string();
 let tabs=vec![
  ("WORKSHOP","F1",c.control_view||(no_project&&!c.muppets_view&&!c.telemetry_view)),
  ("BOOTH","F2",!c.control_view&&!no_project&&!c.muppets_view&&!c.telemetry_view),
  ("MOPS","F3",c.muppets_view),
  ("MAYHEM","F4",c.telemetry_view),
 ];
 let mut spans:Vec<Span<'static>>=Vec::new();
 let tabs:Vec<_>=tabs.into_iter().filter(|(label,_,_)|c.scope.is_none()||*label!="WORKSHOP").collect();
 let required:usize=tabs.iter().map(|(label,key,_)|label.len()+key.len()+3).sum::<usize>()+9;
 for (index,(label,key,active)) in tabs.iter().enumerate(){
  let label=if required<=width{*label}else if width>=50{match *label{"WORKSHOP"=>"Work","BOOTH"=>"Booth","MOPS"=>"MOPS","MAYHEM"=>"Mayhem","CONFIG"=>"Config",_=>"?"}}else{""};
  if index>0{spans.push(Span::styled(" │ ",Style::default().fg(DIM)));}
  if *active{
   spans.push(Span::styled(format!(" {key} {label} "),Style::default().fg(DESK_BG).bg(TEAL).add_modifier(Modifier::BOLD)));
  }else{
   spans.push(Span::styled(format!(" {key} {label} "),Style::default().fg(MUTED)));
  }
 }
 // Right side: what the desk can see right now, so the strip is also a status line.
 let owner=&c.state["animation_worker"]["worker_owner"];
 let slot=if owner.is_object(){format!("slot {}",owner["job"].as_str().unwrap_or("?").rsplit('-').next().unwrap_or("?"))}else{format!("slot {}",c.state["animation_worker"]["worker_owner_status"].as_str().unwrap_or("idle"))};
 let projects=c.state["catalog"]["projects"].as_array().map(|p|p.len()).unwrap_or(0);
 let right=if c.scope.is_some(){"  Project Booth · locked ".into()}else if c.view_name()=="workshop"{"  Arrows select · Enter Booth ".into()}else{format!("  {projects} projects · {slot} ")};
 let used:usize=spans.iter().map(|s|s.content.chars().count()).sum::<usize>();
 if used+right.chars().count()<width{spans.push(Span::styled(right,Style::default().fg(MUTED)));}
 bounded_line(Line::from(spans),width)
}

/// A compact history line: one glyph per sample, scaled to the window's peak.
fn sparkline(values:&[f64],width:usize)->String{
 if values.is_empty(){return "·".repeat(width);}
 let slice=&values[values.len().saturating_sub(width)..];
 let peak=slice.iter().cloned().fold(f64::MIN,f64::max).max(1.0);
 slice.iter().map(|v|{
  let level=((v/peak)*7.0).round().clamp(0.0,7.0) as usize;
  ["▁","▂","▃","▄","▅","▆","▇","█"][level]
 }).collect()
}
/// One core's load as a single glyph.
fn spark_glyph(value:Option<f64>)->char{
 let level=value.map(|v|((v/100.0)*7.0).round().clamp(0.0,7.0) as usize).unwrap_or(0);
 ["▁","▂","▃","▄","▅","▆","▇","█"][level].chars().next().unwrap_or('·')
}
/// The telemetry tab: host dials, the busiest processes, live log tails, commits.
/// CONFIG: TUI and app settings that can be modified without code changes.
fn render_config(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let mut lines:Vec<Line<'static>>=Vec::new();
 lines.push(styled("TUI & App Settings",TEAL));
 lines.push(Line::raw(""));
 lines.push(styled("Press Enter on a field to edit, Esc to cancel",MUTED));
 lines.push(Line::raw(""));
 lines.push(styled(format!("Process count in MAYHEM: {}", c.settings.process_count),DESK_FG));
 lines.push(styled(format!("Agent refresh interval: {}s", c.settings.agent_refresh_secs),DESK_FG));
 lines.push(styled(format!("Studio discovery interval: {}s", c.settings.studio_discovery_secs),DESK_FG));
 lines.push(styled(format!("MCP scan interval: {}s", c.settings.mcp_scan_secs),DESK_FG));
 lines.push(styled(format!("Log tail lines: {}", c.settings.log_tail_lines),DESK_FG));
 lines.push(styled(format!("RSS threshold (KB): {}", c.settings.proc_rss_threshold_kb),DESK_FG));
 lines.push(Line::raw(""));
 lines.push(styled("Skills registered:",TEAL));
 for s in &c.skills{
  let id=s.get("id").and_then(Value::as_str).unwrap_or("skill");
  let status=s.get("status").and_then(Value::as_str).unwrap_or("researching");
  let colour=match status{"implemented"=>GREEN,"not_implemented"=>RED,_=>AMBER};
  lines.push(styled(format!("  {} · {}",id,status),colour));
 }
 lines.push(Line::raw(""));
 lines.push(styled("To change values, edit ~/.config/magicstudio/tui-settings.json",MUTED));
 lines.push(styled("and restart the TUI. Values: process_count, agent_refresh_secs,",MUTED));
 lines.push(styled("studio_discovery_secs, mcp_scan_secs",MUTED));
 f.render_widget(Paragraph::new(lines).block(round_panel("CONFIG · F5 toggle · Pg Scroll",TEAL,false)),area);
}
fn render_telemetry(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 if area.width<118{
  let p=Layout::vertical([Constraint::Length(13),Constraint::Length(12),Constraint::Length(12),Constraint::Length(14),Constraint::Length(14),Constraint::Length(16),Constraint::Min(9),Constraint::Length(10)]).split(area);
  render_host(f,c,p[0]);render_known_apps(f,c,p[1]);render_processes(f,c,p[2]);render_studios(f,c,p[3]);render_mcp_servers(f,c,p[4]);render_trend(f,c,p[5]);render_github_health(f,c,p[6]);render_logs(f,c,p[7]);
  return;
 }
 let p=Layout::vertical([Constraint::Min(12),Constraint::Length(16)]).split(area);
 let columns=Layout::horizontal([Constraint::Length(42),Constraint::Length(48),Constraint::Min(30)]).split(p[0]);
 render_host(f,c,columns[0]);render_known_apps(f,c,columns[1]);render_logs(f,c,columns[2]);
 let bottom=Layout::horizontal([Constraint::Percentage(17),Constraint::Percentage(17),Constraint::Percentage(17),Constraint::Percentage(17),Constraint::Percentage(16),Constraint::Percentage(16)]).split(p[1]);
 render_studios(f,c,bottom[0]);render_mcp_servers(f,c,bottom[1]);render_processes(f,c,bottom[2]);render_trend(f,c,bottom[3]);render_github_health(f,c,bottom[4]);render_agent_usage(f,c,bottom[5]);
}
/// GITHUB HEALTH: what an agent did to the repository's GitHub state. Every line is a
/// measured fact from `git` and `gh`; the worst problems come first.
fn render_github_health(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let mut lines:Vec<Line<'static>>=Vec::new();
 let health=&c.github_health;
 if c.scope.is_some(){
  lines.push(styled("GitHub status is unavailable in assigned project view.",MUTED));
 } else if health["refreshing"]==true {
  lines.push(styled("Refreshing GitHub status…",MUTED));
 } else if health["observed_at_ms"].is_null(){
  lines.push(styled("Press F8 to refresh GitHub status.",MUTED));
 } else {
  let row=|label:&str,value:String,tone:Color|Line::from(vec![Span::styled(format!("{label:<12}"),Style::default().fg(MUTED)),Span::styled(value,Style::default().fg(tone))]);
  let value=|field:&str|health[field].as_str().map(str::to_owned).unwrap_or_else(||"unknown".into());
  let count=|field:&str|health[field].as_u64().map(|n|n.to_string()).unwrap_or_else(||"unknown".into());
  lines.push(row("Repository",value("repository"),DESK_FG));
  lines.push(row("Branch",format!("{} · {}",value("branch"),value("local_sha")),DESK_FG));
  lines.push(row("Main",value("main_sha"),DESK_FG));
  lines.push(row("origin/main",value("remote_sha"),DESK_FG));
  let diverged=health["ahead"].as_u64().is_some_and(|n|n>0)||health["behind"].as_u64().is_some_and(|n|n>0);
  lines.push(row("Upstream",format!("{} · +{} / −{}",value("upstream"),count("ahead"),count("behind")),if diverged{AMBER}else{DESK_FG}));
  let dirty=health["dirty_files"].as_u64();
  lines.push(row("Working tree",format!("{} changed file(s)",dirty.map(|n|n.to_string()).unwrap_or_else(||"unknown".into())),if dirty.is_some_and(|n|n>0){AMBER}else{DESK_FG}));
  match health["pull_requests"].as_array(){
   Some(rows)=>{
    lines.push(row("Pull requests",format!("{} open",rows.len()),DESK_FG));
    for pr in rows.iter().take(2){
     lines.push(row(&format!("#{}",pr["number"].as_u64().unwrap_or(0)),format!("{} · checks {}",pr["title"].as_str().unwrap_or("title unknown"),pr["checks"]["state"].as_str().unwrap_or("unknown")),if pr["checks"]["state"]=="failed"{RED}else{DESK_FG}));
    }
   }
   _=>lines.push(row("Pull requests","unavailable".into(),AMBER)),
  }
  let age=health["observed_at_ms"].as_u64().map(|stamp|crate::worker::now_ms().saturating_sub(stamp));
  lines.push(row("Observed",age.map(|ms|format!("{:.1}s ago",ms as f64/1000.0)).unwrap_or_else(||"unknown".into()),MUTED));
  for error in health["errors"].as_array().into_iter().flatten().take(2){lines.push(styled(format!("Error: {}",error.as_str().unwrap_or("unknown")),RED));}
 }
 f.render_widget(Paragraph::new(lines).block(round_panel("GITHUB · F8 refresh",TEAL,false)),area);
}
/// The measured trend since the first recorded sample: are we closing processes and
/// using less memory, or not? Every number comes from a recorded sample.
fn render_trend(f:&mut Frame,_c:&Center,area:ratatui::layout::Rect){
 let mut lines:Vec<Line<'static>>=Vec::new();
 let history=crate::mayhem::history();
 let now=history.last().cloned();
 let Some(now)=now else {
  lines.push(styled("No sample recorded yet. Open MAYHEM to start measuring.",MUTED));
  f.render_widget(Paragraph::new(lines).block(round_panel("TREND · are we improving?",TEAL,false)),area);
  return;
 };
 lines.push(styled(format!("{} servers · {} agents · {} MiB MCP RAM",now.mcp_servers,now.mcp_agents,now.mcp_rss_kb/1024),DESK_FG));
 lines.push(styled(format!("{} collisions · {} studios · {} stale",now.mcp_collisions,now.studios,now.stale_studios),DESK_FG));
 lines.push(styled(format!("Host RAM {} MiB / {} MiB",now.host_mem_used_kb/1024,now.host_mem_total_kb/1024),MUTED));
 lines.push(Line::raw(""));
 match crate::mayhem::trend(){
  Some(trend)=>{
   let span=trend.span_ms/1000;
   let row=|label:&str,delta:i64,unit:&str|{
    let better=crate::mayhem::improved(delta);
    let colour=if delta==0{MUTED}else if better{GREEN}else{RED};
    let arrow=if better{"▼"}else if delta<0{"▲"}else{"▲"};
    Line::from(vec![
     Span::styled(format!("{label:<14}"),Style::default().fg(MUTED)),
     Span::styled(format!("{arrow} {:>12}",crate::mayhem::delta_text(delta,unit)),Style::default().fg(colour)),
    ])
   };
   lines.push(styled(format!("Over {} samples / {}s:",trend.samples,span),MUTED));
   lines.push(row("MCP servers",trend.servers_delta,"servers"));
   lines.push(row("MCP agents",trend.agents_delta,"agents"));
   lines.push(row("MCP memory",trend.rss_delta_kb,"MiB"));
   lines.push(row("Collisions",trend.collisions_delta,"collisions"));
   lines.push(row("Stale studios",trend.stale_delta,"stale"));
   lines.push(row("Host RAM",trend.host_mem_delta_kb,"MiB"));
   let better=trend.servers_delta<0||trend.rss_delta_kb<0;
   lines.push(Line::raw(""));
   lines.push(styled(if better{"IMPROVING: fewer processes / less memory"}else{"NOT YET: the footprint has not shrunk"},if better{GREEN}else{AMBER}));
  }
  None=>lines.push(styled("One sample so far; a trend needs two. Open MAYHEM again in a minute.",MUTED)),
 }
 f.render_widget(Paragraph::new(lines).block(round_panel("TREND · are we improving?",TEAL,false)),area);
}
fn render_host(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let mut lines:Vec<Line<'static>>=Vec::new();
 let Some(host)=&c.dials else {
  lines.push(styled("Reading /proc…",MUTED));
  f.render_widget(Paragraph::new(lines).block(round_panel("HOST",TEAL,false)),area);return;
 };
 let gauge=|label:&str,percent:Option<u64>,value:String|{
  let (bar,colour)=meter(percent,Some(100),12);
  Line::from(vec![Span::styled(format!("{label:<7}"),Style::default().fg(MUTED)),Span::styled(bar,Style::default().fg(colour)),Span::styled(format!("  {value}"),Style::default().fg(DESK_FG))])
 };
 let cpu=host.cpu_pct;
 lines.push(gauge("CPU",cpu.map(|v|v.round() as u64),format!("{:>4}%",cpu.map(|v|format!("{v:.0}")).unwrap_or("?".into()))));
 lines.push(styled(format!("       {}",sparkline(&c.history,12)),TEAL));
 if host.cores.is_empty(){
  lines.push(styled(format!("{:<7}{} cores · rate in one second","CORES",host.core_count),MUTED));
 }else{
  let glyphs:String=host.cores.iter().take(24).map(|core|spark_glyph(*core)).collect();
  lines.push(styled(format!("{:<7}{}","CORES",glyphs),MUTED));
 }
 let mem_pct=if host.mem_total_kb>0{Some(host.mem_used_kb*100/host.mem_total_kb)}else{None};
 lines.push(gauge("RAM",mem_pct,format!("{:.1}G / {:.1}G",host.mem_used_kb as f64/1048576.0,host.mem_total_kb as f64/1048576.0)));
 let swap_pct=if host.swap_total_kb>0{Some(host.swap_used_kb*100/host.swap_total_kb)}else{None};
 lines.push(gauge("SWAP",swap_pct,format!("{:.1}G / {:.1}G",host.swap_used_kb as f64/1048576.0,host.swap_total_kb as f64/1048576.0)));
 lines.push(styled(format!("{:<7}{}","LOAD",host.load),MUTED));
 lines.push(styled(format!("{:<7}sampled {:.1}s ago","AGE",host.at.elapsed().as_secs_f32()),MUTED));
 f.render_widget(Paragraph::new(lines).block(round_panel("DRUM KIT · HOST",TEAL,false)),area);
}
fn render_processes(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let inner=area.width.saturating_sub(2) as usize;
 let name_width=inner.saturating_sub(31).clamp(8,24);
 let mut lines=vec![styled(format!("{:<8}{:<w$}{:<14}{:>9}","PID","NAME","CPU","RSS",w=name_width),MUTED)];
 for row in &c.procs{
  let cpu=row.cpu_pct.map(|v|v.round() as u64);
  let (bar,colour)=meter(cpu,Some(100),8);
  lines.push(Line::from(vec![
   Span::styled(format!("{:<8}",row.pid),Style::default().fg(MUTED)),
   Span::styled(format!("{:<w$}",fit_text(&row.name,name_width.saturating_sub(1)),w=name_width),Style::default().fg(DESK_FG)),
   Span::styled(bar,Style::default().fg(colour)),
   Span::styled(format!("{:>6}",row.cpu_pct.map(|v|format!("{v:.0}%")).unwrap_or("?".into())),Style::default().fg(DESK_FG)),
   Span::styled(format!("{:>9}",format!("{:.2}G",row.rss_kb as f64/1048576.0)),Style::default().fg(MUTED)),
  ]));
 }
 if c.procs.is_empty(){lines.push(styled("No process sample yet.",MUTED));}
 f.render_widget(Paragraph::new(lines).block(round_panel("PROCESSES · busiest first",TEAL,false)),area);
}
fn render_logs(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let inner=area.width.saturating_sub(2) as usize;
 let mut lines:Vec<Line<'static>>=Vec::new();
 let mut section=|title:&str,values:&Value|{
  lines.push(styled(title.to_string(),TEAL));
  for line in values.as_array().into_iter().flatten().take(7){lines.push(styled(fit_text(line.as_str().unwrap_or(""),inner),MUTED));}
  lines.push(Line::raw(""));
 };
 section("BLENDER · worker.log",&c.logs["blender"]);
 section("MAGIC STUDIO · activity",&c.logs["studio"]);
 section("OMARCHY · journal --user",&c.logs["omarchy"]);
 section("COMMITS · this repository",&c.logs["commits"]);
 f.render_widget(Paragraph::new(lines).block(round_panel("LOGS · live tails",TEAL,false)),area);
}
/// MCP servers as panel rows that fit `rows`: warnings first (they are why you look), then
/// families by server count with who started each, then "+N more" for whatever does not fit.
fn mcp_lines(servers:&[crate::mcp_watch::Server],rows:usize)->Vec<Line<'static>>{
 let (by,findings)=crate::mcp_watch::analyze(servers);
 if by.is_empty(){return vec![styled("No MCP servers running",MUTED)];}
 let agents:std::collections::BTreeSet<u32>=servers.iter().map(|s|s.host_pid).collect();
 let mut lines=vec![styled(format!("{} servers · {} agents · {}",servers.len(),agents.len(),crate::telemetry::mib(servers.iter().map(|s|s.rss_kb).sum())),DESK_FG)];
 for finding in &findings{lines.push(styled(format!("! {}: {}",finding.family,finding.text),AMBER));}
 let mut families:Vec<_>=by.iter().collect();
 families.sort_by(|a,b|b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
 let room=rows.saturating_sub(lines.len()+1).max(1);
 let hidden=families.len().saturating_sub(room);
 for(family,group) in families.into_iter().take(room){
  let mut hosts:std::collections::BTreeMap<&str,usize>=std::collections::BTreeMap::new();
  for s in group{*hosts.entry(s.host.as_str()).or_default()+=1;}
  let who=hosts.iter().map(|(h,n)|if *n>1{format!("{h}×{n}")}else{(*h).to_owned()}).collect::<Vec<_>>().join(" ");
  let warned=findings.iter().any(|x|&x.family==family);
  lines.push(Line::from(vec![
   Span::styled(format!("{:<11}",fit_text(family,10)),Style::default().fg(if warned{AMBER}else{DESK_FG})),
   Span::styled(format!("×{:<3}",group.len()),Style::default().fg(MUTED)),
   Span::styled(who,Style::default().fg(MUTED)),
  ]));
 }
 if hidden>0{lines.push(styled(format!("+{hidden} more"),MUTED));}
 lines
}
/// Magic Studio apps open: every Studio window's project and endpoint, so you can see
/// what is running and disconnect stale sessions.
fn render_studios(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let mut lines:Vec<Line<'static>>=Vec::new();
 let sessions=c.studios["sessions"].as_array().cloned().unwrap_or_default();
 let errors=c.studios["errors"].as_array().cloned().unwrap_or_default();
 if sessions.is_empty() && errors.is_empty(){
  lines.push(styled("No Magic Studio apps open",MUTED));
 } else {
  for s in sessions.iter().take(8){
   let project=s["active_project"].as_str().or(s["canonical_project"].as_str()).map(|p|p.rsplit('/').next().unwrap_or(p)).unwrap_or("?");
   let endpoint=s["endpoint"].as_str().unwrap_or("?");
   let pid=s["pid"].as_u64().map(|p|p.to_string()).unwrap_or_default();
   lines.push(Line::from(vec![
    Span::styled(format!("{:<20}",fit_text(project,18)),Style::default().fg(DESK_FG)),
    Span::styled(format!(" pid {pid}"),Style::default().fg(MUTED)),
   ]));
   lines.push(styled(format!("  {}",fit_text(endpoint,area.width.saturating_sub(4) as usize)),MUTED));
  }
  for e in errors.iter().take(4){
   let endpoint=e["endpoint"].as_str().unwrap_or("?");
   let state=e["state"].as_str().unwrap_or("error");
   lines.push(styled(format!("! {} ({})",fit_text(endpoint,area.width.saturating_sub(12) as usize),state),AMBER));
  }
  if sessions.len()>8{lines.push(styled(format!("+{} more",sessions.len()-8),MUTED));}
 }
 f.render_widget(Paragraph::new(lines).block(round_panel("MAGIC STUDIO APPS · open windows",TEAL,false)),area);
}
/// MCP servers by family: how many, from which agents, and what collides. Sampled with the tab.
fn render_mcp_servers(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let lines=mcp_lines(&c.mcp_servers,area.height.saturating_sub(2) as usize);
 f.render_widget(Paragraph::new(lines).block(round_panel("MCP SERVERS · who started them · what collides",TEAL,false)),area);
}
fn render_agent_usage(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let mut lines:Vec<Line<'static>>=Vec::new();
 for usage in &c.usage{
  let name=usage["name"].as_str().unwrap_or("agent");
  let tier=usage["tierLabel"].as_str().unwrap_or("");
  let mut parts=vec![Span::styled(format!("{name}"),Style::default().fg(DESK_FG)),Span::styled(format!(" {tier}"),Style::default().fg(MUTED))];
  for limit in usage["limits"].as_array().into_iter().flatten(){
   let label=limit["label"].as_str().unwrap_or("limit");
   let percent=(limit["percent"].as_f64().unwrap_or(0.0)*100.0).round() as u64;
   let (bar,colour)=meter(Some(percent),Some(100),12);
   parts.push(Span::styled(format!("  {label} "),Style::default().fg(MUTED)));
   parts.push(Span::styled(bar,Style::default().fg(colour)));
   parts.push(Span::styled(format!(" {percent:>3}%"),Style::default().fg(DESK_FG)));
  }
  let tokens=usage["todayTotalTokens"].as_u64().unwrap_or(0);
  parts.push(Span::styled(format!("  ·  {:.0}M tokens today · {} sessions",tokens as f64/1_000_000.0,usage["todaySessions"].as_u64().unwrap_or(0)),Style::default().fg(MUTED)));
  lines.push(Line::from(parts));
  let days:Vec<f64>=usage["recentDays"].as_array().into_iter().flatten().map(|d|d["messageCount"].as_f64().unwrap_or(0.0)).collect();
  if !days.is_empty(){lines.push(styled(format!("       {}",sparkline(&days,24)),TEAL));}
 }
 if lines.is_empty(){lines.push(styled("No agent usage records on this machine.",MUTED));}
 f.render_widget(Paragraph::new(lines).block(round_panel("AGENT USAGE · omarchy records",TEAL,false)),area);
}


fn render(f: &mut Frame,c: &Center) {
    c.hits.borrow_mut().clear();
    if c.show_form { render_form(f,c); return; }
    render_desk(f,c,c.scope.is_none()&&(c.control_view || !c.state["project"].is_string()));
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
    if let Some(delay)=std::env::var("MAGICSTUDIO_RESTART_DELAY_MS").ok().and_then(|v|v.parse::<u64>().ok()){
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }
    match std::env::var("MAGICSTUDIO_VIEW").as_deref(){
        Ok("animal"|"telemetry")=>c.telemetry_view=true,
        Ok("mops"|"muppets"|"moppets")=>c.muppets_view=true,
        Ok("workshop")=>c.control_view=true,
        _=>{}
    }
    let server=crate::tui::Server::bind()?;
    let mut reload=false;
    let mut frame_number=0u64;
    let mut terminal = ratatui::init();
    let _=crossterm::execute!(io::stdout(),crossterm::event::EnableMouseCapture);
    let result = (|| -> io::Result<()> {
        let mut refreshed=std::time::Instant::now();
        let mut worker=crate::worker::Poller::default();
        loop {
            c.poll_studios();c.poll_chat();c.poll_doctor();
            if refreshed.elapsed()>Duration::from_secs(1){c.refresh();refreshed=std::time::Instant::now();}
            if let Some(observation)=worker.poll(c.state["project"].as_str()){c.state["animation_worker"]=if let Some(scope)=&c.scope{scope.redact(observation)}else{observation};}
            let mut replies=Vec::new();let mut flow=control::Flow::Continue;
            for pending in server.receive(){
                let action=pending.request["action"].as_str().unwrap_or("look");
                let result=control::action(&mut c,action,&pending.request["params"]);
                if let Ok(next)=result.as_ref(){if *next!=control::Flow::Continue{flow=*next;}}
                replies.push((pending,result));
            }
            frame_number+=1;
            let frame=terminal.draw(|f|render(f,&c))?;
            for(pending,result)in replies{
                let mut look=control::look(&c,frame.buffer,frame_number,&server.endpoint());
                look["generation"]=json!(server.generation);
                look["action"]=pending.request["action"].clone();
                match result{Ok(next)=>{look["ok"]=json!(true);if next==control::Flow::Reload{look["reload_requested"]=json!(true);}},Err(error)=>{look["ok"]=json!(false);look["error"]=json!(error);}}
                pending.reply(&look);
            }
            if flow==control::Flow::Quit{break;}
            if flow==control::Flow::Reload{reload=true;break;}
            if !event::poll(Duration::from_millis(120))? {
                continue;
            }
            let key=match event::read()?{
                Event::Key(key) if key.kind==event::KeyEventKind::Press=>key,
                Event::Mouse(mouse) if mouse.kind==event::MouseEventKind::Down(event::MouseButton::Left)=>{
                    match control::mouse(&mut c,mouse.column,mouse.row){
                        Ok(control::Flow::Quit)=>break,
                        Ok(control::Flow::Reload)=>{reload=true;break;},
                        Ok(control::Flow::Continue)=>{},
                        Err(error)=>c.message=error,
                    }
                    continue;
                },
                _=>continue,
            };
            match control::key(&mut c,key){
                control::Flow::Continue=>{},
                control::Flow::Quit=>break,
                control::Flow::Reload=>{reload=true;break;},
            }
        }
        Ok(())
    })();
    let _=crossterm::execute!(io::stdout(),crossterm::event::DisableMouseCapture);
    ratatui::restore();
    drop(server);
    result.map_err(|e|e.to_string())?;
    if reload{control::reload(&c)?;}
    Ok(())
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
    #[test]
    fn check_in_never_interrupts_busy_agents_and_names_them() {
        let mut c=fixture();
        // Only busy agents here: a reachable one would receive a real prompt through Herdr.
        c.agent_gang=json!([{"name":"beaker","pane_id":"wG:pB","agent_status":"working"},{"name":"waldorf","pane_id":"wG:pN","agent_status":"running"}]);
        c.scope=None;
        c.check_in_all();
        assert!(c.message.contains("busy, not interrupted: beaker, waldorf"),"{}",c.message);
        assert!(c.message.starts_with("Check-in sent to nobody"),"{}",c.message);
        assert!(!c.message.contains("sent to beaker"),"a working agent is never sent the check-in");
    }
    pub(super) fn fixture()->Center{
        Center{session:Session::default(),scope:None,onboarding:None,fields:std::array::from_fn(|_|String::new()),selected:0,editing:false,state:json!({"project":"/fixture.magic","ui":{"connection":"connected","active_project":"/fixture.magic","engine_unsaved":false,"draft_fields":[],"ui_ready":true,"engine_revision":2,"ack_engine_revision":2,"frontend_report_age_ms":10},"activity":{"next_cursor":42,"actions":[{"root_request_id":"save","first_sequence":40,"last_sequence":42,"events":[{"stage":"dispatch_running","request_id":"save","command":"storyboard.save_version"},{"stage":"engine_success","request_id":"save","command":"storyboard.save_version","engine_revision":2},{"stage":"frontend_ack","request_id":"save","engine_revision":2}]}]}}),message:String::new(),scroll:0,show_form:false,control_view:false,command_selected:0,seen_cursor:0,flash_after:0,flash_started:None,pinned_instance:None,stage_selected:0,details_open:false,chat_draft:String::new(),chat_draft_project:None,chat_editing:false,chat_sent:String::new(),chat_sent_request:None,chat_receiver:None,doctor_receiver:None,doctor_error:None,agent_gang:json!([]),agent_gang_at:None,studios:json!({"sessions":[],"errors":[]}),studios_at:None,studios_receiver:None,telemetry_view:false,config_view:false,dials:None,dials_at:None,metrics_at:None,proc_snapshot:Vec::new(),mcp_servers:Vec::new(),mcp_sessions:Vec::new(),touches:Vec::new(),skills:Vec::new(),settings:crate::settings::TuiSettings::default(),procs:Vec::new(),history:Vec::new(),logs:json!({"blender":[],"studio":[],"omarchy":[],"commits":[]}),logs_at:None,usage:Vec::new(),gang_selected:0,event_index:0,event_focus:false,event_open:false,event_detail:None,hits:Default::default(),poke:None,muppets_view:false,catalog_selected:0,catalog_show_inactive:None,catalog_columns:std::cell::Cell::new(1),pulse:None,pulse_at:None,spy:None,spy_at:None,panel_config:magicstudio_engine::panels::Config::default(),panel_config_error:None,panel_report:magicstudio_engine::panels::PanelReport::default(),last_panels_revision:None,panel_rows:Vec::new(),panel_detail:Vec::new(),github_health:crate::github_health::empty(),github_health_receiver:None}
    }
    fn panel_text(lines:&[Line])->String{lines.iter().map(|line|line.to_string()).collect::<Vec<_>>().join("\n")}
    fn panel_screen(c:&Center)->String{
        let mut terminal=ratatui::Terminal::new(ratatui::backend::TestBackend::new(72,16)).unwrap();
        terminal.draw(|f|panels::render(f,c,f.area())).unwrap();
        let buffer=terminal.backend().buffer();
        (0..16).map(|y|(0..72).map(|x|buffer[(x,y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
    }
    #[test]
    fn panels_unavailable_renders_unavailable(){
        let mut c=fixture();
        assert!(c.state["ui"].get("panels").is_none());
        c.panel_config.elements.insert("view-home".into(),magicstudio_engine::panels::Entry{component:"workspace".into(),label:Some("Home".into()),..Default::default()});
        panels::sync(&mut c);
        let text=panel_text(&c.panel_rows);
        assert!(text.contains("Panels unavailable"),"{text}");
        assert!(text.contains("Home"),"{text}");
        assert!(text.contains('·'),"{text}");
        assert!(!text.contains("open")&&!text.contains("dirty"),"{text}");
        let screen=panel_screen(&c);
        assert!(screen.contains("Panels unavailable"),"{screen}");
        assert!(screen.contains("Home"),"{screen}");
    }
    #[test]
    fn same_revision_does_not_rebuild(){
        let mut c=fixture();
        c.panel_config.elements.insert("block-song".into(),magicstudio_engine::panels::Entry{component:"block".into(),label:Some("Song".into()),..Default::default()});
        c.state["ui"]["panels"]=json!({"revision":4,"available":true,"items":[],"unconfigured":[]});
        panels::sync(&mut c);
        c.panel_rows=vec![styled("SENTINEL",RED)];
        panels::sync(&mut c);
        assert_eq!(c.panel_rows[0].to_string(),"SENTINEL");
        c.state["ui"]["panels"]["revision"]=json!(5);
        c.state["ui"]["panels"]["items"]=json!([{"id":"block-song","label":"Song","signal":1,"busy":true}]);
        panels::sync(&mut c);
        let text=panel_text(&c.panel_rows);
        assert!(!text.contains("SENTINEL"),"{text}");
        assert!(text.contains("Song"),"{text}");
        assert!(text.contains("busy"),"{text}");
        assert!(text.contains("every element tracked and styled"),"{text}");
        assert!(!text.contains("open")&&!text.contains("dirty"),"{text}");
        let song=c.panel_rows.iter().find(|line|line.to_string().contains("Song")).unwrap();
        assert!(song.to_string().starts_with('●'));
        assert_eq!(song.spans[0].style.fg,Some(GREEN));
    }
    #[test]
    fn unconfigured_count_shown(){
        let mut c=fixture();
        c.state["ui"]["panels"]=json!({"revision":1,"available":true,"items":[],"unconfigured":[
            {"id":"button-17","tag":"button","reason":"untracked"},
            {"id":"button-18","tag":"button","reason":"untracked"},
            {"id":"button-19","tag":"button","reason":"untracked"},
            {"id":"place-a","tag":"div","reason":"unstyled"},
            {"id":"place-b","tag":"div","reason":"unstyled"}
        ]});
        panels::sync(&mut c);
        let text=panel_text(&c.panel_rows);
        assert!(text.contains("untracked 3"),"{text}");
        assert!(text.contains("unstyled 2"),"{text}");
        let detail=panel_text(&c.panel_detail);
        for id in ["button-17","button-18","button-19","place-a","place-b"]{assert!(detail.contains(id),"{detail}");}
        c.details_open=false;
        let hidden=panel_screen(&c);
        assert!(hidden.contains("untracked 3"),"{hidden}");
        assert!(!hidden.contains("button-17"),"{hidden}");
        c.details_open=true;
        let shown=panel_screen(&c);
        assert!(shown.contains("button-17")&&shown.contains("place-a"),"{shown}");
    }
    #[test]
    fn moppets_lists_every_agents_task_only_where_there_is_room_and_the_selected_details_always(){
        let mut c=fixture();c.muppets_view=true;c.gang_selected=1;
        c.agent_gang=json!([
            {"name":"beaker","agent":"omp","agent_status":"idle","terminal_title_stripped":"UNSELECTED_TASK","pane_id":"p1"},
            {"name":"gonzo","agent":"codex","agent_status":"working","terminal_title_stripped":"SELECTED_TASK","cwd":"/work/film","pane_id":"p2"}
        ]);c.state["agent_gang"]=c.agent_gang.clone();
        for (w,h) in [(54,20),(90,32),(140,40)]{
            let mut terminal=ratatui::Terminal::new(ratatui::backend::TestBackend::new(w,h)).unwrap();
            terminal.draw(|f|render_desk(f,&c,false)).unwrap();
            let b=terminal.backend().buffer();
            let rows:Vec<String>=(0..h).map(|y|(0..w).map(|x|b[(x,y)].symbol()).collect()).collect();
            let screen=rows.join("\n");
            assert!(screen.contains("SELECTED_TASK"),"{w}: selected task missing\n{screen}");
            // The roster lists every agent's task from 70 columns; a narrower pane shows the
            // selected agent's details only and must not squeeze the others in.
            assert_eq!(screen.contains("UNSELECTED_TASK"),w>=70,"{w}: roster task column\n{screen}");
            assert!(rows[0].contains('▪'),"top film strip missing");
            assert!(rows[h as usize-1].contains('▪'),"bottom film strip missing");
            if let Ok(dir)=std::env::var("MAGICSTUDIO_TEST_PREVIEW"){
                let cells:Vec<_>=(0..h).flat_map(|y|(0..w).map(move|x|{
                    let cell=&b[(x,y)];json!({"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg)})
                })).collect();
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(std::path::Path::new(&dir).join(format!("moppets-{w}.json")),json!({"width":w,"height":h,"cells":cells}).to_string()).unwrap();
            }
        }
    }
    #[test]
    fn agent_cards_keep_names_and_states_without_task_or_provider_text(){
        let mut c=fixture();c.state["agent_gang"]=json!([{"name":"gonzo","agent":"codex","agent_status":"working","terminal_title_stripped":"LONG_TASK"}]);
        let mut terminal=ratatui::Terminal::new(ratatui::backend::TestBackend::new(60,3)).unwrap();
        terminal.draw(|f|render_gang(f,&c,f.area())).unwrap();
        let b=terminal.backend().buffer();
        let screen=(0..3).map(|y|(0..60).map(|x|b[(x,y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
        assert!(screen.contains("gonzo")&&screen.contains("WORKING"));
        assert!(!screen.contains("codex")&&!screen.contains("LONG_TASK"));
    }
    #[test]
    fn cycling_agents_clears_previous_output_and_includes_sprocket(){
        let mut c=fixture();c.agent_gang=json!([{"name":"beaker","pane_id":"p1"},{"name":"gonzo","pane_id":"p2"}]);
        c.spy=Some("Old agent output".into());c.spy_at=Some(std::time::Instant::now());c.scroll=12;
        c.cycle_agent(1);assert_eq!(c.selected_agent().unwrap()["pane_id"],"p2");
        assert!(c.spy.is_none()&&c.spy_at.is_none());assert_eq!(c.scroll,0);
        c.cycle_agent(1);assert_eq!(c.gang_selected,2);assert!(c.selected_agent().is_none());
        c.cycle_agent(1);assert_eq!(c.gang_selected,0);
        c.cycle_agent(-1);assert_eq!(c.gang_selected,2);
        c.state["agent_gang"]=c.agent_gang.clone();
        assert!(agent_strip(&c,30).to_string().contains("Sprocket"));
    }
    #[test]
    fn mops_lists_every_agent_with_its_facts_and_never_turns_unknown_into_zero(){
        let mut c=fixture();
        c.usage=vec![json!({"name":"claude","todayTotalTokens":2_500_000u64,"todaySessions":3})];
        c.agent_gang=json!([
         {"name":"claude","agent":"claude","agent_status":"idle","pane_id":"p1","agent_session":{"source":"herdr:claude","value":"03fc8a53-4d50"},"terminal_title_stripped":"Lyrics","cwd":"/work/omhas-animation"},
         {"name":"codex","agent":"codex","agent_status":"working","pane_id":"p2","terminal_title_stripped":"Render","cwd":"/work/x"}]);
        c.state["agent_gang"]=c.agent_gang.clone();c.gang_selected=0;
        c.mcp_sessions=vec![json!({"pane":"p1","version":"0.1.0","loaded_at":0,"bridge":null,"stale":true})];
        c.spy=Some("old\n▸ Editing center.rs\n  Press up to edit queued messages      Claude Sonnet 5.5 · low\nesc to cancel\nctrl+o to expand\n".into());
        let agents=mops_agents(&c);
        assert_eq!(agents.len(),3,"two herdr agents and Sprocket");
        let roster:Vec<String>=mops_roster_lines(&agents,0,150,6).iter().map(|l|l.to_string()).collect();
        assert!(roster[0].contains("AGENT")&&roster[0].contains("MCP")&&roster[0].contains("FOLDER"),"{roster:?}");
        assert!(roster[1].starts_with("›1 🎬")&&roster[1].contains("claude")&&roster[1].contains("idle")&&roster[1].contains("OUT OF DATE")&&roster[1].contains("omhas-animation"),"{roster:?}");
        assert!(roster[2].contains("codex")&&roster[2].contains("working")&&roster[2].contains("Render")&&roster[2].contains("—"),"{roster:?}");
        assert!(roster[3].contains("Sprocket"),"{roster:?}");
        assert!(roster[2].contains("working codex"),"state and provider columns must not touch: {roster:?}");
        assert_eq!(latest_activity("Press up to edit queued messages   Claude 5.5\nsome plain output\n").as_deref(),Some("some plain output"));
        let narrow:Vec<String>=mops_roster_lines(&agents,0,60,6).iter().map(|l|l.to_string()).collect();
        assert!(!narrow[0].contains("MCP")&&!narrow[0].contains("PROVIDER")&&!narrow[0].contains("TASK"),"a pane with no room lists names and states only: {narrow:?}");
        assert!(!narrow.iter().any(|l|l.contains("Render")),"{narrow:?}");
        let middle:Vec<String>=mops_roster_lines(&agents,0,75,6).iter().map(|l|l.to_string()).collect();
        assert!(middle[0].contains("TASK")&&!middle[0].contains("PROVIDER")&&middle.iter().any(|l|l.contains("Render")),"{middle:?}");
        let facts=mops_facts(&c,&agents[0]);
        let get=|k:&str|facts.iter().find(|f|f.0==k).cloned().unwrap();
        assert_eq!(get("Harness").1,"claude");assert_eq!(get("Session").1,"03fc8a53");
        assert!(get("Tokens").1.starts_with("2.5M today"));
        assert!(get("MCP").1.contains("OUT OF DATE")&&get("MCP").2==RED);
        assert_eq!(get("Activity").1,"▸ Editing center.rs");
        assert_eq!((get("Model").1.as_str(),get("Model").2),("not reported",DIM));
        assert!(!facts.iter().any(|f|matches!(f.0,"Tasks"|"Folders")),"plural placeholders duplicate the actual Task and Folder facts");
        c.touches=vec![
         json!({"client":"herdr:p1","command":"ui.click","control":"sb-next","at":0}),
         json!({"client":"herdr:p2","command":"project.save","control":null,"at":0})];
        let touch=|i:usize|mops_facts(&c,&agents[i]).into_iter().find(|f|f.0=="Last touch").unwrap();
        assert!(touch(0).1.starts_with("sb-next · ui.click · "),"the control the pane acted on: {:?}",touch(0));
        assert!(touch(1).1.starts_with("project.save · "),"{:?}",touch(1));
        assert_eq!((touch(2).1.as_str(),touch(2).2),("nothing seen from this pane",DIM),"Sprocket has no pane");
        let codex=mops_facts(&c,&agents[1]);
        assert!(codex.iter().any(|f|f.0=="Tokens"&&f.1=="not reported")&&codex.iter().any(|f|f.0=="Activity"&&f.1=="waiting for output"),"only the selected agent has output: {codex:?}");
    }
    #[test]
    fn mops_shows_live_port_project_and_build_with_one_selection_target_per_agent() {
        let mut c=fixture();c.muppets_view=true;
        c.agent_gang=json!([{"name":"beaker","pane_id":"ownership-fixture","agent_status":"working"}]);
        c.state["agent_gang"]=c.agent_gang.clone();
        c.studios=json!({"sessions":[{"endpoint":"unix:/owned.sock","active_project":"/songs/A.magic","writer_owner":true,
            "identity":{"agent_pane":"ownership-fixture","control_port":19971,"pid":123,"build_id":"build-proof","branch":"main"}}]});
        let agents=mops_agents(&c);let facts=mops_facts(&c,&agents[0]);
        for(label,value)in[("Port","19971"),("Project","A.magic"),("Build","main · build-proof"),("Access","project writer")]{
            assert!(facts.iter().any(|f|f.0==label&&f.1==value),"{label}: {facts:?}");
        }
        for width in [60,100,140] {
            let mut terminal=ratatui::Terminal::new(ratatui::backend::TestBackend::new(width,38)).unwrap();
            terminal.draw(|f|render(f,&c)).unwrap();
            let hits=c.hits.borrow();
            assert_eq!(hits.iter().filter(|h|h.action=="agent"&&h.params["pane"]=="ownership-fixture").count(),1,"duplicate roster/strip target at {width}");
            let look=control::look(&c,terminal.backend().buffer(),0,"fixture");
            assert_eq!(look["agents"][0]["studio"]["port"],19971);
        }
    }

    #[test]
    fn mcp_panel_shows_warnings_first_and_cuts_families_not_warnings_when_short(){
        let sv=|pid:u32,host:u32,name:&str,family:&str,exe:&str|crate::mcp_watch::Server{pid,host_pid:host,host:name.into(),family:family.into(),exe:exe.into(),write_root:None,age_secs:1,rss_kb:1};
        let mut servers=vec![sv(1,100,"claude","blender","py"),sv(2,200,"codex","blender","py")];
        for (i,family) in ["aaa","bbb","ccc","ddd","eee","fff","ggg","hhh"].iter().enumerate(){servers.push(sv(10+i as u32,100,"claude",family,"x"));}
        let text:Vec<String>=mcp_lines(&servers,6).iter().map(|l|l.to_string()).collect();
        assert_eq!(text.len(),6,"{text:?}");
        assert!(text[1].starts_with("! blender: 2 servers from 2 agents"),"{text:?}");
        assert!(text[2].starts_with("blender"),"largest family first: {text:?}");
        assert!(text.last().unwrap().starts_with("+") && text.last().unwrap().ends_with("more"),"{text:?}");
        assert!(text[2].contains("claude")&&text[2].contains("codex"),"{text:?}");
        assert_eq!(mcp_lines(&[],6)[0].to_string(),"No MCP servers running");
    }
    #[test]
    fn narrow_panes_keep_a_one_line_agent_status_band_and_wide_panes_keep_tiles(){
        let mut c=fixture();
        c.agent_gang=json!([{"name":"beaker","pane_id":"p1","agent_status":"working"},{"name":"gonzo","pane_id":"p2","agent_status":"idle"}]);
        c.state["agent_gang"]=c.agent_gang.clone();
        c.muppets_view=false;
        let strip=agent_strip(&c,76).to_string();
        assert!(strip.contains("beaker")&&strip.contains("gonzo"),"{strip}");
        assert!(strip.contains(agent_glyph("WORKING"))&&strip.contains(agent_glyph("IDLE")),"each agent shows its own status: {strip}");
        let rows=|w:u16|{let mut t=ratatui::Terminal::new(ratatui::backend::TestBackend::new(w,30)).unwrap();t.draw(|f|render_desk(f,&c,false)).unwrap();let b=t.backend().buffer().clone();(0..30u16).map(|y|(0..w).map(|x|b[(x,y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>()};
        let narrow=rows(76);
        assert!(narrow.iter().any(|l|l.contains("beaker")&&l.contains("gonzo")),"narrow keeps the band: {narrow:#?}");
        assert!(!narrow.iter().any(|l|l.contains("╭ › 1")),"narrow uses a line, not tiles");
        assert!(rows(140).iter().any(|l|l.contains("╭ › 1")),"wide keeps the tiles");
    }
    #[test]
    fn tall_narrow_panes_list_agents_vertically_with_their_task_and_short_ones_keep_one_line(){
        let mut c=fixture();
        c.agent_gang=json!([{"name":"beaker","pane_id":"p1","agent_status":"working","terminal_title_stripped":"Lyrics timing pass"},{"name":"gonzo","pane_id":"p2","agent_status":"idle","terminal_title_stripped":"Frontend review"}]);
        c.state["agent_gang"]=c.agent_gang.clone();c.muppets_view=false;c.gang_selected=1;
        let draw=|w:u16,h:u16|{let mut t=ratatui::Terminal::new(ratatui::backend::TestBackend::new(w,h)).unwrap();t.draw(|f|render_desk(f,&c,false)).unwrap();let b=t.backend().buffer().clone();(0..h).map(|y|(0..w).map(|x|b[(x,y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>()};
        let tall=draw(76,40);
        let row=|name:&str|tall.iter().find(|l|l.contains(name)&&l.contains("│")).cloned().unwrap_or_default();
        assert!(tall.iter().any(|l|l.contains("AGENTS · 1–9 select")),"{tall:#?}");
        assert!(row("beaker").contains("WORKING")&&row("beaker").contains("Lyrics timing pass"),"{}",row("beaker"));
        assert!(row("gonzo").contains('›')&&row("gonzo").contains("IDLE")&&row("gonzo").contains("Frontend review"),"{}",row("gonzo"));
        assert!(row("Sprocket").contains("IDLE"),"the in-app agent is listed too: {tall:#?}");
        let short=draw(76,24);
        assert!(!short.iter().any(|l|l.contains("AGENTS · 1–9")),"short panes keep the one-line band");
        assert!(short.iter().any(|l|l.contains("beaker")&&l.contains("gonzo")));
        assert!(!draw(140,40).iter().any(|l|l.contains("AGENTS · 1–9")),"wide panes keep the tiles");
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
        c.chat_draft="hello".into();c.observe_chat_delivery();assert!(c.chat_draft.is_empty());assert!(c.chat_editing,"accepted messages keep the chat window open");assert!(c.message.contains("[OK]"));
    }
    #[test]
    fn approved_booth_uses_shared_stage_evidence_and_replied_state_at_both_sizes(){
        let mut c=fixture();c.message="Chat running · old request".into();c.state["chat"]=json!({"session":"sessionA","selection":{"provider":"opencode","model":"free"},"request":{"status":"replied","request_id":"replyA"}});c.state["chat_turns"]=json!([{"role":"assistant","text":"ACTUAL_REPLY"}]);
        assert!(chat_outcome(&c.state).contains("[OK]"));assert!(!chat_outcome(&c.state).contains("running"));assert!(stage_evidence(&c,2).is_none(),"no prerequisites in the fixture");
        let mut lyrics=c.state.clone();lyrics["prerequisites"]=json!([{"id":"lyrics","state":"ready"}]);let mut l=fixture();l.state=lyrics;assert_eq!(stage_evidence(&l,2).and_then(|v|v["state"].as_str()),Some("ready"),"the lyrics light reads the engine's lyrics item");assert_eq!(PROJECT_STAGES[STORYBOARD_STAGE].0,"Storyboard");
        for(w,h)in [(90,38),(140,44)]{let backend=ratatui::backend::TestBackend::new(w,h);let mut terminal=ratatui::Terminal::new(backend).unwrap();terminal.draw(|f|render_desk(f,&c,false)).unwrap();let b=terminal.backend().buffer();let screen=(0..h).map(|y|(0..w).map(|x|b[(x,y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");for label in ["Lyrics & notes","Animation","PROJECT","Last actions"]{assert!(screen.contains(label),"{w}: missing {label}");}assert!(!screen.contains("Chat running"));assert!(!screen.contains("ACTUAL_REPLY"),"chat is opened on demand");}
        control::action(&mut c,"chat",&json!({})).unwrap();assert!(c.chat_editing);
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
    fn top_bar_centres_magic_studio_and_keeps_unknown_metrics_explicit(){
        let c=fixture();
        for (w,h) in [(60,32),(90,38),(140,44),(240,50)] {
            let backend=ratatui::backend::TestBackend::new(w,h);
            let mut terminal=ratatui::Terminal::new(backend).unwrap();
            terminal.draw(|f|render_desk(f,&c,false)).unwrap();
            let b=terminal.backend().buffer();
            let row=(0..w).map(|x|b[(x,0)].symbol()).collect::<String>();
            if w>=120{assert!(row.contains("CPU ?")&&row.contains("RAM ?"),"{w}: {row}");}
            let byte=row.find("MAGIC STUDIO").expect("application title must survive narrowing");
            let start=row[..byte].chars().count();
            assert!((start as isize-((w as usize-"MAGIC STUDIO · BOOTH".chars().count())/2) as isize).abs()<=1,"{w}: title not centred: {row}");
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
        assert!(top_bar(&c,140,false).to_string().contains("CPU ? · RAM ?"));
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
            if width>=120{assert!(line.to_string().contains("CPU ?"));}
        }
    }
    #[test]
    fn workshop_reclaims_the_oversized_logo_rows(){
        let c=fixture();
        let backend=ratatui::backend::TestBackend::new(90,38);
        let mut terminal=ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f|render_desk(f,&c,true)).unwrap();
        let b=terminal.backend().buffer();
        // The film status and tabs occupy two rows; there is no duplicate status box.
        let body:Vec<String>=(0..12).map(|y|(0..90).map(|x|b[(x,y)].symbol()).collect::<String>()).collect();
        assert!(body[2].contains("PROJECTS"),"catalog must start immediately below the tabs: {:?}",body);
        assert!(!body.iter().any(|row|row.contains("╭ 🎬 MAGIC STUDIO")));
    }
    /// Design review aid: print the rendered desk as plain text so alignment and
    /// truncation can be judged without a live terminal. Run with --ignored --nocapture.
    #[test]
    #[ignore = "screen dump for design review"]
    fn dump_screen_for_design_review(){
        let mut c=fixture();
        c.state["project"]=json!("/run/media/jesse/Speedy/media/animation-studies/scooter-gaffer-ui-23/study.magic");
        c.state["ui"]["active_project"]=c.state["project"].clone();
        c.state["catalog"]=json!({"projects":[
            {"path":"/run/media/jesse/Speedy/media/animation-studies/scooter-gaffer-ui-23/study.magic","name":"study","readiness":{"next_step":"Choose existing song audio","problems":["Storyboard requires valid timed words/captions"]}},
            {"path":"/run/media/jesse/Speedy/media/hey-ho-little-fish/project/Hey Ho Little Fish.magic","name":"Hey Ho Little Fish","readiness":{"next_step":"Review project; editor setup or server is still required","problems":[]}},
            {"path":"/run/media/jesse/Speedy/media/merriweather/bumble-bee/Bumble Bee.magic","name":"Bumble Bee","readiness":{"next_step":"Review project; editor setup or server is still required","problems":[]}}],"error":null});
        c.state["animation_worker"]=json!({"project":c.state["project"].clone(),"job":"magicstudio-blender-test","state":"running","phase":"rendering","frames_completed":42,"frames_total":180,"telemetry":{"memory_current_bytes":419430400,"memory_max_bytes":4294967296u64}});
        c.telemetry_view=true;
        c.dials=crate::dials::sample_host(None);
        c.procs=crate::dials::top_processes(&[],&crate::telemetry::all_procs(),12);
        c.logs=json!({"blender":["frame 120 of 180","writing clip to job dir"],"studio":["#1433 UI · close_requested · app.quit","#1432 image_block_rendered · image.generate"],"omarchy":["13:31 omarchy-shell WARN scene: QML QQuickImage cannot open agy.svg","13:38 omasnap captured workspace 12"],"commits":["a0a7195 feat(backstage): readable lanes, metered worker panel, CLI text mirror","4ab5622 feat(backstage): the project roster becomes a live fleet table"]});
        for control in [false,true]{
            for (w,h) in [(140,44),(90,38)]{
                let backend=ratatui::backend::TestBackend::new(w,h);
                let mut terminal=ratatui::Terminal::new(backend).unwrap();
                terminal.draw(|f|render_desk(f,&c,control)).unwrap();
                let b=terminal.backend().buffer();
                println!("=== control={control} {w}x{h} ===");
                for y in 0..h{println!("{:>3}|{}",y,(0..w).map(|x|b[(x,y)].symbol()).collect::<String>());}
            }
        }
    }
    #[test]
    fn only_idle_and_stalled_agents_can_be_poked(){
        assert!(Center::pokeable("idle"));
        assert!(Center::pokeable("done"));
        assert!(Center::pokeable("stalled"));
        assert!(Center::pokeable("blocked"));
        assert!(!Center::pokeable("working"));
        assert!(!Center::pokeable("busy"));
        assert!(!Center::pokeable("unknown"));
    }
    #[test]
    fn a_poke_needs_a_selected_agent_and_refuses_to_interrupt_work(){
        let mut c=fixture();
        c.agent_gang=json!([{"name":"gonzo","agent":"opencode","agent_status":"working","pane_id":"wG:p7"}]);
        c.gang_selected=0;
        c.begin_poke();
        assert!(c.poke.is_some(),"composing a draft must be possible while an agent works");
        assert!(control::action(&mut c,"note.send",&json!({})).is_err());
        assert!(c.poke.is_some(),"refused submission must preserve the draft");
        assert!(c.message.contains("working"),"{}",c.message);
        c.poke=None;
        c.agent_gang=json!([{"name":"beaker","agent":"omp","agent_status":"idle","pane_id":"wG:pB"}]);
        c.begin_poke();
        let poke=c.poke.clone().expect("an idle agent can be poked");
        assert_eq!(poke.target,"wG:pB");
        assert_eq!(poke.name,"beaker");
        assert!(poke.text.contains("Status check"),"{}",poke.text);
    }
    #[test]
    fn the_tab_strip_names_every_view_and_marks_the_open_one(){
        let mut c=fixture();
        c.telemetry_view=true;
        let strip=tab_strip(&c,140);
        for label in ["WORKSHOP","BOOTH","MOPS","MAYHEM"]{
            assert!(strip.to_string().contains(label),"{label} must be reachable from the strip");
        }
        let filled:Vec<&str>=strip.spans.iter().filter(|s|s.style.bg.is_some()).map(|s|s.content.as_ref()).collect();
        assert_eq!(filled.len(),1,"exactly one tab is the open one");
        assert!(filled[0].contains("MAYHEM"),"{filled:?}");
        c.telemetry_view=false;c.muppets_view=true;
        let strip=tab_strip(&c,140);
        let filled:Vec<&str>=strip.spans.iter().filter(|s|s.style.bg.is_some()).map(|s|s.content.as_ref()).collect();
        assert_eq!(filled.len(),1);
        assert!(filled[0].contains("MOPS"),"{filled:?}");
    }
    #[test]
    fn both_views_show_the_shared_status_line_and_booth_follows_the_tail(){
        let mut c=fixture();
        c.state["snapshot"]=json!({"project":"/fixture.magic","subject":"bee","clip":"fly","playhead_sec":1.5,"selected_part":"wing","unsaved_edits":1,"hyperframes":false,"fps":24,"last_error":null});
        c.state["tail"]=merge_tail(&json!([]),crate::snapshot::tail(&json!({"events":[{"sequence":9,"stage":"saved_state","command":"project.save"}]}),None));
        assert_eq!(merge_tail(&c.state["tail"],crate::snapshot::tail(&json!({"events":[{"sequence":9,"stage":"saved_state","command":"project.save"}]}),None)).as_array().unwrap().len(),1,"polling the same journal row twice adds it once");
        for control in [false,true]{for(w,h)in [(90,38),(140,44)]{
            let backend=ratatui::backend::TestBackend::new(w,h);let mut terminal=ratatui::Terminal::new(backend).unwrap();terminal.draw(|f|render_desk(f,&c,control)).unwrap();let b=terminal.backend().buffer();let screen=(0..h).map(|y|(0..w).map(|x|b[(x,y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
            assert!(screen.contains("MAGIC STUDIO"),"{w}: application identity missing");
            assert!(crate::snapshot::status_strip(&c.state["snapshot"]).contains("@fly · ▶1.50s · part wing · unsaved 1"));
            if !control{assert!(screen.contains("EVENTS")&&screen.contains("project.save"),"{w}: missing event tail\n{screen}");}
        }}
    }

}

/// Known apps that we watch specifically: magicstudio-shell, *-craft, blender, mc, opencode.
/// Returns (pid, name, argv_string, rss_kb).
fn known_app_procs() -> Vec<(u32, String, String, u64)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else { return out; };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue; };
        let raw = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
        let argv: Vec<String> = raw.split(|b| *b == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect();
        if argv.is_empty() { continue; }
        let joined = argv.join(" ");
        let base = argv[0].rsplit('/').next().unwrap_or(&argv[0]);
        let name = if base == "magicstudio-shell" {
            // Show the project name from argv
            let proj = argv.iter().find(|a| a.ends_with(".magic")).map(|p| p.rsplit('/').next().unwrap_or(p).trim_end_matches(".magic")).unwrap_or("magicstudio");
            format!("magicstudio · {}", proj)
        } else if base.ends_with("-craft") || base.ends_with("craft") {
            base.to_owned()
        } else if base == "blender" || joined.contains("blender") {
            "blender".to_owned()
        } else if base == "mc" {
            // Identify subcommand
            let sub = argv.get(1).map(String::as_str).unwrap_or("");
            if sub == "tui" || sub == "backstage" || sub == "booth" || sub == "center" {
                format!("mc · {}", sub)
            } else if argv.iter().any(|x| x == "mcp") {
                "mc · mcp".to_owned()
            } else {
                continue // skip generic mc invocations
            }
        } else if base == "opencode" {
            "opencode".to_owned()
        } else {
            continue // not a known app
        };
        // Get RSS
        let rss_kb = std::fs::read_to_string(format!("/proc/{pid}/status"))
            .ok()
            .and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmRSS:")).and_then(|v| v.split_whitespace().next()).and_then(|v| v.parse().ok()))
            .unwrap_or(0);
        out.push((pid, name, joined, rss_kb));
    }
    out.sort_by(|a, b| b.3.cmp(&a.3)); // sort by RSS descending
    out
}

/// Opinionated process watch: only known apps (craft, blender, magicstudio, mc, opencode).
fn render_known_apps(f: &mut Frame, c: &Center, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let apps = known_app_procs();
    if apps.is_empty() {
        lines.push(styled("No known apps running", MUTED));
    } else {
        lines.push(styled(format!("{:<8}{:<28}{:>8}", "PID", "APP", "RSS"), MUTED));
        for (pid, name, _argv, rss_kb) in apps.iter().take(c.settings.process_count) {
            let rss = if *rss_kb >= 1024 { format!("{:.0}M", *rss_kb as f64 / 1024.0) } else { format!("{}K", rss_kb) };
            let colour = if *rss_kb > c.settings.proc_rss_threshold_kb { AMBER } else { DESK_FG };
            lines.push(Line::from(vec![
                Span::styled(format!("{:<8}", pid), Style::default().fg(MUTED)),
                Span::styled(format!("{:<28}", fit_text(name, 26)), Style::default().fg(colour)),
                Span::styled(format!("{:>8}", rss), Style::default().fg(MUTED)),
            ]));
        }
    }
    f.render_widget(Paragraph::new(lines).block(round_panel("KNOWN APPS · craft/blender/magicstudio/mc/opencode", TEAL, false)), area);
}
