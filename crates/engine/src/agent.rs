//! Project conversations are commands, shared by the desktop, CLI and MCP.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command, time::{SystemTime, UNIX_EPOCH}};

fn invalid(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }
fn stamp() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d|d.as_nanos()).unwrap_or(0) }

/// Read only chat settings from the same file the app edits. Never return credential fields.
/// The coding agent Omarchy runs by default.
///
/// Omarchy moved its command surface behind a command centre, so the stable
/// `omarchy-default-agent` binary is tried first and `omarchy default agent`
/// (a read-only query with no argument) is the fallback. A launch is never
/// triggered from here.
pub fn default_agent() -> Option<String> {
    let read = |output: std::process::Output| String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let legacy = Command::new("omarchy-default-agent")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(read)
        .filter(|s| !s.is_empty() && !s.contains(char::is_whitespace));
    legacy.or_else(|| {
        Command::new("omarchy")
            .args(["default", "agent"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(read)
            .filter(|s| !s.is_empty() && !s.contains(char::is_whitespace))
    })
}

pub fn settings() -> Result<Value> {
    let base=std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
        .or_else(||std::env::var_os("HOME").map(|h|std::path::PathBuf::from(h).join(".config")));
    let stored=if let Some(base)=base {
        match fs::read_to_string(base.join("magicstudio/agent.json")) {
            Ok(text)=>serde_json::from_str::<Value>(&text).map_err(|_|invalid("Invalid Magic Studio agent.json; repair existing settings without changing credentials"))?,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>json!({}),
            Err(_)=>return Err(invalid("Cannot read Magic Studio chat settings; no fallback agent selected")),
        }
    } else {json!({})};
    if stored.get("agent").is_some_and(|v|!v.is_string()) {return Err(invalid("Invalid saved agent; choose an installed chat agent in the app. No fallback selected"));}
    if stored.get("extra_args").is_some_and(|v|!v.is_array()) {return Err(invalid("Invalid saved extra_args; expected an array. No arguments ignored"));}
    let agent=stored["agent"].as_str().filter(|s|!s.trim().is_empty()).map(str::to_owned)
        .unwrap_or_else(||default_agent().unwrap_or_else(||"codex".into()));
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

/// Persist a repaired chat selection to the same settings file the app edits.
/// Every other key (including credentials) is preserved exactly.
fn persist_selection(selection:&Value)->Result<Value>{
    let base=std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
        .or_else(||std::env::var_os("HOME").map(|h|std::path::PathBuf::from(h).join(".config")))
        .ok_or_else(||invalid("Cannot locate the app settings directory; selection not persisted"))?;
    let path=base.join("magicstudio/agent.json");
    let mut stored=fs::read_to_string(&path).ok().and_then(|t|serde_json::from_str::<Value>(&t).ok()).unwrap_or_else(||json!({}));
    if !stored.is_object(){stored=json!({});}
    for key in ["agent","provider","model","thinking"]{
        if let Some(value)=selection[key].as_str(){stored[key]=json!(value);}
    }
    if let Some(extra)=selection["extra_args"].as_array(){
        stored["extra_args"]=json!(extra);
    }
    fs::create_dir_all(path.parent().ok_or_else(||invalid("App settings directory unavailable; selection not persisted"))?)?;
    fs::write(&path,serde_json::to_vec_pretty(&stored).map_err(|_|invalid("Cannot encode the app settings; selection not persisted"))?)
        .map_err(|_|invalid("Cannot write the app settings; selection not persisted"))?;
    Ok(stored)
}

/// Agents Omarchy knows about; the app offers the installed ones and marks the system default.
pub const AGENT_NAMES: [&str; 15] = [
    "pi", "omp", "opencode", "ori", "claude", "codex", "grok", "openclaw", "agy", "hermes",
    "copilot", "crush", "cursor-agent", "muse", "kimi",
];

fn which_program(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(name)).find(|candidate| candidate.is_file())
}

pub fn agent_catalog() -> Value {
    let default = default_agent();
    let agents: Vec<Value> = AGENT_NAMES
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "installed": which_program(name).is_some(),
                "headless": matches!(*name, "opencode" | "omp" | "claude" | "codex"),
                "default": default.as_deref() == Some(*name),
            })
        })
        .collect();
    json!({"agents": agents, "default": default})
}

