//! Process telemetry for the booth's PULSE panel.
//!
//! Reads `/proc` directly: the Studio's own process tree (the shell plus its webview),
//! memory and CPU, so the panel shows real numbers instead of guesses.

use std::{fs, path::Path};

/// One process in the Studio's tree.
#[derive(Clone, Debug)]
pub struct Proc {
    pub pid: u32,
    pub name: String,
    pub rss_kb: u64,
    pub cpu_pct: f64,
}

/// Every descendant of `root`, deepest last.
pub fn tree(root: u32) -> Vec<Proc> {
    let children = children_map();
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        if let Some(proc) = read_proc(pid) {
            out.push(proc);
        }
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids.iter().copied());
        }
    }
    out
}

fn children_map() -> std::collections::HashMap<u32, Vec<u32>> {
    let mut map: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for entry in fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        if let Some(ppid) = parent_of(pid) {
            map.entry(ppid).or_default().push(pid);
        }
    }
    map
}

fn parent_of(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The comm field can contain spaces and parentheses; fields after it are fixed.
    let after = stat.rsplit_once(") ")?.1;
    after.split_whitespace().nth(1)?.parse().ok()
}

fn read_proc(pid: u32) -> Option<Proc> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat.get(open + 1..close)?.to_string();
    let after = stat.get(close + 2..)?;
    let fields: Vec<&str> = after.split_whitespace().collect();
    // After the comm field: state is 0, ppid 1, … utime 11, stime 12.
    let ticks = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    let rss_kb = fs::read_to_string(format!("/proc/{pid}/status")).ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmRSS:")).and_then(|v| v.trim().split_whitespace().next()).and_then(|v| v.parse().ok()))?;
    Some(Proc { pid, name, rss_kb, cpu_pct: ticks as f64 / 100.0 })
}

/// CPU percent for the tree since the previous sample. Linux `getconf CLK_TCK` is 100.
pub fn cpu_since(previous_ticks: f64, tree: &[Proc]) -> f64 {
    let now: f64 = tree.iter().map(|p| p.cpu_pct).sum();
    let delta = now - previous_ticks;
    delta / 2.0 // seconds of CPU per second of wall clock, sampled over ~2s
}

/// Total resident memory of the tree, in megabytes.
pub fn rss_mb(tree: &[Proc]) -> f64 {
    tree.iter().map(|p| p.rss_kb as f64).sum::<f64>() / 1024.0
}

/// The Studio pid from its socket name (`<pid>-<nonce>.sock`).
pub fn pid_from_socket(socket: &Path) -> Option<u32> {
    socket.file_name()?.to_str()?.split('-').next()?.parse().ok()
}

/// Human-readable byte size.
pub fn mib(bytes_kb: u64) -> String {
    format!("{:.0}M", bytes_kb as f64 / 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_is_readable() {
        let me = std::process::id();
        let tree = tree(me);
        assert!(tree.iter().any(|p| p.pid == me));
        assert!(rss_mb(&tree) > 0.0);
    }

    #[test]
    fn a_socket_name_yields_its_pid() {
        let path = Path::new("/run/user/1000/magicstudio-studio/4242-1.sock");
        assert_eq!(pid_from_socket(path), Some(4242));
    }
}
