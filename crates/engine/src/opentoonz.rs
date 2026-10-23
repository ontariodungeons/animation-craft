//! OpenToonz source adapter: render an already-authored `.tnz` scene to PNG frames with
//! `tcomposer`. It never authors rigs or edits scenes, and it owns no scheduler: the shared
//! bounded worker runs `opentoonz.render` and owns limits, status and cancellation.
//!
//! Invocation facts come from OpenToonz `065cc14` (`tcomposer.cpp`, `sceneproperties.cpp`,
//! `tcamera.cpp`, `tfilepath.cpp`): `-range` is one-based, a scene's stored output range is
//! zero-based with `0 -1` meaning "whole scene", files are named `frame.NNNN.png` by default
//! (a scene may change padding and separator), and there is no FPS or size override.
use std::{ffi::OsString, fs, io::Read, path::{Path, PathBuf}, process::Command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use crate::{EngineError, Result, Session};

const SCENE_LIMIT: u64 = 64 * 1024 * 1024;
const MAX_FRAMES: u64 = 18000;
const MISSING_TOOL: &str = "OpenToonz tcomposer is not installed or not on PATH. Set OPENTOONZ_TCOMPOSER to its absolute path; no installation was attempted";
const STUFF_CANDIDATES: &[&str] = &["/usr/share/opentoonz/stuff", "/opt/opentoonz/stuff", "/usr/local/share/opentoonz/stuff"];
// Default project folder aliases; a project file may redefine them (reported, not guessed).
const ALIASES: &[&str] = &["drawings", "extras", "inputs", "outputs", "scenes", "scripts", "palettes"];

fn invalid(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }

/// Resolve tcomposer from an explicit path, then `PATH`. Never installs or guesses further.
fn find_tcomposer(explicit: Option<OsString>, path_var: Option<OsString>) -> Result<PathBuf> {
    if let Some(path) = explicit.map(PathBuf::from) {
        if path.is_absolute() && path.is_file() { return Ok(path); }
        return Err(invalid(format!("OPENTOONZ_TCOMPOSER must be an existing absolute file: {}", path.display())));
    }
    path_var.and_then(|v| std::env::split_paths(&v).map(|d| d.join("tcomposer")).find(|p| p.is_file()))
        .ok_or_else(|| invalid(MISSING_TOOL))
}

fn tcomposer() -> Result<PathBuf> {
    find_tcomposer(std::env::var_os("OPENTOONZ_TCOMPOSER"), std::env::var_os("PATH"))
}

/// tcomposer needs the OpenToonz "stuff" resource folder (passed as `-TOONZROOT`).
fn stuff_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/OpenToonz/stuff"));
    std::env::var_os("OPENTOONZ_STUFF").map(PathBuf::from)
        .into_iter()
        .chain(STUFF_CANDIDATES.iter().map(PathBuf::from))
        .chain(home)
        .find(|p| p.is_dir())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut fs::File::open(path)?, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

// ---------- minimal .tnz reading (the fields tcomposer uses; no general XML model) ----------

/// Inner text of every `<name ...>…</name>` element, in document order.
fn elements<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{name}"), format!("</{name}>"));
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        // Skip longer tag names that share the prefix (`<range` vs `<rangeX`).
        if !after.starts_with(['>', ' ', '\t', '\n', '\r', '/']) { rest = after; continue; }
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') { found.push(""); rest = &after[gt + 1..]; continue; }
        let body = &after[gt + 1..];
        let Some(end) = body.find(&close) else { break };
        found.push(&body[..end]);
        rest = &body[end + close.len()..];
    }
    found
}

fn element<'a>(xml: &'a str, name: &str) -> Option<&'a str> { elements(xml, name).into_iter().next() }

fn numbers(text: &str) -> Vec<f64> { text.split_whitespace().filter_map(|t| t.parse().ok()).collect() }

/// The `<output name="main">` block of a scene: the settings tcomposer renders with.
fn main_output(xml: &str) -> Option<&str> {
    let outputs = element(xml, "outputs")?;
    let mut rest = outputs;
    while let Some(start) = rest.find("<output") {
        let after = &rest[start + 7..];
        let gt = after.find('>')?;
        let body = &after[gt + 1..];
        let end = body.find("</output>")?;
        if after[..gt].contains("name=\"main\"") || after[..gt].contains("name='main'") { return Some(&body[..end]); }
        rest = &body[end..];
    }
    None
}

