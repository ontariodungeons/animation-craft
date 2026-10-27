//! Lightweight host dials — reads `/proc` directly with zero subprocesses (`journalctl`, `git`, `omarchy`).
use serde_json::Value;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use crate::telemetry::Proc;

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

pub struct ProcRow {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: Option<f64>,
    pub rss_kb: u64,
}

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
        let numbers: Vec<u64> = values
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
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
    let load = fs::read_to_string("/proc/loadavg")
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or("?")
        .to_string();
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
        host.cpu_pct = percent(
            previous.total_ticks,
            previous.idle_ticks,
            host.total_ticks,
            host.idle_ticks,
        );
        host.cores = previous
            .core_ticks
            .iter()
            .zip(host.core_ticks.iter())
            .map(|((old_total, old_idle), (new_total, new_idle))| {
                percent(*old_total, *old_idle, *new_total, *new_idle)
            })
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
    (
        total,
        total.saturating_sub(available),
        swap_total,
        swap_total.saturating_sub(swap_free),
    )
}

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
            ProcRow {
                pid: proc.pid,
                name: proc.name.clone(),
                cpu_pct: cpu,
                rss_kb: proc.rss_kb,
            }
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

pub fn file_tail(path: &str, lines: usize) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out: Vec<String> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect();
    while out.len() > lines {
        out.remove(0);
    }
    out
}

pub fn journal_tail(_lines: usize) -> Vec<String> {
    Vec::new()
}

pub fn agent_usage() -> Vec<Value> {
    Vec::new()
}

pub fn commits(_directory: &Path, _lines: usize) -> Vec<String> {
    Vec::new()
}
