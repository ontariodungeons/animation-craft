//! Stubbed MCP process watcher — direct CLI execution does not spawn persistent MCP servers.
use std::collections::BTreeMap;

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

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub family: String,
    pub text: String,
}

pub fn family(_argv: &[String]) -> Option<String> {
    None
}

pub fn scan() -> Vec<Server> {
    Vec::new()
}

pub fn analyze(servers: &[Server]) -> (BTreeMap<String, Vec<&Server>>, Vec<Finding>) {
    let mut by: BTreeMap<String, Vec<&Server>> = BTreeMap::new();
    for s in servers {
        by.entry(s.family.clone()).or_default().push(s);
    }
    (by, Vec::new())
}
