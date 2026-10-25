//! Which MCP servers are running, who spawned them, and where they collide.
//! Read straight from `/proc`; sampled only while the telemetry tab is open.
use std::{collections::BTreeMap, fs};

/// One MCP server process (wrappers such as `uv run` and sandbox shims are not counted).
#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub pid: u32,
    pub host_pid: u32,
    pub host: String,
    pub family: String,
    pub exe: String,
    pub write_root: Option<String>,
    pub age_secs: u64,
    pub rss_kb: u64,
}

/// A family's servers that share one fixed backend (one Blender, one Studio window).
const SHARED_BACKEND: &[&str] = &["blender"];

/// Short name for an MCP server from its argv, or `None` when it is not one.
pub fn family(argv: &[String]) -> Option<String> {
    let exe = argv.first()?;
    let base = exe.rsplit('/').next().unwrap_or(exe);
    // Launch wrappers: the real server is a separate process, counted on its own.
    if matches!(base, "uv" | "disclaimer" | "npx" | "npm" | "sh" | "bash" | "env") {
        return None;
    }
    let last = argv.last().map(String::as_str).unwrap_or("");
    let joined = argv.join(" ");
    let name = if base.ends_with("-cli") && argv.get(1).map(String::as_str) == Some("mcp") {
        base.trim_end_matches("-cli").to_owned()
    } else if base == "mc" && argv.get(1..).is_some_and(|a| a.iter().any(|x| x == "mcp")) {
        "mc".to_owned()
    } else if joined.contains("mcp-for-blender") || joined.contains("blender-mcp") {
        "blender".to_owned()
    } else if joined.contains("mcp-pdf-server") {
        "pdf".to_owned()
    } else if base == "egui-mcp" {
        "egui".to_owned()
    } else if last.ends_with("mcp/server.mjs") {
        "server.mjs".to_owned()
    } else if base == "python" || base == "python3" || base == "node" {
        return None;
    } else {
        return None;
    };
    Some(name)
}

fn argv_of(pid: u32) -> Option<Vec<String>> {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let argv: Vec<String> = raw.split(|b| *b == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect();
    (!argv.is_empty()).then_some(argv)
}

fn stat_of(pid: u32) -> Option<(u32, u64)> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after: Vec<&str> = stat.rsplit_once(") ")?.1.split_whitespace().collect();
    Some((after.get(1)?.parse().ok()?, after.get(19)?.parse().ok()?))
}

fn comm_of(pid: u32) -> String {
    fs::read_to_string(format!("/proc/{pid}/comm")).map(|s| s.trim().to_owned()).unwrap_or_default()
}

fn rss_of(pid: u32) -> u64 {
    fs::read_to_string(format!("/proc/{pid}/status")).ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmRSS:")).and_then(|v| v.split_whitespace().next()).and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

/// The agent that owns a server: the nearest ancestor that is not a launch wrapper.
fn host_of(mut pid: u32) -> (u32, String) {
    for _ in 0..8 {
        let Some((ppid, _)) = stat_of(pid) else { break };
        if ppid <= 1 {
            return (ppid, comm_of(ppid));
        }
        let wrapper = argv_of(ppid).and_then(|a| a.first().map(|e| e.rsplit('/').next().unwrap_or(e).to_owned()))
            .is_some_and(|b| matches!(b.as_str(), "uv" | "disclaimer" | "npx" | "npm" | "sh" | "bash" | "env" | "node") && comm_of(ppid) != "claude");
        if !wrapper {
            return (ppid, comm_of(ppid));
        }
        pid = ppid;
    }
    (0, String::new())
}

/// Every MCP server on the machine.
pub fn scan() -> Vec<Server> {
    let uptime_ticks = fs::read_to_string("/proc/uptime").ok().and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok())).unwrap_or(0.0);
    let hz = crate::telemetry::clock_ticks().unwrap_or(100) as f64;
    fs::read_dir("/proc").into_iter().flatten().flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(|pid| {
            let argv = argv_of(pid)?;
            let family = family(&argv)?;
            let (_, start) = stat_of(pid)?;
            let (host_pid, host) = host_of(pid);
            let write_root = argv.iter().position(|a| a == "--automation-write-root").and_then(|i| argv.get(i + 1)).cloned();
            Some(Server {
                pid, host_pid, host, family, exe: argv[0].clone(), write_root,
                age_secs: (uptime_ticks - start as f64 / hz).max(0.0) as u64,
                rss_kb: rss_of(pid),
            })
        })
        .collect()
}

/// Something that will break or already collides; every finding is worth acting on.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding { pub family: String, pub text: String }

