//! AnimationCraft shell: one window holding the project view (a webview fed by
//! `animationcraft-engine`) and, as a tab, the project's video studio (HyperFrames or Remotion
//! Studio) in a second webview of the same window.
//!
//!   animationcraft-shell [<project.acproj>] [<studio url>] [--view project|studio] [--open <block>|agent]

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
            _ => positional.push(a),
        }
    }
    json!({"project": positional.first(), "studio": positional.get(1), "view": view, "open": open})
}

/// The project's workshop state and storyboard, straight from the engine.
#[tauri::command]
fn project_state(path: String) -> Result<Value, String> {
    let mut s = Session::default();
    s.execute("project.open", json!({"path": path})).map_err(|e| e.to_string())?;
    let status = s.execute("workshop.status", json!({})).map_err(|e| e.to_string())?;
    let storyboard = s.execute("workshop.storyboard", json!({})).unwrap_or(Value::Null);
    Ok(json!({"status": status, "storyboard": storyboard}))
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

/// Starts (`on`) or stops the project's studio server with the engine's own commands
/// (`video.server_start` / `video.server_stop`), then returns the project's fresh state.
/// Nothing starts a server on its own: quick one-off work costs no memory.
#[tauri::command]
fn studio_server(path: String, on: bool) -> Result<Value, String> {
    let mut s = Session::default();
    s.execute("project.open", json!({"path": path})).map_err(|e| e.to_string())?;
    s.execute(if on { "video.server_start" } else { "video.server_stop" }, json!({})).map_err(|e| e.to_string())?;
    project_state(path)
}

/// The project folder of a `.acproj` path.
fn project_dir(project: &str) -> Result<std::path::PathBuf, String> {
    std::path::Path::new(project).parent().map(std::path::Path::to_path_buf).ok_or_else(|| format!("bad project path: {project}"))
}

/// `opencode api <args…> [-d data]` from the project folder, against the user's opencode service
/// (their subscription, models and MCP servers). opencode can exit before a pipe is drained, so its
/// reply is written to a file and read back whole.
#[tauri::command]
async fn opencode(project: String, args: Vec<String>, data: Option<Value>) -> Result<Value, String> {
    let dir = project_dir(&project)?;
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let reply = std::env::temp_dir().join(format!(
        "animationcraft-shell-opencode-{}-{}.json",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut cmd = std::process::Command::new("opencode");
    cmd.current_dir(&dir).arg("api").args(&args);
    if let Some(d) = &data {
        cmd.arg("-d").arg(d.to_string());
    }
    let file = std::fs::File::create(&reply).map_err(|e| e.to_string())?;
    let out = cmd.stdout(file).output().map_err(|e| format!("opencode: {e}"))?;
    let text = std::fs::read_to_string(&reply).unwrap_or_default();
    let _ = std::fs::remove_file(&reply);
    if !out.status.success() {
        return Err(format!("opencode {}: {}", args.first().map_or("", String::as_str), String::from_utf8_lossy(&out.stderr).trim()));
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("opencode reply: {e}"))
}

/// The project's agent session id (`<project>/.agent/opencode-session`); `set` stores a new one.
#[tauri::command]
fn agent_session(project: String, set: Option<String>) -> Result<Option<String>, String> {
    let file = project_dir(&project)?.join(".agent").join("opencode-session");
    if let Some(id) = set {
        std::fs::create_dir_all(file.parent().ok_or("no folder")?).map_err(|e| e.to_string())?;
        std::fs::write(&file, &id).map_err(|e| e.to_string())?;
        return Ok(Some(id));
    }
    Ok(std::fs::read_to_string(&file).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

/// `herdr <args…>`: agents running in Herdr panes (list them, or send one a prompt).
#[tauri::command]
async fn herdr(args: Vec<String>) -> Result<Value, String> {
    let out = std::process::Command::new("herdr").args(&args).output().map_err(|e| format!("herdr: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(serde_json::from_slice(&out.stdout).unwrap_or_else(|_| json!({"text": String::from_utf8_lossy(&out.stdout)})))
}

/// Point: WebKit's snapshot of the project view cropped to `rect` (CSS px, x y w h), saved to
/// `<project>/.agent/points/`; returns the file.
#[tauri::command]
async fn snapshot(app: tauri::AppHandle, project: String, rect: [f64; 4]) -> Result<String, String> {
    let view = app.get_webview("project").ok_or("no project view")?;
    let scale = app.get_window("main").and_then(|w| w.scale_factor().ok()).unwrap_or(1.0);
    let dir = project_dir(&project)?.join(".agent").join("points");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let out = dir.join(format!("point-{stamp}.png"));
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
                    let saved = res.map_err(|e| e.to_string()).and_then(|surface| crop_png(&surface, rect, scale, &target));
                    let _ = tx.send(saved);
                },
            );
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (platform, tx.send(Err("Point needs Linux for now".into())), &target, rect, scale);
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(10)).map_err(|_| "the snapshot did not arrive".to_string())??;
    Ok(out.to_string_lossy().into_owned())
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

/// Gives a webview an exact height (logical px; None = any). On Linux a window's webviews stack
/// top to bottom in one GTK box and positions are ignored; GTK also hands a webview its large
/// natural height, so each gets an explicit height and together they fill the window exactly.
fn pack(view: &tauri::Webview, height: Option<f64>) {
    #[cfg(target_os = "linux")]
    let _ = view.with_webview(move |platform| {
        use gtk::prelude::*;
        let widget: gtk::Widget = platform.inner().upcast();
        widget.set_size_request(-1, height.map_or(-1, |h| h.round() as i32));
        if let Some(parent) = widget.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
            parent.set_child_packing(&widget, height.is_none(), true, 0, gtk::PackType::Start);
        }
    });
    #[cfg(not(target_os = "linux"))]
    let _ = (view, height);
}

/// Lays the webviews out for the current view: the project view alone; split, the project view
/// above the studio at the dragged ratio; or the studio filling the window under the top bar.
fn layout(app: &tauri::AppHandle) {
    let (Some(window), Some(project)) = (app.get_window("main"), app.get_webview("project")) else { return };
    let (Ok(physical), Ok(scale)) = (window.inner_size(), window.scale_factor()) else { return };
    let height = physical.to_logical::<f64>(scale).height;
    let (view, ratio) = app.state::<View>().0.lock().map(|v| v.clone()).unwrap_or(("project".into(), 0.5));
    let top = match view.as_str() {
        "split" => (height * ratio).clamp(BAR + 80.0, height - 80.0),
        "studio" => BAR,
        _ => height,
    };
    match app.get_webview("studio") {
        Some(studio) if view != "project" => {
            pack(&project, Some(top));
            pack(&studio, Some(height - top));
            let _ = studio.show();
        }
        studio => {
            if let Some(studio) = studio {
                let _ = studio.hide();
            }
            pack(&project, None);
        }
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
                .add_child(WebviewBuilder::new("studio", url), LogicalPosition::new(0.0, BAR), LogicalSize::new(1.0, 1.0))
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
        .manage(View::default())
        .invoke_handler(tauri::generate_handler![startup, project_state, studio_server, opencode, agent_session, herdr, snapshot, object_preview, open_path, save_attachment, studio_show, studio_hide, set_split, window_height])
        .setup(|app| {
            let window = WindowBuilder::new(app, "main").title("AnimationCraft").inner_size(1440.0, 900.0).build()?;
            let size = window.inner_size()?.to_logical::<f64>(window.scale_factor()?);
            window.add_child(
                WebviewBuilder::new("project", WebviewUrl::App("index.html".into())),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(size.width, size.height),
            )?;
            // Every resize re-applies the layout for the open tab.
            let handle = app.handle().clone();
            window.on_window_event(move |event| {
                if let WindowEvent::Resized(_) = event {
                    layout(&handle);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("AnimationCraft shell failed to start");
}
