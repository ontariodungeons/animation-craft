//! Magic Studio shell: one window holding the project view (a webview fed by
//! `magicstudio-engine`) and, as a tab, the project's video studio (HyperFrames or Remotion
//! Studio) in a second webview of the same window.
//!
//!   magicstudio-shell [<project.magic>] [<studio url>] [--view project|studio] [--open <block>|agent] [--control <port>]

use magicstudio_engine::Session;
use serde_json::{Value, json};
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowBuilder, WindowEvent};

fn default_animate_project() -> Option<String> {
    let dir = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))?
        .join("magicstudio")
        .join("animate");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("Character.magic");
    if !path.exists() {
        let doc = json!({
            "format": "magicstudio.project",
            "version": 1,
            "name": "Character",
            "assets": [],
            "workshop": { "audio": null, "draft_approved": true, "target": "hyperframes", "video_project": null }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&doc).ok()?).ok()?;
    }
    Some(path.to_string_lossy().into_owned())
}

/// The command line, for the project view on load: project and studio URL, plus `--view <project|studio>`
/// and `--open <song|lyrics|transcript|animate|agent>` to start on a given tab and block (or with the agent open).
fn launch_options() -> Value {
    let (mut positional, mut view, mut open, mut character) = (Vec::new(), None, None, None);
    let (mut new_project, mut animate_only) = (false, false);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--new" => new_project=true,
            "--animate" | "-a" => {
                animate_only = true;
                if view.is_none() {
                    view = Some("animation".to_string());
                }
            }
            "--character" => character = args.next(),
            "--view" => view = args.next(),
            "--open" => open = args.next(),
            "--control" => {
                args.next();
            }
            _ => positional.push(a),
        }
    }
    let project = positional.first().cloned().or_else(|| {
        if animate_only && !new_project {
            default_animate_project()
        } else {
            None
        }
    });
    json!({"project": project, "studio": positional.get(1), "view": view, "open": open, "new_project":new_project, "animate_only": animate_only, "character": character})
}

#[derive(Default)]
struct ControlPort(std::sync::Mutex<Option<u16>>);
pub fn studio_identity(app: &tauri::AppHandle) -> Value {
    json!({"application":"Magic Studio","frontend":"Tauri","pid":std::process::id(),"executable":std::env::current_exe().ok(),"version":env!("CARGO_PKG_VERSION"),"build_id":env!("MAGICSTUDIO_BUILD_ID"),"branch":env!("MAGICSTUDIO_BUILD_BRANCH"),"agent_pane":std::env::var("HERDR_PANE_ID").ok(),"control_port":app.state::<ControlPort>().0.lock().ok().and_then(|v|*v)})
}
#[tauri::command]
fn startup(app:tauri::AppHandle) -> Value {
    let mut start=launch_options();start["identity"]=studio_identity(&app);start
}

mod bridge;
mod feedback;
mod menu;
mod project_dispatch;

/// The engine session of the open project: one for the window, shared by the page's commands and
/// the `--control` bridge (the CLI and MCP), like the session of the older UI.
pub struct Engine(pub std::sync::Mutex<Session>);
impl Default for Engine{fn default()->Self{let mut s=Session::user_session();if let Err(e)=magicstudio_engine::agent::cmd_agent_config(&mut s,&json!({})){s.agent_selection=Some(json!({"selection_error":e.to_string()}));}Self(std::sync::Mutex::new(s))}}

impl Engine {
    /// Runs `f` on the session with `path` open (opened first when another project is).
    pub fn with<T>(&self, path: Option<&str>, f: impl FnOnce(&mut Session) -> Result<T, String>) -> Result<T, String> {
        let mut s = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(path) = path
            && s.project_path.as_deref() != Some(std::path::Path::new(path))
        {
            return Err("Different workspace requested: use the guarded project.open command explicitly; current Studio state was preserved.".into());
        }
        f(&mut s)
    }
}

/// The open project's workshop state and storyboard.
pub fn state_of(s: &mut Session) -> Result<Value, String> {
    let status = s.execute("workshop.status", json!({})).map_err(|e| e.to_string())?;
    let storyboard = s.execute("workshop.storyboard", json!({})).unwrap_or(Value::Null);
    let assets=s.execute("project.list_assets",json!({})).map_err(|e|e.to_string())?;
    let design=match s.execute("storyboard.design",json!({})) {
        Ok(design)=>design,
        Err(error)=>json!({"placements":[],"text_fields":[],"error":error.to_string(),
            "readiness":{"state":"failed","reason":format!("Cannot read the editable storyboard: {error}")}}),
    };
    let monitoring=s.execute("center.status",json!({})).map_err(|e|e.to_string())?;
    let storyboard_contents=s.execute("storyboard.read",json!({})).unwrap_or(Value::Null);
    let storyboard_template=s.execute("storyboard.template",json!({})).map_err(|e|e.to_string())?;
    Ok(json!({"storyboard_template":storyboard_template,"storyboard_contents":storyboard_contents,"status": status, "storyboard": storyboard,"assets":assets,"design":design,"monitoring":monitoring}))
}

