//! `doctor` subcommand (`mc doctor [--clean]` and `app.doctor` command):
//! checks for lingering AnimationCraft/Craft processes, stale bridge/dev-server ports,
//! external tool availability, and source code smells worth running `/ponytail-review` on.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::workspace::{resolve_craft_cli, workspace_root};
use crate::{Result, Session};

pub fn cmd_app_doctor(s: &mut Session, params: &Value) -> Result<Value> {
    let clean = params
        .get("clean")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let keep_port = params
        .get("keep_port")
        .and_then(Value::as_u64)
        .map(|p| p as u16);
    let my_pid = std::process::id();
    let mc_program = std::env::current_exe().ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("mc")))
        .and_then(|exe| std::fs::canonicalize(exe).ok());

    let mut processes = Vec::new();
    let mut cleaned_pids = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            if pid == my_pid {
                continue;
            }
            let Ok(raw_cmd) = std::fs::read(entry.path().join("cmdline")) else {
                continue;
            };
            if raw_cmd.is_empty() {
                continue;
            }
            let args: Vec<String> = raw_cmd
                .split(|&b| b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect();
            let Some(exe) = args.first() else { continue };
            let exe_name = Path::new(exe)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(exe);
            // A short executable name can belong to an unrelated program. Match the actual
            // kernel executable against this installation before listing or stopping mc.
            if exe_name == "mc" && !matching_mc_process(&entry.path().join("exe"), mc_program.as_deref()) {
                continue;
            }
            let is_ac = matches!(
                exe_name,
                "animationcraft"
                    | "animationcraft-shell"
                    | "mc"
                    | "photocraft-cli"
                    | "effectcraft-cli"
                    | "filmcraft-cli"
                    | "pdfcraft-cli"
            );
            if !is_ac {
                continue;
            }
            let control_port = args
                .windows(2)
                .find(|w| w[0] == "--control")
                .and_then(|w| w[1].parse::<u16>().ok());
            let bridge_alive = control_port.map(port_open);
            let is_mcp = args.iter().any(|a| a == "mcp");
            let is_gui = matches!(exe_name, "animationcraft" | "animationcraft-shell");
            let kind = if is_gui {
                "gui"
            } else if is_mcp {
                "mcp_server"
            } else {
                "cli_worker"
            };
            let (rss_mb, ppid) = read_proc_status(pid);
            let should_clean = clean
                && ((is_gui && keep_port.is_none_or(|kp| control_port != Some(kp)))
                    || ((exe_name == "mc" || exe_name.ends_with("-cli")) && !is_mcp));
            let mut action = "running";
            if should_clean {
                action = if stop_process(pid, control_port) {
                    cleaned_pids.push(pid);
                    "stopped"
                } else {
                    "stop_failed"
                };
            }
            processes.push(json!({
                "pid": pid,
                "ppid": ppid,
                "exe": exe_name,
                "kind": kind,
                "rss_mb": rss_mb,
                "control_port": control_port,
                "bridge_alive": bridge_alive,
                "cmdline": args.join(" "),
                "status": action,
            }));
        }
    }

    // Identify duplicate MCP server instances per executable (keep the newest PID per exe).
    let mut newest_mcp_by_exe = std::collections::HashMap::<String, u32>::new();
    for p in &processes {
        if p["kind"] == "mcp_server" && p["status"] == "running" {
            if let (Some(exe), Some(pid)) = (p["exe"].as_str(), p["pid"].as_u64()) {
                let pid = pid as u32;
                newest_mcp_by_exe
                    .entry(exe.to_string())
                    .and_modify(|max_pid| *max_pid = (*max_pid).max(pid))
                    .or_insert(pid);
            }
        }
    }
    let mut duplicate_mcp_count = 0usize;
    for p in &mut processes {
        if p["kind"] == "mcp_server" && p["status"] == "running" {
            let exe = p["exe"].as_str().unwrap_or_default().to_string();
            let pid = p["pid"].as_u64().unwrap_or(0) as u32;
            let ppid = p["ppid"].as_u64().unwrap_or(0) as u32;
            let is_dup = newest_mcp_by_exe.get(&exe).is_some_and(|&newest| pid < newest) || ppid == 1;
            if is_dup {
                duplicate_mcp_count += 1;
                if clean {
                    if stop_process(pid, None) {
                        cleaned_pids.push(pid);
                        p["status"] = json!("stopped_duplicate");
                    } else {
                        p["status"] = json!("stop_failed");
                    }
                } else {
                    p["duplicate"] = json!(true);
                }
            }
        }
    }

    // Check stale `.video-server.json` in the open project or current directory.
    let mut cleaned_files = Vec::new();
    let project_dir = s
        .project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| s.cwd.clone());
    let server_file = project_dir.join(".video-server.json");
    let mut video_server_state = json!("none");
    if let Ok(raw) = std::fs::read_to_string(&server_file)
        && let Ok(saved) = serde_json::from_str::<Value>(&raw)
    {
        let port = saved.get("port").and_then(Value::as_u64).map(|p| p as u16);
        let alive = port.is_some_and(port_open);
        if !alive && clean {
            let _ = std::fs::remove_file(&server_file);
            cleaned_files.push(server_file.display().to_string());
            video_server_state = json!("removed_stale_lock");
        } else if !alive {
            video_server_state = json!({"stale": true, "file": server_file, "port": port});
        } else {
            video_server_state = json!({"running": true, "port": port, "pid": saved.get("pid")});
        }
    }

    // Check external Craft CLI tools.
    let tools: Vec<Value> = [
        "photocraft-cli",
        "effectcraft-cli",
        "filmcraft-cli",
        "pdfcraft-cli",
        "ffmpeg",
        "ffprobe",
    ]
    .iter()
    .map(|name| {
        let resolved = resolve_craft_cli(name);
        let found = Path::new(&resolved).is_file() || which_in_path(&resolved);
        json!({ "tool": name, "resolved": resolved, "available": found })
    })
    .collect();

    // Scan workspace Rust sources for Ponytail review candidates (oversized files, UI-thread I/O smells).
    let root = workspace_root(s);
    let ponytail_suggestions = scan_ponytail_candidates(&root);

    let lingering_gui_count = processes
        .iter()
        .filter(|p| p["kind"] == "gui" && p["status"] == "running")
        .count();
    let mut recommendations = Vec::new();
    if lingering_gui_count > 1 {
        recommendations.push(format!(
            "{lingering_gui_count} AnimationCraft GUI instances are running concurrently; run `mc doctor --clean` to close lingering instances."
        ));
    } else if lingering_gui_count == 1 && !clean {
        if let Some(p) = processes.iter().find(|p| p["kind"] == "gui") {
            let port_hint = p["control_port"]
                .as_u64()
                .map(|port| format!(" (`mc --bridge {port} app.quit`)"))
                .unwrap_or_default();
            recommendations.push(format!(
                "1 AnimationCraft GUI instance is running (pid {}){port_hint}. Pass `--clean` to close it.",
                p["pid"]
            ));
        }
    }
    if duplicate_mcp_count > 0 && !clean {
        recommendations.push(format!(
            "{duplicate_mcp_count} duplicate Craft MCP server processes are running; run `mc doctor --clean` to stop duplicates."
        ));
    }
    if video_server_state.get("stale") == Some(&json!(true)) {
        recommendations.push(
            "Stale `.video-server.json` found for a stopped dev server; run `mc doctor --clean`."
                .into(),
        );
    }
    for item in ponytail_suggestions.iter().take(3) {
        if let Some(cmd) = item.get("ponytail_cmd").and_then(Value::as_str) {
            recommendations.push(format!(
                "Run `{cmd}` ({})",
                item["reason"].as_str().unwrap_or("large module")
            ));
        }
    }

    Ok(json!({
        "ok": true,
        "cleaned": {
            "stopped_pids": cleaned_pids,
            "removed_files": cleaned_files,
        },
        "processes": processes,
        "video_server": video_server_state,
        "tools": tools,
        "ponytail_candidates": ponytail_suggestions,
        "recommendations": recommendations,
    }))
}

