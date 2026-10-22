//! Project conversations are commands, shared by the desktop, CLI and MCP.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

fn invalid(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }
fn stamp() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d|d.as_nanos()).unwrap_or(0) }

/// Read only chat settings from the same file the app edits. Never return credential fields.
pub fn settings() -> Result<Value> {
    let base=std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
        .or_else(||std::env::var_os("HOME").map(|h|std::path::PathBuf::from(h).join(".config")));
    let stored=if let Some(base)=base {
        match fs::read_to_string(base.join("animationcraft/agent.json")) {
            Ok(text)=>serde_json::from_str::<Value>(&text).map_err(|_|invalid("Invalid AnimationCraft agent.json; repair existing settings without changing credentials"))?,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>json!({}),
            Err(_)=>return Err(invalid("Cannot read AnimationCraft chat settings; no fallback agent selected")),
        }
    } else {json!({})};
    if stored.get("agent").is_some_and(|v|!v.is_string()) {return Err(invalid("Invalid saved agent; choose an installed chat agent in the app. No fallback selected"));}
    if stored.get("extra_args").is_some_and(|v|!v.is_array()) {return Err(invalid("Invalid saved extra_args; expected an array. No arguments ignored"));}
    let agent=stored["agent"].as_str().filter(|s|!s.trim().is_empty()).map(str::to_owned)
        .unwrap_or_else(||Command::new("omarchy-default-agent").output().ok()
            .filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .filter(|s|!s.is_empty()).unwrap_or_else(||"codex".into()));
    let mut selected=json!({"agent":agent});
    for key in ["provider","model","thinking"] {
        if stored.get(key).is_some_and(|v|!v.is_string()) {return Err(invalid(format!("Invalid saved {key}; expected a string. No selection fallback attempted")));}
        let value=stored[key].as_str().unwrap_or_default().trim();
        if value.len()>256 || value.chars().any(char::is_control) {return Err(invalid("Invalid chat selection; use a catalog provider/model ID"));}
        selected[key]=json!(value);
    }
    selected["extra_args"]=stored["extra_args"].as_array().cloned().unwrap_or_default().into();
    Ok(selected)
}

pub fn cmd_agent_config(s: &mut Session, p: &Value) -> Result<Value> {
    let mut value=match &s.agent_selection{Some(v)=>v.clone(),None=>settings()?};
    if let Some(error)=value["selection_error"].as_str(){return Err(invalid(error));}
    if let Some(patch)=p.get("selection") {
        let fields=patch.as_object().ok_or_else(||invalid("Active selection must be an object"))?;
        if fields.keys().any(|k|!matches!(k.as_str(),"agent"|"provider"|"model"|"thinking"|"extra_args")){return Err(invalid("Only active chat selection can be changed; credentials remain untouched"));}
        for (key,v) in fields {
            if (key=="extra_args"&&!v.is_array()) || (key!="extra_args"&&(!v.is_string()||v.as_str().is_some_and(|x|x.len()>256||x.chars().any(char::is_control)))) {return Err(invalid("Invalid active chat selection"));}
            value[key]=v.clone();
        }
    }
    s.agent_selection=Some(value.clone());value["scope"]=json!("active engine/window; shared defaults read only at initialization");Ok(value)
}


fn selected(s:&mut Session, params: &Value) -> Result<Value> {
    let selection=cmd_agent_config(s,&json!({}))?;
    if params["agent"].as_str().is_some_and(|a|a!=selection["agent"].as_str().unwrap_or_default())
        || params.get("provider").is_some() || params.get("model").is_some() {
        return Err(invalid("Chat selection changed or overridden; choose provider/model in the app, then read agent.config. No request sent"));
    }
    if !matches!(selection["agent"].as_str(),Some("opencode"|"codex"|"omp"|"claude")) {
        return Err(invalid("Selected agent has no supported non-interactive adapter; no executable launched"));
    }
    if selection["extra_args"].as_array().is_some_and(|a|!a.is_empty()) {
        return Err(invalid("Extra chat arguments are not supported by the shared route; remove them in app settings. No selection/security override applied"));
    }
    Ok(selection)
}

