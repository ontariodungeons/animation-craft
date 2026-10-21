//! AnimationCraft shell: one window holding the project view (a webview fed by
//! `animationcraft-engine`) and, as a tab, the project's video studio (HyperFrames or Remotion
//! Studio) in a second webview of the same window.
//!
//!   animationcraft-shell [<project.acproj>] [<studio url>] [--view project|studio] [--open <block>]

use animationcraft_engine::Session;
use serde_json::{Value, json};
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowBuilder, WindowEvent};

/// The command line, for the project view on load: project and studio URL, plus `--view <project|studio>`
/// and `--open <song|lyrics|transcript|animate>` to start on a given tab and block.
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
        .invoke_handler(tauri::generate_handler![startup, project_state, studio_server, object_preview, open_path, studio_show, studio_hide, set_split, window_height])
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
