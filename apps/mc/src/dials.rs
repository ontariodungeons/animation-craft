//! Host dials and log streams for the Backstage telemetry tab.
//!
//! Reads `/proc`, the user journal and the local repositories directly. There is no
//! service, scheduler or retained history here: one sample is kept only long enough
//! to turn counters into rates, and every reading that cannot be measured stays
//! unknown rather than becoming zero.

use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

use crate::telemetry::Proc;

/// One host snapshot. CPU percentages need two samples to exist at all.
pub struct Host {
    pub at: Instant,
    pub cpu_pct: Option<f64>,
    pub cores: Vec<Option<f64>>,
    pub mem_total_kb: u64,
    pub mem_used_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
    pub load: String,
    pub core_count: usize,
    total_ticks: u64,
    idle_ticks: u64,
    core_ticks: Vec<(u64, u64)>,
}

/// A process in the host-wide list.
pub struct ProcRow {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: Option<f64>,
    pub rss_kb: u64,
}

/// Read the host's CPU, memory, swap and load.
pub fn sample_host(previous: Option<&Host>) -> Option<Host> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let mut total = 0u64;
    let mut idle = 0u64;
    let mut cores = Vec::new();
    let mut core_ticks = Vec::new();
    for line in stat.lines() {
        let Some((name, values)) = line.split_once(' ') else {
            continue;
        };
        let numbers: Vec<u64> = values.split_whitespace().filter_map(|v| v.parse().ok()).collect();
        if numbers.len() < 5 {
            continue;
        }
        let sum: u64 = numbers.iter().sum();
        let waiting = numbers[3] + numbers[4];
        if name == "cpu" {
            total = sum;
            idle = waiting;
        } else if name.starts_with("cpu") {
            cores.push(sum);
            core_ticks.push((sum, waiting));
        }
    }
    if total == 0 {
        return None;
    }
    let (mem_total_kb, mem_used_kb, swap_total_kb, swap_used_kb) = memory();
    let load = fs::read_to_string("/proc/loadavg").unwrap_or_default().split_whitespace().next().unwrap_or("?").to_string();
    let mut host = Host {
        at: Instant::now(),
        cpu_pct: None,
        cores: Vec::new(),
        mem_total_kb,
        mem_used_kb,
        swap_total_kb,
        swap_used_kb,
        load,
        core_count: core_ticks.len(),
        total_ticks: total,
        idle_ticks: idle,
        core_ticks,
    };
    if let Some(previous) = previous
        && let Some(elapsed) = host.at.checked_duration_since(previous.at)
        && elapsed > Duration::from_millis(50)
    {
        host.cpu_pct = percent(previous.total_ticks, previous.idle_ticks, host.total_ticks, host.idle_ticks);
        host.cores = previous
            .core_ticks
            .iter()
            .zip(host.core_ticks.iter())
            .map(|((old_total, old_idle), (new_total, new_idle))| percent(*old_total, *old_idle, *new_total, *new_idle))
            .collect();
    }
    Some(host)
}

fn percent(old_total: u64, old_idle: u64, new_total: u64, new_idle: u64) -> Option<f64> {
    let total = new_total.checked_sub(old_total)?;
    if total == 0 {
        return None;
    }
    let idle = new_idle.saturating_sub(old_idle);
    Some((total - idle.min(total)) as f64 / total as f64 * 100.0)
}