/// A local runtime (Ollama, LM Studio) needs no account, no quota and no network.
fn is_local_provider(provider:&str)->bool{
    matches!(provider,"ollama"|"lmstudio"|"llamacpp"|"local")
}
fn model_is_free(model:&Value)->bool{
    if model["local"]==true{return true;}
    fn zero(v:&Value)->bool{v.as_f64()==Some(0.0)||v.as_str().and_then(|s|s.parse::<f64>().ok())==Some(0.0)}
    model["cost"].as_array().filter(|a|!a.is_empty()).map(|c|c.iter().all(|c|zero(&c["input"])&&zero(&c["output"])&&zero(&c["cache"]["read"])&&zero(&c["cache"]["write"]))).unwrap_or(false)
}
/// Known models for an installed agent, read from Omarchy usage state.
fn omarchy_agent_models(agent: &str) -> Option<Vec<Value>> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state")))?;
    let path = base.join(format!("omarchy/agents/usage/{agent}.json"));
    let bytes = fs::read(path).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    let obj = v["modelUsage"].as_object()?;
    let list: Vec<Value> = obj.keys().map(|k| {
        let local = is_local_provider(agent) || k.contains("local") || k.contains("ollama");
        let free = k.contains("free") || local;
        json!({
            "providerID": agent,
            "id": k,
            "name": k,
            "enabled": true,
            "local": local,
            "free": free,
            "cost": []
        })
    }).collect();
    (!list.is_empty()).then_some(list)
}

fn catalog_dir(s:&Session)->std::path::PathBuf {
    crate::workshop::project_dir(s).unwrap_or_else(|_|crate::workspace::workspace_root(s))
}

fn omp_local_models(dir:&Path)->Result<Vec<Value>> {
    let raw=run(dir,"omp",&["models".into(),"ollama".into(),"--json".into()])?;
    let response:Value=serde_json::from_str(&raw).map_err(|_|invalid("OMP returned an invalid local model catalog; no inference attempted"))?;
    let list=response["models"].as_array().ok_or_else(||invalid("OMP local model catalog is unavailable"))?;
    Ok(list.iter().filter(|m|m["provider"]=="ollama").map(|m|json!({"providerID":"ollama","id":m["id"],"name":m["name"],"enabled":true,"local":true,"free":true,"cost":[]})).collect())
}

fn discover_agent_catalog(s: &Session, agent: &str) -> Result<Vec<Value>> {
    let dir = catalog_dir(s);
    if agent == "omp" {
        omp_local_models(&dir)
    } else if agent == "opencode" {
        models(&dir).or_else(|_| {
            omarchy_agent_models(agent).ok_or_else(|| invalid("No models found for opencode"))
        })
    } else {
        omarchy_agent_models(agent).ok_or_else(|| {
            invalid(format!(
                "Catalog discovery for `{agent}`: no recorded models found in Omarchy usage state"
            ))
        })
    }
}

/// Sprocket's catalog: local first, then free, then paid. The order is the recommendation.
pub fn cmd_sprocket_models(s:&mut Session,_p:&Value)->Result<Value>{
    let selection=cmd_agent_config(s,&json!({}))?;
    let agent = selection["agent"].as_str().unwrap_or("opencode");
    let catalog = discover_agent_catalog(s, agent)?;
    let current=selection["model"].as_str().unwrap_or_default().to_string();
    let mut ranked:Vec<Value>=catalog.iter().filter(|m|m["enabled"]==true).cloned().collect();
    ranked.sort_by_key(|m|{
        let local=m["local"]==true;let free=model_is_free(m);
        (if local{0}else if free{1}else{2}, m["providerID"].as_str().unwrap_or_default().to_string(), m["id"].as_str().unwrap_or_default().to_string())
    });
    let list:Vec<Value>=ranked.iter().map(|m|json!({"provider":m["providerID"],"model":m["id"],"name":m["name"],"local":m["local"],"free":model_is_free(m),"current":m["id"].as_str()==Some(current.as_str())&&m["providerID"].as_str()==selection["provider"].as_str()})).collect();
    let recommended=list.iter().find(|m|m["current"]==true).or_else(||list.iter().find(|m|m["local"]==true)).or_else(||list.iter().find(|m|m["free"]==true)).or_else(||list.first());
    Ok(json!({"agent":agent,"models":list,"recommended":recommended,"current":{"provider":selection["provider"],"model":selection["model"]},"note":"Local models run on this machine with no quota; free models need no account credit."}))
}
/// Persist the active selection to the shared settings file. Explicit and separate from the
/// per-session selection: the engine never writes shared defaults on its own.
pub fn cmd_agent_persist(s:&mut Session,_p:&Value)->Result<Value>{
    let selection=cmd_agent_config(s,&json!({}))?;
    let stored=persist_selection(&selection)?;
    Ok(json!({"ok":true,"persisted":true,"selection":{"agent":selection["agent"],"provider":selection["provider"],"model":selection["model"]},"path":stored}))
}

