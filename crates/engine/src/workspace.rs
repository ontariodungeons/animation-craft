//! Workspace & external Craft CLI resolution.
//!
//! Resolves the repository root (`studio_pipeline_cli.py`), the installed Craft
//! CLIs (`photocraft-cli`, `effectcraft-cli`, `filmcraft-cli`), builds exact
//! CLI argument vectors, and runs subprocess tools. Pure path logic: no session
//! state is mutated here.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result};

/// Return the canonical current Tauri/engine workspace, independent of the caller working directory.
pub fn workspace_root(s: &crate::Session) -> PathBuf {
    let current_root = |root:&Path| root.join("Cargo.toml").is_file() && root.join("apps/magicstudio-shell/Cargo.toml").is_file();
    if current_root(&s.cwd) {
        return s.cwd.clone();
    }
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Ok(canon) = manifest_root.canonicalize()
        && current_root(&canon)
    {
        return canon;
    }
    s.cwd.clone()
}

/// Resolve a potentially relative path against the session workspace root.
pub fn resolve_workspace_path(s: &crate::Session, raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let from_cwd = s.cwd.join(p);
    if from_cwd.exists() {
        return from_cwd;
    }
    workspace_root(s).join(p)
}

/// Resolve an installed Craft CLI binary (`photocraft-cli`, `effectcraft-cli`, `filmcraft-cli`),
/// preferring an explicit `<TOOL_NAME>` env override, then `~/.local/share/artcraft-launcher/installed.json`, then `PATH`.
pub fn resolve_craft_cli(name: &str) -> String {
    std::env::var(name.to_uppercase().replace('-', "_"))
        .ok()
        .filter(|p| !p.is_empty())
        .or_else(|| resolve_artcraft_launcher_cli(name))
        .unwrap_or_else(|| name.to_string())
}

fn resolve_artcraft_launcher_cli(name: &str) -> Option<String> {
    let is_cli = name.ends_with("-cli");
    let app = name.strip_suffix("-cli").unwrap_or(name);
    let home = std::env::var("HOME").ok()?;
    let installed_path = Path::new(&home).join(".local/share/artcraft-launcher/installed.json");
    let text = std::fs::read_to_string(installed_path).ok()?;
    let parsed: Value = serde_json::from_str(&text).ok()?;
    let app_entry = &parsed["apps"][app];
    if !is_cli {
        if let Some(appimage) = app_entry["path"].as_str() {
            let p = Path::new(appimage);
            if p.is_file() {
                return Some(p.to_string_lossy().into_owned());
            }
        }
    }
    let ver = app_entry["version"].as_str()?;
    let cli_name = format!("{app}-cli");
    let extracted = Path::new(&home)
        .join(".local/share")
        .join(app)
        .join(ver)
        .join(&cli_name);
    if extracted.is_file() {
        return Some(extracted.to_string_lossy().into_owned());
    }
    None
}

/// Build the exact CLI argument vector for `photocraft-cli run <input> --cmd <id> [--params <json>] --out <output>`.
pub fn build_photocraft_run_args(
    input: &str,
    cmd_id: &str,
    params_json: Option<&str>,
    output: &str,
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        input.to_string(),
        "--cmd".to_string(),
        cmd_id.to_string(),
    ];
    if let Some(p) = params_json {
        args.push("--params".to_string());
        args.push(p.to_string());
    }
    args.push("--out".to_string());
    args.push(output.to_string());
    args
}

/// Build the exact CLI argument vector for `effectcraft-cli script <script.jsx>`.
pub fn build_effectcraft_script_args(script_path: &str) -> Vec<String> {
    vec!["script".to_string(), script_path.to_string()]
}

/// Build the exact CLI argument vector for `studio_pipeline_cli.py filmcraft-load <media> --project <fcproj> --frame-out <png>`.
pub fn build_filmcraft_load_args(media: &str, project: &str, frame_out: &str) -> Vec<String> {
    vec![
        "filmcraft-load".to_string(),
        media.to_string(),
        "--project".to_string(),
        project.to_string(),
        "--frame-out".to_string(),
        frame_out.to_string(),
    ]
}

