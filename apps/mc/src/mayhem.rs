//! MAYHEM trend metrics: a small, append-only history of resource observations so the
//! debug view can show whether the machine is actually improving over time.
//!
//! One file per user: `~/.config/magicstudio/mayhem-metrics.json`. It holds a bounded
//! list of samples; each sample is one observation of the whole machine. Nothing here
//! invents a value: every number is measured from `/proc` or the MCP scan.

use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
};

/// How many samples are kept. Oldest are dropped first.
pub const MAX_SAMPLES: usize = 240;

/// One observation of the machine's craft-app footprint.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub at_unix_ms: u128,
    /// Every MCP server process on the machine.
    pub mcp_servers: usize,
    /// Distinct agents (host processes) that started them.
    pub mcp_agents: usize,
    /// Total resident memory of all MCP servers, in KiB.
    pub mcp_rss_kb: u64,
    /// MCP servers whose family shares a backend or a write root with another.
    pub mcp_collisions: usize,
    /// Magic Studio windows answering on their socket.
    pub studios: usize,
    /// Studio sockets with no listener.
    pub stale_studios: usize,
    /// Host memory in use, in KiB.
    pub host_mem_used_kb: u64,
    /// Host memory total, in KiB.
    pub host_mem_total_kb: u64,
}

impl Sample {
    /// One measurement taken right now.
    pub fn capture(servers: &[crate::mcp_watch::Server], studios: usize, stale: usize) -> Sample {
        let (_, findings) = crate::mcp_watch::analyze(servers);
        let agents: std::collections::BTreeSet<u32> = servers.iter().map(|s| s.host_pid).collect();
        let (mem_used, mem_total) = host_memory();
        Sample {
            at_unix_ms: now_ms(),
            mcp_servers: servers.len(),
            mcp_agents: agents.len(),
            mcp_rss_kb: servers.iter().map(|s| s.rss_kb).sum(),
            mcp_collisions: findings.len(),
            studios,
            stale_studios: stale,
            host_mem_used_kb: mem_used,
            host_mem_total_kb: mem_total,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "at_unix_ms": self.at_unix_ms,
            "mcp_servers": self.mcp_servers,
            "mcp_agents": self.mcp_agents,
            "mcp_rss_kb": self.mcp_rss_kb,
            "mcp_collisions": self.mcp_collisions,
            "studios": self.studios,
            "stale_studios": self.stale_studios,
            "host_mem_used_kb": self.host_mem_used_kb,
            "host_mem_total_kb": self.host_mem_total_kb,
        })
    }

    fn from_json(v: &Value) -> Option<Sample> {
        Some(Sample {
            at_unix_ms: v["at_unix_ms"].as_u64()? as u128,
            mcp_servers: v["mcp_servers"].as_u64()? as usize,
            mcp_agents: v["mcp_agents"].as_u64()? as usize,
            mcp_rss_kb: v["mcp_rss_kb"].as_u64()?,
            mcp_collisions: v["mcp_collisions"].as_u64()? as usize,
            studios: v["studios"].as_u64()? as usize,
            stale_studios: v["stale_studios"].as_u64()? as usize,
            host_mem_used_kb: v["host_mem_used_kb"].as_u64()?,
            host_mem_total_kb: v["host_mem_total_kb"].as_u64()?,
        })
    }
}

/// The file the history lives in. `None` when no config directory can be found.
pub fn file() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("magicstudio").join("mayhem-metrics.json"))
}

/// Every recorded sample, oldest first.
pub fn history() -> Vec<Sample> {
    let Some(path) = file() else { return Vec::new(); };
    let Ok(bytes) = fs::read(path) else { return Vec::new(); };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else { return Vec::new(); };
    doc["samples"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Sample::from_json)
        .collect()
}

