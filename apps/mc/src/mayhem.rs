//! Stubbed MCP collision history — direct CLI execution does not use shared MCP daemons.
use crate::mcp_watch;

#[derive(Clone, Debug, Default)]
pub struct Sample {
    pub ts: u64,
    pub mcp_servers: usize,
    pub mcp_agents: usize,
    pub mcp_rss_kb: u64,
    pub mcp_collisions: usize,
    pub studios: usize,
    pub stale_studios: usize,
    pub host_mem_used_kb: u64,
    pub host_mem_total_kb: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Trend {
    pub span_ms: u64,
    pub samples: usize,
    pub servers_delta: i64,
    pub agents_delta: i64,
    pub rss_delta_kb: i64,
    pub collisions_delta: i64,
    pub stale_delta: i64,
    pub host_mem_delta_kb: i64,
}

impl Sample {
    pub fn capture(_servers: &[mcp_watch::Server], _studios: usize, _stale: usize) -> Self {
        Self::default()
    }
}

pub fn record(_s: &Sample) {}

pub fn history() -> Vec<Sample> {
    Vec::new()
}

pub fn trend() -> Option<Trend> {
    None
}

pub fn improved(_delta: i64) -> bool {
    false
}

pub fn delta_text(_delta: i64, unit: &str) -> String {
    format!("0 {unit}")
}
