//! One production project root, independent of navigation history and per-intake overrides.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub const DEFAULT_PROJECTS_ROOT: &str = "/run/media/jesse/Speedy/media";

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn file() -> Result<PathBuf> {
    crate::project_catalog::file()
        .and_then(|p| p.parent().map(|p| p.join("settings.json")))
        .ok_or_else(|| {
            invalid("Magic Studio config directory unavailable; no recent-project fallback used")
        })
}

fn parse(bytes: &[u8]) -> Result<Value> {
    serde_json::from_slice::<Value>(bytes).ok().filter(Value::is_object)
        .ok_or_else(||invalid("Invalid Magic Studio settings.json; repair it explicitly. No history fallback used"))
}

fn stored() -> Result<Value> {
    let mut input = match fs::File::open(file()?) {
        Ok(input) => input,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(e) => return Err(e.into()),
    };
    input.lock_shared()?;
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    parse(&bytes)
}

fn absolute(value: &str) -> Result<PathBuf> {
    let path = Path::new(value.trim());
    if value.trim().is_empty() || !path.is_absolute() || value.chars().any(char::is_control) {
        return Err(invalid(
            "Project root must be a nonempty absolute directory path",
        ));
    }
    Ok(path.to_path_buf())
}

fn configured_root(settings: &Value) -> Result<PathBuf> {
    match settings.get("projects_root") {
        Some(v) => absolute(
            v.as_str()
                .ok_or_else(|| invalid("Saved projects_root must be a string"))?,
        ),
        None => Ok(PathBuf::from(DEFAULT_PROJECTS_ROOT)),
    }
}

pub fn root() -> Result<PathBuf> {
    configured_root(&stored()?)
}

/// Without `root`, inspect the setting. Only this explicit command changes it.
pub fn command(_: &mut Session, params: &Value) -> Result<Value> {
    if let Some(value) = params.get("root") {
        let root = absolute(
            value
                .as_str()
                .ok_or_else(|| invalid("Project root must be a string"))?,
        )?;
        if !root.is_dir() {
            return Err(invalid(
                "Project root directory is unavailable; mount or choose the intended directory first",
            ));
        }
        let root = fs::canonicalize(root)?;
        let path = file()?;
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| invalid("Config directory unavailable"))?,
        )?;
        let mut output = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        output.lock()?;
        let mut bytes = Vec::new();
        output.read_to_end(&mut bytes)?;
        let mut settings = if bytes.is_empty() {
            json!({})
        } else {
            parse(&bytes)?
        };
        settings["projects_root"] = json!(root);
        let bytes = serde_json::to_vec_pretty(&settings).map_err(|e| invalid(e.to_string()))?;
        output.seek(SeekFrom::Start(0))?;
        output.write_all(&bytes)?;
        output.set_len(bytes.len() as u64)?;
        output.sync_all()?;
    }
    let settings = stored()?;
    Ok(
        json!({"root":configured_root(&settings)?,"config":file()?,"persisted":settings.get("projects_root").is_some(),"source":if settings.get("projects_root").is_some(){"saved production setting"}else{"production default"},"overrides":"project.intake root= or path= applies only to that request; recent history never changes this setting"}),
    )
}
