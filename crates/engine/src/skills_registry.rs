//! Skill registry: knowledge-discovery info blocks for the UI.
//!
//! Each "skill" is a tech skill/tool (e.g. ffmpeg-skill) logged with what it does,
//! whether it is implemented in Magic Studio yet, and where it would plug in.
//! Persisted to ~/.config/magicstudio/skills.json (same config dir as
//! recent_projects.json and settings.json). The UI renders one info block per
//! skill in a dedicated Skills view.

use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{PathBuf},
};

/// ~/.config/magicstudio/skills.json (XDG_CONFIG_HOME honoured like the catalog).
pub fn file() -> Result<PathBuf> {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("magicstudio"))
        .ok_or_else(|| EngineError::InvalidParams("Config directory unavailable".into()))?;
    fs::create_dir_all(&dir).map_err(|e| EngineError::Io(e))?;
    Ok(dir.join("skills.json"))
}

fn parse(bytes: &[u8]) -> Result<Value> {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .filter(Value::is_object)
        .ok_or_else(|| EngineError::InvalidParams("Invalid skills.json; repair it explicitly".into()))
}

/// Read the stored registry. Empty / missing file -> {}.
fn read_skills() -> Result<Value> {
    let mut input = match fs::File::open(file()?) {
        Ok(input) => input,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(e) => return Err(EngineError::Io(e)),
    };
    input.lock_shared().map_err(|e| EngineError::Io(e))?;
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes).map_err(|e| EngineError::Io(e))?;
    if bytes.is_empty() {
        return Ok(json!({}));
    }
    parse(&bytes)
}

/// Atomically rewrite the whole registry file in place (preserves the inode).
fn write_skills(skills: &Value) -> Result<()> {
    let path = file()?;
    let mut output = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| EngineError::Io(e))?;
    output.lock().map_err(|e| EngineError::Io(e))?;
    let bytes = serde_json::to_vec_pretty(skills)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    output.seek(SeekFrom::Start(0)).map_err(|e| EngineError::Io(e))?;
    output.write_all(&bytes).map_err(|e| EngineError::Io(e))?;
    output.set_len(bytes.len() as u64).map_err(|e| EngineError::Io(e))?;
    output.sync_all().map_err(|e| EngineError::Io(e))?;
    Ok(())
}

/// Code-level seeds returned when the store is empty. These are not persisted
/// by a read; `skills.add` persists anything the user (or an intake) adds.
fn default_skills() -> Value {
    json!({
        "ffmpeg-skill": {
            "id": "ffmpeg-skill",
            "name": "ffmpeg-skill",
            "description": "Local video editor for coding agents: 42 FFmpeg tools with a machine-readable contract and MCP server. Cut, captions, reframe, loudness, platform exports. No cloud, no API keys.",
            "status": "researching",
            "source": "https://github.com/kajisho5/ffmpeg-skill",
            "implemented_at": [],
            "implemented_in": [],
            "integration_notes": "Maps onto Magic Studio delivery: export.py presets -> video.export, check.py --platform -> delivery QC, loudness.py -> audio loudness, render.py --export-timeline fcpxml/edl/otio -> FilmCraft round-trip. Not yet wired into the engine."
        }
    })
}

/// Merge stored skills over the seeds, sorted by name.
fn merged_skills() -> Result<Value> {
    let stored = read_skills()?;
    let defaults = default_skills();
    let mut merged = defaults.as_object().cloned().unwrap_or_default();
    if let Some(obj) = stored.as_object() {
        for (k, v) in obj {
            merged.insert(k.clone(), v.clone());
        }
    }
    let mut items: Vec<Value> = merged
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    items.sort_by(|a, b| {
        let an = a.get("name").and_then(Value::as_str).unwrap_or("");
        let bn = b.get("name").and_then(Value::as_str).unwrap_or("");
        an.cmp(bn)
    });
    Ok(json!({"skills": items, "count": items.len()}))
}

/// `skills.list` — every registered skill as info-block data.
pub fn cmd_skills_list(_s: &mut Session, _: &Value) -> Result<Value> {
    let merged = merged_skills()?;
    let mut out = json!({"ok": true});
    if let Some(obj) = merged.as_object() {
        for (k, v) in obj {
            out[k.clone()] = v.clone();
        }
    }
    Ok(out)
}

/// `skills.info <id>` — one skill's full block.
pub fn cmd_skills_info(_s: &mut Session, params: &Value) -> Result<Value> {
    let id = params
        .get("id")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing skill 'id'".into()))?;
    let merged = merged_skills()?;
    let skills = merged
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| EngineError::InvalidParams("skills registry is not a list".into()))?;
    let skill = skills
        .iter()
        .find(|v| v.get("id").and_then(Value::as_str) == Some(id))
        .cloned()
        .ok_or_else(|| EngineError::InvalidParams(format!("skill '{id}' not registered")))?;
    Ok(json!({"ok": true, "skill": skill}))
}

/// `skills.add` — register (or update) one skill. Persisted to skills.json.
pub fn cmd_skills_add(_s: &mut Session, params: &Value) -> Result<Value> {
    let id = params
        .get("id")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing skill 'id'".into()))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing skill 'name'".into()))?;
    let description = params
        .get("description")
        .and_then(Value::as_str)
        .map(|v| v.to_string())
        .unwrap_or_default();
    let status = params
        .get("status")
        .and_then(Value::as_str)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "researching".to_string());
    let source = params
        .get("source")
        .and_then(Value::as_str)
        .map(|v| v.to_string())
        .unwrap_or_default();
    let implemented_at = params
        .get("implemented_at")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let implemented_in = params
        .get("implemented_in")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let integration_notes = params
        .get("integration_notes")
        .and_then(Value::as_str)
        .map(|v| v.to_string())
        .unwrap_or_default();

    let mut skills = read_skills()?;
    let skills_obj = skills
        .as_object_mut()
        .ok_or_else(|| EngineError::InvalidParams("skills registry is not an object".into()))?;
    skills_obj.insert(
        id.to_string(),
        json!({
            "id": id,
            "name": name,
            "description": description,
            "status": status,
            "source": source,
            "implemented_at": implemented_at,
            "implemented_in": implemented_in,
            "integration_notes": integration_notes,
        }),
    );
    let skill = skills_obj.get(id).cloned().unwrap_or_default();
    write_skills(&skills)?;
    Ok(json!({"ok": true, "skill": skill}))
}
