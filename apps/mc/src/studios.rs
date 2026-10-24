//! Which Studio is running, how to confirm it, and how to reach it. Nothing here assumes a desktop,
//! a workspace or a panel layout: a Studio is a socket that answers `ui.feedback`, and that answer
//! carries its project, view and readiness. Address it by its endpoint; confirm it by asking again.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The pinned endpoint for later calls, or None. Runtime dir only: a pin must not outlive the session.
pub fn pinned_path() -> Option<PathBuf> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
    Some(PathBuf::from(runtime).join("magicstudio").join("active-studio"))
}

pub fn pinned_endpoint() -> Option<String> {
    let value = std::fs::read_to_string(pinned_path()?).ok()?;
    let value = value.trim();
    value.starts_with("unix:").then(|| value.to_string())
}

pub fn pin(endpoint: &str) -> Result<PathBuf, String> {
    let path = pinned_path().ok_or("XDG_RUNTIME_DIR is unavailable; cannot pin a Studio")?;
    std::fs::create_dir_all(path.parent().ok_or("Pin directory unavailable")?)
        .map_err(|e| format!("Cannot create pin directory: {e}"))?;
    std::fs::write(&path, format!("{endpoint}\n")).map_err(|e| format!("Cannot write pin: {e}"))?;
    Ok(path)
}

pub fn unpin() -> bool {
    pinned_path().is_some_and(|path| std::fs::remove_file(path).is_ok())
}

/// One row per live Studio, from the only authority there is: the feedback it answers with.
pub fn rows() -> (Vec<Value>, Vec<Value>) {
    let discovery = magicstudio_engine::studio_sessions::discover();
    let rows = discovery["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|s| {
            let endpoint = s["endpoint"].as_str().unwrap_or_default();
            // The socket name is "<pid>-<start>.sock"; the pid is the readable part.
            let short = endpoint.rsplit('/').next().unwrap_or(endpoint).split('-').next().unwrap_or(endpoint);
            json!({
                "endpoint": endpoint,
                "studio": short,
                "pid": s["identity"]["pid"],
                "project": s["active_project"],
                "view": s["view"],
                "open_block": s["open_block"],
                "agent_open": s["agent_open"],
                "ready": s["ui_ready"],
                "unsaved": s["unsaved_state"],
                "build": s["identity"]["build_id"],
            })
        })
        .collect();
    (rows, discovery["errors"].as_array().cloned().unwrap_or_default())
}

fn name(row: &Value) -> String {
    row["project"]
        .as_str()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("no project")
        .to_string()
}

fn bridge(row: &Value) -> String {
    row["endpoint"].as_str().unwrap_or("?").to_string()
}

fn check(row: &Value) -> String {
    format!("mc --bridge {} exec ui.feedback", bridge(row))
}

/// Print the answer. Two ways forward when nothing is running: start a window, or read the TUI.
pub fn run(as_json: bool, target: Option<&str>, unpin: bool) -> i32 {
    let (rows, errors) = rows();
    if unpin {
        println!("{}", if self::unpin() { "Unpinned." } else { "Nothing was pinned." });
        return 0;
    }
    if let Some(target) = target {
        let Some(row) = rows.iter().find(|r| r["studio"] == target || r["endpoint"] == target) else {
            eprintln!("mc studios: no live Studio matches {target}. Run `mc studios` for the list.");
            return 1;
        };
        return match pin(bridge(row).as_str()) {
            Ok(path) => {
                println!("Pinned {}. Plain `mc exec` now uses it; clear with `mc studios --unpin`.", bridge(row));
                println!("{}", path.display());
                0
            }
            Err(error) => { eprintln!("mc studios: {error}"); 1 }
        };
    }
    if as_json {
        println!("{}", serde_json::to_string_pretty(&json!({"studios": rows, "errors": errors, "pinned": pinned_endpoint()})).unwrap_or_default());
        return 0;
    }
    if rows.is_empty() {
        println!("No Studio is answering.");
        if !errors.is_empty() {
            println!("Endpoints present but unresponsive: {}", errors.len());
        }
        println!();
        println!("Two ways forward, no owner input needed:");
        println!("  1. Start a window:  magicstudio-shell [<project.magic>] --control <port>");
        println!("     then confirm it:  mc --bridge unix:<endpoint> exec ui.feedback   (ui_ready must be true)");
        println!("  2. Read it in the TUI:  mc backstage --print project=<project.magic>");
        println!("     same project state, same commands, no window to render.");
        return 0;
    }
    println!("{:<8} {:<8} {:<30} {:>7}  {:<20} {}", "STUDIO", "VIEW", "PROJECT", "PID", "READY", "ENDPOINT");
    for row in &rows {
        let ready = if row["ready"] == true { "ready".to_string() } else { row["unsaved"].as_str().unwrap_or("unknown").to_string() };
        println!("{:<8} {:<8} {:<30} {:>7}  {:<20} {}", row["studio"].as_str().unwrap_or("-"), row["view"].as_str().unwrap_or("-"), name(row), row["pid"].as_u64().unwrap_or(0), ready, bridge(row));
    }
    if !errors.is_empty() {
        println!("\n{} endpoint(s) did not answer: {}", errors.len(), errors.iter().filter_map(|e| e["error"].as_str()).collect::<Vec<_>>().join("; "));
    }
    println!();
    for row in &rows {
        println!("{} → {}   confirm with: {}", row["studio"].as_str().unwrap_or("-"), name(row), check(row));
        if row["ready"] != true && row["unsaved"] != "no_detected_unsaved_changes" {
            println!("   not clean: {} — read ui.feedback before you open or quit anything", row["unsaved"].as_str().unwrap_or("unknown"));
        }
    }
    match rows.as_slice() {
        [one] => println!("\nOne Studio: address it with --bridge {} (or `mc studios {}` to pin it).", bridge(one), one["studio"].as_str().unwrap_or("-")),
        _ => println!("\n{} Studios: pass project=<path> or --bridge <endpoint>, or pin one with `mc studios <studio>`.", rows.len()),
    }
    if let Some(endpoint) = pinned_endpoint() {
        println!("Pinned: {endpoint} (`mc studios --unpin` to clear)");
    }
    println!("This answer is complete; do not ask the owner where it is.");
    0
}

/// Launch-free liveness check for scripts: exit 0 only when a UI-ready Studio answers.
pub fn verify() -> i32 {
    let (rows, _) = rows();
    match rows.iter().find(|r| r["ready"] == true) {
        Some(row) => {
            println!("running: {} {} — {}", row["studio"].as_str().unwrap_or("-"), name(row), check(row));
            0
        }
        None => {
            if rows.is_empty() { println!("not running: no Studio answers; start one or use mc backstage --print"); }
            else { println!("running but not UI-ready: {} — {}; read ui.feedback first", rows.len(), rows.iter().map(check).collect::<Vec<_>>().join(", ")); }
            1
        }
    }
}