/// One row per family, plus what is wrong with it.
pub fn analyze(servers: &[Server]) -> (BTreeMap<String, Vec<&Server>>, Vec<Finding>) {
    let mut by: BTreeMap<String, Vec<&Server>> = BTreeMap::new();
    for s in servers { by.entry(s.family.clone()).or_default().push(s); }
    let mut findings = Vec::new();
    for (family, group) in &by {
        let mut hosts: Vec<u32> = group.iter().map(|s| s.host_pid).collect();
        hosts.sort_unstable(); hosts.dedup();
        if SHARED_BACKEND.contains(&family.as_str()) && hosts.len() > 1 {
            findings.push(Finding { family: family.clone(), text: format!("{} servers from {} agents share one backend; commands can collide", group.len(), hosts.len()) });
        }
        let mut roots: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
        for s in group { if let Some(r) = &s.write_root { roots.entry(r).or_default().push(s.host_pid); } }
        for (root, mut who) in roots {
            who.sort_unstable(); who.dedup();
            if who.len() > 1 && !SHARED_BACKEND.contains(&family.as_str()) {
                findings.push(Finding { family: family.clone(), text: format!("{} agents can write {root} through this server at once", who.len()) });
            }
        }
        let mut exes: Vec<&str> = group.iter().map(|s| s.exe.as_str()).collect();
        exes.sort_unstable(); exes.dedup();
        if exes.len() > 1 {
            findings.push(Finding { family: family.clone(), text: format!("{} different builds running: {}", exes.len(), exes.join(" · ")) });
        }
    }
    (by, findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn argv(s: &str) -> Vec<String> { s.split_whitespace().map(str::to_owned).collect() }
    fn server(pid: u32, host: u32, family: &str, exe: &str, root: Option<&str>) -> Server {
        Server { pid, host_pid: host, host: "agent".into(), family: family.into(), exe: exe.into(), write_root: root.map(str::to_owned), age_secs: 1, rss_kb: 1 }
    }

    #[test]
    fn families_come_from_what_was_observed_and_wrappers_are_not_counted() {
        for (line, want) in [
            ("filmcraft-cli mcp", Some("filmcraft")),
            ("/home/jesse/.local/share/magic-studio/bin/mc mcp", Some("mc")),
            ("/home/jesse/.cargo/bin/egui-mcp", Some("egui")),
            ("/home/jesse/.local/share/uv/tools/mcp-for-blender/bin/python /home/jesse/.local/share/uv/tools/mcp-for-blender/bin/mcp-for-blender", Some("blender")),
            ("node /home/jesse/.npm/_npx/x/node_modules/.bin/mcp-pdf-server --stdio", Some("pdf")),
            ("node ./mcp/server.mjs", Some("server.mjs")),
            ("/home/jesse/.local/bin/uv tool uvx mcp-for-blender", None),
            ("/usr/lib/claude-desktop/resources/disclaimer --pgroup -- /home/jesse/.local/bin/uv run blender-mcp", None),
            ("/usr/lib/chatgpt/ChatGPT --type=renderer", None),
            ("mc help", None),
        ] {
            assert_eq!(family(&argv(line)).as_deref(), want, "{line}");
        }
    }

    #[test]
    fn two_agents_on_one_blender_and_one_write_root_and_two_mc_builds_are_found() {
        let servers = vec![
            server(1, 100, "blender", "py", None), server(2, 200, "blender", "py", None),
            server(3, 100, "photocraft", "photocraft-cli", Some("/w")), server(4, 200, "photocraft", "photocraft-cli", Some("/w")),
            server(5, 100, "mc", "/installed/mc", None), server(6, 200, "mc", "/repo/target/debug/mc", None),
            server(7, 100, "cadcraft", "cadcraft-cli", None),
        ];
        let (by, findings) = analyze(&servers);
        assert_eq!(by["blender"].len(), 2);
        let has = |family: &str, text: &str| findings.iter().any(|f| f.family == family && f.text.contains(text));
        assert!(has("blender", "share one backend"), "{findings:?}");
        assert!(has("photocraft", "write /w"), "{findings:?}");
        assert!(has("mc", "2 different builds"), "{findings:?}");
        assert!(findings.iter().all(|f| f.family != "cadcraft"), "a single clean server raises nothing: {findings:?}");
    }

    #[test]
    fn one_agent_with_its_own_servers_is_clean() {
        let servers = vec![server(1, 100, "blender", "py", None), server(2, 100, "photocraft", "p", Some("/w")), server(3, 100, "mc", "mc", None)];
        assert!(analyze(&servers).1.is_empty());
    }

    #[test]
    fn the_live_scan_finds_a_real_mcp_shaped_process_and_names_the_process_that_started_it() {
        let dir = std::env::temp_dir().join(format!("mcp-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(std::process::Command::new("mkfifo").arg(dir.join("mcp")).status().unwrap().success());
        // argv is ["probecraft-cli", "mcp"]; reading the fifo keeps it alive until killed.
        let mut child = std::process::Command::new("bash").current_dir(&dir)
            .args(["-c", "exec -a probecraft-cli cat mcp"]).spawn().unwrap();
        let mut found = None;
        for _ in 0..50 {
            found = scan().into_iter().find(|s| s.pid == child.id());
            if found.is_some() { break; }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let _ = child.kill(); let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
        let server = found.expect("the scan must see a live process whose argv is `probecraft-cli mcp`");
        assert_eq!(server.family, "probecraft");
        assert_eq!(server.host_pid, std::process::id());
    }
}