/// Login and chat resolve the same installed OpenCode executable; never install a fallback.
pub fn opencode_program() -> Result<std::path::PathBuf> {
    let paths = std::env::var_os("PATH").ok_or_else(|| invalid("OpenCode unavailable: PATH is unset"))?;
    for directory in std::env::split_paths(&paths) {
        let path = directory.join("opencode");
        let Ok(metadata) = fs::metadata(&path) else { continue };
        if !metadata.is_file() { continue; }
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 { continue; }
        }
        return fs::canonicalize(path).map_err(Into::into);
    }
    Err(invalid("Installed OpenCode executable not found on PATH; no fallback installed"))
}

/// Never propagate raw child output into UI, journal or logs on failure: it can contain credentials.
fn run(dir:&Path, tool:&str, args:&[String]) -> Result<String> {
    let program = if tool == "opencode" { opencode_program()? } else { tool.into() };
    let reply=std::env::temp_dir().join(format!("ac-agent-{}-{}.reply",std::process::id(),stamp()));
    let mut options=fs::OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let file=options.open(&reply)?;
    let output=Command::new(program).current_dir(dir).args(args).stdout(file).output();
    let bytes=fs::read(&reply).unwrap_or_default();let _=fs::remove_file(&reply);
    let output=output.map_err(|_|invalid(format!("Cannot start selected agent `{tool}`; check its installed CLI locally. No fallback attempted")))?;
    if !output.status.success() {
        let detail=format!("{} {}",String::from_utf8_lossy(&output.stderr),String::from_utf8_lossy(&bytes)).to_lowercase();
        let cause=if ["401","403","unauthorized","authentication","api key","api_key"].iter().any(|s|detail.contains(s)) {
            "authentication rejected; inspect the existing agent login locally; credentials were not changed"
        } else {"agent command failed; inspect its local diagnostics; raw output withheld to protect credentials"};
        return Err(invalid(format!("{tool} exit {:?}: {cause}. No model/provider fallback attempted",output.status.code())));
    }
    String::from_utf8(bytes).map_err(|_|invalid("Agent returned invalid UTF-8; raw output withheld"))
}
fn api(dir:&Path, operation:&str, session:Option<&str>, body:Option<Value>) -> Result<Value> {
    let mut args=vec!["api".into(),operation.into()];
    if let Some(id)=session {args.extend(["--param".into(),format!("sessionID={id}")]);}
    if let Some(body)=body {args.extend(["-d".into(),body.to_string()]);}
    let raw=run(dir,"opencode",&args).map_err(|error|if operation=="session.prompt" {
        invalid(format!("{error}; prompt delivery outcome uncertain. Inspect existing OpenCode session {} before retrying; no duplicate prompt sent",session.unwrap_or("unknown")))
    } else {error})?;
    serde_json::from_str(&raw).map_err(|_|invalid(format!("OpenCode {operation} returned invalid JSON; raw output withheld")))
}
fn data(value:&Value)->&Value { value.get("data").unwrap_or(value) }
fn models(dir:&Path)->Result<Vec<Value>> {
    let response=api(dir,"model.list",None,None)?;
    let list=data(&response).as_array().ok_or_else(||invalid("OpenCode model.list did not return a model array; no inference attempted"))?;
    Ok(list.iter().map(|m|json!({"providerID":m["providerID"],"id":m["id"],"name":m["name"],"enabled":m["enabled"],"capabilities":m["capabilities"],"cost":m["cost"]})).collect())
}
pub fn cmd_agent_models(s:&mut Session, p: &Value)->Result<Value> {
    let mut selection=cmd_agent_config(s,&json!({}))?;
    if let Some(agent)=p["agent"].as_str(){selection["agent"]=json!(agent);}
    if selection["agent"]!="opencode" {return Ok(json!({"agent":selection["agent"],"models":[],"available":false,"reason":"Catalog discovery is supported by the existing OpenCode API route"}));}
    Ok(json!({"agent":"opencode","models":models(&crate::workshop::project_dir(s)?)?,"available":true}))
}
fn model_ref(dir:&Path,selection:&Value,free_only:bool)->Result<Value> {
    let provider=selection["provider"].as_str().unwrap_or_default();
    let model=selection["model"].as_str().unwrap_or_default();
    let wanted=if provider.is_empty() && model.is_empty() {
        data(&api(dir,"model.default",None,None)?).clone()
    } else if provider.is_empty() || model.is_empty() {
        return Err(invalid("OpenCode needs both provider and model IDs from app settings; no default fallback attempted"));
    } else {json!({"providerID":provider,"id":model})};
    let catalog=models(dir)?;
    let found=catalog.iter().find(|m|m["providerID"]==wanted["providerID"] && m["id"]==wanted["id"] && m["enabled"]==true)
        .ok_or_else(||invalid("Selected provider/model is absent or disabled in agent.models; choose an available model in the app. No fallback attempted"))?;
    if free_only {
        fn zero(v:&Value)->bool {v.as_f64()==Some(0.0) || v.as_str().and_then(|s|s.parse::<f64>().ok())==Some(0.0)}
        let costs=found["cost"].as_array().filter(|a|!a.is_empty()).ok_or_else(||invalid("Selected model cost is unknown; free-only request refused before inference"))?;
        if !costs.iter().all(|c|zero(&c["input"])&&zero(&c["output"])&&zero(&c["cache"]["read"])&&zero(&c["cache"]["write"])) {
            return Err(invalid("Selected model has nonzero or unknown cost; free-only request refused before inference"));
        }
    }
    Ok(wanted)
}
fn state_dir(s:&Session)->Result<std::path::PathBuf> {Ok(crate::workshop::project_dir(s)?.join(".agent"))}
fn chat_identity(s:&Session)->Result<Value>{
 let path=s.project_path.as_ref().ok_or_else(||invalid("Open the target project before chat"))?;
 let dir=crate::workshop::project_dir(s)?;
 let files=fs::read_dir(&dir)?.filter_map(std::result::Result::ok).filter(|e|e.path().extension().is_some_and(|v|v.eq_ignore_ascii_case("acproj"))).count();
 if files>1{return Err(invalid("Multiple project files share this chat directory; chat blocked to prevent cross-project messages. Give each project its own directory; history was preserved"));}
 let project=fs::canonicalize(path).unwrap_or_else(|_|path.clone());
 let owner=state_dir(s)?.join("chat-project.json");
 if let Ok(bytes)=fs::read(owner){let saved:Value=serde_json::from_slice(&bytes).map_err(|_|invalid("Chat project binding is invalid; no message sent"))?;if saved["project"]!=json!(project){return Err(invalid("Chat belongs to another project; no message sent or history changed"));}}
 Ok(json!(project))
}
fn request_file(s:&Session)->Result<std::path::PathBuf>{Ok(state_dir(s)?.join("chat-request.json"))}
fn request_read(s:&Session)->Value{request_file(s).ok().and_then(|p|fs::read(p).ok()).and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)}
fn request_write(s:&Session,v:&Value)->Result<()>{fs::create_dir_all(state_dir(s)?)?;fs::write(request_file(s)?,serde_json::to_vec_pretty(v).map_err(|_|invalid("Cannot encode chat receipt"))?)?;Ok(())}
fn refresh_reply(s:&Session)->Result<Value>{
 chat_identity(s)?;let mut r=request_read(s);
 if !matches!(r["status"].as_str(),Some("running"|"stalled")){return Ok(r);}
 let previous_status=r["status"].clone();
 let dir=crate::workshop::project_dir(s)?;let id=r["session"].as_str().ok_or_else(||invalid("Pending chat has no session"))?;
 let reply=api(&dir,"session.message.list",Some(id),None)?;
 let messages=data(&reply).as_array().ok_or_else(||invalid("OpenCode message list unavailable; pending request preserved"))?;
 let since=r["sent_at"].as_u64().ok_or_else(||invalid("Pending chat has no native prompt receipt; inspect existing session, do not resend"))?;
 let recent:Vec<_>=messages.iter().filter(|m|m["time"]["created"].as_u64().is_some_and(|t|t>=since)).collect();
 let permissions=api(&dir,"session.permission.list",Some(id),None)?;let forms=api(&dir,"session.form.list",Some(id),None)?;
 let pending=data(&permissions).as_array().is_some_and(|a|!a.is_empty())||data(&forms).as_array().is_some_and(|a|!a.is_empty());
 let text=recent.iter().filter(|m|m["type"]=="assistant").flat_map(|m|m["content"].as_array().into_iter().flatten()).filter(|v|v["type"]=="text").filter_map(|v|v["text"].as_str()).collect::<Vec<_>>().join("\n");
 if pending {r["status"]=json!("stalled");r["reason"]=json!("Existing OpenCode permission or question requires attention; no automatic approval");}
 else if recent.iter().any(|m|m["type"]=="idle") {
  if text.trim().is_empty(){r["status"]=json!("error");r["error"]=json!("OpenCode became idle without a retrievable text reply");}
  else {verify_model(&dir,id,&r["model_ref"])?;log_push(s,json!({"role":"assistant","text":text,"request_id":r["request_id"],"project":r["project"],"session":id,"agent":"opencode","provider":r["provider"],"model":r["model"]}))?;r["text"]=json!(text);r["status"]=json!("replied");}
 }
 r["checked_at"]=json!(stamp());request_write(s,&r)?;
 if s.record_user_state && previous_status!=r["status"]{crate::activity::append(json!({"stage":if r["status"]=="replied"{"engine_success"}else if r["status"]=="error"{"engine_error"}else{"agent_progress"},"command":"agent.send","request_id":r["request_id"],"project":r["project"],"project_id":r["project"],"command_id":r["request_id"],"status":r["status"],"outputs":{"status":r["status"],"session":r["session"]},"ui_ack":"reply observed; Studio rendering acknowledgement separate"}));}
 Ok(r)
}
fn log_file(s:&Session)->Result<std::path::PathBuf> {Ok(state_dir(s)?.join("agent-log.json"))}
pub(crate) fn log_read(s:&Session)->Value {log_file(s).ok().and_then(|p|fs::read(p).ok()).and_then(|b|serde_json::from_slice(&b).ok()).filter(Value::is_array).unwrap_or_else(||json!([]))}
fn log_push(s:&Session,entry:Value)->Result<()> {
    let file=log_file(s)?;fs::create_dir_all(state_dir(s)?)?;
    let mut turns=log_read(s);if let Some(list)=turns.as_array_mut(){list.push(entry);}
    fs::write(file,serde_json::to_vec_pretty(&turns).map_err(|_|invalid("Cannot serialize chat log"))?)?;Ok(())
}
fn archive(s:&Session)->Result<()> {
    let file=log_file(s)?;
    if file.exists(){fs::rename(&file,file.with_file_name(format!("agent-log.{}.json",stamp())))?;}
    Ok(())
}
fn verify_model(dir:&Path,id:&str,wanted:&Value)->Result<()> {
    let got=api(dir,"session.get",Some(id),None)?;
    if data(&got)["model"]["id"]!=wanted["id"] || data(&got)["model"]["providerID"]!=wanted["providerID"] {
        return Err(invalid("OpenCode session model differs from the app selection; no prompt sent or fallback attempted"));
    } Ok(())
}
fn open_session(s:&Session,wanted:&Value,fresh:bool)->Result<String> {
    let project=chat_identity(s)?;let dir=crate::workshop::project_dir(s)?;let state=state_dir(s)?;let file=state.join("opencode-session");
    if !fresh {
        let id=fs::read_to_string(&file).map_err(|_|invalid("No chat to continue; use agent.new_chat"))?;
        verify_model(&dir,id.trim(),wanted)?;return Ok(id.trim().into());
    }
    let response=api(&dir,"session.create",None,Some(json!({"title":"AnimationCraft fresh chat","model":wanted})))?;
    let id=data(&response)["id"].as_str().filter(|id|id.starts_with("ses_")&&!id.chars().any(char::is_control)).ok_or_else(||invalid("OpenCode did not return a valid session ID; no prompt sent"))?;
    verify_model(&dir,id,wanted)?;
    fs::create_dir_all(&state)?;archive(s)?;
    if let Ok(previous)=fs::read_to_string(&file) {
        use std::io::Write;
        let mut history=fs::OpenOptions::new().create(true).append(true).open(state.join("chat-history.jsonl"))?;
        writeln!(history,"{}",json!({"session":previous.trim(),"replaced_by":id}))?;
    }
    fs::write(state.join("chat-project.json"),json!({"project":project}).to_string())?;
    fs::write(file,id)?;Ok(id.into())
}
pub fn cmd_agent_log(s:&mut Session,p:&Value)->Result<Value> {chat_identity(s)?;if p["refresh"]==true{refresh_reply(s)?;}Ok(log_read(s))}
pub fn cmd_agent_session(s:&mut Session,_:&Value)->Result<Value> {
    let project=chat_identity(s)?;
    let session=fs::read_to_string(state_dir(s)?.join("opencode-session")).ok().map(|s|s.trim().to_owned());
    let selection=cmd_agent_config(s,&json!({}))?;Ok(json!({"project":project,"session":session,"selection":selection,"request":request_read(s),"runtime_model_verified":false}))
}
pub fn cmd_agent_new_chat(s:&mut Session,params:&Value)->Result<Value> {
    chat_identity(s)?;
    let selection=selected(s,params)?;let dir=crate::workshop::project_dir(s)?;
    if matches!(request_read(s)["status"].as_str(),Some("queued"|"running"|"stalled")){return Err(invalid("Existing chat request is pending; retrieve it before starting a new chat"));}
    let session=if selection["agent"]=="opencode" {
        let model=model_ref(&dir,&selection,params["free_only"]==true)?;
        Some(open_session(s,&model,true)?)
    } else {archive(s)?;None};
    Ok(json!({"ok":true,"session":session,"fresh":true,"turns":[],"selection":selection,"inference_started":false}))
}
/// Compatibility entry point; use the registered handler rather than a separate chat implementation.
pub fn new_chat(s:&mut Session,p:&Value)->Result<Value> {cmd_agent_new_chat(s,p)}