/// Scene facts tcomposer will use: frame count, main-output range/step/shrink/fps, camera.
fn scene_info(scene: &Path) -> Result<Value> {
    if scene.extension().is_none_or(|e| e != "tnz") { return Err(invalid("scene must be an OpenToonz .tnz file")); }
    let size = fs::metadata(scene)?.len();
    if size > SCENE_LIMIT { return Err(invalid("scene file is larger than 64 MiB")); }
    let mut xml = String::new();
    fs::File::open(scene)?.read_to_string(&mut xml).map_err(|_| invalid("scene is not a text .tnz document"))?;
    let root = xml.find("<tnz").map(|i| &xml[i..]).ok_or_else(|| invalid("scene has no <tnz> root element"))?;
    let root_tag = &root[..root.find('>').unwrap_or(0)];
    let frame_count = root_tag.split("framecount=").nth(1)
        .and_then(|v| v.trim_start_matches(['"', '\'']).split(['"', '\'']).next())
        .and_then(|v| v.parse::<u64>().ok());
    let out = main_output(&xml);
    let pair = |name: &str| out.and_then(|o| element(o, name)).map(numbers);
    let range = pair("range").filter(|r| r.len() == 2).map(|r| (r[0] as i64, r[1] as i64));
    let step = pair("step").and_then(|v| v.first().copied()).map(|v| v as u64);
    let shrink = pair("shrink").and_then(|v| v.first().copied()).map(|v| v as u64);
    let fps = pair("fps").and_then(|v| v.first().copied()).filter(|f| *f > 0.0);
    let cameras: Vec<Value> = element(&xml, "cameras").map(|c| elements(c, "camera")).unwrap_or_default().iter()
        .filter_map(|c| element(c, "cameraRes").map(numbers).filter(|r| r.len() == 2))
        .map(|r| json!([r[0] as u64, r[1] as u64])).collect();
    Ok(json!({"frame_count":frame_count,"output_range_zero_based":range.map(|(a,b)|json!([a,b])),
        "step":step,"shrink":shrink,"fps":fps,"camera_resolutions":cameras,
        "frame_format":out.and_then(|o|element(o,"frameFormat")).map(|f|json!({"padding":element(f,"padding"),"sepchar":element(f,"sepchar")}))}))
}

// ---------- source bundle ----------

/// The project root holds the folders `+drawings` etc. resolve against.
fn project_root(scene: &Path) -> Option<PathBuf> {
    scene.ancestors().skip(1).find(|dir| fs::read_dir(dir).ok().is_some_and(|mut entries| entries.any(|e| {
        e.ok().and_then(|e| e.file_name().into_string().ok()).is_some_and(|n| n.ends_with("_otprj.xml") || n == "scenes.xml")
    }))).map(Path::to_path_buf)
}

/// Files a level path names: one file, or every frame of a `name..png` sequence.
fn level_files(path: &Path) -> Vec<PathBuf> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let Some((stem, ext)) = name.split_once("..") else { return vec![path.to_path_buf()] };
    let mut frames: Vec<PathBuf> = path.parent().and_then(|d| fs::read_dir(d).ok()).into_iter().flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            n.strip_prefix(stem).and_then(|r| r.strip_suffix(&format!(".{ext}")))
                .and_then(|r| r.strip_prefix(['.', '_'])).is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        })).collect();
    frames.sort();
    frames
}