/// Switch Sprocket's model, verified against the catalog and persisted for the app and the CLI.
pub fn cmd_sprocket_use(s:&mut Session,p:&Value)->Result<Value>{
    let wanted=p["model"].as_str().map(str::to_owned).or_else(||p["id"].as_str().map(str::to_owned)).ok_or_else(||invalid("Give model=<provider/id>, e.g. ollama/llama3.2:latest"))?;
    let selection_current = cmd_agent_config(s, &json!({}))?;
    let current_agent = selection_current["agent"].as_str().unwrap_or("opencode");
    let default_provider = selection_current["provider"].as_str().filter(|p| !p.is_empty()).unwrap_or(if current_agent == "omp" { "ollama" } else { current_agent });
    let (provider,id)=if let Some((p, i)) = wanted.split_once('/') {
        (p, i)
    } else {
        (default_provider, wanted.as_str())
    };
    let catalog = discover_agent_catalog(s, current_agent)?;
    let found=catalog.iter().find(|m|m["providerID"]==provider&&m["id"]==id&&m["enabled"]==true)
        .ok_or_else(||invalid(format!("{provider}/{id} is not an enabled model; read sprocket.models for the catalog")))?;
    let previous=cmd_agent_config(s,&json!({}))?;
    let selection=json!({"agent":current_agent,"provider":provider,"model":id});
    cmd_agent_config(s,&json!({"selection":selection.clone()}))?;
    persist_selection(&selection)?;
    Ok(json!({"ok":true,"selection":selection,"local":found["local"],"free":model_is_free(found),"persisted":true,"previous":{"provider":previous["provider"],"model":previous["model"]}}))
}
pub fn cmd_agent_config(s: &mut Session, p: &Value) -> Result<Value> {
    let mut value=match &s.agent_selection{Some(v)=>v.clone(),None=>settings()?};
    if p.get("selection").is_none() {
        if let Some(error)=value["selection_error"].as_str(){return Err(invalid(error));}
    }
    if let Some(patch)=p.get("selection") {
        let fields=patch.as_object().ok_or_else(||invalid("Active selection must be an object"))?;
        if fields.keys().any(|k|!matches!(k.as_str(),"agent"|"provider"|"model"|"thinking"|"extra_args")){return Err(invalid("Only active chat selection can be changed; credentials remain untouched"));}
        for (key,v) in fields {
            if (key=="extra_args"&&!v.is_array()) || (key!="extra_args"&&(!v.is_string()||v.as_str().is_some_and(|x|x.len()>256||x.chars().any(char::is_control)))) {return Err(invalid("Invalid active chat selection"));}
            value[key]=v.clone();
        }
        if let Some(agent)=fields.get("agent").and_then(Value::as_str) {
            if !AGENT_NAMES.contains(&agent) { return Err(invalid(format!("unknown agent `{agent}`"))); }
            if which_program(agent).is_none() { return Err(invalid(format!("`{agent}` is not installed on this machine"))); }
        }
        if let Some(obj)=value.as_object_mut(){obj.remove("selection_error");}
    }
    s.agent_selection=Some(value.clone());
    if p["persist"] == true {
        persist_selection(&value)?;
    }
    let root = crate::workspace::workspace_root(s);
    value["instructions"] = json!(root.join("apps/magicstudio-shell/agent.md").to_string_lossy());
    if let (Some(base), Value::Object(catalog)) = (value.as_object_mut(), agent_catalog()) {
        for (key, val) in catalog {
            base.insert(key, val);
        }
    }
    value["scope"]=json!("active engine/window; shared defaults read only at initialization");Ok(value)
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
        // Name-based launchers (e.g. mise shims) require the discovered argv[0].
        return Ok(path);
    }
    Err(invalid("Installed OpenCode executable not found on PATH; no fallback installed"))
}