pub fn cmd_agent_send(s:&mut Session,params:&Value)->Result<Value> {
    chat_identity(s)?;
    let dir=crate::workshop::project_dir(s)?;let selection=selected(s,params)?;
    let agent=selection["agent"].as_str().ok_or_else(||invalid("No chat agent selected"))?;
    let mut said=params["message"].as_str().unwrap_or_default().trim().to_string();
    for path in params["attachments"].as_array().into_iter().flatten().filter_map(Value::as_str) {said.push_str(&format!("\n[AnimationCraft picture: {path}]"));}
    if said.is_empty(){return Err(invalid("message is empty"));}
    let instructions=fs::read_to_string(crate::workspace::workspace_root(s).join("apps/animationcraft-shell/agent.md")).unwrap_or_default();
    let fresh=params["new_chat"].as_bool().unwrap_or(true);
    if fresh && params["session"].is_string(){return Err(invalid("An expected session requires new_chat=false; no chat created or message sent"));}
    let prompt=format!("Freshness: discover current animationcraft-cli help, commands and actual project state before acting. Do not infer tools or project state from an earlier chat. Current project: {}.\n\n{instructions}\n\n{said}",s.project_path.as_deref().unwrap_or(&dir).display());
    if agent=="opencode" {
        if !selection["thinking"].as_str().unwrap_or_default().is_empty(){return Err(invalid("OpenCode reasoning variants are not wired to this setting; clear Thinking rather than silently ignoring it"));}
        let prior=request_read(s);if matches!(prior["status"].as_str(),Some("queued"|"running"|"stalled")){return Err(invalid("A project chat request is still pending; read agent.log refresh=true before sending another message"));}
        let model=model_ref(&dir,&selection,params["free_only"]==true)?;
        let id=open_session(s,&model,fresh)?;
        if params["session"].as_str().is_some_and(|expected|expected!=id){return Err(invalid("Project agent session changed; inspect agent.session before sending. No prompt sent"));}
        let request_id=params["_request_id"].as_str().map(str::to_owned).unwrap_or_else(||format!("chat-{}",stamp()));
        let mut receipt=json!({"request_id":request_id,"project":chat_identity(s)?,"session":id,"provider":model["providerID"],"model":model["id"],"model_ref":model,"status":"queued","queued_at":stamp()});request_write(s,&receipt)?;
        log_push(s,json!({"role":"user","text":said,"request_id":request_id,"project":receipt["project"],"session":id}))?;
        let sent=match api(&dir,"session.prompt",Some(&id),Some(json!({"text":prompt}))){Ok(v)=>v,Err(e)=>{receipt["status"]=json!("error");receipt["error"]=json!(e.to_string());receipt["delivery"]=json!("uncertain; inspect existing native session before retry");request_write(s,&receipt)?;return Err(e);}};
        let since=data(&sent)["time"]["created"].as_u64().ok_or_else(||invalid("OpenCode prompt receipt lacks creation time; pending request retained, do not resend"))?;
        receipt["status"]=json!("running");receipt["sent_at"]=json!(since);receipt["native_message_id"]=data(&sent)["id"].clone();request_write(s,&receipt)?;
        let wait=params["wait"].as_u64().unwrap_or(300).min(300);let start=Instant::now();
        loop {
            let r=if wait==0 {receipt.clone()}else{refresh_reply(s)?};
            if wait==0 || matches!(r["status"].as_str(),Some("replied"|"stalled"|"error")) {
                let mut result=r.clone();result["ok"]=json!(matches!(r["status"].as_str(),Some("running"|"replied")));result["fresh"]=json!(fresh);result["turns"]=log_read(s);result["next_command"]=json!("agent.log refresh=true; agent.session reports request state");return Ok(result);
            }
            if start.elapsed()>=Duration::from_secs(wait){return Err(invalid(format!("Reply still running after {wait}s; retrieve with agent.log refresh=true. Request {request_id}; no duplicate prompt sent")));}
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    if params["free_only"]==true {return Err(invalid("Free-only cost verification is unavailable for this agent; no inference attempted"));}
    if !selection["provider"].as_str().unwrap_or_default().is_empty(){return Err(invalid("Explicit provider selection is verified only for the OpenCode API route; no provider override attempted"));}
    let model=selection["model"].as_str().unwrap_or_default();let thinking=selection["thinking"].as_str().unwrap_or_default();
    let mut args=match agent {
        "codex"=>vec!["exec".into(),"--skip-git-repo-check".into(),"-s".into(),"workspace-write".into(),"-C".into(),dir.to_string_lossy().into_owned()],
        "omp"=>vec!["-p".into(),"--cwd".into(),dir.to_string_lossy().into_owned(),"--append-system-prompt".into(),instructions.clone()],
        "claude"=>vec!["-p".into(),"--permission-mode".into(),"auto".into(),"--append-system-prompt".into(),instructions],
        _=>return Err(invalid("Unsupported chat agent")),
    };
    if !model.is_empty(){args.extend(["--model".into(),model.into()]);}
    if !thinking.is_empty() {
        match agent {"codex"=>args.extend(["-c".into(),format!("model_reasoning_effort={}",json!(thinking))]),"omp"=>args.extend(["--thinking".into(),thinking.into()]),_=>return Err(invalid("Thinking override is unsupported for this agent; no inference attempted"))}
    }
    // No adapter implicitly resumes. OpenCode alone supports explicit continuation with model checks.
    if !fresh{return Err(invalid("Explicit continuation is supported only for OpenCode; use a fresh chat"));}
    args.push(prompt);let text=run(&dir,agent,&args)?;archive(s)?;
    log_push(s,json!({"role":"user","text":said}))?;log_push(s,json!({"role":"assistant","text":text,"agent":agent}))?;
    Ok(json!({"ok":true,"agent":agent,"provider":null,"model":if model.is_empty(){Value::Null}else{json!(model)},"selection_source":"app settings; inherited values unverified","fresh":true,"text":text,"turns":log_read(s)}))
}