/// The project's workshop state and storyboard, straight from the engine.
#[tauri::command]
fn project_state(engine: tauri::State<Engine>, path: String) -> Result<Value, String> {
    engine.with(Some(&path), state_of)
}

/// Execute the shared engine registry without blocking the UI thread.
async fn execute_engine_command(app: tauri::AppHandle, id: String, params: Value) -> Result<Value, String> {
    let long = magicstudio_engine::find_command(&id).is_some_and(|c| c.long_running);
    // Image generation attaches an asset and saves the project. Keep that mutation serialized;
    // merging an old background snapshot could overwrite concurrent asset edits.
    if !long || id == "image.generate" {
        return tauri::async_runtime::spawn_blocking(move || {
            app.state::<Engine>().with(None, |s| s.execute(&id, params).map_err(|e| e.to_string()))
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    // Exports, renders and agent runs take minutes. They work on a snapshot of the session so the
    // window keeps answering (clicks, look, other engine commands); their changes merge back after.
    let (base, mut session) = {
        let engine = app.state::<Engine>();
        let shared = engine.0.lock().map_err(|e| e.to_string())?;
        let base = Session {
            cwd: shared.cwd.clone(),
            assets: shared.assets.clone(),
            project_assets: shared.project_assets.clone(),
            project_path: shared.project_path.clone(),
            video_project_path: shared.video_project_path.clone(),
            song_audio: shared.song_audio.clone(),
            draft_approved: shared.draft_approved,
            video_target: shared.video_target.clone(),
            active_asset: shared.active_asset.clone(),
            ..Session::default()
        };
        (base, shared.clone())
    };
    let (finished, result) = tauri::async_runtime::spawn_blocking(move || {
        let result = session.execute(&id, params).map_err(|e| e.to_string());
        (session, result)
    })
    .await
    .map_err(|e| e.to_string())?;
    {
        let engine = app.state::<Engine>();
        let mut shared = engine.0.lock().map_err(|e| e.to_string())?;
        merge_back(&mut shared, &base, &finished);
    }
    result
}

/// Writes back only what a background command changed relative to `base`, the copy it started
/// from, so edits the window made meanwhile survive. Assets and project assets merge by id.
fn merge_back(shared: &mut Session, base: &Session, finished: &Session) {
    macro_rules! differ { ($a:expr, $b:expr) => { serde_json::to_value($a).ok() != serde_json::to_value($b).ok() }; }
    for (id, asset) in &finished.assets {
        if base.assets.get(id).is_none_or(|old| differ!(old, asset)) { shared.assets.insert(id.clone(), asset.clone()); }
    }
    for id in base.assets.keys().filter(|id| !finished.assets.contains_key(*id)) { shared.assets.remove(id); }
    for asset in &finished.project_assets {
        if base.project_assets.iter().find(|old| old.id == asset.id).is_none_or(|old| differ!(old, asset)) {
            match shared.project_assets.iter_mut().find(|slot| slot.id == asset.id) {
                Some(slot) => *slot = asset.clone(),
                None => shared.project_assets.push(asset.clone()),
            }
        }
    }
    shared.project_assets.retain(|asset| {
        !base.project_assets.iter().any(|old| old.id == asset.id) || finished.project_assets.iter().any(|new| new.id == asset.id)
    });
    macro_rules! scalar { ($($field:ident),*) => { $(if finished.$field != base.$field { shared.$field = finished.$field.clone(); })* }; }
    scalar!(project_path, active_asset, draft_approved, video_target, video_project_path, song_audio);
    shared.last_result = finished.last_result.clone();
}

#[tauri::command]
async fn engine_command(app:tauri::AppHandle,id:String,mut params:Value)->Result<Value,String>{
    feedback::guard(&app,&id,true)?;
    if params.is_object(){params["_origin"]=json!("UI control");}
    if params["_request_id"].is_null(){params["_request_id"]=json!(feedback::new_request());}
    let request_id=params["_request_id"].as_str().map(str::to_owned);
    let mut result=execute_engine_command(app.clone(),id.clone(),params).await?;
    if magicstudio_engine::find_command(&id).is_some_and(|c|!c.journal){return Ok(result);}
    let revision=feedback::command_finished(&app,&id,&result,request_id.as_deref());
    if result.is_object(){result["_ui_engine_revision"]=json!(revision);}
    Ok(result)
}

/// Recent projects are navigation only; they never select the production root.
#[tauri::command]
fn recent_projects(engine:tauri::State<Engine>)->Result<Value,String>{engine.with(None,|s|s.execute("project.catalog",json!({})).map(|v|v["projects"].clone()).map_err(|e|e.to_string()))}

/// Takes a project off Recent (the files are untouched).
#[tauri::command]
fn recent_forget(path:String)->Result<(),String>{magicstudio_engine::project_catalog::forget(&path)}

/// Shared production setting; navigation history never selects a new song's location.
#[tauri::command]
fn projects_root() -> Result<String, String> {
    magicstudio_engine::project_settings::root().map(|p|p.to_string_lossy().into_owned()).map_err(|e|e.to_string())
}

/// Dispatch directly through this Studio's guarded command path. The engine stays in
/// this window; the initiating frontend applies the returned state and acknowledges its revision.
#[tauri::command]
async fn project_open(app:tauri::AppHandle, path: String, request: Option<String>, control:Option<String>) -> Result<Value, String> {
    let request=request.unwrap_or_else(feedback::new_request);
    if let Some(result)=feedback::reuse_open(&app,&json!({"path":path,"_request_id":request,"_origin":"UI control"}))?{return Ok(result);}
    feedback::guard(&app,"project.open",true)?;
    tauri::async_runtime::spawn_blocking(move || project_dispatch::execute(&app,"project.open",json!({"path":path,"_client":"Studio user (unverified label)","_request_id":request,"_control":control}))).await.map_err(|e|e.to_string())?
}

/// File > New creates an independent window when this one already contains a document.
#[tauri::command]
async fn new_window() -> Result<Value,String> {
    tauri::async_runtime::spawn_blocking(|| {
        let exe=std::env::current_exe().map_err(|e|e.to_string())?;
        magicstudio_engine::studio_sessions::open(&exe,None)
    }).await.map_err(|e|e.to_string())?
}

/// New song uses the shared project.intake command and preserves its complete receipt.
#[tauri::command]
async fn project_create(app:tauri::AppHandle,name:String,audio:String,root:String,request:Option<String>,control:Option<String>)->Result<Value,String>{
 feedback::guard(&app,"project.intake",true)?;
 let request=request.unwrap_or_else(feedback::new_request);
 tauri::async_runtime::spawn_blocking(move || project_dispatch::execute(&app,"project.intake",json!({"root":root,"name":name,"audio":audio,"prepare_only":true,"_client":"Studio user (unverified label)","_request_id":request,"_control":control}))).await.map_err(|e|e.to_string())?
}

fn project_needs_auto_save(app: &tauri::AppHandle, project: &str, id: &str) -> Result<bool, String> {
 if id == "project.save" || !magicstudio_engine::find_command(id).is_some_and(|c| c.journal) {
  return Ok(false);
 }
 app.state::<Engine>().with(Some(project), |session| {
  let path = session.project_path.as_ref().ok_or("No project is open")?;
  let saved: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
  Ok(magicstudio_engine::project::project_is_dirty(session, &saved))
 })
}

/// Project workflow controls share guarded dispatch with CLI and MCP callers.
#[tauri::command]
async fn project_command(app:tauri::AppHandle, project:String, id:String, mut params:Value)->Result<Value,String>{
 if !params.is_object(){return Err("Command parameters must be an object".into());}
 if magicstudio_engine::find_command(&id).is_none(){return Err("Unknown project command".into());}
 let active=app.state::<Engine>().with(None,|s|Ok(s.project_path.clone()))?;
 if active.as_deref()!=Some(std::path::Path::new(&project)){return Err("The open project changed; select the intended project first".into());}
 params["_client"]=json!("Studio project workflow");
 tauri::async_runtime::spawn_blocking(move || {
  let mut result=project_dispatch::execute(&app,&id,params)?;
  if project_needs_auto_save(&app,&project,&id)? {
   let saved=project_dispatch::execute(&app,"project.save",json!({"_client":"Studio project workflow"}))?;
   result["_ui_engine_revision"]=saved["_ui_engine_revision"].clone();
  }
  result["project"]=json!(project);Ok(result)
 }).await.map_err(|e|e.to_string())?
}

/// The project folder of a `.magic` path.
fn project_dir(project: &str) -> Result<std::path::PathBuf, String> {
    std::path::Path::new(project).parent().map(std::path::Path::to_path_buf).ok_or_else(|| format!("bad project path: {project}"))
}

// ---- The in-app agent ---------------------------------------------------------------------
//
// The agent belongs to this window: the app stores its settings, supplies its instructions and
// runs it as a child process in the project folder. It needs no service, session or tool that
// someone started elsewhere.

/// Defaults and runtime instructions supplied by this app.
fn agent_defaults() -> Value {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = crate_dir.parent().and_then(std::path::Path::parent).unwrap_or(crate_dir);
    json!({
        "agent": "omp",
        "model": "",
        "provider": "",
        "thinking": "",
        "extra_args": [],
        "instructions": crate_dir.join("agent.md").to_string_lossy(),
        "repo": repo.to_string_lossy(),
    })
}

/// Agents Omarchy knows about; the app offers the installed ones and marks the system default.
const AGENT_NAMES: [&str; 15] = [
    "pi", "omp", "opencode", "ori", "claude", "codex", "grok", "openclaw", "agy", "hermes",
    "copilot", "crush", "cursor-agent", "muse", "kimi",
];

fn which(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(name)).find(|candidate| candidate.is_file())
}

/// The agent Omarchy launches by default (`omarchy default agent`).
fn omarchy_default_agent() -> Option<String> {
    magicstudio_engine::agent::default_agent()
}

/// The agents installed on this machine, with the one the system runs by default.
fn agent_catalog() -> Value {
    let default = omarchy_default_agent();
    let agents: Vec<Value> = AGENT_NAMES
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "installed": which(name).is_some(),
                "headless": matches!(*name, "opencode" | "omp" | "claude" | "codex"),
                "default": default.as_deref() == Some(*name),
            })
        })
        .collect();
    json!({"agents": agents, "default": default})
}

