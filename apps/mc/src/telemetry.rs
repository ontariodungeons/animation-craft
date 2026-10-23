//! Process telemetry for the booth's PULSE panel.
//!
//! Reads `/proc` directly: the Studio's own process tree (the shell plus its webview),
//! memory and CPU, so the panel shows real numbers instead of guesses.

use std::{fs, path::Path, time::{Duration, Instant}};

/// One process in the Studio's tree.
#[derive(Clone, Debug)]
pub struct Proc {
    pub pid: u32,
    pub name: String,
    pub rss_kb: u64,
    pub cpu_ticks: u64,
    pub start_ticks: u64,
}

/// A bounded snapshot for one selected Studio. CPU uses one core = 100%.
pub struct Sample {
    pub pid: u32,
    pub instance: String,
    pub at: Instant,
    pub procs: Vec<Proc>,
    pub cpu_pct: Option<f64>,
}

impl Sample {
    pub fn capture(instance: &str, previous: Option<&Self>) -> Option<Self> {
        let pid = pid_from_socket(Path::new(instance))?;
        let procs = tree(pid);
        if !procs.iter().any(|p| p.pid == pid) { return None; }
        let at = Instant::now();
        let cpu_pct = previous.filter(|p| p.instance == instance).and_then(|p| {
            cpu_percent(&p.procs, &procs, at.duration_since(p.at), clock_ticks()?)
        });
        Some(Self { pid, instance: instance.into(), at, procs, cpu_pct })
    }
}

pub fn clock_ticks() -> Option<u64> {
    static TICKS: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *TICKS.get_or_init(|| {
        let out = std::process::Command::new("getconf").arg("CLK_TCK").output().ok()?;
        if !out.status.success() { return None; }
        String::from_utf8(out.stdout).ok()?.trim().parse().ok()
    })
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

/// Every readable process on the machine, for a host-wide list.
pub fn all_procs() -> Vec<Proc> {
    fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(read_proc)
        .collect()
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

pub fn read_proc(pid: u32) -> Option<Proc> {
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
    let start_ticks = fields.get(19)?.parse().ok()?;
    Some(Proc { pid, name, rss_kb, cpu_ticks: ticks, start_ticks })
}

/// Only compare processes present in both samples, guarding against PID reuse.
pub fn cpu_percent(previous: &[Proc], current: &[Proc], elapsed: Duration, hz: u64) -> Option<f64> {
    if elapsed.is_zero() || hz == 0 { return None; }
    let mut delta = 0u64;
    let mut matched = false;
    for p in current {
        if let Some(old) = previous.iter().find(|old| old.pid == p.pid && old.start_ticks == p.start_ticks)
            && let Some(ticks) = p.cpu_ticks.checked_sub(old.cpu_ticks) {
            delta = delta.saturating_add(ticks);
            matched = true;
        }
    }
    matched.then(|| delta as f64 / hz as f64 / elapsed.as_secs_f64() * 100.0)
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

    #[test]
    fn cpu_uses_elapsed_time_and_keeps_observed_zero() {
        let old = Proc { pid: 1, name: "test".into(), rss_kb: 0, cpu_ticks: 100, start_ticks: 10 };
        let mut now = old.clone();
        now.cpu_ticks = 175;
        assert_eq!(cpu_percent(&[old.clone()], &[now], Duration::from_secs(3), 100), Some(25.0));
        assert_eq!(cpu_percent(&[old.clone()], &[old], Duration::from_secs(2), 100), Some(0.0));
    }

    #[test]
    fn cpu_does_not_mix_reused_pids_or_new_children() {
        let old = Proc { pid: 1, name: "test".into(), rss_kb: 0, cpu_ticks: 100, start_ticks: 10 };
        let mut reused = old.clone();
        reused.start_ticks = 20;
        reused.cpu_ticks = 900;
        assert_eq!(cpu_percent(&[old.clone()], &[reused], Duration::from_secs(2), 100), None);
        assert_eq!(cpu_percent(&[], &[old.clone()], Duration::from_secs(2), 100), None);
        assert_eq!(cpu_percent(&[old.clone()], &[old], Duration::ZERO, 100), None);
    }

    #[test]
    fn first_sample_has_memory_but_no_cpu_rate() {
        let sample = Sample::capture(&format!("{}-test.sock",std::process::id()), None).unwrap();
        assert!(rss_mb(&sample.procs) > 0.0);
        assert_eq!(sample.cpu_pct, None);
    }
}