/// Every level the scene references, resolved and hashed. Missing files fail the job.
fn dependencies(scene: &Path, xml: &str) -> Result<(Option<PathBuf>, Vec<Value>)> {
    let root = project_root(scene);
    let levels = element(xml, "levelSet").unwrap_or_default();
    let mut paths: Vec<&str> = elements(levels, "path");
    let mut rest = levels;
    while let Some(i) = rest.find("path=\"") {
        let tail = &rest[i + 6..];
        let Some(end) = tail.find('"') else { break };
        paths.push(&tail[..end]);
        rest = &tail[end..];
    }
    let (mut found, mut missing, mut unresolved) = (Vec::new(), Vec::new(), Vec::new());
    for raw in paths.into_iter().map(str::trim).filter(|p| !p.is_empty()) {
        let resolved = if let Some(aliased) = raw.strip_prefix('+') {
            let (alias, tail) = aliased.split_once('/').unwrap_or((aliased, ""));
            match (&root, ALIASES.contains(&alias)) {
                (Some(root), true) => root.join(alias).join(tail),
                _ => { unresolved.push(raw.to_owned()); continue; }
            }
        } else if Path::new(raw).is_absolute() { PathBuf::from(raw) } else { scene.parent().unwrap_or(Path::new(".")).join(raw) };
        let files = level_files(&resolved);
        if files.is_empty() || files.iter().any(|f| !f.is_file()) { missing.push(raw.to_owned()); continue; }
        for file in files {
            let palette = file.with_extension("tpl");
            for f in std::iter::once(file).chain((palette.is_file()).then_some(palette)) {
                if !found.iter().any(|v: &Value| v["path"] == json!(f)) { found.push(json!({"path":f,"sha256":sha256_file(&f)?})); }
            }
        }
    }
    if !unresolved.is_empty() {
        return Err(invalid(format!("Scene uses project aliases this adapter cannot resolve without a project file defining them: {}", unresolved.join(", "))));
    }
    if !missing.is_empty() { return Err(invalid(format!("Scene references missing levels: {}", missing.join(", ")))); }
    Ok((root, found))
}

// ---------- plan ----------

/// One-based inclusive range tcomposer will render, from the request or the scene.
fn resolve_range(info: &Value, p: &Value) -> Result<(u64, u64, u64)> {
    let count = info["frame_count"].as_u64();
    let (scene_first, scene_last) = match info["output_range_zero_based"].as_array().map(|r| (r[0].as_i64(), r[1].as_i64())) {
        Some((Some(0), Some(-1))) | None => (1, count),
        Some((Some(a), Some(b))) if a >= 0 && b >= a => (a as u64 + 1, Some(b as u64 + 1)),
        _ => return Err(invalid("scene has an invalid stored output range")),
    };
    let get = |key: &str| match p.get(key) { None | Some(Value::Null) => Ok(None), Some(v) => v.as_u64().filter(|n| *n >= 1).map(Some).ok_or_else(|| invalid(format!("{key} must be a positive integer"))) };
    let first = get("first")?.unwrap_or(scene_first);
    let last = get("last")?.or(scene_last).ok_or_else(|| invalid("scene frame count is unknown; pass last (one-based)"))?;
    let step = get("step")?.or(info["step"].as_u64().filter(|s| *s >= 1)).unwrap_or(1);
    if last < first { return Err(invalid("last must not be before first")); }
    if let Some(count) = count.filter(|c| last > *c) { return Err(invalid(format!("last {last} is past the scene's {count} frames"))); }
    if (last - first) / step + 1 > MAX_FRAMES { return Err(invalid(format!("range renders more than {MAX_FRAMES} frames"))); }
    Ok((first, last, step))
}

