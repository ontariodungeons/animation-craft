//! AnimationCraft shell: one window holding the project view (a webview fed by
//! `animationcraft-engine`) and, as a tab, the project's video studio (HyperFrames or Remotion
//! Studio) in a second webview of the same window.
//!
//!   animationcraft-shell [<project.acproj>] [<studio url>] [--view project|studio] [--open <block>|agent] [--control <port>]

use animationcraft_engine::Session;
use serde_json::{Value, json};
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowBuilder, WindowEvent};

/// The command line, for the project view on load: project and studio URL, plus `--view <project|studio>`
/// and `--open <song|lyrics|transcript|animate|agent>` to start on a given tab and block (or with the agent open).
#[tauri::command]
fn startup() -> Value {
    let (mut positional, mut view, mut open) = (Vec::new(), None, None);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--view" => view = args.next(),
            "--open" => open = args.next(),
            "--control" => {
                args.next();
            }
            _ => positional.push(a),
        }
    }
    json!({"project": positional.first(), "studio": positional.get(1), "view": view, "open": open})
}

mod bridge;
mod feedback;
mod project_cli;

/// The engine session of the open project: one for the window, shared by the page's commands and
/// the `--control` bridge (the CLI and MCP), like the session of the older UI.
pub struct Engine(pub std::sync::Mutex<Session>);
impl Default for Engine{fn default()->Self{Self(std::sync::Mutex::new(Session::user_session()))}}

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
    let design=s.execute("storyboard.design",json!({})).unwrap_or(json!({"placements":[]}));
    let monitoring=s.execute("center.status",json!({})).map_err(|e|e.to_string())?;
    Ok(json!({"status": status, "storyboard": storyboard,"assets":assets,"design":design,"monitoring":monitoring}))
}

/// The project's workshop state and storyboard, straight from the engine.
#[tauri::command]
fn project_state(engine: tauri::State<Engine>, path: String) -> Result<Value, String> {
    engine.with(Some(&path), state_of)
}

/// Execute the shared engine registry without blocking the UI thread.
async fn execute_engine_command(app: tauri::AppHandle, id: String, params: Value) -> Result<Value, String> {
    let long = animationcraft_engine::find_command(&id).is_some_and(|c| c.long_running);
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
    let mut session = { app.state::<Engine>().0.lock().map_err(|e| e.to_string())?.clone() };
    let (finished, result) = tauri::async_runtime::spawn_blocking(move || {
        let result = session.execute(&id, params).map_err(|e| e.to_string());
        (session, result)
    })
    .await
    .map_err(|e| e.to_string())?;
    {
        let engine = app.state::<Engine>();
        let mut shared = engine.0.lock().map_err(|e| e.to_string())?;
        shared.assets = finished.assets;
        shared.project_assets = finished.project_assets;
        shared.project_path = finished.project_path;
        shared.active_asset = finished.active_asset;
        shared.last_result = finished.last_result;
        shared.draft_approved = finished.draft_approved;
        shared.video_target = finished.video_target;
        shared.song_audio = finished.song_audio;
    }
    result
}

#[tauri::command]
async fn engine_command(app:tauri::AppHandle,id:String,mut params:Value)->Result<Value,String>{
    feedback::guard(&app,&id,true)?;
    if params.is_object(){params["_origin"]=json!("UI control");}
    if params["_request_id"].is_null(){params["_request_id"]=json!(feedback::new_request());}
    let request_id=params["_request_id"].as_str().map(str::to_owned);
    let mut result=execute_engine_command(app.clone(),id.clone(),params).await?;
    if animationcraft_engine::find_command(&id).is_some_and(|c|!c.journal){return Ok(result);}
    let revision=feedback::command_finished(&app,&id,&result,request_id.as_deref());
    if result.is_object(){result["_ui_engine_revision"]=json!(revision);}
    Ok(result)
}

/// An object's layers stacked and cropped to what is drawn, cached in the project's
/// `storyboard/.theme/` (the engine's own assembler, also used for the PDF's cast cards).
#[tauri::command]
fn object_preview(project: String, name: String, layers: Vec<String>) -> Result<String, String> {
    let dir = std::path::Path::new(&project).parent().ok_or("bad project path")?.join("storyboard").join(".theme");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = dir.join(format!("object-{name}.png"));
    let parts: Vec<std::path::PathBuf> = layers.iter().map(std::path::PathBuf::from).collect();
    animationcraft_engine::storyboard::assembled_parts(&parts, &out)
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{name}: its layers do not share one canvas"))
}

/// Recent projects are navigation only; they never select the production root.
#[tauri::command]
fn recent_projects(engine:tauri::State<Engine>)->Result<Value,String>{engine.with(None,|s|s.execute("project.catalog",json!({})).map(|v|v["projects"].clone()).map_err(|e|e.to_string()))}

