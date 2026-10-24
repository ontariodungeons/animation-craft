use super::*;
use crossterm::event::KeyEvent;

pub(super) fn key_error(c:&Center,key:KeyEvent)->Option<String>{
 let scope=c.scope.as_ref()?;
 let chat_readonly=c.chat_editing&&!scope.permits("agent.send")&&matches!(key.code,KeyCode::Enter|KeyCode::Backspace|KeyCode::Char(_));
 let blocked=chat_readonly||matches!(key.code,KeyCode::F(1)|KeyCode::F(5)|KeyCode::F(6)|KeyCode::F(7))
  || key.modifiers.contains(KeyModifiers::CONTROL)&&key.code==KeyCode::Char('s')&&!scope.permits("project.save")
  || key.code==KeyCode::F(10)&&!scope.permits("storyboard.doctor");
 blocked.then(||"Restricted: Workshop and this control belong to Jesse and Gonzo.".into())
}
pub(super) fn action_error(c:&Center,name:&str,params:&Value)->Option<String>{
 let scope=c.scope.as_ref()?;
 let blocked=name.starts_with("project.")&&!(name=="project.save"&&scope.permits("project.save"))||name=="project"||name.starts_with("intake.")||name=="input"||name=="booth.prepare"||name=="storyboard.open"
  ||name=="doctor"&&!scope.permits("storyboard.doctor")||name=="chat.send"&&!scope.permits("agent.send")
  ||name=="chat"&&params.get("text").is_some()&&!scope.permits("agent.send")||name=="text"&&c.chat_editing&&!scope.permits("agent.send")
  ||name=="view"&&params["name"]=="workshop";
 blocked.then(||format!("Restricted: {name} is outside this project Booth's permissions"))
}
pub(super) fn controls(c:&Center)->Vec<Value>{
 crate::tui::CONTROLS.iter().filter(|(action,_)|{
  if c.scope.is_none(){return true;}
  let name=action.split_whitespace().next().unwrap_or("");
  !name.starts_with("project")&&!name.starts_with("intake")&&name!="input"&&name!="booth.prepare"&&name!="doctor"&&!(name=="chat.send"&&c.scope.as_ref().is_some_and(|s|!s.permits("agent.send")))
 }).map(|(action,description)|json!({"action":if c.scope.is_some()&&action.starts_with("view "){"view name=booth|mops|animal"}else if c.scope.as_ref().is_some_and(|s|!s.permits("agent.send"))&&action.starts_with("chat ["){"chat"}else{action},"description":if c.scope.is_some()&&action.starts_with("view "){"F2 Booth · F3 MOPS · F4 ANIMAL"}else if c.scope.is_some()&&action.starts_with("key "){"Project Booth keys; Workshop and project switching are unavailable"}else{description}})).chain(if c.scope.is_some(){vec![json!({"action":"details | refresh","description":"D project details · F8 refresh"})]}else{vec![]}).collect()
}
pub(super) fn prepare(c:&mut Center,params:&Value)->Result<(),String>{
 if c.scope.is_some(){return Err("Restricted: only the master Workshop prepares Booths".into());}
 let project=params["project"].as_str().or_else(||workshop::selected(c).and_then(|p|p["path"].as_str())).ok_or("Select a project first")?.to_owned();
 c.onboarding=Some(crate::scope::prepare(std::path::Path::new(&project),params)?);c.scroll=0;Ok(())
}
pub(super) fn render_onboarding(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let Some(value)=&c.onboarding else{return;};
 let name=value["project"].as_str().and_then(|p|std::path::Path::new(p).file_stem()).and_then(|v|v.to_str()).unwrap_or("Project");
 let mut lines=vec![styled(format!("{name} · BOOTH / MOPS / ANIMAL"),TEAL),styled("Jesse + Gonzo keep Workshop and master controls.",MUTED),Line::raw(""),styled("START THE PROJECT BOOTH · C copies",DESK_FG),styled(value["launch"].as_str().unwrap_or(""),BLUE),Line::raw(""),styled("Give the agent this assignment environment for its CLI and MCP too.",MUTED),styled("Unlisted teammates and unrelated terminal history are excluded.",MUTED)];
 let members=value["members"].as_array().map_or(0,Vec::len);
 lines.push(styled(format!("{members} assigned project members · no agent launched or messaged"),AMBER));
 for member in value["members"].as_array().into_iter().flatten(){lines.push(styled(format!("{} · {}",member["name"].as_str().unwrap_or("agent"),member["task"].as_str().unwrap_or("")),DESK_FG));}
 lines.push(Line::raw(""));lines.push(styled("ASSIGN A TEAMMATE · master CLI",DESK_FG));lines.push(styled("mc tui booth.prepare pane=<pane-id> name=<name> task='<assigned task>'",BLUE));
 lines.push(styled("Default: project inspection and team notes. Extra engine actions require allow=<command IDs>.",MUTED));
 f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).scroll((c.scroll,0)).block(booth::panel("AGENT BOOTH · C Copy · Esc close",true)),area);
}
pub(super) fn activity(c:&Center,pane:&str)->String{
 let events:Vec<_>=c.state["tail"].as_array().into_iter().flatten().filter(|e|e["pane_id"].as_str()==Some(pane)).rev().take(12).map(|e|format!("{} {}",e["type"].as_str().unwrap_or("event"),e["command"].as_str().unwrap_or(""))).collect();
 if events.is_empty(){"No project receipts recorded for this agent yet.".into()}else{events.join("\n")}
}
pub(super) fn render_animal(f:&mut Frame,c:&Center,area:ratatui::layout::Rect){
 let parts=Layout::vertical([Constraint::Length(3),Constraint::Length(10.min(area.height.saturating_sub(3))),Constraint::Min(1)]).split(area);
 f.render_widget(Paragraph::new(activity_wave(c)).block(booth::panel("🥁 ANIMAL · PROJECT BEAT",false)),parts[0]);
 if let Some(worker)=worker_observation(c){render_worker(f,worker,parts[1]);}else{f.render_widget(Paragraph::new("No render observed for this project.").block(booth::panel("THE KIT · project worker",false)),parts[1]);}
 let mut lines=vec![styled("Project worker only · D details · host processes stay in Workshop",MUTED)];
 if let Some(worker)=worker_observation(c){
  let cpu=worker["telemetry"]["cpu_usage_ns"].as_u64().map(|v|format!("{:.1}s",v as f64/1e9)).unwrap_or("Unknown".into());
  lines.push(styled(format!("CPU time  {cpu}"),DESK_FG));
 }
 for event in c.state["tail"].as_array().into_iter().flatten().rev().take(5){lines.push(styled(format!("{} {}",event["type"].as_str().unwrap_or("event"),event["command"].as_str().unwrap_or("")),MUTED));}
 f.render_widget(Paragraph::new(lines).wrap(Wrap{trim:false}).block(booth::panel("THE SET · project receipts",false)),parts[2]);
}