fn matching_mc_process(executable_link: &Path, expected: Option<&Path>) -> bool {
    expected.is_some_and(|expected| {
        std::fs::canonicalize(executable_link).ok().as_deref() == Some(expected)
    })
}

#[cfg(test)]
mod mc_process_tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn unrelated_mc_and_unavailable_executables_are_not_owned() {
        let root = std::env::temp_dir().join(format!("mc-doctor-{}", std::process::id()));
        std::fs::create_dir_all(root.join("ours")).unwrap();
        std::fs::create_dir_all(root.join("other")).unwrap();
        let ours = root.join("ours/mc");
        let other = root.join("other/mc");
        std::fs::write(&ours, b"own executable fixture").unwrap();
        std::fs::write(&other, b"unrelated executable fixture").unwrap();
        let link = root.join("exe");
        std::os::unix::fs::symlink(&other, &link).unwrap();
        let expected = ours.canonicalize().unwrap();
        assert!(!matching_mc_process(&link, Some(&expected)));
        assert!(!matching_mc_process(&link, None));
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&ours, &link).unwrap();
        assert!(matching_mc_process(&link, Some(&expected)));
        std::fs::remove_file(&ours).unwrap();
        assert!(!matching_mc_process(&link, Some(&expected)));
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn read_proc_status(pid: u32) -> (Option<u64>, Option<u32>) {
    let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
        return (None, None);
    };
    let mut rss_mb = None;
    let mut ppid = None;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok());
            rss_mb = kb.map(|v| (v + 512) / 1024);
        } else if let Some(rest) = line.strip_prefix("PPid:") {
            ppid = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u32>().ok());
        }
    }
    (rss_mb, ppid)
}