/// Takes a project off Recent (the files are untouched).
#[tauri::command]
fn recent_forget(path:String)->Result<(),String>{animationcraft_engine::project_catalog::forget(&path)}

/// Shared production setting; navigation history never selects a new song's location.
#[tauri::command]
fn projects_root() -> Result<String, String> {
    animationcraft_engine::project_settings::root().map(|p|p.to_string_lossy().into_owned()).map_err(|e|e.to_string())
}

/// Run the adjacent animationcraft-cli against this Studio's private bridge. The engine stays in
/// this window; the initiating frontend applies the returned state and acknowledges its revision.
#[tauri::command]
async fn project_open(app:tauri::AppHandle, path: String, request: Option<String>, control:Option<String>) -> Result<Value, String> {
    let request=request.unwrap_or_else(feedback::new_request);
    if let Some(result)=feedback::reuse_open(&app,&json!({"path":path,"_request_id":request,"_origin":"UI control"}))?{return Ok(result);}
    feedback::guard(&app,"project.open",true)?;
    tauri::async_runtime::spawn_blocking(move || project_cli::execute(&app,"project.open",json!({"path":path,"_client":"Studio user (unverified label)","_request_id":request,"_control":control}))).await.map_err(|e|e.to_string())?
}

/// New song uses the actual CLI project.intake command and preserves its complete receipt.
#[tauri::command]
async fn project_create(app:tauri::AppHandle,name:String,audio:String,root:String,request:Option<String>,control:Option<String>)->Result<Value,String>{
 feedback::guard(&app,"project.intake",true)?;
 let request=request.unwrap_or_else(feedback::new_request);
 tauri::async_runtime::spawn_blocking(move || project_cli::execute(&app,"project.intake",json!({"root":root,"name":name,"audio":audio,"_client":"Studio user (unverified label)","_request_id":request,"_control":control}))).await.map_err(|e|e.to_string())?
}

/// Starts (`on`) or stops the project's studio server with the engine's own commands
/// (`video.server_start` / `video.server_stop`), then returns the project's fresh state.
/// Nothing starts a server on its own: quick one-off work costs no memory.
#[tauri::command]
fn studio_server(engine: tauri::State<Engine>, path: String, on: bool) -> Result<Value, String> {
    engine.with(Some(&path), |s| {
        s.execute(if on { "video.server_start" } else { "video.server_stop" }, json!({})).map_err(|e| e.to_string())?;
        state_of(s)
    })
}

/// The project folder of a `.acproj` path.
fn project_dir(project: &str) -> Result<std::path::PathBuf, String> {
    std::path::Path::new(project).parent().map(std::path::Path::to_path_buf).ok_or_else(|| format!("bad project path: {project}"))
}

// ---- The in-app agent ---------------------------------------------------------------------
//
// The agent belongs to this window: the app stores its settings, supplies its instructions and
// runs it as a child process in the project folder. It needs no service, session or tool that
// someone started elsewhere.

/// The app's agent settings file folder (`$XDG_CONFIG_HOME/animationcraft`).
fn agent_home() -> std::path::PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("animationcraft")
}

/// Settings the app supplies when the file does not name them.
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

