//! What an agent's MCP session loaded, so MOPS can show it and say when it is out of date.
//! One private file per live `mc mcp` process under `$XDG_RUNTIME_DIR/magicstudio-mcp/`;
//! the guard removes it when the session ends and readers ignore files whose process is gone.
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::SystemTime};

fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("magicstudio-mcp"))
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn exe_modified(path: &std::path::Path) -> u64 {
    fs::metadata(path).and_then(|m| m.modified()).map(secs).unwrap_or(0)
}

/// Removes this session's file on drop.
pub struct Guard(Option<PathBuf>);
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = fs::remove_file(p);
        }
    }
}

/// Records this session; `bridge` is the `--bridge` port when it drives an open window.
pub fn announce(bridge: Option<&str>) -> Guard {
    match dir() {
        Some(dir) => announce_in(&dir, bridge),
        None => Guard(None),
    }
}

fn announce_in(dir: &std::path::Path, bridge: Option<&str>) -> Guard {
    let Ok(exe) = std::env::current_exe() else { return Guard(None) };
    let pid = std::process::id();
    let record = json!({
        "pid": pid,
        "pane": std::env::var("HERDR_PANE_ID").ok(),
        "cwd": std::env::current_dir().ok(),
        "version": env!("CARGO_PKG_VERSION"),
        "exe": exe,
        "exe_modified": exe_modified(&exe),
        "loaded_at": secs(SystemTime::now()),
        "bridge": bridge,
    });
    let path = dir.join(format!("{pid}.json"));
    let written = fs::create_dir_all(dir).is_ok() && {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        }
        fs::write(&path, record.to_string()).is_ok()
    };
    Guard(written.then_some(path))
}

/// Live sessions, each with `stale` set when the installed binary changed after it loaded.
pub fn sessions() -> Vec<Value> {
    dir().map(|d| sessions_in(&d)).unwrap_or_default()
}

fn sessions_in(dir: &std::path::Path) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|e| {
            let mut v: Value = serde_json::from_slice(&fs::read(e.path()).ok()?).ok()?;
            let pid = v["pid"].as_u64()?;
            if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
                return None;
            }
            let now = v["exe"].as_str().map(|p| exe_modified(std::path::Path::new(p))).unwrap_or(0);
            v["stale"] = json!(now > v["exe_modified"].as_u64().unwrap_or(0));
            Some(v)
        })
        .collect()
}

/// The orientation an agent gets when its MCP session starts. Every line is read live.
pub fn orientation(bridge: Option<&str>) -> String {
    let run = |cmd: &str, args: &[&str]| {
        std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).filter(|s| !s.is_empty())
    };
    let mut out = String::from("\n\nYou are in:");
    out.push_str(&format!("\n- Omarchy {}", run("omarchy", &["version"]).unwrap_or_else(|| "(version unknown)".into())));
    match std::env::var("HERDR_PANE_ID") {
        Ok(pane) => out.push_str(&format!("\n- herdr {} · pane {pane}", run("herdr", &["--version"]).unwrap_or_default().trim_start_matches("herdr "))),
        Err(_) => out.push_str("\n- no herdr pane detected"),
    }
    if let Ok(cwd) = std::env::current_dir() {
        out.push_str(&format!("\n- folder {}", cwd.display()));
    }
    out.push_str("\n- Magic Studio: a custom animation app; projects are .magic files");
    out.push_str(&match bridge {
        Some(port) => format!("\n- this session drives the open window on control port {port}"),
        None => "\n- this session is headless (no window attached)".into(),
    });
    out.push_str(&format!("\n- mc {} loaded this session; restart the MCP to pick up a newer build", env!("CARGO_PKG_VERSION")));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_is_listed_while_alive_flagged_when_the_binary_changes_and_gone_after_the_guard() {
        let tmp = std::env::temp_dir().join(format!("mcp-presence-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let guard = announce_in(&tmp, Some("9100"));
        let mine = |s: &Value| s["pid"].as_u64() == Some(std::process::id() as u64);
        let found: Vec<Value> = sessions_in(&tmp).into_iter().filter(mine).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["bridge"], "9100");
        assert_eq!(found[0]["stale"], false);
        // The record claims an older build than the file on disk now: stale.
        let path = tmp.join(format!("{}.json", std::process::id()));
        let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["exe_modified"] = json!(1);
        fs::write(&path, v.to_string()).unwrap();
        assert_eq!(sessions_in(&tmp).into_iter().find(mine).unwrap()["stale"], true);
        // A record for a dead process is ignored.
        let mut dead = v.clone();
        dead["pid"] = json!(u32::MAX);
        fs::write(tmp.join("dead.json"), dead.to_string()).unwrap();
        assert!(sessions_in(&tmp).iter().all(|s| s["pid"] != json!(u32::MAX)));
        drop(guard);
        assert!(sessions_in(&tmp).into_iter().all(|s| !mine(&s)));
        assert!(orientation(None).contains("headless") && orientation(Some("9100")).contains("9100"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