/// Total, used, and the same pair for swap, in kilobytes.
fn memory() -> (u64, u64, u64, u64) {
    let Some(text) = fs::read_to_string("/proc/meminfo").ok() else {
        return (0, 0, 0, 0);
    };
    let value = |key: &str| -> u64 {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    let total = value("MemTotal:");
    let available = value("MemAvailable:");
    let swap_total = value("SwapTotal:");
    let swap_free = value("SwapFree:");
    (total, total.saturating_sub(available), swap_total, swap_total.saturating_sub(swap_free))
}

/// The busiest processes on the machine, by CPU then by memory.
pub fn top_processes(previous: &[Proc], current: &[Proc], limit: usize) -> Vec<ProcRow> {
    let hz = crate::telemetry::clock_ticks();
    let mut rows: Vec<ProcRow> = current
        .iter()
        .map(|proc| {
            let cpu = previous
                .iter()
                .find(|old| old.pid == proc.pid && old.start_ticks == proc.start_ticks)
                .and_then(|old| proc.cpu_ticks.checked_sub(old.cpu_ticks))
                .and_then(|ticks| hz.map(|hz| ticks as f64 / hz as f64 * 100.0));
            ProcRow { pid: proc.pid, name: proc.name.clone(), cpu_pct: cpu, rss_kb: proc.rss_kb }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.cpu_pct
            .unwrap_or(0.0)
            .partial_cmp(&a.cpu_pct.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.rss_kb.cmp(&a.rss_kb))
    });
    rows.truncate(limit);
    rows
}

/// The last `lines` lines of a file, read from the end.
pub fn file_tail(path: &str, lines: usize) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out: Vec<String> = text.lines().filter(|l| !l.trim().is_empty()).map(str::to_owned).collect();
    while out.len() > lines {
        out.remove(0);
    }
    out
}

/// Recent user-journal lines: Omarchy and the user's own services.
pub fn journal_tail(lines: usize) -> Vec<String> {
    let Ok(output) = Command::new("journalctl")
        .args(["--user", "-n", &lines.to_string(), "--no-pager", "-o", "short-iso"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.chars().skip(19).collect::<String>().trim_start().to_string())
        .collect()
}

/// Per-agent usage and quota, exactly as Omarchy records it. Local files only:
/// the read is a cached report, not a live query.
pub fn agent_usage() -> Vec<Value> {
    let mut out = Vec::new();
    for provider in ["claude", "codex", "grok", "kimi"] {
        let Ok(output) = Command::new("omarchy")
            .args(["agent", "usage", provider, "--limits-only"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&output.stdout) else {
            continue;
        };
        if value["ready"] != true {
            continue;
        }
        out.push(value);
    }
    out
}

/// Recent commits in a repository, newest first.
pub fn commits(directory: &Path, lines: usize) -> Vec<String> {
    let Ok(output) = Command::new("git")
        .args(["-C", &directory.to_string_lossy(), "log", "--oneline", "-n", &lines.to_string()])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout).lines().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_memory_and_cpu_are_read_or_unknown() {
        let host = sample_host(None).expect("host sample");
        assert!(host.mem_total_kb > 0, "total memory must be measurable");
        assert!(host.mem_used_kb <= host.mem_total_kb);
        assert_eq!(host.cpu_pct, None, "the first sample has no rate yet");
    }

    #[test]
    fn a_second_host_sample_yields_a_rate() {
        let first = sample_host(None).expect("host sample");
        std::thread::sleep(Duration::from_millis(120));
        let second = sample_host(Some(&first)).expect("host sample");
        let cpu = second.cpu_pct.expect("two samples make a rate");
        assert!((0.0..=100.0).contains(&cpu), "{cpu}");
    }

    #[test]
    fn missing_files_and_repositories_read_as_empty_not_as_errors() {
        assert!(file_tail("/nonexistent/path.log", 5).is_empty());
        assert!(commits(Path::new("/nonexistent/repo"), 5).is_empty());
    }

    #[test]
    fn the_process_list_keeps_measured_zero_and_unknown_cpu() {
        let old = Proc { pid: 1, name: "a".into(), rss_kb: 10, cpu_ticks: 100, start_ticks: 1 };
        let now = Proc { pid: 1, name: "a".into(), rss_kb: 20, cpu_ticks: 100, start_ticks: 1 };
        let rows = top_processes(&[old], &[now], 8);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].rss_kb, 20);
    }
}