/// The settings the agent pane shows and edits, plus the installed agents to choose from.
#[tauri::command]
fn agent_config(engine:tauri::State<Engine>) -> Result<Value, String> {
    let mut settings = engine.with(None,|s|s.execute("agent.config",json!({})).map_err(|e|e.to_string()))?;
    settings["instructions"] = agent_defaults()["instructions"].clone();
    if let (Some(base), Value::Object(catalog)) = (settings.as_object_mut(), agent_catalog()) {
        for (key, value) in catalog {
            base.insert(key, value);
        }
    }
    Ok(settings)
}

/// Merge `patch` into the agent settings; returns the settings actually stored.
#[tauri::command]
fn agent_config_save(engine:tauri::State<Engine>,patch: Value) -> Result<Value, String> {
    let fields = patch.as_object().ok_or("Chat settings must be an object")?;
    if fields.get("agent").is_some_and(|v|!v.is_string()) { return Err("Chat agent must be a string".into()); }
    if fields.get("extra_args").is_some_and(|v|!v.is_array()) { return Err("Extra arguments must be an array".into()); }
    if fields.keys().any(|k| !matches!(k.as_str(), "agent" | "provider" | "model" | "thinking" | "extra_args")) {
        return Err("Only chat selection fields can be changed here; credentials and image settings are separate".into());
    }
    for key in ["provider", "model", "thinking"] {
        if let Some(value) = fields.get(key) {
            let value = value.as_str().ok_or("Chat selection fields must be strings")?;
            if value.len() > 256 || value.chars().any(char::is_control) { return Err("Invalid chat selection".into()); }
        }
    }
    if let Some(agent) = patch.get("agent").and_then(Value::as_str) {
        if !AGENT_NAMES.contains(&agent) {
            return Err(format!("unknown agent `{agent}`"));
        }
        if which(agent).is_none() {
            return Err(format!("`{agent}` is not installed on this machine"));
        }
    }
    engine.with(None,|s|s.execute("agent.config",json!({"selection":patch})).map_err(|e|e.to_string()))?;
    // The app's own switch is the user's choice: keep it for the next launch and the CLI.
    if let Err(error)=engine.with(None,|s|s.execute("agent.persist",json!({})).map_err(|e|e.to_string())){
        eprintln!("chat selection not persisted: {error}");
    }
    agent_config(engine)
}

