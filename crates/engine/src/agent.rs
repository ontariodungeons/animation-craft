//! Project conversations are commands, shared by the desktop, CLI and MCP.
use crate::{EngineError, Result, Session};
use serde_json::{Value,json};

pub fn new_chat(s:&mut Session,params:&Value)->Result<Value>{
    let dir=crate::workshop::project_dir(s)?;
    let state=dir.join(".agent");std::fs::create_dir_all(&state)?;
    let file=state.join("opencode-session");
    let previous=std::fs::read_to_string(&file).ok().map(|v|v.trim().to_owned());
    let title=s.project_path.as_ref().and_then(|p|p.file_stem()).and_then(|v|v.to_str()).unwrap_or("Project");
    let body=json!({"title":format!("AnimationCraft · {title}")}).to_string();
    let raw=crate::workspace::run_tool_in(&dir,"opencode",&["api".into(),"session.create".into(),"-d".into(),body])?;
    let made:Value=serde_json::from_str(&raw).map_err(|e|EngineError::InvalidParams(e.to_string()))?;
    let id=made["data"]["id"].as_str().or_else(||made["id"].as_str()).ok_or_else(||EngineError::InvalidParams("OpenCode did not return a session id".into()))?;
    if params["model"].is_object(){
        crate::workspace::run_tool_in(&dir,"opencode",&["api".into(),"session.switchModel".into(),"--param".into(),format!("sessionID={id}"),"-d".into(),json!({"model":params["model"]}).to_string()])?;
    }
    if let Some(old)=&previous {
        use std::io::Write;
        let mut history=std::fs::OpenOptions::new().create(true).append(true).open(state.join("chat-history.jsonl"))?;
        writeln!(history,"{}",json!({"session":old,"replaced_by":id}))?;
    }
    let instructions=std::fs::read_to_string(crate::workspace::workspace_root(s).join("apps/animationcraft-shell/agent.md"))?;
    let context=format!("Read these AnimationCraft application instructions before doing any work. This is a fresh conversation. Current project: {}. Use the globally configured AnimationCraft MCP to discover the live session; do not assume a control port. Do not start tasks yet; reply only Ready.\n\n{}",dir.display(),instructions);
    crate::workspace::run_tool_in(&dir,"opencode",&["api".into(),"session.prompt".into(),"--param".into(),format!("sessionID={id}"),"-d".into(),json!({"text":context}).to_string()])?;
    std::fs::write(&file,id)?;
    Ok(json!({"ok":true,"session":id,"previous_session":previous,"project":s.project_path,"history":state.join("chat-history.jsonl")}))
}