/// Record this sample. Keeps the newest `MAX_SAMPLES`; a write failure is not fatal.
pub fn record(sample: &Sample) {
    let Some(path) = file() else { return; };
    let mut samples = history();
    // The same second twice is the same observation; do not fill the history with it.
    if samples.last().is_some_and(|s| {
        s.at_unix_ms / 1000 == sample.at_unix_ms / 1000
    }) {
        return;
    }
    samples.push(sample.clone());
    if samples.len() > MAX_SAMPLES {
        let drop = samples.len() - MAX_SAMPLES;
        samples.drain(0..drop);
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let doc = json!({"samples": samples.iter().map(Sample::to_json).collect::<Vec<_>>()});
    let _ = fs::write(path, serde_json::to_string_pretty(&doc).unwrap_or_default());
}

/// How the machine changed between the oldest recorded sample and now.
#[derive(Clone, Debug, PartialEq)]
pub struct Trend {
    pub samples: usize,
    pub span_ms: u128,
    pub servers_delta: i64,
    pub agents_delta: i64,
    pub rss_delta_kb: i64,
    pub collisions_delta: i64,
    pub stale_delta: i64,
    pub host_mem_delta_kb: i64,
}

/// Compare the oldest sample with the newest. `None` until two samples exist.
pub fn trend() -> Option<Trend> {
    let history = history();
    let first = history.first()?;
    let last = history.last()?;
    if history.len() < 2 {
        return None;
    }
    Some(Trend {
        samples: history.len(),
        span_ms: last.at_unix_ms.saturating_sub(first.at_unix_ms),
        servers_delta: last.mcp_servers as i64 - first.mcp_servers as i64,
        agents_delta: last.mcp_agents as i64 - first.mcp_agents as i64,
        rss_delta_kb: last.mcp_rss_kb as i64 - first.mcp_rss_kb as i64,
        collisions_delta: last.mcp_collisions as i64 - first.mcp_collisions as i64,
        stale_delta: last.stale_studios as i64 - first.stale_studios as i64,
        host_mem_delta_kb: last.host_mem_used_kb as i64 - first.host_mem_used_kb as i64,
    })
}

/// A "better" delta is a smaller number of processes or bytes.
pub fn improved(delta: i64) -> bool {
    delta < 0
}

/// "(no change)" or a signed, human-sized number.
pub fn delta_text(delta: i64, unit: &str) -> String {
    if delta == 0 {
        return "no change".to_string();
    }
    let sign = if delta < 0 { "−" } else { "+" };
    let magnitude = delta.abs();
    if unit == "MiB" {
        format!("{sign}{:.1} MiB", magnitude as f64 / 1024.0)
    } else {
        format!("{sign}{magnitude} {unit}")
    }
}

/// Host memory used and total, in KiB, read from /proc/meminfo.
fn host_memory() -> (u64, u64) {
    let Ok(text) = fs::read_to_string("/proc/meminfo") else { return (0, 0) };
    let value = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let total = value("MemTotal:");
    let available = value("MemAvailable:");
    (total.saturating_sub(available), total)
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(servers: u64, rss: u64) -> Sample {
        Sample {
            at_unix_ms: 1_000,
            mcp_servers: servers as usize,
            mcp_agents: servers as usize,
            mcp_rss_kb: rss,
            mcp_collisions: 0,
            studios: 1,
            stale_studios: 0,
            host_mem_used_kb: 1_000,
            host_mem_total_kb: 2_000,
        }
    }

    #[test]
    fn a_smaller_process_count_and_less_memory_read_as_improvement() {
        assert!(improved(-5), "fewer servers is better");
        assert!(improved(-1024 * 1024), "less memory is better");
        assert!(!improved(5), "more servers is worse");
        assert!(!improved(0), "no change is not an improvement");
    }

    #[test]
    fn delta_text_is_signed_and_human_sized() {
        assert_eq!(delta_text(0, "servers"), "no change");
        assert_eq!(delta_text(-5, "servers"), "−5 servers");
        assert_eq!(delta_text(5, "servers"), "+5 servers");
        assert_eq!(delta_text(-2048, "MiB"), "−2.0 MiB");
    }

    #[test]
    fn a_sample_round_trips_through_json() {
        let s = sample(3, 4096);
        let back = Sample::from_json(&s.to_json()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn a_sample_without_every_field_is_not_read_as_one() {
        let mut doc = sample(1, 1).to_json();
        doc.as_object_mut().unwrap().remove("mcp_collisions");
        assert!(Sample::from_json(&doc).is_none());
    }

    #[test]
    fn capturing_a_sample_measures_the_servers_it_was_given() {
        let servers = vec![
            crate::mcp_watch::Server {
                pid: 1,
                host_pid: 10,
                host: "claude".into(),
                family: "mc".into(),
                exe: "mc".into(),
                write_root: None,
                age_secs: 5,
                rss_kb: 1024,
            },
            crate::mcp_watch::Server {
                pid: 2,
                host_pid: 10,
                host: "claude".into(),
                family: "mc".into(),
                exe: "mc".into(),
                write_root: None,
                age_secs: 5,
                rss_kb: 2048,
            },
        ];
        let s = Sample::capture(&servers, 2, 3);
        assert_eq!(s.mcp_servers, 2);
        assert_eq!(s.mcp_agents, 1, "one agent started both");
        assert_eq!(s.mcp_rss_kb, 3072);
        assert_eq!(s.studios, 2);
        assert_eq!(s.stale_studios, 3);
        assert!(s.host_mem_total_kb > 0, "host memory is read from /proc");
    }

    #[test]
    fn a_trend_needs_two_samples_and_reports_each_direction() {
        // No history yet: no trend to report.
        let first = sample(10, 1000);
        let second = sample(4, 400);
        let change = Trend {
            samples: 2,
            span_ms: 60_000,
            servers_delta: second.mcp_servers as i64 - first.mcp_servers as i64,
            agents_delta: second.mcp_agents as i64 - first.mcp_agents as i64,
            rss_delta_kb: second.mcp_rss_kb as i64 - first.mcp_rss_kb as i64,
            collisions_delta: 0,
            stale_delta: -2,
            host_mem_delta_kb: -4096,
        };
        assert_eq!(change.servers_delta, -6, "fewer servers");
        assert_eq!(change.rss_delta_kb, -600, "less memory");
        assert!(improved(change.servers_delta), "fewer servers is an improvement");
        assert!(improved(change.rss_delta_kb), "less memory is an improvement");
        assert!(improved(change.stale_delta), "fewer stale sockets is an improvement");
        assert!(!improved(0), "no change is not an improvement");
    }
}
