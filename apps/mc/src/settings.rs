//! TUI settings: configurable values stored in ~/.config/magicstudio/tui-settings.json.
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

pub struct TuiSettings {
    pub process_count: usize,
    pub agent_refresh_secs: u64,
    pub studio_discovery_secs: u64,
    pub mcp_scan_secs: u64,
    pub log_tail_lines: usize,
    pub proc_rss_threshold_kb: u64,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            process_count: 18,
            agent_refresh_secs: 3,
            studio_discovery_secs: 3,
            mcp_scan_secs: 5,
            log_tail_lines: 10,
            proc_rss_threshold_kb: 500_000,
        }
    }
}

impl TuiSettings {
    pub fn file() -> Option<PathBuf> {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("magicstudio").join("tui-settings.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::file() else {
            return Self::default();
        };
        let Ok(bytes) = fs::read(&path) else {
            return Self::default();
        };
        let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
            return Self::default();
        };
        Self {
            process_count: v["process_count"]
                .as_u64()
                .map(|n| n as usize)
                .unwrap_or(18),
            agent_refresh_secs: v["agent_refresh_secs"].as_u64().unwrap_or(3),
            studio_discovery_secs: v["studio_discovery_secs"].as_u64().unwrap_or(3),
            mcp_scan_secs: v["mcp_scan_secs"].as_u64().unwrap_or(5),
            log_tail_lines: v["log_tail_lines"]
                .as_u64()
                .map(|n| n as usize)
                .unwrap_or(10),
            proc_rss_threshold_kb: v["proc_rss_threshold_kb"].as_u64().unwrap_or(500_000),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::file().ok_or("Config directory unavailable")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let v = json!({
            "process_count": self.process_count,
            "agent_refresh_secs": self.agent_refresh_secs,
            "studio_discovery_secs": self.studio_discovery_secs,
            "mcp_scan_secs": self.mcp_scan_secs,
            "log_tail_lines": self.log_tail_lines,
            "proc_rss_threshold_kb": self.proc_rss_threshold_kb,
        });
        fs::write(
            &path,
            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    pub fn to_json(&self) -> Value {
        json!({
            "process_count": self.process_count,
            "agent_refresh_secs": self.agent_refresh_secs,
            "studio_discovery_secs": self.studio_discovery_secs,
            "mcp_scan_secs": self.mcp_scan_secs,
            "log_tail_lines": self.log_tail_lines,
            "proc_rss_threshold_kb": self.proc_rss_threshold_kb,
        })
    }
}
