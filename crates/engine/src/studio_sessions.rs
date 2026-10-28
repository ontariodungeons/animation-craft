//! Existing Studio endpoints, selected by canonical project path. No daemon or separate registry.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::Duration,
};
pub fn canonical(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))
}
/// A busy Studio can refuse a connection with EAGAIN while its accept queue is full.
/// Retry briefly: a dropped command is worse than a short wait.
#[cfg(unix)]
fn connect_with_retry(path: &Path) -> Result<std::os::unix::net::UnixStream, String> {
    let mut last = String::new();
    for attempt in 0..20 {
        match std::os::unix::net::UnixStream::connect(path) {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    return Err(format!("stale endpoint: {e}"));
                }
                last = e.to_string();
                if attempt < 19 {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
    Err(last)
}
#[cfg(unix)]
pub fn request(path: &Path, id: &str, params: Value) -> Result<Value, String> {
    request_within(path, id, params, 3)
}

/// A command can take a while (a model turn, an import). Give it room, and say plainly
/// that the Studio was busy instead of reporting a bare socket timeout.
pub fn request_slow(path: &Path, id: &str, params: Value) -> Result<Value, String> {
    request_within(path, id, params, if crate::find_command(id).is_some_and(|c|c.long_running){3600}else{30})
}
#[cfg(unix)]
pub fn request_within(path: &Path, id: &str, params: Value, timeout_seconds: u64) -> Result<Value, String> {
    crate::local_ipc::private_path(path.parent().ok_or("Endpoint directory missing")?, true)?;
    crate::local_ipc::private_path(path, false)?;
    let mut stream = connect_with_retry(path)?;
    crate::local_ipc::peer(&stream).map_err(|e|format!("Studio peer check failed: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(timeout_seconds)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(timeout_seconds)))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "{}", json!({"id":id,"params":params})).map_err(|e| e.to_string())?;
    let mut line = String::new();
    let mut reader = BufReader::new(stream);
    for attempt in 0..4 {
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => break,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted
                ) && attempt < 3 =>
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!(
                    "The Studio did not answer within {timeout_seconds}s ({e}); it is busy or the request is still running. Inspect the Studio before retrying — no duplicate request was sent."
                ))
            }
        }
    }
    let reply: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    if reply["ok"] == true {
        Ok(reply["result"].clone())
    } else {
        Err(reply["error"]
            .as_str()
            .unwrap_or("Studio request failed")
            .into())
    }
}
pub fn discover() -> Value {
    #[cfg(unix)]
    {
        let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
            return json!({"sessions":[],"errors":["XDG_RUNTIME_DIR unavailable"]});
        };
        let dir = PathBuf::from(runtime).join("magicstudio-studio");
        if !dir.exists() {
            return json!({"sessions":[],"errors":[]});
        }
        if let Err(error) = crate::local_ipc::private_path(&dir, true) {
            return json!({"sessions":[],"errors":[error]});
        }
        let mut sessions = Vec::new();
        let mut errors = Vec::new();
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .starts_with(&format!("{}-", std::process::id()))
            }) {
                continue;
            }
            if path.extension().is_none_or(|e| e != "sock") {
                continue;
            }
            match request_within(&path,"ui.feedback",json!({}),1){
   Ok(mut value)=>{value["endpoint"]=json!(format!("unix:{}",path.display()));value["instance"]=json!(path.file_name().and_then(|p|p.to_str()));if let Some(raw)=value["active_project"].as_str(){value["canonical_project"]=json!(canonical(Path::new(raw)).ok());}sessions.push(value);},
   Err(_) if matches!(path.try_exists(),Ok(false))=>{},
   Err(error)=>errors.push(json!({"endpoint":path,"error":error,"state":if error.starts_with("stale endpoint:"){"stale endpoint; no listener"}else{"unresponsive or inaccessible; ownership unknown"}}))
  }
        }
        json!({"sessions":sessions,"errors":errors})
    }
    #[cfg(not(unix))]
    {
        json!({"sessions":[],"errors":["Private Studio discovery unsupported on this platform"]})
    }
}
pub fn for_project(path: &Path) -> Result<Option<Value>, String> {
    let project = canonical(path)?;
    let found = discover();
    let matches: Vec<Value> = found["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["canonical_project"].as_str() == project.to_str())
        .cloned()
        .collect();
    if matches.is_empty()&&found["errors"].as_array().is_some_and(|v|v.iter().any(|e|!e["error"].as_str().is_some_and(|s|s.starts_with("stale endpoint:")))){return Err(format!("Studio ownership is unknown: {}",found["errors"]));}
    select(&matches)
}

fn select(matches:&[Value])->Result<Option<Value>,String>{
    if matches.len()<2{return Ok(matches.first().cloned());}
    let writers=matches.iter().filter(|s|s["writer_owner"]==true).collect::<Vec<_>>();
    if writers.len()==1{return Ok(Some(writers[0].clone()));}
    // Older builds lack the owner field. An explicit read-only attachment cannot be the writer.
    let possible=matches.iter().filter(|s|s["writer_owner"]!=false).collect::<Vec<_>>();
    if writers.is_empty()&&possible.len()==1{return Ok(Some(possible[0].clone()));}
    Err("Multiple Studio sessions report this project; no unique writer. Inspect magic-cli studio list and select the intended project; no command executed".into())
}

/// Shared app opening for desktop, CLI and MCP. Existing windows are never replaced.
pub fn open(executable: &Path, project: Option<&str>) -> Result<Value, String> {
    open_with_options(executable, project, false, None)
}

/// Open Magic Studio with optional `--animate` focus mode and `--character` selection.
pub fn open_with_options(
    executable: &Path,
    project: Option<&str>,
    animate_only: bool,
    character: Option<&str>,
) -> Result<Value, String> {
    let project = project.map(Path::new).map(canonical).transpose()?;
    if let Some(path) = &project {
        if let Some(existing) = for_project(path)? {
            let endpoint = existing["endpoint"].as_str().ok_or("Studio endpoint missing")?;
            let socket = Path::new(endpoint.trim_start_matches("unix:"));
            match request(socket, "ui.focus", json!({"_expected_instance":existing["instance"],"_expected_project":path})) {
                Ok(_) => return Ok(existing),
                Err(_) if matches!(socket.try_exists(), Ok(false)) => {},
                Err(error) => return Err(error),
            }
        }
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR missing")?;
    let dir = PathBuf::from(runtime).join("magicstudio-studio");
    let mut command = std::process::Command::new(executable);
    if animate_only {
        command.arg("--animate");
    }
    if let Some(ch) = character {
        command.args(["--character", ch]);
    }
    if let Some(path) = &project {
        command.arg(path);
    } else if !animate_only {
        command.arg("--new");
    }
    #[cfg(unix)] { use std::os::unix::process::CommandExt; command.process_group(0); }
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let mut child = command.spawn().map_err(|e|format!("Cannot launch {}: {e}",executable.display()))?;
    let pid = child.id();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(30) {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.file_name().is_some_and(|n|n.to_string_lossy().starts_with(&format!("{pid}-"))) {continue;}
            if let Ok(mut live) = request(&path,"ui.feedback",json!({})) {
                if live["ui_ready"] != true || live["identity"]["pid"] != pid {continue;}
                if project.as_ref().is_some_and(|p|live["active_project"].as_str()!=p.to_str()) {continue;}
                let endpoint = format!("unix:{}",path.display());
                live["endpoint"]=json!(endpoint);live["instance"]=json!(path.file_name().and_then(|v|v.to_str()));
                return Ok(live);
            }
        }
        if let Some(status)=child.try_wait().map_err(|e|e.to_string())? {return Err(format!("Studio process {pid} exited during opening: {status}"));}
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(format!("Studio process {pid} started but has not reported a ready window; it was left running. No other window was changed."))
}
