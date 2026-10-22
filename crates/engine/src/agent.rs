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


// ---- The app's system agent, as commands ---------------------------------------------------
//
// The drawer in the desktop app is the first caller, but the whole turn lives here so the CLI and
// MCP can send the same message and read the same transcript.

/// The agent the app is configured with (`~/.config/animationcraft/agent.json`), else the Omarchy
/// system default, else codex.
pub(crate) fn configured_agent() -> String {
    let settings = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .map(|base| base.join("animationcraft").join("agent.json"));
    if let Some(path) = settings
        && let Ok(text) = std::fs::read_to_string(path)
        && let Ok(value) = serde_json::from_str::<Value>(&text)
        && let Some(agent) = value["agent"].as_str().filter(|a| !a.trim().is_empty())
    {
        return agent.to_string();
    }
    let default = std::process::Command::new("omarchy-default-agent")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|name| !name.is_empty());
    default.unwrap_or_else(|| "codex".to_string())
}

pub(crate) fn installed(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

fn log_file(s: &Session) -> Result<std::path::PathBuf> {
    let dir = crate::workshop::project_dir(s)?;
    Ok(dir.join(".agent").join("agent-log.json"))
}

pub(crate) fn log_read(s: &Session) -> Value {
    log_file(s)
        .ok()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_array)
        .unwrap_or_else(|| json!([]))
}

fn log_push(s: &Session, entry: Value) -> Result<()> {
    let file = log_file(s)?;
    std::fs::create_dir_all(file.parent().ok_or_else(|| EngineError::InvalidParams("no .agent folder".into()))?)?;
    let mut turns = log_read(s);
    if let Some(list) = turns.as_array_mut() {
        list.push(entry);
    }
    std::fs::write(&file, serde_json::to_string_pretty(&turns).map_err(|e| EngineError::InvalidParams(e.to_string()))?)?;
    Ok(())
}

/// `agent.log`: the project's conversation with the in-app agent.
pub fn cmd_agent_log(s: &mut Session, _: &Value) -> Result<Value> {
    Ok(log_read(s))
}

/// `agent.new_chat`: keep the old transcript beside the new one.
pub fn cmd_agent_new_chat(s: &mut Session, _: &Value) -> Result<Value> {
    let file = log_file(s)?;
    if file.exists() {
        std::fs::rename(&file, file.with_file_name("agent-log.prev.json"))?;
    }
    Ok(json!([]))
}

/// `agent.send`: one message to the configured system agent, run in the project folder with the
/// app's instructions. The same call the Agent drawer makes.
pub fn cmd_agent_send(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let agent = params
        .get("agent")
        .and_then(Value::as_str)
        .filter(|a| !a.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(configured_agent);
    if !installed(&agent) {
        return Err(EngineError::InvalidParams(format!("agent `{agent}` is not installed")));
    }
    let mut said = params.get("message").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    for path in params.get("attachments").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        said.push_str(&format!("\n[Pointed at something in AnimationCraft — picture: {path}]"));
    }
    if said.is_empty() {
        return Err(EngineError::InvalidParams("message is empty".into()));
    }
    let instructions = crate::workspace::workspace_root(s)
        .join("apps/animationcraft-shell/agent.md");
    let instructions = std::fs::read_to_string(instructions).unwrap_or_default();
    let new_chat = params.get("new_chat").and_then(Value::as_bool).unwrap_or(false);

    let args: Vec<String> = match agent.as_str() {
        "codex" => {
            let prompt = if instructions.trim().is_empty() {
                said.clone()
            } else {
                format!("{}\n\n---\n\n{said}", instructions.trim())
            };
            vec!["exec".into(), prompt, "--skip-git-repo-check".into(), "-s".into(), "workspace-write".into(),
                 "-C".into(), dir.to_string_lossy().into_owned()]
        }
        "claude" => {
            let mut args = vec!["-p".into(), said.clone(), "--permission-mode".into(), "auto".into()];
            if !instructions.trim().is_empty() {
                args.extend(["--append-system-prompt".into(), instructions.clone()]);
            }
            args
        }
        "omp" => {
            let mut args = vec!["-p".into(), said.clone(), "--cwd".into(), dir.to_string_lossy().into_owned()];
            if !instructions.trim().is_empty() {
                args.extend(["--append-system-prompt".into(), instructions.clone()]);
            }
            if !new_chat {
                args.push("--continue".into());
            }
            args
        }
        other => {
            return Err(EngineError::InvalidParams(format!(
                "the app runs omp, claude or codex non-interactively; `{other}` has no adapter yet"
            )));
        }
    };
    let text = crate::workspace::run_tool_in(&dir, &agent, &args)?;
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    log_push(s, json!({"role": "user", "text": said, "at": at}))?;
    log_push(s, json!({"role": "assistant", "text": text, "agent": agent, "at": at}))?;
    Ok(json!({"ok": true, "agent": agent, "text": text, "turns": log_read(s)}))
}