/// Point: WebKit's snapshot of the project view cropped to `rect` (CSS px, x y w h), saved to
/// `<project>/.agent/points/`; returns the file.
#[tauri::command]
async fn snapshot(app: tauri::AppHandle, project: String, rect: [f64; 4]) -> Result<String, String> {
    let dir = project_dir(&project)?.join(".agent").join("points");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let out = dir.join(format!("point-{stamp}.png"));
    snapshot_to(&app, Some(rect), &out)?;
    Ok(out.to_string_lossy().into_owned())
}

/// WebKit's picture of the project view, cropped to `rect` (CSS px; None = all of it), as PNG.
pub fn snapshot_to(app: &tauri::AppHandle, rect: Option<[f64; 4]>, out: &std::path::Path) -> Result<(), String> {
    capture_view(app, "project", None, out)?;
    let studio_visible = app.state::<View>().0.lock().map(|v|*v=="studio").unwrap_or(false);
    #[cfg(target_os="linux")]
    if studio_visible && app.get_webview("studio").is_some() {
        let studio_path=out.with_extension("studio.png");
        capture_view(app,"studio",None,&studio_path)?;
        let scale=app.get_window("main").and_then(|w|w.scale_factor().ok()).unwrap_or(1.0);
        let mut base_file=std::fs::File::open(out).map_err(|e|e.to_string())?;
        let base=cairo::ImageSurface::create_from_png(&mut base_file).map_err(|e|e.to_string())?;
        let mut studio_file=std::fs::File::open(&studio_path).map_err(|e|e.to_string())?;
        let studio=cairo::ImageSurface::create_from_png(&mut studio_file).map_err(|e|e.to_string())?;
        let cr=cairo::Context::new(&base).map_err(|e|e.to_string())?;
        cr.set_source_surface(&studio,0.0,BAR*scale).map_err(|e|e.to_string())?;
        cr.paint().map_err(|e|e.to_string())?;
        let crop=rect.unwrap_or([0.0,0.0,base.width() as f64/scale,base.height() as f64/scale]);
        crop_png(&base,crop,scale,out)?;
    } else if let Some(rect)=rect { capture_view(app,"project",Some(rect),out)?; }
    Ok(())
}

