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

fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file() && path.metadata().is_ok_and(|m|m.permissions().mode() & 0o111 != 0)
}
fn find_executable(name: &str, path_var: Option<OsString>) -> Option<PathBuf> {
    path_var.and_then(|v| std::env::split_paths(&v).map(|d| d.join(name)).find(|p| executable(p)))
        .and_then(|p| p.canonicalize().ok())
}
/// Explicit invalid overrides never fall back to another installed renderer.
fn find_tcomposer(explicit: Option<OsString>, path_var: Option<OsString>) -> Result<PathBuf> {
    if let Some(path) = explicit.map(PathBuf::from) {
        if path.is_absolute() && executable(&path) { return Ok(path.canonicalize()?); }
        return Err(invalid(format!("OPENTOONZ_TCOMPOSER must be an existing absolute executable: {}", path.display())));
    }
    find_executable("tcomposer",path_var).ok_or_else(|| invalid(MISSING_TOOL))
}
fn tcomposer() -> Result<PathBuf> {
    find_tcomposer(std::env::var_os("OPENTOONZ_TCOMPOSER"), std::env::var_os("PATH"))
}
// Arch's OpenToonz launcher supplies <prefix>/lib/opentoonz for its private
// libraries. Apply the same layout only to the renderer child, never this process.
fn private_libraries(tool: &Path) -> Option<PathBuf> {
    let dir = tool.parent()?.parent()?.join("lib/opentoonz");
    dir.join("libtfarm.so").is_file().then_some(dir)
}
fn renderer_command(tool: &Path) -> Result<Command> {
    let mut command = Command::new(tool);
    if let Some(dir) = private_libraries(tool) {
        let inherited = std::env::var_os("LD_LIBRARY_PATH");
        let paths = std::iter::once(dir).chain(inherited.as_ref().into_iter().flat_map(std::env::split_paths));
        command.env("LD_LIBRARY_PATH", std::env::join_paths(paths).map_err(|e| invalid(e.to_string()))?);
    }
    command.env("QT_QPA_PLATFORM", "offscreen");
    Ok(command)
}
fn renderer_error(tool: &Path, timeout: std::time::Duration) -> Option<String> {
    use std::{process::Stdio, time::{Duration, Instant}};
    let mut command = match renderer_command(tool) { Ok(c) => c, Err(e) => return Some(e.to_string()) };
    // A nonblocking bounded read also avoids hanging on inherited stderr handles.
    let (mut output, writer) = match std::os::unix::net::UnixStream::pair() { Ok(pair) => pair, Err(e) => return Some(e.to_string()) };
    if let Err(e) = output.set_nonblocking(true) { return Some(e.to_string()); }
    // No scene or render arguments: only exercise the native loader.
    let mut child = match command.arg("-version").stdin(Stdio::null()).stdout(Stdio::null())
        .stderr(Stdio::from(std::os::fd::OwnedFd::from(writer))).spawn() {
        Ok(child) => child, Err(e) => return Some(format!("tcomposer readiness probe could not start: {e}")),
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut bytes = Vec::new();
                let _ = (&mut output).take(4096).read_to_end(&mut bytes);
                // OpenToonz 1.8 reports its version on stderr and exits 1.
                let text = String::from_utf8_lossy(&bytes);
                let version = text.split_whitespace().collect::<Vec<_>>();
                let identified = version.first() == Some(&"OpenToonz") && version.get(1).is_some_and(|v|v.starts_with('v') && v.chars().nth(1).is_some_and(|c|c.is_ascii_digit()));
                return (!(matches!(status.code(), Some(0 | 1)) && identified)).then(|| format!("tcomposer -version failed ({status}); check renderer shared-library/runtime dependencies"));
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill(); let _ = child.wait();
                return Some(match result { Err(e) => format!("tcomposer readiness probe failed: {e}"), _ => "tcomposer readiness probe timed out".into() });
            }
        }
    }
}
fn usable_stuff(path: &Path) -> bool {
    path.is_absolute() && path.is_dir() && fs::read_dir(path).is_ok_and(|mut entries|entries.next().is_some_and(|e|e.is_ok()))
}
fn find_stuff(explicit: Option<OsString>, candidates: impl Iterator<Item=PathBuf>) -> Option<PathBuf> {
    let path=if let Some(path)=explicit.map(PathBuf::from) { usable_stuff(&path).then_some(path) }
        else { candidates.into_iter().find(|p|usable_stuff(p)) };
    path.and_then(|p|p.canonicalize().ok())
}
/// tcomposer resources must be readable and nonempty; an invalid override is an error.
fn stuff_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/OpenToonz/stuff"));
    find_stuff(std::env::var_os("OPENTOONZ_STUFF"), STUFF_CANDIDATES.iter().map(PathBuf::from).chain(home))
}
fn pillow_error() -> Option<String> {
    use std::{process::Stdio,time::{Duration,Instant}};
    let mut child=match Command::new("/usr/bin/python3").args(["-c","from PIL import Image"])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(child)=>child,Err(error)=>return Some(format!("Python/Pillow probe could not start: {error}")),
    };
    let deadline=Instant::now()+Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status))=>return (!status.success()).then(||"Python Pillow unavailable in /usr/bin/python3; install python-pillow".into()),
            Ok(None) if Instant::now()<deadline=>std::thread::sleep(Duration::from_millis(10)),
            result=>{let _=child.kill();let _=child.wait();return Some(match result {Err(error)=>format!("Python/Pillow probe failed: {error}"),_=>"Python/Pillow readiness probe timed out after 5 seconds".into()});}
        }
    }
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
pub(crate) fn project_root(scene: &Path) -> Option<PathBuf> {
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

// Native TIStream writes filenames as quoted strings, escaping quotes and
// backslashes. Resolve that value, not the enclosing serialization syntax.
fn native_path_text(raw: &str) -> Result<String> {
    let raw = raw.trim();
    let Some(quoted) = raw.strip_prefix('"') else { return Ok(raw.replace('\\', "/")) };
    let quoted = quoted.strip_suffix('"').ok_or_else(||invalid("Unterminated native level path"))?;
    let mut chars = quoted.chars().peekable();
    let mut path = String::new();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some_and(|next| matches!(next, '\\' | '"' | '\'')) {
            if let Some(next) = chars.next() { path.push(next); }
        } else { path.push(c); }
    }
    Ok(path.replace('\\', "/"))
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
        let decoded = native_path_text(raw)?;
        let raw = decoded.as_str();
        let resolved = if let Some(tail) = raw.strip_prefix("$scenefolder/") {
            scene.parent().ok_or_else(||invalid("Scene has no parent directory"))?.join(tail)
        } else if let Some(aliased) = raw.strip_prefix('+') {
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
        match number { Some(n) if expected.contains(&n) => { if by_frame.insert(n, path).is_some() { return Err(invalid("Duplicate native frame number")); } }, _ if name == "tcomposer.log" => {}, _ => unexpected.push(name) }
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

/// Public submission only: the existing worker owns validation/snapshot/render/encode.
pub fn cmd_opentoonz_start(s: &mut Session, p: &Value) -> Result<Value> {
    let readiness=cmd_opentoonz_discover(s,&json!({}))?;
    if readiness["render_capable"]!=true {return Err(invalid(readiness["render_reason"].as_str().unwrap_or("OpenToonz prerequisites unavailable")));}
    let tool=&readiness["tcomposer"];let stuff=&readiness["stuff"];
    let scene=crate::workspace::resolve_workspace_path(s,p["scene"].as_str().ok_or_else(||invalid("scene is required"))?).canonicalize()?;
    if scene.extension().is_none_or(|e|e!="tnz") {return Err(invalid("scene must be .tnz"));}
    let bundle=p["bundle"].as_str().map(|v|crate::workspace::resolve_workspace_path(s,v)).or_else(||project_root(&scene)).or_else(||scene.parent().map(Path::to_path_buf)).ok_or_else(||invalid("bundle required"))?.canonicalize()?;
    if !scene.starts_with(&bundle){return Err(invalid("scene must be inside bundle"));}
    if ["fps","width","height"].iter().any(|k|p.get(*k).is_some()){return Err(invalid("Use saved scene fps/size; overrides unsupported"));}
    let mc=&readiness["prerequisites"]["magic_cli"]["path"];
    let mut request=p.clone();request["scene"]=json!(scene);request["bundle"]=json!(bundle);request["stuff"]=json!(stuff);request["tcomposer"]=json!(tool);request["magic_cli"]=json!(mc);request["ffmpeg"]=readiness["prerequisites"]["ffmpeg"]["path"].clone();
    crate::blender::submit(s,p,request,"opentoonz")
}

/// `opentoonz.discover`: tool availability and, given a scene, what would render. No rendering.
pub fn cmd_opentoonz_discover(s: &mut Session, p: &Value) -> Result<Value> {
    let tool = tcomposer();
    let stuff=stuff_dir();
    let helper=std::env::current_exe().ok().and_then(|p|[p.with_file_name("magic-cli")].into_iter().find(|c|executable(c)));
    let ffmpeg=find_executable("ffmpeg",std::env::var_os("PATH"));
    let mut missing=Vec::new();
    if let Err(error)=&tool { missing.push(error.to_string()); }
    if stuff.is_none() { missing.push("Readable nonempty OpenToonz stuff directory required; check OPENTOONZ_STUFF (invalid overrides do not fall back)".into()); }
    if helper.is_none() { missing.push("Executable magic-cli must be installed beside the caller for bounded native preparation".into()); }
    if ffmpeg.is_none() { missing.push("FFmpeg executable not found on PATH; required for alpha encoding".into()); }
    let renderer_error=tool.as_ref().ok().and_then(|tool|renderer_error(tool,std::time::Duration::from_secs(5)));
    if let Some(error)=&renderer_error {missing.push(error.clone());}
    let python_checked=missing.is_empty();
    let python_error=if python_checked {pillow_error()}else{None};
    if let Some(error)=&python_error {missing.push(error.clone());}
    let render_reason=(!missing.is_empty()).then(||missing.join("; "));
    let prerequisites=json!({"renderer":{"checked":tool.is_ok(),"available":if tool.is_ok(){json!(renderer_error.is_none())}else{Value::Null},"error":renderer_error,"private_library_dir":tool.as_ref().ok().and_then(|tool|private_libraries(tool))},"resources":{"available":stuff.is_some(),"path":stuff},"magic_cli":{"path":helper,"available":helper.is_some()},"ffmpeg":{"path":ffmpeg,"available":ffmpeg.is_some()},
        "python_pillow":{"path":"/usr/bin/python3","checked":python_checked,"available":if python_checked {json!(python_error.is_none())}else{Value::Null},"error":python_error}});
    let mut result = json!({"backend":"opentoonz","available":tool.is_ok(),"render_capable":render_reason.is_none(),"render_reason":render_reason,"prerequisites":prerequisites,"native_render_verified":Value::Null,"tcomposer":tool.as_ref().ok(),
        "tcomposer_sha256":match &tool { Ok(path) => json!(sha256_file(path)?), Err(_) => Value::Null },
        "version":Value::Null,"version_note":"Readiness runs -version only; record the package or build version at deployment",
        "stuff":stuff,"reason":tool.as_ref().err().map(|e| e.to_string()),
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
    let path = p["request"].as_str().ok_or_else(||invalid("Use opentoonz.start; render is a bounded-worker-only helper"))?;
    if s.project_path.is_some() { return Err(invalid("Worker rendering requires a separate headless worker session")); }
    let path = PathBuf::from(path);
    let request: Value = serde_json::from_slice(&fs::read(&path)?).map_err(|e| invalid(e.to_string()))?;
    crate::blender::require_worker_invocation(&request)?;
    let job = path.parent().ok_or_else(|| invalid("request has no job folder"))?;
    let mut params = json!({"out":job.join("frames")});
    for key in ["scene", "bundle", "first", "last", "step", "shrink", "nthreads", "fps", "width", "height"] { params[key] = request[key].clone(); }
    let mut result = render(s, &params)?;
    let manifest = json!({"backend":"opentoonz","job":request["job"],"frames":result["frames"],"frame_mapping":result["frame_mapping"],
        "first_frame":result["first_frame"],"last_frame":result["last_frame"],"fps":result["fps"],"width":result["width"],
        "height":result["height"],"scene_frame_count":result["scene_frame_count"],"alpha":result["alpha"],"source_bundle":result["source_bundle"],"tool":result["tool"],"range":result["range"]});
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
    let size=plan["declared_camera_resolution"].as_array().filter(|v|v.len()==2).ok_or_else(||invalid("One declared camera resolution is required for bounded rendering"))?;
    let shrink=plan["shrink"].as_u64().unwrap_or(1);
    let width=size[0].as_u64().unwrap_or(0)/shrink;
    let height=size[1].as_u64().unwrap_or(0)/shrink;
    let count=plan["range"]["last"].as_u64().unwrap_or(0)-plan["range"]["first"].as_u64().unwrap_or(0)+1;
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) || count>18000 || width*height*count>256*1024*1024 {
        return Err(invalid("Native scene exceeds bounded frame/pixel budget"));
    }
    if let Some(bundle)=p["bundle"].as_str() {
        let bundle=Path::new(bundle).canonicalize()?;
        for dependency in plan["source_bundle"]["dependencies"].as_array().into_iter().flatten() {
            let path=Path::new(dependency["path"].as_str().ok_or_else(||invalid("Invalid dependency path"))?).canonicalize()?;
            if !path.starts_with(&bundle){return Err(invalid("Native dependency escapes frozen bundle"));}
        }
    }
    let dir = PathBuf::from(plan["frames_dir"].as_str().unwrap_or_default());
    if dir.exists() && fs::read_dir(&dir)?.next().is_some() { return Err(invalid("out must be a new or empty folder so no stale frame can count")); }
    fs::create_dir_all(&dir)?;
    let log = fs::File::create(dir.join("tcomposer.log"))?;
    let args: Vec<String> = plan["argv"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_owned)).collect();
    let mut command = renderer_command(&tool)?;
    command.args(&args).stdout(log.try_clone()?).stderr(log);
    let status = command.status().map_err(|e| EngineError::ExternalTool("tcomposer".into(), e.to_string()))?;
    if !status.success() {
        let code = status.code().map(|c| c as u8 as i32);
        return Err(EngineError::ExternalTool("tcomposer".into(), format!("exit {:?}: {}; see {}", status.code(), exit_meaning(code), dir.join("tcomposer.log").display())));
    }
    let frames = collect(&plan)?;
    let mut result = json!({"ok":true,"state":"completed","backend":"opentoonz",
        "tool":{"path":tool,"sha256":sha256_file(&tool)?,"version":Value::Null},
        "source_bundle":plan["source_bundle"],"scene_frame_count":plan["scene"]["frame_count"],"range":plan["range"],"fps":plan["fps"],"shrink":plan["shrink"],
        "argv":plan["argv"],"log":dir.join("tcomposer.log")});
    for (key, value) in frames.as_object().into_iter().flatten() { result[key] = value.clone(); }
    Ok(result)
}
