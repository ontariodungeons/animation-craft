//! Minimal scope stub — no agent RBAC or Project Booth permission barriers.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const ENV: &str = "MAGICSTUDIO_ASSIGNMENT";

#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub file: PathBuf,
    pub project: PathBuf,
    pub project_dir: PathBuf,
    pub role: String,
    pub members: Vec<String>,
    pub allow_mc_commands: Vec<String>,
    pub writable_paths: Vec<PathBuf>,
    pub visible_panes: Vec<String>,
}

pub fn current() -> Result<Option<&'static Scope>, String> {
    Ok(None)
}

impl Scope {
    pub fn key(&self) -> String {
        "default".into()
    }

    pub fn permits(&self, _id: &str) -> bool {
        true
    }

    pub fn params(&self, _id: &str, params: Value) -> Result<Value, String> {
        Ok(params)
    }

    pub fn commands(&self, _filter: &str) -> Value {
        json!({ "commands": [] })
    }

    pub fn catalog(&self) -> Value {
        json!({ "projects": [] })
    }

    pub fn inherit(&self, _command: &mut std::process::Command) {}

    pub fn check_command(&self, _id: &str, _params: &Value) -> Result<(), String> {
        Ok(())
    }

    pub fn check_project_arg(&self, _path: &Path) -> Result<(), String> {
        Ok(())
    }

    pub fn check_tui_action(&self, _action: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn allows_literal(&self, _literal: &str) -> bool {
        true
    }

    pub fn roster(&self, agents: Vec<Value>) -> Vec<Value> {
        agents
    }

    pub fn redact(&self, state: Value) -> Value {
        state
    }
}