/// Validate a request without rendering: tool, scene, bundle, range and the exact argv.
pub fn plan(s: &Session, p: &Value) -> Result<Value> {
    let scene = crate::workspace::resolve_workspace_path(s, p["scene"].as_str().ok_or_else(|| invalid("scene (.tnz path) is required"))?)
        .canonicalize().map_err(|_| invalid("scene does not exist"))?;
    let info = scene_info(&scene)?;
    // tcomposer has no FPS or size qualifier: a requested value must already be the scene's.
    for (key, have) in [("fps", info["fps"].as_f64()), ("width", None), ("height", None)] {
        if let Some(want) = p.get(key).filter(|v| !v.is_null()) {
            if key != "fps" || have.is_none_or(|h| want.as_f64().is_none_or(|w| (w - h).abs() > 1e-6)) {
                return Err(invalid(format!("tcomposer cannot override {key}; change the scene's output settings in OpenToonz (scene {key}: {})", have.map_or("unknown".into(), |h| h.to_string()))));
            }
        }
    }
    let (first, last, step) = resolve_range(&info, p)?;
    let shrink = match p.get("shrink") { None | Some(Value::Null) => info["shrink"].as_u64().filter(|s| *s >= 1).unwrap_or(1),
        Some(v) => v.as_u64().filter(|s| (1..=16).contains(s)).ok_or_else(|| invalid("shrink must be 1-16"))? };
    let threads = match p.get("nthreads") { None | Some(Value::Null) => "2".to_owned(),
        Some(v) => v.as_u64().filter(|n| *n >= 1).map(|n| n.to_string()).or_else(|| v.as_str().filter(|t| ["single", "half", "all"].contains(t)).map(str::to_owned))
            .ok_or_else(|| invalid("nthreads must be a positive integer, single, half or all"))? };
    let xml = fs::read_to_string(&scene)?;
    let (project, deps) = dependencies(&scene, &xml)?;
    let out = PathBuf::from(p["out"].as_str().ok_or_else(|| invalid("out (new frames folder) is required"))?);
    if !out.is_absolute() { return Err(invalid("out must be an absolute folder path")); }
    let target = out.join("frame.png");
    let mut argv: Vec<String> = vec![scene.to_string_lossy().into_owned(), "-o".into(), target.to_string_lossy().into_owned(),
        "-range".into(), first.to_string(), last.to_string(), "-step".into(), step.to_string(),
        "-shrink".into(), shrink.to_string(), "-nthreads".into(), threads];
    let stuff = stuff_dir();
    if let Some(stuff) = &stuff { argv.extend(["-TOONZROOT".into(), stuff.to_string_lossy().into_owned()]); }
    let expected: Vec<u64> = (first..=last).step_by(step as usize).collect();
    let camera = info["camera_resolutions"].as_array().filter(|c| c.len() == 1).map(|c| c[0].clone());
    Ok(json!({"backend":"opentoonz","argv":argv,"stuff":stuff,"frames_dir":out,
        "range":{"first":first,"last":last,"step":step,"one_based":true},"expected_frames":expected,
        "fps":info["fps"],"shrink":shrink,"declared_camera_resolution":camera,"scene":info,
        "source_bundle":{"scene":scene,"sha256":sha256_file(&scene)?,"project_root":project,"dependencies":deps}}))
}

// ---------- render and collect ----------

/// What tcomposer's exit status means (OpenToonz 065cc14): -1 failed frames, -2 scene/project.
fn exit_meaning(code: Option<i32>) -> &'static str {
    match code {
        Some(1) => "invalid arguments or thread count",
        Some(2) => "out of contiguous memory",
        Some(254) | Some(-2) => "scene or project could not be loaded",
        Some(255) | Some(-1) => "one or more frames failed to render",
        None => "terminated by a signal (an undefined TOONZROOT aborts)",
        _ => "unexpected failure",
    }
}

/// PNG width, height and colour type straight from the IHDR chunk.
fn png_header(path: &Path) -> Result<(u32, u32, u8)> {
    let mut head = [0u8; 26];
    fs::File::open(path)?.read_exact(&mut head).map_err(|_| invalid(format!("{} is not a PNG", path.display())))?;
    if &head[..8] != b"\x89PNG\r\n\x1a\n" || &head[12..16] != b"IHDR" { return Err(invalid(format!("{} is not a PNG", path.display()))); }
    Ok((u32::from_be_bytes(head[16..20].try_into().unwrap()), u32::from_be_bytes(head[20..24].try_into().unwrap()), head[25]))
}