fn capture_view(app: &tauri::AppHandle, label: &str, rect: Option<[f64;4]>, out: &std::path::Path) -> Result<(),String> {
    let view = app.get_webview(label).ok_or("no requested view")?;
    let scale = app.get_window("main").and_then(|w| w.scale_factor().ok()).unwrap_or(1.0);
    let out = out.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    let target = out.clone();
    view.with_webview(move |platform| {
        #[cfg(target_os = "linux")]
        {
            use webkit2gtk::WebViewExt;
            platform.inner().snapshot(
                webkit2gtk::SnapshotRegion::Visible,
                webkit2gtk::SnapshotOptions::NONE,
                None::<&gio::Cancellable>,
                move |res| {
                    let saved = res.map_err(|e| e.to_string()).and_then(|surface| {
                        let rect = rect.unwrap_or_else(|| {
                            let s = cairo::ImageSurface::try_from(surface.clone()).ok();
                            s.map_or([0.0, 0.0, 1.0, 1.0], |s| [0.0, 0.0, f64::from(s.width()) / scale, f64::from(s.height()) / scale])
                        });
                        crop_png(&surface, rect, scale, &target)
                    });
                    let _ = tx.send(saved);
                },
            );
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (platform, tx.send(Err("Point needs Linux for now".into())), &target, rect, scale);
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(10)).map_err(|_| "the snapshot did not arrive".to_string())?
}

#[cfg(target_os = "linux")]
fn crop_png(surface: &cairo::Surface, rect: [f64; 4], scale: f64, out: &std::path::Path) -> Result<(), String> {
    let [x, y, w, h] = rect.map(|v| v * scale);
    let img = cairo::ImageSurface::create(cairo::Format::ARgb32, w.round().max(1.0) as i32, h.round().max(1.0) as i32).map_err(|e| e.to_string())?;
    let cr = cairo::Context::new(&img).map_err(|e| e.to_string())?;
    cr.set_source_surface(surface, -x, -y).map_err(|e| e.to_string())?;
    cr.paint().map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    img.write_to_png(&mut file).map_err(|e| e.to_string())
}

/// A pasted or dropped picture for the agent, saved to `<project>/.agent/points/`; returns the file.
#[tauri::command]
fn save_attachment(request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let header = |k: &str| request.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let (Some(project), name) = (header("x-project"), header("x-name").unwrap_or_else(|| "image.png".into())) else {
        return Err("no project".into());
    };
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err("expected the image bytes".into()) };
    let dir = project_dir(&project)?.join(".agent").join("points");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let name = percent_decode(&name);
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
    let out = dir.join(format!("pasted-{stamp}-{safe}"));
    std::fs::write(&out, bytes).map_err(|e| e.to_string())?;
    Ok(out.to_string_lossy().into_owned())
}

/// Header values are ASCII; the file name arrives URI-encoded.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(v)) => { out.push(v); i += 3; }
            (c, _) => { out.push(c); i += 1; }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Opens a file with the desktop's default app (one click from any file link).
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    std::process::Command::new("xdg-open").arg(&path).spawn().map(|_| ()).map_err(|e| format!("xdg-open {path}: {e}"))
}

/// Height of the project view's top bar (logical px).
const BAR: f64 = 48.0;
/// Height of the shell's unified footer, kept clear below the Studio webview (logical px).
const FOOTER: f64 = 30.0;

