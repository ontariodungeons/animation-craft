//! Which Studio is running, how to confirm it, and how to reach it. Nothing here assumes a desktop,
//! a workspace or a panel layout: a Studio is a socket that answers `ui.feedback`, and that answer
//! carries its project, view and readiness. Address it by its endpoint; confirm it by asking again.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The pinned endpoint for later calls, or None. Runtime dir only: a pin must not outlive the session.
pub fn pinned_path() -> Option<PathBuf> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
    pin_path(Path::new(&runtime), std::env::var("HERDR_PANE_ID").ok().as_deref())
}

fn pin_path(runtime: &Path, pane: Option<&str>) -> Option<PathBuf> {
    let base = runtime.join("magicstudio");
    match pane {
        Some(pane) if !pane.is_empty() && pane.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b)) =>
            Some(base.join("agent-studios").join(pane).join("active-studio")),
        Some(_) => None,
        None => Some(base.join("active-studio")),
    }
}

fn read_pin(path: &Path) -> Option<String> {
    let value = std::fs::read_to_string(path).ok()?;
    let value = value.trim();
    value.starts_with("unix:").then(|| value.to_string())
}

pub fn pinned_endpoint() -> Option<String> {
    read_pin(&pinned_path()?)
}

pub fn pin(endpoint: &str) -> Result<PathBuf, String> {
    let path = pinned_path().ok_or("XDG_RUNTIME_DIR is unavailable; cannot pin a Studio")?;
    std::fs::create_dir_all(path.parent().ok_or("Pin directory unavailable")?)
        .map_err(|e| format!("Cannot create pin directory: {e}"))?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temp, format!("{endpoint}\n")).map_err(|e| format!("Cannot write pin: {e}"))?;
    std::fs::rename(&temp, &path).map_err(|e| format!("Cannot install pin: {e}"))?;
    Ok(path)
}

/// Join an explicit pane assignment to live feedback; never infer ownership from a name or folder.
pub fn assignment(pane: &str, discovery: &Value) -> Value {
    let pin = std::env::var_os("XDG_RUNTIME_DIR").and_then(|runtime|
        pin_path(Path::new(&runtime), Some(pane)).and_then(|path| read_pin(&path)));
    assignment_from(pane, pin.as_deref(), discovery["sessions"].as_array().map(Vec::as_slice).unwrap_or(&[]))
}

fn assignment_from(pane: &str, pin: Option<&str>, sessions: &[Value]) -> Value {
    let matches: Vec<_> = sessions.iter().filter(|s| match pin {
        Some(endpoint) => s["endpoint"].as_str() == Some(endpoint),
        None => s["identity"]["agent_pane"].as_str() == Some(pane),
    }).collect();
    match matches.as_slice() {
        [s] => json!({"state":"connected", "pane":pane, "endpoint":s["endpoint"],
            "port":s["identity"]["control_port"], "pid":s["identity"]["pid"],
            "project":s["active_project"], "build":s["identity"]["build_id"],
            "branch":s["identity"]["branch"], "writer_owner":s["writer_owner"]}),
        [] => json!({"state":if pin.is_some(){"closed"}else{"unassigned"}, "pane":pane, "endpoint":pin}),
        _ => json!({"state":"ambiguous", "pane":pane, "ports":matches.iter().map(|s|s["identity"]["control_port"].clone()).collect::<Vec<_>>()}),
    }
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
                "port": s["identity"]["control_port"],
                "writer_owner": s["writer_owner"],
                "project": s["active_project"],
                "view": s["view"],
                "open_block": s["open_block"],
                "agent_open": s["agent_open"],
                "ready": s["ui_ready"],
                "unsaved": s["unsaved_state"],
                "build": s["identity"]["build_id"],
                "branch": s["identity"]["branch"],
                "agent_pane": s["identity"]["agent_pane"],
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
        let Some(row) = rows.iter().find(|r| r["studio"] == target || r["port"].as_u64().is_some_and(|port|port.to_string()==target) || r["endpoint"] == target) else {
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
    println!("{:<8} {:<7} {:<8} {:<30} {:>7}  {:<20} {}", "STUDIO", "PORT", "VIEW", "PROJECT", "PID", "READY", "ENDPOINT");
    for row in &rows {
        let ready = if row["ready"] == true { "ready".to_string() } else { row["unsaved"].as_str().unwrap_or("unknown").to_string() };
        println!("{:<8} {:<7} {:<8} {:<30} {:>7}  {:<20} {}", row["studio"].as_str().unwrap_or("-"), row["port"].as_u64().map(|v|v.to_string()).unwrap_or_else(||"unknown".into()), row["view"].as_str().unwrap_or("-"), name(row), row["pid"].as_u64().unwrap_or(0), ready, bridge(row));
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

#[cfg(test)]
mod assignment_tests {
    use super::*;
    #[test]
    fn pins_are_isolated_by_pane_and_do_not_inherit_the_human_pin() {
        let runtime = Path::new("/runtime");
        let first = pin_path(runtime, Some("wG:p1")).unwrap();
        let second = pin_path(runtime, Some("wG:p2")).unwrap();
        assert_ne!(first, second);
        assert_ne!(first, pin_path(runtime, None).unwrap());
        assert!(pin_path(runtime, Some("../other")).is_none());
    }
    #[test]
    fn ownership_uses_live_endpoints_and_reports_closed_or_multiple_windows() {
        let a = json!({"endpoint":"unix:/a.sock","active_project":"/a.magic","identity":{"agent_pane":"p1","control_port":9001,"pid":1,"build_id":"a"},"writer_owner":true});
        let b = json!({"endpoint":"unix:/b.sock","active_project":"/b.magic","identity":{"agent_pane":"p2","control_port":9002,"pid":2,"build_id":"b"},"writer_owner":false});
        let sessions = vec![a.clone(),b.clone()];
        assert_eq!(assignment_from("p1",None,&sessions)["port"],9001);
        assert_eq!(assignment_from("p2",None,&sessions)["project"],"/b.magic");
        assert_eq!(assignment_from("p1",Some("unix:/b.sock"),&sessions)["writer_owner"],false);
        assert_eq!(assignment_from("p1",Some("unix:/closed.sock"),&sessions)["state"],"closed");
        assert_eq!(assignment_from("unassigned",None,&sessions)["state"],"unassigned");
        assert_eq!(assignment_from("p1",None,&[a.clone(),a])["state"],"ambiguous");
    }
}