/// Map rendered files to scene frames. Gaps, extra files and mixed sizes are failures.
fn collect(plan: &Value) -> Result<Value> {
    let dir = PathBuf::from(plan["frames_dir"].as_str().unwrap_or_default());
    let expected: Vec<u64> = plan["expected_frames"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
    let mut by_frame = std::collections::BTreeMap::new();
    let mut unexpected = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        let number = name.strip_prefix("frame").and_then(|r| r.strip_suffix(".png")).and_then(|r| r.strip_prefix(['.', '_']))
            .filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())).and_then(|d| d.parse::<u64>().ok());
        match number { Some(n) if expected.contains(&n) => { by_frame.insert(n, path); }, _ if name == "tcomposer.log" => {}, _ => unexpected.push(name) }
    }
    let missing: Vec<u64> = expected.iter().copied().filter(|n| !by_frame.contains_key(n)).collect();
    if !missing.is_empty() || !unexpected.is_empty() {
        return Err(invalid(format!("Rendered frames do not match the requested range: missing {missing:?}, unexpected {unexpected:?}")));
    }
    let mut size = None;
    let mut alpha = true;
    for path in by_frame.values() {
        let (w, h, colour) = png_header(path)?;
        if size.is_some_and(|s| s != (w, h)) { return Err(invalid("Rendered frames have different sizes")); }
        size = Some((w, h));
        alpha &= matches!(colour, 4 | 6);
    }
    let (width, height) = size.ok_or_else(|| invalid("No frames were rendered"))?;
    if let (Some(cam), Some(1)) = (plan["declared_camera_resolution"].as_array(), plan["shrink"].as_u64())
        && (cam[0].as_u64() != Some(width as u64) || cam[1].as_u64() != Some(height as u64)) {
        return Err(invalid(format!("Rendered {width}x{height} does not match the scene camera {}x{}", cam[0], cam[1])));
    }
    Ok(json!({"frames":by_frame.values().collect::<Vec<_>>(),
        "frame_mapping":by_frame.iter().map(|(n, p)| json!({"scene_frame":n,"file":p})).collect::<Vec<_>>(),
        "first_frame":expected.first(),"last_frame":expected.last(),"width":width,"height":height,
        "alpha":{"channel_present":alpha,"measured_transparency":false}}))
}

/// `opentoonz.discover`: tool availability and, given a scene, what would render. No rendering.
pub fn cmd_opentoonz_discover(s: &mut Session, p: &Value) -> Result<Value> {
    let tool = tcomposer();
    let mut result = json!({"backend":"opentoonz","available":tool.is_ok(),"tcomposer":tool.as_ref().ok(),
        "tcomposer_sha256":match &tool { Ok(path) => json!(sha256_file(path)?), Err(_) => Value::Null },
        "version":Value::Null,"version_note":"tcomposer has no version flag; record the package or build version at deployment",
        "stuff":stuff_dir(),"reason":tool.as_ref().err().map(|e| e.to_string()),
        "controls":["first","last","step","shrink","nthreads"],
        "unsupported":["fps, width and height overrides (use the scene's output settings)","rig, bone or Plastic authoring","action names"]});
    if p["scene"].is_string() {
        let mut probe = p.clone();
        if probe["out"].is_null() { probe["out"] = json!(std::env::temp_dir().join("opentoonz-discover-not-rendered")); }
        result["plan"] = plan(s, &probe)?;
    }
    Ok(result)
}

/// `opentoonz.render`: run one validated plan to a fresh folder and verify its frames.
/// Blocking; the shared bounded worker owns limits, progress, cancellation and receipts.
/// With `request` (a job's request.json) it runs only inside that job's worker invocation,
/// renders into `<job>/frames` and writes `<job>/opentoonz-manifest.json`.
pub fn cmd_opentoonz_render(s: &mut Session, p: &Value) -> Result<Value> {
    let Some(path) = p["request"].as_str() else { return render(s, p) };
    if s.project_path.is_some() { return Err(invalid("Worker rendering requires a separate headless worker session")); }
    let path = PathBuf::from(path);
    let request: Value = serde_json::from_slice(&fs::read(&path)?).map_err(|e| invalid(e.to_string()))?;
    crate::blender::require_worker_invocation(&request)?;
    let job = path.parent().ok_or_else(|| invalid("request has no job folder"))?;
    let mut params = json!({"out":job.join("frames")});
    for key in ["scene", "first", "last", "step", "shrink", "nthreads", "fps", "width", "height"] { params[key] = request[key].clone(); }
    let mut result = render(s, &params)?;
    let manifest = json!({"backend":"opentoonz","job":request["job"],"frames":result["frames"],"frame_mapping":result["frame_mapping"],
        "first_frame":result["first_frame"],"last_frame":result["last_frame"],"fps":result["fps"],"width":result["width"],
        "height":result["height"],"alpha":result["alpha"],"source_bundle":result["source_bundle"],"tool":result["tool"],"range":result["range"]});
    let target = job.join("opentoonz-manifest.json");
    let temporary = target.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&manifest).map_err(|e| invalid(e.to_string()))?)?;
    fs::rename(&temporary, &target)?;
    result["manifest"] = json!(target);
    Ok(result)
}

