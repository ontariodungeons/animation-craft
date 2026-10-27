//! Minimal MCP presence stub — no subprocesses (`omarchy`/`herdr`) or `/proc` scans.
use serde_json::Value;

pub struct Guard;

pub fn announce(_bridge: Option<&str>) -> Guard {
    Guard
}

pub fn sessions() -> Vec<Value> {
    Vec::new()
}

pub fn orientation(_bridge: Option<&str>) -> String {
    format!(
        "\n\nMagic Studio (mc {}) — direct CLI execution; projects are .magic files.",
        env!("CARGO_PKG_VERSION")
    )
}