/// Which native view is showing.
struct View(std::sync::Mutex<String>);

impl Default for View {
    fn default() -> Self {
        Self(std::sync::Mutex::new("project".into()))
    }
}

/// Native webviews share one allocation. Studio occupies the content rectangle;
/// the shell remains full-height so its docked agent can sit beside Studio.
fn place_webview(view: &tauri::Webview, top: i32, right: i32, bottom: i32, is_studio: bool) {
    #[cfg(target_os = "linux")]
    let _ = view.with_webview(move |platform| {
        use gtk::prelude::*;
        let widget: gtk::Widget = platform.inner().upcast();
        if let Some(parent) = widget.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
            let overlay = parent.children().into_iter().find_map(|c|c.downcast::<gtk::Overlay>().ok()).unwrap_or_else(|| {
                let overlay=gtk::Overlay::new();
                parent.pack_start(&overlay,true,true,0);
                overlay.show();
                overlay
            });
            parent.remove(&widget);
            if is_studio { overlay.add_overlay(&widget); } else { overlay.add(&widget); }
        }
        widget.set_size_request(0,0);
        widget.set_hexpand(true); widget.set_vexpand(true);
        widget.set_halign(gtk::Align::Fill); widget.set_valign(gtk::Align::Fill);
        widget.set_margin_top(top); widget.set_margin_end(right); widget.set_margin_bottom(bottom);
    });
    #[cfg(not(target_os = "linux"))]
    let _ = (view, top, right, bottom, is_studio);
}

#[derive(Default)]
struct DrawerHeight(std::sync::Mutex<f64>);

#[tauri::command]
fn drawer_layout(app: tauri::AppHandle, height: f64) -> Result<(), String> {
    *app.state::<DrawerHeight>().0.lock().map_err(|e|e.to_string())?=height.max(0.0);
    layout(&app);
    Ok(())
}

fn layout(app: &tauri::AppHandle) {
    let (Some(window), Some(project)) = (app.get_window("main"), app.get_webview("project")) else { return };
    let (Ok(physical), Ok(scale)) = (window.inner_size(), window.scale_factor()) else { return };
    let size = physical.to_logical::<f64>(scale);
    let view = app.state::<View>().0.lock().map(|v|v.clone()).unwrap_or_else(|_|"project".into());
    let top=BAR+app.state::<DrawerHeight>().0.lock().map(|v|*v).unwrap_or(0.0).min((size.height-BAR-FOOTER).max(0.0));
    place_webview(&project,0,0,0,false);
    #[cfg(not(target_os="linux"))]
    { let _=project.set_position(LogicalPosition::new(0.0,0.0)); let _=project.set_size(size); }
    if let Some(studio)=app.get_webview("studio") {
        if view=="studio" {
            place_webview(&studio,top.round() as i32,0,FOOTER as i32,true);
            #[cfg(not(target_os="linux"))]
            { let _=studio.set_position(LogicalPosition::new(0.0,top)); let _=studio.set_size(LogicalSize::new(size.width,(size.height-top-FOOTER).max(0.0))); }
            let _=studio.show();
        } else { let _=studio.hide(); }
    }
}

/// Shows the contained project in HyperFrames Studio, creating its webview on first use.
#[tauri::command]
fn studio_show(app: tauri::AppHandle, url: String, mode: String) -> Result<(), String> {
    let window = app.get_window("main").ok_or("no main window")?;
    match app.get_webview("studio") {
        Some(view) => {
            if view.url().map(|u| u.as_str().trim_end_matches('/') != url.trim_end_matches('/')).unwrap_or(true) {
                view.navigate(url.parse().map_err(|e| format!("studio url: {e}"))?).map_err(|e| e.to_string())?;
            }
        }
        None => {
            let url = WebviewUrl::External(url.parse().map_err(|e| format!("studio url: {e}"))?);
            window
                .add_child(WebviewBuilder::new("studio", url).initialization_script(include_str!("../ui/studio-caption-compat.js")), LogicalPosition::new(0.0, BAR), LogicalSize::new(1.0, 1.0))
                .map_err(|e| e.to_string())?;
        }
    }
    let state = app.state::<View>();
    if !matches!(mode.as_str(),"studio"|"split"){return Err("Studio view requires mode=studio".into());}
    *state.0.lock().map_err(|e| e.to_string())? = "studio".into();
    layout(&app);
    Ok(())
}

/// Back to the project view alone (the studio keeps its state for next time).
#[tauri::command]
fn studio_hide(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<View>();
    *state.0.lock().map_err(|e| e.to_string())? = "project".into();
    layout(&app);
    Ok(())
}