fn render(s: &mut Session, p: &Value) -> Result<Value> {
    let tool = tcomposer()?;
    let plan = plan(s, p)?;
    let dir = PathBuf::from(plan["frames_dir"].as_str().unwrap_or_default());
    if dir.exists() && fs::read_dir(&dir)?.next().is_some() { return Err(invalid("out must be a new or empty folder so no stale frame can count")); }
    fs::create_dir_all(&dir)?;
    let log = fs::File::create(dir.join("tcomposer.log"))?;
    let args: Vec<String> = plan["argv"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_owned)).collect();
    let mut command = Command::new(&tool);
    command.args(&args).stdout(log.try_clone()?).stderr(log);
    if std::env::var_os("QT_QPA_PLATFORM").is_none() { command.env("QT_QPA_PLATFORM", "offscreen"); }
    let status = command.status().map_err(|e| EngineError::ExternalTool("tcomposer".into(), e.to_string()))?;
    if !status.success() {
        let code = status.code().map(|c| c as u8 as i32);
        return Err(EngineError::ExternalTool("tcomposer".into(), format!("exit {:?}: {}; see {}", status.code(), exit_meaning(code), dir.join("tcomposer.log").display())));
    }
    let frames = collect(&plan)?;
    let mut result = json!({"ok":true,"state":"completed","backend":"opentoonz",
        "tool":{"path":tool,"sha256":sha256_file(&tool)?,"version":Value::Null},
        "source_bundle":plan["source_bundle"],"range":plan["range"],"fps":plan["fps"],"shrink":plan["shrink"],
        "argv":plan["argv"],"log":dir.join("tcomposer.log")});
    for (key, value) in frames.as_object().into_iter().flatten() { result[key] = value.clone(); }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("opentoonz-adapter-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SCENE: &str = r#"<?xml version="1.0"?>
<tnz framecount="12" version="71.1"><properties>
<cameras><camera><cameraSize>16 9</cameraSize><cameraRes>320 180</cameraRes><cameraXPrevalence>1</cameraXPrevalence></camera></cameras>
<outputs><output name="main"><range>0 -1</range><step>1</step><shrink>1</shrink><fps>24</fps><path>+outputs/.tif</path></output>
<output name="preview"><range>2 5</range><step>2</step><shrink>2</shrink><fps>12</fps></output></outputs>
</properties><levelSet><levels><level name="A"><path>+drawings/A.png</path></level></levels></levelSet></tnz>"#;

    /// A generic project: project file, one drawing, one scene.
    fn project(name: &str, scene: &str) -> (PathBuf, PathBuf) {
        let root = temp(name);
        fs::write(root.join("demo_otprj.xml"), "<project/>").unwrap();
        fs::create_dir_all(root.join("drawings")).unwrap();
        fs::create_dir_all(root.join("scenes")).unwrap();
        image::RgbaImage::new(4, 4).save(root.join("drawings/A.png")).unwrap();
        let scene_path = root.join("scenes/demo.tnz");
        fs::write(&scene_path, scene).unwrap();
        (root, scene_path)
    }

    #[test]
    fn scene_info_reads_main_output_and_camera() {
        let (_, scene) = project("info", SCENE);
        let info = scene_info(&scene).unwrap();
        assert_eq!(info["frame_count"], 12);
        assert_eq!(info["output_range_zero_based"], json!([0, -1]));
        assert_eq!(info["fps"], 24.0);
        assert_eq!(info["shrink"], 1);
        assert_eq!(info["camera_resolutions"], json!([[320, 180]]));
    }

    #[test]
    fn range_is_one_based_and_defaults_to_scene() {
        let whole = json!({"frame_count":12,"output_range_zero_based":[0,-1],"step":1});
        assert_eq!(resolve_range(&whole, &json!({})).unwrap(), (1, 12, 1));
        let stored = json!({"frame_count":12,"output_range_zero_based":[2,5],"step":2});
        assert_eq!(resolve_range(&stored, &json!({})).unwrap(), (3, 6, 2));
        assert_eq!(resolve_range(&whole, &json!({"first":4,"last":8})).unwrap(), (4, 8, 1));
        assert!(resolve_range(&whole, &json!({"last":13})).is_err());
        assert!(resolve_range(&whole, &json!({"first":0})).is_err());
        assert!(resolve_range(&json!({"output_range_zero_based":[0,-1]}), &json!({})).is_err());
    }

    #[test]
    fn plan_builds_exact_argv_and_hashes_the_bundle() {
        let (root, scene) = project("plan", SCENE);
        let out = root.join("job/frames");
        let plan = plan(&Session::default(), &json!({"scene":scene,"out":out,"first":2,"last":4})).unwrap();
        let argv: Vec<&str> = plan["argv"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        assert_eq!(&argv[..12], &[scene.canonicalize().unwrap().to_str().unwrap(), "-o", out.join("frame.png").to_str().unwrap(),
            "-range", "2", "4", "-step", "1", "-shrink", "1", "-nthreads", "2"]);
        assert_eq!(plan["expected_frames"], json!([2, 3, 4]));
        assert_eq!(plan["fps"], 24.0);
        assert_eq!(plan["source_bundle"]["dependencies"][0]["path"], json!(root.canonicalize().unwrap().join("drawings/A.png")));
    }

    #[test]
    fn plan_rejects_overrides_and_missing_levels() {
        let (root, scene) = project("reject", SCENE);
        let out = root.join("frames");
        for bad in [json!({"fps":30}), json!({"width":640}), json!({"height":360}), json!({"shrink":0}), json!({"nthreads":"many"})] {
            let mut p = bad.clone();
            p["scene"] = json!(scene);
            p["out"] = json!(out);
            assert!(plan(&Session::default(), &p).is_err(), "{bad}");
        }
        assert!(plan(&Session::default(), &json!({"scene":scene,"out":out,"fps":24})).is_ok());
        fs::remove_file(root.join("drawings/A.png")).unwrap();
        let error = plan(&Session::default(), &json!({"scene":scene,"out":out})).unwrap_err().to_string();
        assert!(error.contains("missing levels") && error.contains("+drawings/A.png"), "{error}");
    }

    #[test]
    fn collect_requires_exact_fresh_frames() {
        let dir = temp("collect");
        let plan = json!({"frames_dir":dir,"expected_frames":[1,2,3],"declared_camera_resolution":[8,6],"shrink":1});
        for n in [1, 2] { image::RgbaImage::new(8, 6).save(dir.join(format!("frame.{n:04}.png"))).unwrap(); }
        assert!(collect(&plan).unwrap_err().to_string().contains("missing [3]"));
        image::RgbaImage::new(8, 6).save(dir.join("frame.0003.png")).unwrap();
        let done = collect(&plan).unwrap();
        assert_eq!((done["first_frame"].clone(), done["last_frame"].clone(), done["width"].clone()), (json!(1), json!(3), json!(8)));
        assert_eq!(done["alpha"]["channel_present"], true);
        image::RgbaImage::new(8, 6).save(dir.join("frame.0009.png")).unwrap();
        assert!(collect(&plan).unwrap_err().to_string().contains("frame.0009.png"));
        fs::remove_file(dir.join("frame.0009.png")).unwrap();
        image::RgbImage::new(10, 6).save(dir.join("frame.0003.png")).unwrap();
        assert!(collect(&plan).is_err());
    }

    #[test]
    fn missing_tool_is_reported_without_installing() {
        let empty = temp("no-tool");
        let error = find_tcomposer(None, Some(empty.clone().into_os_string())).unwrap_err().to_string();
        assert!(error.contains("not installed") && error.contains("OPENTOONZ_TCOMPOSER"), "{error}");
        assert!(find_tcomposer(Some("relative/tcomposer".into()), None).is_err());
        fs::write(empty.join("tcomposer"), "").unwrap();
        assert_eq!(find_tcomposer(None, Some(empty.clone().into_os_string())).unwrap(), empty.join("tcomposer"));
        assert_eq!(exit_meaning(Some(255)), "one or more frames failed to render");
        assert_eq!(exit_meaning(Some(254)), "scene or project could not be loaded");
    }
}
