//! Stubbed GitHub/Git health — the animation TUI does not poll git or GitHub PRs.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn repo_root(start: &Path) -> PathBuf {
    start.to_path_buf()
}

pub fn empty() -> Value {
    json!({
        "schema": "mc.github-health/1",
        "ok": true,
        "refreshing": false,
        "observed_at_ms": null,
        "git": {},
        "pull_requests": null,
        "errors": []
    })
}

pub fn inspect(_repo: &Path) -> Value {
    empty()
}