fn run_headless() -> i32 {
    use std::io::BufRead;
    let args: Vec<String> = std::env::args().skip(2).collect();
    if args.first().map(String::as_str) == Some("commands") {
        let specs: Vec<Value> = magicstudio_engine::command_specs().iter().map(|s| s.describe()).collect();
        println!("{}", serde_json::to_string_pretty(&specs).unwrap_or_default());
        return 0;
    }
    let mut s = Session::user_session();
    if args.first().map(String::as_str) == Some("run") {
        let dry_run = args.iter().any(|a| a == "--dry-run");
        let stdin = std::io::stdin();
        let mut failed = 0;
        for (n, line) in stdin.lock().lines().map_while(Result::ok).enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(trimmed) else {
                println!("{}", json!({"line": n + 1, "ok": false, "error": "invalid json"}));
                failed += 1;
                continue;
            };
            let id = v["id"]
                .as_str()
                .or_else(|| v["command"].as_str())
                .unwrap_or("");
            let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
            match headless_dispatch(&mut s, id, params, dry_run) {
                Ok(r) => println!("{}", json!({"line": n + 1, "id": id, "ok": true, "result": r})),
                Err(e) => {
                    println!("{}", json!({"line": n + 1, "id": id, "ok": false, "error": e}));
                    failed += 1;
                }
            }
        }
        return if failed == 0 { 0 } else { 1 };
    }
    let (id, param_start) = if args.first().map(String::as_str) == Some("exec") {
        (args.get(1).map(String::as_str).unwrap_or(""), 2)
    } else {
        (args.first().map(String::as_str).unwrap_or(""), 1)
    };
    if id.is_empty() {
        eprintln!("magicstudio-shell --headless <command_id> [params_json | key=value ...]");
        return 2;
    }
    let mut params = json!({});
    let mut dry_run = false;
    for arg in args.iter().skip(param_start) {
        if arg == "--dry-run" {
            dry_run = true;
            continue;
        }
        if arg.starts_with('{')
            && let Ok(v) = serde_json::from_str::<Value>(arg)
        {
            params = v;
            continue;
        }
        if let Some((k, v)) = arg.split_once('=')
            && let Some(obj) = params.as_object_mut()
        {
            let parsed = serde_json::from_str::<Value>(v).unwrap_or_else(|_| json!(v));
            obj.insert(k.to_string(), parsed);
        }
    }
    match headless_dispatch(&mut s, id, params, dry_run) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            if v.get("ok").and_then(Value::as_bool) == Some(false) { 1 } else { 0 }
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}