#[cfg(test)]
mod tests{
 use super::*;
 fn fixture()->Center{
  let mut c=super::super::feedback_tests::fixture();let scope=crate::scope::fixture();
  c.state["project"]=json!(scope.project);c.state["catalog"]=scope.catalog();c.agent_gang=json!(scope.roster(vec![]));c.state["agent_gang"]=c.agent_gang.clone();c.scope=Some(scope);c
 }
 #[test]
 fn project_tabs_and_actions_never_offer_workshop_or_project_switching(){
  let mut c=fixture();let strip=tab_strip(&c,140).to_string();assert!(!strip.contains("WORKSHOP"));for label in ["BOOTH","MOPS","ANIMAL"]{assert!(strip.contains(label));}
  for (name,params) in [("view",json!({"name":"workshop"})),("project",json!({"index":0})),("input",json!({"field":"project","value":"other"})),("booth.prepare",json!({})),("key",json!({"key":"F1"})),("key",json!({"key":"F6"}))]{assert!(control::action(&mut c,name,&params).is_err(),"{name}");}
  assert_eq!(c.view_name(),"booth");assert!(!c.show_form);
 }
 #[test]
 fn scoped_rendering_and_clicks_keep_native_observations_project_only(){
  let mut c=fixture();c.telemetry_view=true;
  for width in [40,90,140]{
   let mut terminal=ratatui::Terminal::new(ratatui::backend::TestBackend::new(width,38)).unwrap();terminal.draw(|f|super::super::render(f,&c)).unwrap();
   let look=control::look(&c,terminal.backend().buffer(),0,"test");let text=look.to_string();assert_eq!(look["access"]["role"],"project");assert_eq!(look["projects"].as_array().unwrap().len(),1);assert!(text.contains("ANIMAL"));assert!(!text.contains("DRUM KIT · HOST"));assert!(look["telemetry"]["host"].is_null());
   assert!(c.hits.borrow().iter().all(|h|h.id!="view-workshop"));
  }
  control::action(&mut c,"click",&json!({"id":"view-mops"})).unwrap();assert_eq!(c.view_name(),"mops");
  assert!(control::action(&mut c,"agent",&json!({"pane":"unassigned-pane"})).is_err());
 }
 #[test]
 fn scoped_activity_uses_project_receipts_and_never_terminal_history(){
  let mut c=fixture();c.state["tail"]=json!([{"pane_id":"assigned-pane","command":"motion.sample","type":"saved"},{"pane_id":"outside-pane","command":"SECRET_OTHER_WORK"}]);
  assert!(activity(&c,"assigned-pane").contains("motion.sample"));assert!(!activity(&c,"assigned-pane").contains("SECRET_OTHER_WORK"));
 }
}
