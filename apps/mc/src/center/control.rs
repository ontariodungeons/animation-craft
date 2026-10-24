use super::*;
use crossterm::event::KeyEvent;
#[derive(Clone,Copy,PartialEq,Eq)]
pub(super) enum Flow{Continue,Quit,Reload}
pub(super) fn key(c:&mut Center,key:KeyEvent)->Flow{
            if let Some(error)=roles::key_error(c,key){c.message=error;return Flow::Continue;}
            if c.onboarding.is_some(){match key.code{KeyCode::Esc=>c.onboarding=None,KeyCode::Char('c')=>c.copy_command(),KeyCode::PageUp=>c.scroll=c.scroll.saturating_sub(4),KeyCode::PageDown=>c.scroll=c.scroll.saturating_add(4),KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL)=>return Flow::Quit,_=>{}}return Flow::Continue;}
            if !c.show_form&&!c.chat_editing&&c.poke.is_none()&&c.view_name()=="workshop"&&key.code==KeyCode::Char('b'){if let Err(error)=roles::prepare(c,&json!({})){c.message=error;}return Flow::Continue;}

            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
                if c.poke.is_some()||!c.chat_draft.is_empty(){c.message="Resolve or cancel the draft before closing the TUI.".into();return Flow::Continue;}
                return Flow::Quit;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL)&&key.code==KeyCode::Char('s'){
                c.execute("project.save",json!({}));return Flow::Continue;
            }
            if key.code==KeyCode::F(9){
                if c.poke.is_some()||c.show_form{c.message="Finish or cancel the current composer/intake before opening Sproket.".into();}
                else if c.chat_editing{c.chat_editing=false;}else if let Err(error)=open_chat(c){c.message=error;}
                return Flow::Continue;
            }
            if let Some(poke)=c.poke.clone(){
                match key.code{
                    KeyCode::Esc=>c.poke=None,
                    KeyCode::Enter=>{
                        let recipient=c.agent_gang.as_array().into_iter().flatten().find(|g|g["pane_id"].as_str()==Some(poke.target.as_str()));
                        if recipient.is_none_or(|g|!Center::pokeable(g["agent_status"].as_str().unwrap_or(""))){c.message="Not sent: recipient is working or unavailable. Draft retained; send when idle.".into();}
                        else{let(target,text)=(poke.target.clone(),poke.text.trim().to_owned());c.poke=None;if !text.is_empty()&&!c.send_poke(&target,&text){c.poke=Some(poke);}}
                    }
                    KeyCode::Backspace=>{if let Some(poke)=c.poke.as_mut(){poke.text.pop();}}
                    KeyCode::PageUp=>c.scroll=c.scroll.saturating_sub(4),
                    KeyCode::PageDown=>c.scroll=c.scroll.saturating_add(4),
                    KeyCode::Char(ch)=>{if let Some(poke)=c.poke.as_mut(){poke.text.push(ch);}}
                    _=>{}
                }
                return Flow::Continue;
            }
            if c.chat_editing {match key.code{KeyCode::Esc=>c.chat_editing=false,KeyCode::Enter=>c.send_chat(),KeyCode::Backspace=>{c.chat_draft.pop();},KeyCode::PageUp=>c.scroll=c.scroll.saturating_add(4),KeyCode::PageDown=>c.scroll=c.scroll.saturating_sub(4),KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL)=>c.chat_draft.push(ch),_=>{}}return Flow::Continue;}
            if !c.show_form&&key.code==KeyCode::Char('g'){
                return self::key(c,KeyEvent::new(KeyCode::F(3),KeyModifiers::NONE));
            }
            if !c.show_form&&c.muppets_view&&!c.details_open&&!c.event_open{
                let number=match key.code{KeyCode::Char(n @ '1'..='9')=>Some(n as usize-'1' as usize),KeyCode::Char('0')=>Some(9),_=>None};
                if let Some(index)=number{let _=select_agent(c,&json!({"index":index}));return Flow::Continue;}
                match key.code{KeyCode::Left=>{c.cycle_agent(-1);return Flow::Continue;},KeyCode::Right=>{c.cycle_agent(1);return Flow::Continue;},_=>{}}
            }
            if !c.show_form && key.code==KeyCode::Char('e'){
                c.event_focus=!c.event_focus;c.event_open=false;c.control_view=false;c.muppets_view=false;c.telemetry_view=false;c.scroll=0;return Flow::Continue;
            }
            if c.event_focus&&!c.show_form{
                match key.code{
                    KeyCode::Up|KeyCode::Char('k')=>{if c.event_open{c.scroll=c.scroll.saturating_sub(1);}else{c.event_index=c.event_index.saturating_sub(1);c.scroll=0;}return Flow::Continue;},
                    KeyCode::Down|KeyCode::Char('j')=>{if c.event_open{c.scroll=c.scroll.saturating_add(1);}else{let count=c.state["tail"].as_array().map_or(0,Vec::len);c.event_index=(c.event_index+1).min(count.saturating_sub(1));c.scroll=0;}return Flow::Continue;},
                    KeyCode::Enter=>{if let Some(event)=selected_event(&c).cloned(){if let Some(index)=event_agent(&c,&event){c.gang_selected=index;}c.event_detail=Some(event);c.event_open=true;c.scroll=0;}return Flow::Continue;},
                    KeyCode::Esc=>{if c.event_open{c.event_open=false;}else{c.event_focus=false;}c.scroll=0;return Flow::Continue;},
                    _=>{}
                }
            }
            if matches!(key.code,KeyCode::F(1)|KeyCode::F(2)|KeyCode::F(3)|KeyCode::F(4)){c.event_open=false;c.event_focus=false;c.details_open=false;}
            if key.code==KeyCode::F(1){c.control_view=true;c.show_form=false;c.muppets_view=false;c.telemetry_view=false;c.config_view=false;return Flow::Continue;}
            if key.code==KeyCode::F(3) && !c.show_form{c.muppets_view=true;c.control_view=false;c.telemetry_view=false;c.config_view=false;return Flow::Continue;}
            if key.code==KeyCode::F(2) && !c.show_form{if c.view_name()=="workshop"&&workshop::selected(c).is_some(){c.observe_selected();return Flow::Continue;}c.control_view=false;c.muppets_view=false;c.telemetry_view=false;return Flow::Continue;}
            if key.code==KeyCode::F(10){c.check_storyboard();return Flow::Continue;}
            // F4 is the telemetry tab: dials, processes and live log tails, sampled only
            // while it is open. F6 opens the intake form; F4 still closes it.
            if key.code==KeyCode::F(4) && !c.show_form{c.telemetry_view=!c.telemetry_view;c.control_view=false;c.muppets_view=false;c.config_view=false;c.details_open=false;return Flow::Continue;}
            if key.code==KeyCode::F(4) && c.show_form{c.show_form=false;return Flow::Continue;}
            if key.code==KeyCode::F(5) && !c.show_form{c.config_view=!c.config_view;c.control_view=false;c.muppets_view=false;c.telemetry_view=false;c.details_open=false;return Flow::Continue;}
            if key.code==KeyCode::F(6) && !c.show_form{c.show_form=true;return Flow::Continue;}
            if !c.show_form&&c.view_name()=="workshop"{
                match key.code{
                    KeyCode::Left|KeyCode::Char('k')=>{workshop::cycle(c,-1);return Flow::Continue;},
                    KeyCode::Right|KeyCode::Char('j')|KeyCode::Tab=>{workshop::cycle(c,1);return Flow::Continue;},
                    KeyCode::Up=>{workshop::cycle(c,-(c.catalog_columns.get() as isize));return Flow::Continue;},
                    KeyCode::Down=>{workshop::cycle(c,c.catalog_columns.get() as isize);return Flow::Continue;},
                    KeyCode::Char('i')=>{workshop::toggle(c);return Flow::Continue;},
                    KeyCode::Enter=>{c.observe_selected();return Flow::Continue;},
                    KeyCode::Esc if c.details_open=>{c.details_open=false;c.scroll=0;return Flow::Continue;},
                    _=>{}
                }
            }
            if !c.show_form {
                match key.code{
                    KeyCode::Char(n @ '1'..='9') if !key.modifiers.contains(KeyModifiers::CONTROL)=>{let index=(n as usize)-('1' as usize);if index<c.copy_commands().len(){c.command_selected=index;}},
                    KeyCode::Up=>c.command_selected=c.command_selected.saturating_sub(1),
                    KeyCode::Down|KeyCode::Tab=>c.command_selected=(c.command_selected+1)%c.copy_commands().len().max(1),
                    KeyCode::Char('c')=>c.copy_command(),
                    KeyCode::Char('[')=>c.stage_selected=(c.stage_selected+PROJECT_STAGES.len()-1)%PROJECT_STAGES.len(),
                    KeyCode::Char(']')=>c.stage_selected=(c.stage_selected+1)%PROJECT_STAGES.len(),
                    KeyCode::Char('d')=>c.details_open=!c.details_open,
                    KeyCode::Char(',')=>c.cycle_agent(-1),
                    KeyCode::Char('.')=>c.cycle_agent(1),
                    KeyCode::Char('p')=>c.begin_poke(),
                    KeyCode::Char('R')=>{if !c.chat_draft.is_empty()||c.show_form{c.message="Resolve draft/intake fields before reloading the TUI.".into();}else{return Flow::Reload;}},
                    KeyCode::Char('P')=>c.quick_poke(),
                    KeyCode::Char('A')=>c.check_in_all(),
                    KeyCode::Char('j') if c.control_view=>c.catalog_selected+=1,
                    KeyCode::Char('k') if c.control_view=>c.catalog_selected=c.catalog_selected.saturating_sub(1),
                    KeyCode::Enter if c.control_view=>c.observe_selected(),
                    KeyCode::PageDown=>c.scroll=c.scroll.saturating_add(4),
                    KeyCode::PageUp=>c.scroll=c.scroll.saturating_sub(4),
                    KeyCode::F(8)=>{c.pinned_instance=None;c.refresh();},
                    _=>{}
                }
                return Flow::Continue;
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
                return Flow::Continue;
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
Flow::Continue
}

