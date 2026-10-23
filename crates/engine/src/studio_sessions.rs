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
#[cfg(unix)]
pub fn request(path: &Path, id: &str, params: Value) -> Result<Value, String> {
    crate::local_ipc::private_path(path.parent().ok_or("Endpoint directory missing")?, true)?;
    crate::local_ipc::private_path(path, false)?;
    let mut stream = std::os::unix::net::UnixStream::connect(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::ConnectionRefused {
            format!("stale endpoint: {e}")
        } else {
            e.to_string()
        }
    })?;
    crate::local_ipc::peer(&stream)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "{}", json!({"id":id,"params":params})).map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
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
        let dir = PathBuf::from(runtime).join("animationcraft-studio");
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
            match request(&path,"ui.feedback",json!({})){
   Ok(mut value)=>{value["endpoint"]=json!(format!("unix:{}",path.display()));value["instance"]=json!(path.file_name().and_then(|p|p.to_str()));if let Some(raw)=value["active_project"].as_str(){value["canonical_project"]=json!(canonical(Path::new(raw)).ok());}sessions.push(value);},
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
    match matches.as_slice(){[]=>{if found["errors"].as_array().is_some_and(|v|v.iter().any(|e|!e["error"].as_str().is_some_and(|s|s.starts_with("stale endpoint:")))){Err(format!("Studio ownership is unknown: {}",found["errors"]))}else{Ok(None)}},[one]=>Ok(Some(one.clone())),_=>Err("Multiple Studio sessions report this project; ownership ambiguous, no command executed".into())}
}