/// Build the exact CLI argument vector for `studio_pipeline_cli.py blender-check`.
pub fn build_blender_check_args(
    archetype: &str,
    verb: &str,
    out_mp4: &str,
    out_png: &str,
    glb: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "blender-check".to_string(),
        "--archetype".to_string(),
        archetype.to_string(),
        "--verb".to_string(),
        verb.to_string(),
        "--out-mp4".to_string(),
        out_mp4.to_string(),
        "--out-png".to_string(),
        out_png.to_string(),
    ];
    if let Some(g) = glb {
        args.push("--glb".to_string());
        args.push(g.to_string());
    }
    args
}

pub(crate) struct ToolSlotGuard;
impl Drop for ToolSlotGuard {
    fn drop(&mut self) {
        let (lock, cvar) = tool_limiter();
        if let Ok(mut active) = lock.lock() {
            *active = active.saturating_sub(1);
            cvar.notify_one();
        }
    }
}

fn tool_limiter() -> &'static (std::sync::Mutex<usize>, std::sync::Condvar) {
    static LIMITER: std::sync::OnceLock<(std::sync::Mutex<usize>, std::sync::Condvar)> =
        std::sync::OnceLock::new();
    LIMITER.get_or_init(|| (std::sync::Mutex::new(0), std::sync::Condvar::new()))
}

pub(crate) fn acquire_tool_slot() -> ToolSlotGuard {
    let (lock, cvar) = tool_limiter();
    if let Ok(mut active) = lock.lock() {
        while *active >= 4 {
            active = match cvar.wait(active) {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
        }
        *active += 1;
    }
    ToolSlotGuard
}

/// Run an external command-line tool in `cwd`, capturing stdout as a String.
pub(crate) fn run_tool_in(cwd: &Path, tool: &str, args: &[String]) -> Result<String> {
    let _slot = acquire_tool_slot();
    let executable = if tool.ends_with("-cli") && !tool.contains('/') {
        resolve_craft_cli(tool)
    } else {
        tool.to_string()
    };
    let output = std::process::Command::new(&executable)
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|e| EngineError::ExternalTool(tool.to_string(), e.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        Err(EngineError::ExternalTool(
            tool.to_string(),
            format!("exit {:?}: {detail}", output.status.code()),
        ))
    }
}

/// Run `python3 <workspace>/studio_pipeline_cli.py <args...>` and parse JSON output when possible.
pub(crate) fn run_pipeline(s: &crate::Session, args: &[String]) -> Result<Value> {
    let root = workspace_root(s);
    let script = root.join("studio_pipeline_cli.py");
    if !script.is_file() {
        return Err(EngineError::ExternalTool(
            "studio_pipeline_cli.py".to_string(),
            format!("not found at {}", script.display()),
        ));
    }
    let mut full_args = Vec::with_capacity(args.len() + 1);
    full_args.push(script.to_string_lossy().into_owned());
    full_args.extend_from_slice(args);
    let stdout = run_tool_in(&root, "python3", &full_args)?;
    Ok(serde_json::from_str(&stdout).unwrap_or_else(|_| json!({ "raw": stdout })))
}

/// Audio file extensions recognized by the workshop.
pub(crate) const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "m4a", "aac", "ogg"];

/// True when `path` has one of `allowed` extensions (case-insensitive).
pub(crate) fn has_extension(path: &Path, allowed: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| allowed.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// Validate and resolve an audio path for the workshop song.
pub(crate) fn song_audio_path(s: &crate::Session, raw: &str) -> Result<PathBuf> {
    let path = resolve_workspace_path(s, raw);
    if !path.is_file() || !has_extension(&path, AUDIO_EXTENSIONS) {
        return Err(EngineError::InvalidParams(format!("not an audio file: {}", path.display())));
    }
    Ok(path)
}

pub(crate) fn try_sanitize_asset_id(raw: &str) -> Option<String> {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '_' || ch == '-' || ch == ' ') && !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Sanitize a user-supplied or file-stem name into a stable asset id.
pub fn sanitize_asset_id(raw: &str) -> String {
    try_sanitize_asset_id(raw).unwrap_or_else(|| "asset".to_string())
}

/// Derive a sanitized asset id from an optional user-typed name, falling back to the file stem
/// only when no name is typed.
pub fn asset_id_for_import(typed_name: Option<&str>, path: &Path) -> String {
    typed_name
        .and_then(try_sanitize_asset_id)
        .or_else(|| {
            path.file_stem()
                .and_then(|st| st.to_str())
                .and_then(try_sanitize_asset_id)
        })
        .unwrap_or_else(|| "asset".to_string())
}