fn press(c:&mut Center,code:KeyCode)->Flow{key(c,KeyEvent::new(code,KeyModifiers::NONE))}
fn required<'a>(p:&'a Value,name:&str)->Result<&'a str,String>{p[name].as_str().ok_or_else(||format!("{name}= is required"))}
fn input_text(p:&Value,name:&str)->Result<String,String>{match p.get(name){Some(Value::String(v))=>Ok(v.clone()),Some(v @ (Value::Number(_)|Value::Bool(_)))=>Ok(v.to_string()),_=>Err(format!("{name}= must contain text"))}}
fn select_agent(c:&mut Center,p:&Value)->Result<(),String>{
 let agents=c.agent_gang.as_array().ok_or("No agent roster has been observed yet")?;
 let index=if let Some(index)=p["index"].as_u64(){index as usize}
 else if let Some(pane)=p["pane"].as_str(){agents.iter().position(|g|g["pane_id"].as_str()==Some(pane)).ok_or("Agent pane not found")?}
 else{
  let name=required(p,"name")?;
  if name.eq_ignore_ascii_case("sprocket"){agents.len()}else{
   let matches:Vec<_>=agents.iter().enumerate().filter(|(_,g)|g["name"].as_str().is_some_and(|v|v.eq_ignore_ascii_case(name))).map(|(i,_)|i).collect();
   match matches.as_slice(){[one]=>*one,[]=>return Err(format!("No agent named {name}; use look section=agents")),_=>return Err(format!("Several agents are named {name}; use pane=<id>"))}
  }
 };
 if index>agents.len(){return Err("Agent index is out of range".into());}
 c.gang_selected=index;c.spy=None;c.spy_at=None;c.scroll=0;Ok(())
}
fn open_chat(c:&mut Center)->Result<(),String>{
 let target=if c.view_name()=="workshop"{workshop::selected(c).and_then(|p|p["path"].as_str())}else{c.state["project"].as_str()}.ok_or("Chat blocked: choose a project first.")?.to_owned();
 if !c.chat_draft.is_empty(){
  let origin=c.chat_draft_project.as_deref().or(c.state["project"].as_str());
  if origin!=Some(target.as_str()){return Err("Chat blocked: retained draft belongs to another project. Reopen that project to continue.".into());}
 }
 if c.view_name()=="workshop"{
  c.observe_selected();
  if c.state["project"].as_str()!=Some(target.as_str())||c.session.project_path.as_ref().map(|p|p.to_string_lossy()).as_deref()!=Some(target.as_str()){
   return Err(format!("Chat blocked: selected project could not be attached. {}",c.message));
  }
 }
 c.chat_draft_project=Some(target);c.chat_editing=true;c.scroll=0;Ok(())
}
pub(super) fn action(c:&mut Center,name:&str,p:&Value)->Result<Flow,String>{
 let previous_message=c.message.clone();
 if let Some(error)=roles::action_error(c,name,p){return Err(error);}
 for field in ["index","sequence"]{if p.get(field).is_some()&&!p[field].is_u64(){return Err(format!("{field}= must be a nonnegative integer"));}}
 if p.get("append").is_some()&&!p["append"].is_boolean(){return Err("append= must be true or false".into());}
 if c.show_form&&matches!(name,"view"|"agent"|"project"|"project.select"|"project.open-selected"|"project.inactive"|"chat"|"chat.send"|"event"|"events"){return Err("Intake is open. Use key key=F4 to close it before changing views.".into());}
 if c.poke.is_some()&&matches!(name,"chat"|"chat.send"){return Err("A note composer is open; finish or cancel that note first".into());}
 if c.chat_editing&&name=="note"{return Err("Project chat is open; finish or cancel it before composing an agent note".into());}
 if !matches!(name,"look"|"read"|"status"|"controls"|"key"|"text"|"note"|"note.send"|"note.cancel"|"chat"|"chat.send"|"chat.close"|"cancel"|"submit"|"scroll")&&(c.poke.is_some()||c.chat_editing){return Err("A composer is open. Finish or cancel it before changing views.".into());}
 let mut flow=Flow::Continue;
 match name{
  "look"|"read"|"status"|"controls"=>{},
  "booth.prepare"=>{roles::prepare(c,p)?;},
  "click"=>{
   let id=required(p,"id")?;
   let hit=c.hits.borrow().iter().find(|h|h.id==id).cloned().ok_or("Control is not visible; use look section=elements")?;
   if !hit.enabled{return Err("This control is unavailable while a composer or intake form is open".into());}
   return mouse(c,hit.area.x+hit.area.width/2,hit.area.y+hit.area.height/2);
  },
  "view"=>{
   c.onboarding=None;
   let view=required(p,"name")?;
   let (target,canonical)=match view{"workshop"=>(1,"workshop"),"booth"=>(2,"booth"),"mops"|"moppets"|"muppets"=>(3,"mops"),"animal"|"telemetry"=>(4,"animal"),_=>return Err("Unknown view: choose workshop, booth, mops or animal".into())};
   if c.event_open{press(c,KeyCode::Esc);}
   if c.event_focus{press(c,KeyCode::Esc);}
   if c.details_open{press(c,KeyCode::Char('d'));}
   if c.view_name()!=canonical{flow=press(c,KeyCode::F(target));}
   if c.telemetry_view{c.refresh();}
  },
  "agent"=>{select_agent(c,p)?;c.details_open=false;flow=press(c,KeyCode::F(3));c.refresh();},
  "recover"=>{
   // Hand an unpushed branch back to the agent that still holds it.
   let pane=required(p,"pane")?;
   let branch=required(p,"branch")?;
   let ahead=p["ahead"].as_u64().unwrap_or(0);
   let worktree=p["worktree"].as_str().unwrap_or("");
   let message=format!("Your branch {branch} is {ahead} commit(s) ahead of its upstream and has not been pushed. Please push it (git push) or tell Jesse what it holds before it is lost.");
   // The agent's own pane receives the note; nothing is pushed from here.
   let note=json!({"action":"note.send","pane":pane,"text":message});
   let sent=crate::center::send_agent_note(pane,&message);
   let _=note;
   match sent{
    Ok(())=>{c.message=format!("Sent {branch} to {pane} ({worktree}): {message}");}
    Err(error)=>return Err(format!("Could not reach {pane}: {error}")),
   }
  },
  "agent.next"=>{c.cycle_agent(1);c.refresh();},
  "agent.previous"=>{c.cycle_agent(-1);c.refresh();},
  "events"=>{if !c.event_focus{flow=press(c,KeyCode::Char('e'));}},
  "remind"=>{if p["pane"].is_string()||p["name"].is_string(){select_agent(c,p)?;}c.quick_poke();if !c.message.starts_with("Submitted"){return Err(c.message.clone());}},
  "project.inactive"=>{if p.get("show").is_some()&&!p["show"].is_boolean(){return Err("show= must be true or false".into());}if let Some(show)=p["show"].as_bool(){if workshop::expanded(c)!=show{workshop::toggle(c);}}else{workshop::toggle(c);}},
  "project.open-selected"=>{if workshop::selected(c).is_none(){return Err("No project selected".into());}c.details_open=false;c.observe_selected();},
  "project"|"project.select"=>{
   let projects=c.state["catalog"]["projects"].as_array().ok_or("Project catalog unavailable")?;
   let index=if let Some(index)=p["index"].as_u64(){index as usize}else{
    let path=required(p,"path")?;projects.iter().position(|v|v["path"].as_str()==Some(path)).ok_or("Project is not in the observed catalog; use input field=project value=<path> then project.open")?
   };
   let selected=projects.get(index).and_then(|v|v["path"].as_str()).ok_or("Project index is out of range")?.to_owned();
   workshop::select(c,index)?;
   if name=="project.select"{c.control_view=true;c.muppets_view=false;c.telemetry_view=false;c.event_open=false;c.event_focus=false;return Ok(Flow::Continue);}
   c.observe_selected();
   if c.session.project_path.as_ref().map(|v|v.to_string_lossy().into_owned()).as_deref()!=Some(selected.as_str()){return Err("Project attachment failed; no Studio was switched".into());}
  },
  "stage"=>{
   let index=if let Some(index)=p["index"].as_u64(){index as usize}else{let name=required(p,"name")?;PROJECT_STAGES.iter().position(|(label,id)|label.eq_ignore_ascii_case(name)||*id==name).ok_or("Unknown stage")?};
   if index>=PROJECT_STAGES.len(){return Err("Stage index is out of range".into());}c.stage_selected=index;
  },
  "event"=>{
   let events=c.state["tail"].as_array().ok_or("No events recorded")?;
   let index=if let Some(seq)=p["sequence"].as_u64(){events.iter().rev().position(|e|e["sequence"].as_u64()==Some(seq)).ok_or("Event sequence is not in the retained tail")?}else{p["index"].as_u64().unwrap_or(0) as usize};
   if index>=events.len(){return Err("Event index is out of range".into());}
   c.event_open=false;c.event_detail=None;c.event_index=index;c.event_focus=true;c.control_view=false;c.telemetry_view=false;c.muppets_view=false;c.details_open=false;
   flow=press(c,KeyCode::Enter);
  },
  "command"=>{let index=p["index"].as_u64().ok_or("index= is required")? as usize;if index>=c.copy_commands().len(){return Err("Command index is out of range".into());}c.command_selected=index;},
  "copy"=>{flow=press(c,KeyCode::Char('c'));if !c.message.starts_with("COPIED"){return Err(c.message.clone());}},
  "note"=>{
   if p["pane"].is_string()||p["name"].is_string()||p["index"].is_u64(){select_agent(c,p)?;}
   if c.poke.is_none(){c.begin_poke();}
   let poke=c.poke.as_mut().ok_or_else(||c.message.clone())?;
   if p.get("text").is_some(){let text=input_text(p,"text")?;if p["append"]==true{poke.text.push_str(&text);}else{poke.text=text;}}
  },
  "note.send"=>{if c.poke.as_ref().is_none_or(|p|p.text.trim().is_empty()){return Err("No nonempty note draft is open".into());}flow=press(c,KeyCode::Enter);if c.poke.is_some(){return Err(c.message.clone());}},
  "note.cancel"=>{if c.poke.is_some(){flow=press(c,KeyCode::Esc);}},
  "chat"=>{if !c.chat_editing{flow=press(c,KeyCode::F(9));}if !c.chat_editing{return Err(c.message.clone());}if p.get("text").is_some(){let text=input_text(p,"text")?;if p["append"]==true{c.chat_draft.push_str(&text);}else{c.chat_draft=text;}}},
  "chat.close"=>{if c.chat_editing{flow=press(c,KeyCode::Esc);}},
  "chat.send"=>{
   if !c.chat_editing||c.chat_draft.trim().is_empty(){return Err("No nonempty chat draft is open".into());}
   if c.chat_receiver.is_some(){return Err("A chat submission is already pending".into());}
   let previous=c.chat_sent_request.clone();flow=press(c,KeyCode::Enter);
   if c.chat_sent_request==previous{return Err(c.message.clone());}
  },
  "input"=>{
   let names=["project","root","name","audio","transcript","captions","audiomap","materials"];
   let field=required(p,"field")?;let index=names.iter().position(|v|*v==field).ok_or("Unknown intake field")?;
   let value=input_text(p,"value")?;if !c.show_form{press(c,KeyCode::F(6));}c.editing=false;c.selected=index;c.fields[index]=value;
  },
  "intake.open"=>{if !c.show_form{flow=press(c,KeyCode::F(6));}},
  "intake.new"|"intake.create"|"intake.resume"|"project.open"|"storyboard.open"=>{
   if !c.show_form{press(c,KeyCode::F(6));}c.editing=false;
   flow=press(c,KeyCode::F(match name{"intake.new"=>2,"intake.create"=>5,"intake.resume"=>6,"project.open"=>3,_=>7}));
  },
  "project.save"=>{flow=key(c,KeyEvent::new(KeyCode::Char('s'),KeyModifiers::CONTROL));},
  "doctor"=>{flow=press(c,KeyCode::F(10));if let Some(error)=&c.doctor_error{return Err(error.clone());}},
  "details"=>{flow=press(c,KeyCode::Char('d'));},
  "refresh"=>{flow=press(c,KeyCode::F(8));},
  "scroll"=>{let rows=p["rows"].as_i64().ok_or("rows= must be a signed integer")?;c.scroll=(i64::from(c.scroll).saturating_add(rows)).clamp(0,u16::MAX as i64) as u16;},
  "cancel"=>{flow=press(c,KeyCode::Esc);},
  "submit"=>{flow=press(c,KeyCode::Enter);},
  "text"=>{
   let text=input_text(p,"text")?;
   if let Some(poke)=&mut c.poke{poke.text.push_str(&text);}else if c.chat_editing{c.chat_draft.push_str(&text);}else if c.show_form&&c.editing{c.fields[c.selected].push_str(&text);}else{return Err("No input has focus; open a note, chat or intake field first".into());}
  },
  "key"=>{let key_event=parse_key(&input_text(p,"key")?)?;if let Some(error)=roles::key_error(c,key_event){return Err(error);}flow=key(c,key_event);},
  "reload"|"quit"=>{
   if c.poke.is_some()||!c.chat_draft.is_empty()||c.show_form{return Err("Resolve draft/intake fields before reloading or closing the TUI".into());}
   flow=if name=="reload"{Flow::Reload}else{Flow::Quit};
  },
  _=>return Err(format!("Unknown TUI action {name}; use mc tui controls")),
 }
 let executed=matches!(name,"intake.create"|"intake.resume"|"project.open"|"project.save"|"storyboard.open");
 if (executed||c.message!=previous_message)&&["Restricted:","BLOCKED:","Not sent:","Chat error:","Project observation failed:"].iter().any(|prefix|c.message.starts_with(prefix)){return Err(c.message.clone());}
 Ok(flow)
}
fn parse_key(value:&str)->Result<KeyEvent,String>{
 let (modifiers,text)=if let Some(rest)=value.strip_prefix("ctrl+"){(KeyModifiers::CONTROL,rest)}else{(KeyModifiers::NONE,value)};
 let code=match text.to_ascii_lowercase().as_str(){
  "enter"=>KeyCode::Enter,"esc"|"escape"=>KeyCode::Esc,"tab"=>KeyCode::Tab,"backspace"=>KeyCode::Backspace,"up"=>KeyCode::Up,"down"=>KeyCode::Down,"left"=>KeyCode::Left,"right"=>KeyCode::Right,"pageup"=>KeyCode::PageUp,"pagedown"=>KeyCode::PageDown,
  name if name.starts_with('f')&&name[1..].parse::<u8>().is_ok_and(|n|(1..=12).contains(&n))=>KeyCode::F(name[1..].parse().map_err(|_|"Invalid function key")?),
  _=>{let mut chars=text.chars();let ch=chars.next().ok_or("Key is empty")?;if chars.next().is_some(){return Err(format!("Unknown key {value}"));}KeyCode::Char(ch)}
 };
 Ok(KeyEvent::new(code,modifiers))
}
fn colour(value:Color)->Value{match value{Color::Rgb(r,g,b)=>json!([r,g,b]),Color::Indexed(n)=>json!({"indexed":n}),_=>json!(format!("{value:?}"))}}
pub(super) fn look(c:&Center,buffer:&ratatui::buffer::Buffer,frame:u64,endpoint:&str)->Value{
 let area=buffer.area;
 let lines:Vec<String>=(area.y..area.bottom()).map(|y|{
  let mut line=String::new();let mut x=area.x;
  while x<area.right(){let text=buffer[(x,y)].symbol();line.push_str(text);x=x.saturating_add(Line::raw(text).width().max(1) as u16);}
  line.trim_end().to_owned()
 }).collect();
 let tiles=gang_tiles(c);
 let agents:Vec<_>=tiles.iter().enumerate().map(|(i,(name,identity,status,word))|{
  let agent=&c.state["agent_gang"][i];
  json!({"index":i,"name":name,"icon":moppet_icon(name),"colour":colour(*identity),"status_colour":colour(*status),"status":word,"pane":agent["pane_id"],"provider":agent["agent"],"task":agent["terminal_title_stripped"],"task_source":agent["task_source"].as_str().unwrap_or("terminal title; not an assignment record"),"selected":i==c.gang_selected})
 }).collect();
 let host=c.dials.as_ref().filter(|_|c.scope.is_none()).map(|h|json!({"cpu_percent":h.cpu_pct,"cores":h.cores,"memory_used_kb":h.mem_used_kb,"memory_total_kb":h.mem_total_kb,"swap_used_kb":h.swap_used_kb,"swap_total_kb":h.swap_total_kb,"load":h.load,"age_ms":h.at.elapsed().as_millis()}));
 let processes:Vec<_>=c.procs.iter().filter(|_|c.scope.is_none()).map(|p|json!({"pid":p.pid,"name":p.name,"cpu_percent":p.cpu_pct,"rss_kb":p.rss_kb})).collect();
 let fields:serde_json::Map<String,Value>=["project","root","name","audio","transcript","captions","audiomap","materials"].iter().enumerate().map(|(i,name)|(name.to_string(),json!(c.fields[i]))).collect();
 let commands:Vec<_>=c.copy_commands().iter().enumerate().map(|(i,(label,literal))|json!({"index":i,"label":label,"literal":literal,"selected":i==c.command_selected})).collect();
 let elements:Vec<_>=c.hits.borrow().iter().map(|h|json!({"id":h.id,"label":h.label,"enabled":h.enabled,"rect":{"x":h.area.x,"y":h.area.y,"width":h.area.width,"height":h.area.height},"action":h.action,"params":h.params})).collect();
 let controls=roles::controls(c);
 json!({"schema":"mc.tui/1","instance":std::process::id(),"endpoint":endpoint,"frame":frame,"rendered_unix_ms":crate::worker::now_ms(),"acknowledgement":"actual frame rendered",
  "access":{"role":if c.scope.is_some(){"project"}else{"master"},"project":c.scope.as_ref().map(|s|s.project.clone()),"members":c.scope.as_ref().map(|s|s.members.clone()),"allowed_commands":c.scope.as_ref().map(|s|s.commands("")["commands"].clone())},"onboarding":c.onboarding,
  "view":if c.onboarding.is_some(){"onboarding"}else if c.show_form{"intake"}else if c.poke.is_some(){"note"}else if c.chat_editing{"chat"}else if c.event_open{"event"}else{c.view_name()},"tab":c.view_name(),"project":c.state["project"],"message":c.message,
  "project_overview":booth::overview(c),"artwork_motion":worker_observation(c).is_some_and(booth::motion_active),
  "status_line":crate::snapshot::status_strip(&c.state["snapshot"]),"timecode":c.state["snapshot"]["playhead_sec"].as_f64().zip(c.state["snapshot"]["fps"].as_f64()).map(|(seconds,fps)|timecode(seconds,fps)),
  "screen":{"columns":area.width,"rows":area.height,"lines":lines},"studio":c.state["ui"],"snapshot":c.state["snapshot"],
  "agents":agents,"agent_detail":{"pane":c.selected_agent().map(|a|a["pane_id"].clone()),"output":c.spy,"age_ms":c.spy_at.map(|t|t.elapsed().as_millis())},
  "projects":c.state["catalog"]["projects"],"catalog_scope":if c.scope.is_some(){"assigned project only"}else{"existing recent-project catalog"},"stages":c.state["prerequisites"],"worker":c.state["animation_worker"],"events":c.state["tail"],"event_detail":c.event_detail,
  "animal":{"scope":if c.scope.is_some(){"project"}else{"system"},"worker":c.state["animation_worker"],"beat":activity_wave(c),"host":host.clone(),"processes":processes.clone()},
  "telemetry":{"sampling":c.telemetry_view,"host":host,"processes":processes,"logs_age_ms":c.logs_at.map(|t|t.elapsed().as_millis())},"logs":if c.scope.is_some(){json!({"studio":c.state["tail"]})}else{c.logs.clone()},"usage":if c.scope.is_some(){json!([])}else{json!(c.usage)},
  "workshop":{"discovery":{"observed_at_ms":c.studios["observed_at_ms"],"pending":c.studios_receiver.is_some(),"error":c.studios["observation_error"],"errors":c.studios["errors"]},"cards":workshop::observations(c),"selected_project":workshop::selected(c),"inactive_expanded":workshop::expanded(c),"visible_order":workshop::order(c),"columns":c.catalog_columns.get()},
  "selected":{"agent":c.gang_selected,"project":c.catalog_selected,"stage":c.stage_selected,"event":c.event_index,"command":c.command_selected,"scroll":c.scroll},
  "composers":{"note":c.poke.as_ref().map(|p|json!({"pane":p.target,"name":p.name,"text":p.text,"delivery":"unsent draft"})),"chat":{"open":c.chat_editing,"text":c.chat_draft,"project":c.chat_draft_project}},
  "elements":elements,"intake":{"open":c.show_form,"editing":c.editing,"selected":c.selected,"fields":fields},"commands":commands,"controls":controls})
}
pub(super) fn reload(c:&Center)->Result<(),String>{
 use std::os::unix::process::CommandExt;
 let current=std::env::current_exe().map_err(|e|e.to_string())?;
 let name=current.to_string_lossy();let executable=std::path::Path::new(name.strip_suffix(" (deleted)").unwrap_or(&name));
 if !executable.is_file(){return Err("Updated TUI executable is unavailable".into());}
 let mut command=std::process::Command::new(executable);
 command.arg("booth");
 if let Some(project)=c.session.project_path.as_ref(){command.arg(format!("project={}",project.display()));}
 if let Some(scope)=&c.scope{scope.inherit(&mut command);}
 command.env("MAGICSTUDIO_VIEW",c.view_name());
 Err(command.exec().to_string())
}

pub(super) fn mouse(c:&mut Center,x:u16,y:u16)->Result<Flow,String>{
 let hit=c.hits.borrow().iter().find(|h|x>=h.area.x&&x<h.area.right()&&y>=h.area.y&&y<h.area.bottom()).cloned();
 match hit{Some(hit) if hit.enabled=>action(c,&hit.action,&hit.params),Some(_)=>Err("Finish or cancel the open composer first.".into()),None=>Ok(Flow::Continue)}
}