fn port_open(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(15)).is_ok()
}

fn stop_process(pid: u32, control_port: Option<u16>) -> bool {
    // Try graceful bridge `app.quit` first when a control port is known.
    if let Some(port) = control_port {
        use std::io::Write;
        use std::net::{SocketAddr, TcpStream};
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        if let Ok(mut stream) =
            TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(40))
        {
            let _ = writeln!(stream, "{{\"id\":\"app.quit\",\"params\":{{}}}}");
            let _ = stream.flush();
            for _ in 0..8 {
                std::thread::sleep(std::time::Duration::from_millis(30));
                if !Path::new(&format!("/proc/{pid}")).exists() {
                    return true;
                }
            }
        }
    }
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return true;
        }
    }
    !Path::new(&format!("/proc/{pid}")).exists()
}

fn which_in_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
        .unwrap_or(false)
}

fn scan_ponytail_candidates(root: &Path) -> Vec<Value> {
    let mut files: Vec<(PathBuf, usize, Vec<&'static str>)> = Vec::new();
    for rel_dir in ["crates/engine/src", "crates/ui-egui/src", "apps/mc/src"] {
        collect_rs_files(&root.join(rel_dir), &mut files);
    }
    files.sort_by(|a, b| b.2.len().cmp(&a.2.len()).then_with(|| b.1.cmp(&a.1)));
    files
        .into_iter()
        .filter(|(_, lines, smells)| *lines > 950 || !smells.is_empty())
        .take(6)
        .map(|(path, lines, smells)| {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let reason = if smells.is_empty() {
                format!("{lines} lines — split by job")
            } else {
                format!("{lines} lines; {}", smells.join(", "))
            };
            json!({
                "file": rel,
                "lines": lines,
                "smells": smells,
                "reason": reason,
                "ponytail_cmd": format!("/ponytail-review {rel}"),
            })
        })
        .collect()
}

fn collect_rs_files(dir: &Path, out: &mut Vec<(PathBuf, usize, Vec<&'static str>)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            let prod_code = text.split("#[cfg(test)]").next().unwrap_or(&text);
            let lines = prod_code.lines().count();
            let path_str = path.to_string_lossy();
            let is_ui = path_str.contains("crates/ui-egui/");
            let is_panel = path_str.contains("crates/ui-egui/src/panels/");
            let mut smells = Vec::new();
            if is_ui && prod_code.contains("TcpStream::connect") {
                smells.push("blocking TCP on UI crate");
            }
            if is_panel && prod_code.contains("request_repaint_after(") {
                smells.push("timer repaint loop in panel");
            }
            if is_panel && prod_code.contains("read_dir(") {
                smells.push("filesystem read_dir inside UI panel");
            }
            if is_panel && prod_code.contains("Command::new(") {
                smells.push("subprocess spawn inside UI panel");
            }
            if prod_code.contains("shortcut:") || prod_code.contains("ponytail:") {
                smells.push("recorded ponytail shortcut");
            }
            out.push((path, lines, smells));
        }
    }
}