fn agent_settings() -> Result<Value, String> {
    let file = agent_home().join("agent.json");
    let stored: Value = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}));
    let mut merged = agent_defaults();
    if let (Some(base), Some(over)) = (merged.as_object_mut(), stored.as_object()) {
        for (key, value) in over {
            base.insert(key.clone(), value.clone());
        }
    }
    Ok(merged)
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
    let out = std::process::Command::new("omarchy-default-agent").output().ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
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
fn agent_config() -> Result<Value, String> {
    let mut settings = animationcraft_engine::agent::settings().map_err(|e| e.to_string())?;
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
fn agent_config_save(patch: Value) -> Result<Value, String> {
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
    let mut next = agent_settings()?;
    if let (Some(base), Some(over)) = (next.as_object_mut(), patch.as_object()) {
        for (key, value) in over {
            base.insert(key.clone(), value.clone());
        }
    }
    let file = agent_home().join("agent.json");
    std::fs::create_dir_all(agent_home()).map_err(|e| e.to_string())?;
    std::fs::write(&file, serde_json::to_string_pretty(&next).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    agent_config()
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
    let studio_visible = app.state::<View>().0.lock().map(|v|v.0=="studio" || v.0=="split").unwrap_or(false);
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

/// Which view is showing (`project`, `split` or `studio`) and, for split, the share of the
/// window the project view takes (the frame between them is dragged in the project view).
struct View(std::sync::Mutex<(String, f64)>);

impl Default for View {
    fn default() -> Self {
        Self(std::sync::Mutex::new(("project".into(), 0.5)))
    }
}

/// Native webviews share one allocation. Studio occupies the content rectangle;
/// the shell remains full-height so its docked agent can sit beside Studio.
fn place_webview(view: &tauri::Webview, top: i32, right: i32, is_studio: bool) {
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
        widget.set_margin_top(top); widget.set_margin_end(right);
    });
    #[cfg(not(target_os = "linux"))]
    let _ = (view, top, right, is_studio);
}

#[derive(Default)]
struct AgentDock(std::sync::Mutex<f64>);

#[tauri::command]
fn agent_dock(app: tauri::AppHandle, width: f64) -> Result<(), String> {
    *app.state::<AgentDock>().0.lock().map_err(|e|e.to_string())?=width.clamp(0.0,600.0);
    layout(&app);
    Ok(())
}

fn layout(app: &tauri::AppHandle) {
    let (Some(window), Some(project)) = (app.get_window("main"), app.get_webview("project")) else { return };
    let (Ok(physical), Ok(scale)) = (window.inner_size(), window.scale_factor()) else { return };
    let size = physical.to_logical::<f64>(scale);
    let (view, _) = app.state::<View>().0.lock().map(|v|v.clone()).unwrap_or(("project".into(),0.5));
    let dock = app.state::<AgentDock>().0.lock().map(|v|*v).unwrap_or(0.0).min((size.width-240.0).max(0.0));
    place_webview(&project,0,0,false);
    #[cfg(not(target_os="linux"))]
    { let _=project.set_position(LogicalPosition::new(0.0,0.0)); let _=project.set_size(size); }
    if let Some(studio)=app.get_webview("studio") {
        if view=="studio" || view=="split" {
            place_webview(&studio,BAR as i32,dock.round() as i32,true);
            #[cfg(not(target_os="linux"))]
            { let _=studio.set_position(LogicalPosition::new(0.0,BAR)); let _=studio.set_size(LogicalSize::new(size.width-dock,size.height-BAR)); }
            let _=studio.show();
        } else { let _=studio.hide(); }
    }
}

/// Moves the frame between the project view and the studio (share of the window, top part).
#[tauri::command]
fn set_split(app: tauri::AppHandle, ratio: f64) -> Result<(), String> {
    let state = app.state::<View>();
    *state.0.lock().map_err(|e| e.to_string())? = ("split".into(), ratio.clamp(0.15, 0.9));
    layout(&app);
    Ok(())
}

/// The window's height in logical px (for dragging the frame).
#[tauri::command]
fn window_height(app: tauri::AppHandle) -> Result<f64, String> {
    let window = app.get_window("main").ok_or("no main window")?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    Ok(window.inner_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale).height)
}

/// Shows the studio on `url` as `split` (under the project view) or `studio` (full), creating
/// its webview on first use.
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
    state.0.lock().map_err(|e| e.to_string())?.0 = if mode == "split" { "split".into() } else { "studio".into() };
    layout(&app);
    Ok(())
}

/// Back to the project view alone (the studio keeps its state for next time).
#[tauri::command]
fn studio_hide(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<View>();
    state.0.lock().map_err(|e| e.to_string())?.0 = "project".into();
    layout(&app);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(View::default())
        .manage(AgentDock::default())
        .manage(Engine::default())
        .manage(project_cli::Pending::default())
        .manage(feedback::Feedback::default())
        .manage(bridge::Pending::default())
        .invoke_handler(tauri::generate_handler![startup, feedback::ui_report, engine_command, project_state, recent_projects, recent_forget, projects_root, project_open, project_create, studio_server, agent_config, agent_config_save, snapshot, object_preview, open_path, save_attachment, bridge::bridge_reply, studio_show, studio_hide, agent_dock, set_split, window_height])
        .setup(|app| {
            let window = WindowBuilder::new(app, "main").title("AnimationCraft").inner_size(1440.0, 900.0).build()?;
            let size = window.inner_size()?.to_logical::<f64>(window.scale_factor()?);
            window.add_child(
                WebviewBuilder::new("project", WebviewUrl::App("index.html".into())),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(size.width, size.height),
            )?;
            #[cfg(unix)]
            if let Err(e)=bridge::serve_local(app.handle().clone()){eprintln!("private UI feedback: {e}");}
            // `--control <port>`: the CLI and MCP drive this window (same protocol as before).
            if let Some(port) = std::env::args().skip_while(|a| a != "--control").nth(1) {
                let port: u16 = port.parse().map_err(|_| format!("--control needs a port, got {port}"))?;
                bridge::serve(app.handle().clone(), port);
            }
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
        .expect("AnimationCraft shell failed to start")
        .run(|app,event|{if matches!(event,tauri::RunEvent::Exit){#[cfg(unix)] bridge::cleanup(app);}});
}