/// Never propagate raw child output into UI, journal or logs on failure: it can contain credentials.
fn run(dir:&Path, tool:&str, args:&[String]) -> Result<String> {
    let _slot = crate::workspace::acquire_tool_slot();
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
    Ok(list.iter().map(|m|json!({"providerID":m["providerID"],"id":m["id"],"name":m["name"],"enabled":m["enabled"],"capabilities":m["capabilities"],"cost":m["cost"],"local":is_local_provider(m["providerID"].as_str().unwrap_or_default())})).collect())
}
/// Sprocket: the app's own in-app agent, reachable from the CLI and the booth.
/// Sending never changes the user's model/provider or persists a replacement.
/// The selected adapter validates availability and refuses unsupported selections.
pub fn cmd_sprocket_send(s:&mut Session, params:&Value)->Result<Value>{
    let selection=cmd_agent_config(s,&json!({}))?;
    let mut send=params.clone();
    if !send.get("free_only").is_some(){send["free_only"]=json!(true);}
    let mut result=cmd_agent_send(s,&send)?;
    result["sprocket"]=json!({"name":"Sprocket","role":"the app's own in-app agent","selection":selection,"selection_repaired":null,"selection_persisted":null});
    Ok(result)
}

pub fn cmd_agent_models(s:&mut Session, p: &Value)->Result<Value> {
    let mut selection=cmd_agent_config(s,&json!({}))?;
    if let Some(agent)=p["agent"].as_str(){selection["agent"]=json!(agent);}
    let agent = selection["agent"].as_str().unwrap_or("opencode");
    match discover_agent_catalog(s, agent) {
        Ok(m) => Ok(json!({"agent": agent, "models": m, "available": true})),
        Err(e) => Ok(json!({"agent": agent, "models": [], "available": false, "reason": e.to_string()})),
    }
}
fn state_dir(s:&Session)->Result<std::path::PathBuf> {Ok(crate::workshop::project_dir(s)?.join(".agent"))}
fn log_file(s:&Session)->Result<std::path::PathBuf> {Ok(state_dir(s)?.join("agent-log.json"))}
pub(crate) fn log_read(s:&Session)->Value {log_file(s).ok().and_then(|p|fs::read(p).ok()).and_then(|b|serde_json::from_slice(&b).ok()).filter(Value::is_array).unwrap_or_else(||json!([]))}
/// Write a file by rename, never in place: the live Studio and the CLI share these
/// paths, and an in-place write on a busy volume fails with EAGAIN.
fn write_atomic(path:&Path,bytes:&[u8])->Result<()>{
    let parent=path.parent().ok_or_else(||invalid("Chat state path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let temporary=parent.join(format!(".{}.{}.tmp",path.file_name().and_then(|n|n.to_str()).unwrap_or("chat"),stamp()));
    fs::write(&temporary,bytes)?;
    fs::rename(&temporary,path).map_err(|e|{let _=fs::remove_file(&temporary);invalid(format!("Cannot publish chat state: {e}"))})?;
    Ok(())
}

fn log_push(s:&Session,entry:Value)->Result<()> {
    let file=log_file(s)?;fs::create_dir_all(state_dir(s)?)?;
    let mut turns=log_read(s);if let Some(list)=turns.as_array_mut(){list.push(entry);}
    write_atomic(&file,&serde_json::to_vec_pretty(&turns).map_err(|_|invalid("Cannot serialize chat log"))?)?;Ok(())
}
/// Archive the previous transcript. Best-effort: the live Studio may hold the file open,
/// and keeping the old log is better than failing the request.
fn archive(s:&Session)->Result<()> {
    let file=log_file(s)?;
    if file.exists(){
        let _=fs::rename(&file,file.with_file_name(format!("agent-log.{}.json",stamp())));
    }
    Ok(())
}
// Native per-session policy for bounded Sproket tool/diagnostic tasks; never image generation.
fn bounded_task_cli() -> Result<std::path::PathBuf> {
    let exe=std::env::current_exe()?;
    let folder=exe.parent().ok_or_else(||invalid("Cannot locate the installed magic CLI"))?;
    for name in ["magic-cli"] {
        let candidate=folder.join(name);
        let candidate=if candidate.is_file(){candidate}else{folder.parent().unwrap_or(folder).join(name)};
        if let Ok(resolved) = fs::canonicalize(&candidate) {
            return Ok(resolved);
        }
    }
    let expected = folder.join("magic-cli");
    Err(invalid(format!("Bounded task needs the matching installed CLI at {}; no alternative installed",expected.display())))
}
pub fn cmd_agent_log(s:&mut Session,_p:&Value)->Result<Value> {Ok(log_read(s))}
pub fn cmd_agent_session(s:&mut Session,_:&Value)->Result<Value> {
    let selection=cmd_agent_config(s,&json!({}))?;
    Ok(json!({"project":s.project_path,"selection":selection,"turns":log_read(s)}))
}
pub fn cmd_agent_new_chat(s:&mut Session,params:&Value)->Result<Value> {
    let selection=selected(s,params)?;
    let _=archive(s);
    Ok(json!({"ok":true,"fresh":true,"turns":[],"selection":selection,"inference_started":false}))
}
/// Compatibility entry point; use the registered handler rather than a separate chat implementation.
pub fn new_chat(s:&mut Session,p:&Value)->Result<Value> {cmd_agent_new_chat(s,p)}

// One visible Markdown source, included once; never add a second system-prompt copy.
fn chat_prompt(project:&Path, instructions:&str, task:&str, message:&str)->String {
    format!("{task}Current project: {}.\n\n{instructions}\n\n{message}",project.display())
}

fn chat_args(agent:&str, dir:&Path)->Result<Vec<String>> {
    match agent {
        "codex"=>Ok(vec!["exec".into(),"--skip-git-repo-check".into(),"-s".into(),"workspace-write".into(),"-C".into(),dir.to_string_lossy().into_owned()]),
        "omp"=>Ok(vec!["-p".into(),"--cwd".into(),dir.to_string_lossy().into_owned()]),
        "claude"=>Ok(vec!["-p".into(),"--permission-mode".into(),"auto".into()]),
        "opencode"=>Ok(vec!["run".into()]),
        _=>Err(invalid("Unsupported chat agent")),
    }
}

pub fn cmd_agent_send(s:&mut Session,params:&Value)->Result<Value> {
    let dir=crate::workshop::project_dir(s)?;let selection=selected(s,params)?;
    let agent=selection["agent"].as_str().ok_or_else(||invalid("No chat agent selected"))?;
    let mut said=params["message"].as_str().unwrap_or_default().trim().to_string();
    for path in params["attachments"].as_array().into_iter().flatten().filter_map(Value::as_str) {said.push_str(&format!("\n[Magic Studio picture: {path}]"));}
    if said.is_empty(){return Err(invalid("message is empty"));}
    let instructions=fs::read_to_string(crate::workspace::workspace_root(s).join("apps/magicstudio-shell/agent.md"))
        .map_err(|_|invalid("Cannot read apps/magicstudio-shell/agent.md; restore or edit the Markdown instructions before sending. No prompt sent"))?;
    let fresh=params["new_chat"].as_bool().unwrap_or(false);
    let task=if params["image_task"]==true {format!("You are Sproket, the bounded tasks, tools, and error-sniffing assistant (never image generation; Gaffer handles image_gen). Use `{0} exec` diagnostic commands to inspect project state and UI errors, and report concise findings.\n",bounded_task_cli()?.display())}else{String::new()};
    let prompt=chat_prompt(s.project_path.as_deref().unwrap_or(&dir),&instructions,&task,&said);
    let provider=selection["provider"].as_str().unwrap_or_default();
    let chosen_model=selection["model"].as_str().unwrap_or_default();
    let local_omp=agent=="omp"&&provider=="ollama";
    if local_omp && !omp_local_models(&dir)?.iter().any(|m|m["id"]==chosen_model){return Err(invalid("Selected Ollama model is unavailable; choose an installed local model. No fallback or inference attempted"));}
    if params["free_only"]==true && !is_local_provider(provider) && agent!="opencode" {
        return Err(invalid("Free-only cost verification is unavailable for this agent; no inference attempted"));
    }
    let model=if (agent=="opencode" || local_omp) && !provider.is_empty() && !chosen_model.is_empty() && !chosen_model.contains('/') {
        format!("{provider}/{chosen_model}")
    } else {
        chosen_model.to_owned()
    };
    let thinking=selection["thinking"].as_str().unwrap_or_default();
    let mut args=chat_args(agent,&dir)?;
    if !model.is_empty(){args.extend(["--model".into(),model.clone()]);}
    if !thinking.is_empty() && thinking != "off" {
        match agent {"codex"=>args.extend(["-c".into(),format!("model_reasoning_effort={}",json!(thinking))]),"omp"=>args.extend(["--thinking".into(),thinking.into()]),_=>return Err(invalid("Thinking override is unsupported for this agent; no inference attempted"))}
    }
    if fresh { let _=archive(s); }
    args.push(prompt);
    let text=run(&dir,agent,&args)?;
    log_push(s,json!({"role":"user","text":said}))?;
    log_push(s,json!({"role":"assistant","text":text,"agent":agent,"provider":provider,"model":chosen_model}))?;
    Ok(json!({"ok":true,"agent":agent,"provider":if provider.is_empty(){Value::Null}else{json!(provider)},"model":if model.is_empty(){Value::Null}else{json!(model)},"local":is_local_provider(provider),"fresh":fresh,"text":text,"turns":log_read(s)}))
}