fn headless_dispatch(s: &mut Session, id: &str, mut params: Value, dry_run: bool) -> Result<Value, String> {
    let is_dry = dry_run || params.get("_dry_run").and_then(Value::as_bool) == Some(true);
    if let Some(obj) = params.as_object_mut() {
        obj.remove("_dry_run");
    }
    if matches!(id, "project.open" | "project.identity" | "project.attach")
        && params["path"].is_null()
        && params["project"].is_string()
    {
        params["path"] = params["project"].clone();
    }
    if !matches!(
        id,
        "project.open" | "project.new" | "project.intake" | "project.attach" | "project.migrate" | "project.identity"
    ) && let Some(target) = params["project"].as_str()
    {
        let identity = s
            .execute("project.identity", json!({"path": target}))
            .map_err(|e| e.to_string())?;
        if let Some(project) = identity["project"].as_str()
            && s.project_path.as_deref() != Some(std::path::Path::new(project))
        {
            s.execute("project.attach", json!({"path": project}))
                .map_err(|e| e.to_string())?;
        }
    }
    if is_dry && !magicstudio_engine::commands::is_read_only_command(id, &params) {
        if magicstudio_engine::find_command(id).is_none() {
            return Err(format!("Unknown engine command: {id}"));
        }
        return Ok(json!({
            "ok": true,
            "dry_run": true,
            "command": id,
            "params": params,
            "project": s.project_path,
        }));
    }
    if s.assets.is_empty() && !s.project_assets.is_empty() && !id.starts_with("project.") {
        let wanted = params["asset"]
            .as_str()
            .or_else(|| params["character"].as_str())
            .map(str::to_owned)
            .or_else(|| {
                s.project_assets
                    .iter()
                    .find(|a| matches!(a.kind.as_str(), "character" | "cutout_rig"))
                    .or_else(|| s.project_assets.first())
                    .map(|a| a.id.clone())
            });
        if let Some(aid) = wanted {
            let _ = s.execute("project.load_asset", json!({"id": aid}));
        }
    }
    let out = s.execute(id, params).map_err(|e| e.to_string())?;
    if !is_dry
        && magicstudio_engine::find_command(id).is_some_and(|c| c.journal)
        && s.edits_since_save > 0
        && s.project_path.is_some()
    {
        magicstudio_engine::project::write_project_file(s).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--headless") {
        std::process::exit(run_headless());
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(View::default())
        .manage(DrawerHeight::default())
        .manage(ControlPort::default())
        .manage(Engine::default())
        .manage(project_dispatch::Pending::default())
        .manage(feedback::Feedback::default())
        .manage(bridge::Pending::default())
        .invoke_handler(tauri::generate_handler![startup, feedback::ui_report, engine_command, project_state, recent_projects, recent_forget, projects_root, project_open, new_window, project_create, project_command, agent_config, agent_config_save, snapshot, open_path, save_attachment, bridge::bridge_reply, studio_show, studio_hide, drawer_layout])
        .setup(|app| {
            #[cfg(unix)]
            if let Some(path)=launch_options()["project"].as_str() {
                if let Some(existing)=magicstudio_engine::studio_sessions::for_project(std::path::Path::new(path))? {
                    if let Some(endpoint)=existing["endpoint"].as_str().and_then(|v|v.strip_prefix("unix:")) {
                        magicstudio_engine::studio_sessions::request(std::path::Path::new(endpoint),"ui.focus",json!({"_expected_instance":existing["instance"],"_expected_project":path}))?;
                        app.handle().exit(0);return Ok(());
                    }
                }
            }
            let port=std::env::args().skip_while(|a|a!="--control").nth(1).map(|v|v.parse::<u16>()).transpose()?.unwrap_or(0);
            bridge::serve(app.handle().clone(),port)?;
            let window = WindowBuilder::new(app, "main").title("Magic Studio").inner_size(1440.0, 900.0).build()?;
            let size = window.inner_size()?.to_logical::<f64>(window.scale_factor()?);
            window.add_child(
                WebviewBuilder::new("project", WebviewUrl::App("index.html".into())),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(size.width, size.height),
            )?;
            if launch_options()["animate_only"] != true {
                menu::install(&window)?;
            }
            #[cfg(unix)]
            if let Err(e)=bridge::serve_local(app.handle().clone()){eprintln!("private UI feedback: {e}");}
            // Every resize re-applies the layout for the open tab.
            let handle = app.handle().clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested{api,..}=event {
                    if let Err(error)=feedback::guard(&handle,"app.quit",false){api.prevent_close();if let Some(view)=handle.get_webview("project"){let _=view.eval(format!("toast({});",json!(error)));}}
                    else{#[cfg(unix)] bridge::cleanup(&handle);}
                }
                if let WindowEvent::Resized(_) = event {
                    layout(&handle);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Magic Studio shell failed to start")
        .run(|app,event|{if matches!(event,tauri::RunEvent::Exit){#[cfg(unix)] bridge::cleanup(app);}});
}

#[cfg(test)]
mod tests {
    use super::*;
    use magicstudio_engine::session::{Asset, ProjectAsset};

    fn connection(id: &str, name: &str) -> ProjectAsset {
        serde_json::from_value(json!({"id":id,"name":name,"preset":null,"path":null,"kind":"image"})).unwrap()
    }

    #[test]
    fn background_merge_keeps_edits_made_while_it_ran() {
        let mut shared = Session::default();
        shared.assets.insert("fish".into(), Asset { id: "fish".into(), ..Asset::default() });
        shared.project_assets = vec![connection("fish", "Fish"), connection("crab", "Crab")];
        shared.video_target = "hyperframes".into();
        let base = shared.clone();

        // The window renames the crab and adds a bee while the command runs...
        shared.project_assets[1] = connection("crab", "Hermit crab");
        shared.project_assets.push(connection("bee", "Bee"));
        shared.active_asset = Some("bee".into());
        // ...and the command adds an eel, removes the fish and sets the video project.
        let mut finished = base.clone();
        finished.assets.clear();
        finished.assets.insert("eel".into(), Asset { id: "eel".into(), ..Asset::default() });
        finished.project_assets = vec![connection("crab", "Crab"), connection("eel", "Eel")];
        finished.video_project_path = Some("/song/hyperframes".into());

        merge_back(&mut shared, &base, &finished);
        let names: Vec<_> = shared.project_assets.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Hermit crab", "Bee", "Eel"]);
        assert_eq!(shared.assets.keys().collect::<Vec<_>>(), ["eel"]);
        assert_eq!(shared.active_asset.as_deref(), Some("bee"), "untouched by the command");
        assert_eq!(shared.video_target, "hyperframes");
        assert_eq!(shared.video_project_path.as_deref(), Some(std::path::Path::new("/song/hyperframes")));
    }
}
