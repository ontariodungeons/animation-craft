//! Target video project scaffolding (Remotion, FilmCraft, HyperFrames), web dev server
//! management (`video.server_start` / `video.server_stop`), and reference placement.

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};
use crate::storyboard::{clock, storyboard_timing};
use crate::workshop::{
    FILMCRAFT_TICKS_PER_SECOND, panel_files, project_dir, video_project_path, workshop_status,
};
use crate::workspace::{resolve_workspace_path, run_tool_in, sanitize_asset_id};

pub(crate) fn remotion_composer_dir(dir: &Path) -> Option<PathBuf> {
    for ancestor in dir.ancestors().skip(1).take(3) {
        let candidate = ancestor.join("remotion-composer");
        if candidate.join("package.json").is_file() {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn remotion_track_slug(s: &Session, dir: &Path) -> String {
    let folder = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if let Some(num) = folder.strip_prefix("kathy-cd-track-") {
        return format!("track{}", num.replace('-', ""));
    }
    let stem = s
        .project_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .and_then(|n| n.to_str())
        .unwrap_or(if folder.is_empty() { "song" } else { folder });
    sanitize_asset_id(stem)
}

pub(crate) fn target_project_path(s: &Session, dir: &Path, target: &str) -> Option<PathBuf> {
    match target {
        "filmcraft" => video_project_path(dir),
        "hyperframes" => {
            let hf_json = dir.join("hyperframes/hyperframes.json");
            let hf_html = dir.join("hyperframes/index.html");
            if hf_json.is_file() {
                Some(hf_json)
            } else if hf_html.is_file() {
                Some(hf_html)
            } else {
                None
            }
        }
        "remotion" => {
            let local_pkg = dir.join("remotion/package.json");
            if local_pkg.is_file() {
                return Some(local_pkg);
            }
            if let Some(composer) = remotion_composer_dir(dir) {
                let slug = remotion_track_slug(s, dir);
                let comp_dir = composer.join("src").join(&slug);
                if comp_dir.is_dir() {
                    return Some(comp_dir);
                }
                let folder_slug = sanitize_asset_id(
                    dir.file_name().and_then(|n| n.to_str()).unwrap_or("song"),
                );
                let alt_dir = composer.join("src").join(&folder_slug);
                if alt_dir.is_dir() {
                    return Some(alt_dir);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn target_project_chip(s: &Session, dir: &Path, target: &str) -> Value {
    if let Some(path) = target_project_path(s, dir, target) {
        let label = match target {
            "remotion" => format!(
                "remotion/{}",
                path.file_name().and_then(|n| n.to_str()).unwrap_or("remotion")
            ),
            "hyperframes" => "hyperframes/hyperframes.json".to_string(),
            _ => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project.fcproj")
                .to_string(),
        };
        json!({"name": label, "path": path, "exists": true, "target": target})
    } else {
        let expected = match target {
            "remotion" => {
                let slug = remotion_track_slug(s, dir);
                let path = remotion_composer_dir(dir)
                    .map(|c| c.join("src").join(&slug))
                    .unwrap_or_else(|| dir.join("remotion"));
                (format!("remotion/{slug}"), path)
            }
            "hyperframes" => (
                "hyperframes/hyperframes.json".to_string(),
                dir.join("hyperframes/hyperframes.json"),
            ),
            _ => {
                let stem = sanitize_asset_id(
                    s.project_path
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .and_then(|n| n.to_str())
                        .unwrap_or("song"),
                );
                (format!("{stem}.fcproj"), dir.join(format!("{stem}.fcproj")))
            }
        };
        json!({"name": expected.0, "path": expected.1, "exists": false, "target": target})
    }
}

fn port_open(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(5)).is_ok()
}

pub(crate) fn video_server_info(dir: &Path, target: &str) -> Value {
    let state_file = dir.join(".video-server.json");
    if let Ok(raw) = std::fs::read_to_string(&state_file)
        && let Ok(saved) = serde_json::from_str::<Value>(&raw)
        && saved.get("target").and_then(Value::as_str) == Some(target)
        && let Some(port) = saved.get("port").and_then(Value::as_u64).map(|p| p as u16)
        && port_open(port)
    {
        return json!({
            "running": true,
            "target": target,
            "port": port,
            "url": format!("http://localhost:{port}"),
            "pid": saved.get("pid"),
            "managed": true,
        });
    }
    // Only the server this project started counts: a studio on some other port may belong to
    // another project or another person.
    json!({
        "running": false,
        "target": target,
        "port": null,
        "url": null,
        "pid": null,
        "managed": false,
    })
}

/// Resolve only an explicitly selected executable, HYPERFRAMES_CLI, or PATH; never install with npx.
fn hyperframes_cli(explicit: Option<&str>) -> Result<PathBuf> {
    let configured=explicit.map(PathBuf::from).or_else(||std::env::var_os("HYPERFRAMES_CLI").map(PathBuf::from));
    let path=configured.or_else(||std::env::var_os("PATH").and_then(|v|std::env::split_paths(&v).map(|d|d.join("hyperframes")).find(|p|p.is_file())))
        .ok_or_else(||EngineError::InvalidParams("HyperFrames executable unavailable. Pass hyperframes_cli=/absolute/path or set HYPERFRAMES_CLI to an existing official executable; no install or npx lookup was attempted".into()))?;
    if !path.is_absolute() || !path.is_file() {return Err(EngineError::InvalidParams(format!("HyperFrames executable must be an existing absolute file: {}",path.display())));}
    Ok(path)
}

fn generated_files(root:&Path, relative:&Path, files:&mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry=entry?;let path=relative.join(entry.file_name());let kind=entry.file_type()?;
        if kind.is_dir(){generated_files(root,&path,files)?;}else if kind.is_file(){files.push(path);}else{return Err(EngineError::InvalidParams("Official initializer produced an unsupported symlink; generated files retained for inspection".into()));}
    }
    Ok(())
}

fn init_hyperframes(dir:&Path, executable:Option<&str>) -> Result<PathBuf> {
    let cli=hyperframes_cli(executable)?;
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|EngineError::InvalidParams(e.to_string()))?.as_nanos();
    let stage=dir.join(format!(".hyperframes-init-{}-{stamp}",std::process::id()));
    let args=["init".to_string(),stage.to_string_lossy().into_owned(),"--non-interactive".into(),"--skip-transcribe".into()];
    let output=std::process::Command::new(&cli).args(&args).current_dir(dir).env("HYPERFRAMES_SKIP_SKILLS","1").output()
        .map_err(|e|EngineError::ExternalTool(cli.display().to_string(),e.to_string()))?;
    if !output.status.success(){return Err(EngineError::ExternalTool(cli.display().to_string(),format!("official init exited {:?}: {}. Partial output retained at {}",output.status.code(),String::from_utf8_lossy(&output.stderr),stage.display())));}
    let config=stage.join("hyperframes.json");let entry=stage.join("index.html");
    if !entry.is_file() || !std::fs::read(&config).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v|v.is_object()) {
        return Err(EngineError::InvalidParams(format!("Official HyperFrames init did not create a valid hyperframes.json and index.html; output retained at {}",stage.display())));
    }
    let dest=dir.join("hyperframes");let mut files=Vec::new();generated_files(&stage,Path::new(""),&mut files)?;
    // Preflight every collision before moving anything; existing captions/assets remain authoritative.
    for relative in &files {
        let target=dest.join(relative);
        if target.exists() || target.ancestors().skip(1).take_while(|p|p.starts_with(&dest)).any(|p|p.exists()&&!p.is_dir()) {
            return Err(EngineError::InvalidParams(format!("Official init conflicts with existing {}; no existing file overwritten. Generated output retained at {}",target.display(),stage.display())));
        }
    }
    for relative in files {
        let target=dest.join(&relative);std::fs::create_dir_all(target.parent().ok_or_else(||EngineError::InvalidParams("Generated file has no parent".into()))?)?;
        std::fs::rename(stage.join(relative),target)?;
    }
    std::fs::remove_dir_all(&stage)?;
    Ok(dest.join("hyperframes.json"))
}

/// Ensure a video target project exists for the song folder, creating scaffolding when missing.
pub(crate) fn ensure_video_target_project(s: &Session, dir: &Path, target: &str) -> Result<PathBuf> {
    if let Some(existing) = target_project_path(s, dir, target) {
        return Ok(existing);
    }
    let title = s
        .project_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .and_then(|n| n.to_str())
        .unwrap_or("Song")
        .to_string();
    let _duration_secs = probe_audio_duration(s.song_audio.as_deref());
    match target {
        "remotion" => {
            let slug = remotion_track_slug(s, dir);
            let comp_dir = if let Some(composer) = remotion_composer_dir(dir) {
                composer.join("src").join(&slug)
            } else {
                let remotion_root = dir.join("remotion");
                std::fs::create_dir_all(remotion_root.join("src"))?;
                let pkg_json = remotion_root.join("package.json");
                if !pkg_json.exists() {
                    std::fs::write(
                        &pkg_json,
                        serde_json::to_string_pretty(&json!({
                            "name": format!("remotion-{slug}"),
                            "private": true,
                            "scripts": {
                                "dev": "npx remotion studio src/index.ts",
                                "render": format!("npx remotion render src/index.ts {slug}")
                            }
                        }))
                        .unwrap_or_default(),
                    )?;
                }
                let index_ts = remotion_root.join("src/index.ts");
                if !index_ts.exists() {
                    std::fs::write(
                        &index_ts,
                        "// Remotion project scaffolded by AnimationCraft\n",
                    )?;
                }
                remotion_root.join("src").join(&slug)
            };
            Ok(comp_dir)
        }
        "filmcraft" => {
            let fcproj = dir.join(format!("{title}.fcproj"));
            std::fs::create_dir_all(dir)?;
            if !fcproj.is_file() {
                std::fs::write(&fcproj, "{}")?;
            }
            Ok(fcproj)
        }
        "hyperframes" => Err(EngineError::InvalidParams("HyperFrames project missing; use video.init with an existing official executable".into())),
        _ => Err(EngineError::InvalidParams(format!("unknown video target '{target}'"))),
    }
}

fn probe_audio_duration(audio: Option<&Path>) -> f64 {
    if let Some(audio) = audio
        && let Ok(out) = std::process::Command::new("ffprobe")
            .args([
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(audio)
            .output()
        && let Ok(secs) = String::from_utf8_lossy(&out.stdout).trim().parse::<f64>()
        && secs > 0.5
    {
        return secs;
    }
    30.0
}

fn find_free_port(preferred: u16) -> u16 {
    for port in preferred..(preferred + 25) {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    preferred
}

fn is_managed_video_server_pid(pid: u64) -> bool {
    let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    let text = String::from_utf8_lossy(&cmdline).to_ascii_lowercase();
    text.contains("remotion") || text.contains("http.server") || text.contains("hyperframes")
}

pub fn cmd_video_init(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or(&s.video_target)
        .to_string();
    let reused=target_project_path(s,&dir,&target).is_some();
    let created = if target=="hyperframes" && !reused {init_hyperframes(&dir,params["hyperframes_cli"].as_str())?}else{ensure_video_target_project(s, &dir, &target)?};
    if target == "hyperframes" && !created.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "video.init did not create an editable HyperFrames project: {}. Official initialization returned no editable project; existing audio/transcript files were preserved.", created.display()
        )));
    }
    let mut status = workshop_status(s)?;
    status["initialized"] = json!(created);
    status["reused_existing_project"]=json!(reused);
    status["video_complete"]=json!(false);
    status["initialization_scope"]=json!("Official editable scaffold only; song composition and rendered output are separate steps");
    if target=="hyperframes" {status["runtime_executable"]=json!(hyperframes_cli(params["hyperframes_cli"].as_str())?);}
    Ok(status)
}

pub fn cmd_video_server_start(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or(&s.video_target)
        .to_string();
    let current = video_server_info(&dir, &target);
    if current["running"] == json!(true) {
        return workshop_status(s);
    }
    let _ = if target=="hyperframes" && target_project_path(s,&dir,&target).is_none(){init_hyperframes(&dir,params["hyperframes_cli"].as_str())?}else{ensure_video_target_project(s, &dir, &target)?};
    let preferred_port = params
        .get("port")
        .and_then(Value::as_u64)
        .map(|p| p as u16)
        .unwrap_or(if target == "hyperframes" { 3058 } else { 3057 });
    let port = find_free_port(preferred_port);
    let log_file = std::fs::File::create(dir.join(".video-server.log"))?;
    let err_file = log_file.try_clone()?;

    let mut child = match target.as_str() {
        "remotion" => {
            let work_dir = remotion_composer_dir(&dir).unwrap_or_else(|| dir.join("remotion"));
            std::process::Command::new("npx")
                .args(["remotion", "studio", "--port", &port.to_string(), "--no-open"])
                .current_dir(&work_dir)
                .process_group(0)
                .stdout(log_file)
                .stderr(err_file)
                .spawn()
                .map_err(|e| EngineError::ExternalTool("remotion".into(), e.to_string()))?
        }
        "hyperframes" => {
            let work_dir = dir.join("hyperframes");
            // HyperFrames Studio itself (not a plain file server): timeline, preview, lint, and
            // `hyperframes preview --context/--selection` for agents. Foreground keeps the pid ours.
            std::process::Command::new(hyperframes_cli(params["hyperframes_cli"].as_str())?)
                .args(["preview", "--port", &port.to_string(), "--no-open", "--foreground", "--force-new", "."])
                .current_dir(&work_dir)
                .process_group(0)
                .stdout(log_file)
                .stderr(err_file)
                .spawn()
                .map_err(|e| EngineError::ExternalTool("hyperframes-server".into(), e.to_string()))?
        }
        other => {
            return Err(EngineError::InvalidParams(format!(
                "target '{other}' does not use a web dev server"
            )));
        }
    };
    let pid = child.id();
    let state = json!({
        "target": target,
        "port": port,
        "pid": pid,
        "url": format!("http://localhost:{port}"),
    });
    std::fs::write(
        dir.join(".video-server.json"),
        serde_json::to_string_pretty(&state).unwrap_or_default(),
    )?;
    // npx resolves and boots the studio first; give it up to 30 s.
    for _ in 0..200 {
        if port_open(port) {
            break;
        }
        // A studio that quits at once (no composition yet, bad config) reports why, right away.
        if let Ok(Some(status)) = child.try_wait() {
            let _ = std::fs::remove_file(dir.join(".video-server.json"));
            let log = std::fs::read_to_string(dir.join(".video-server.log")).unwrap_or_default();
            let tail: Vec<&str> = log.lines().map(str::trim).filter(|l| !l.is_empty() && !l.chars().all(|c| !c.is_alphanumeric())).collect();
            return Err(EngineError::ExternalTool(
                format!("{target} studio"),
                format!("exited ({status}): {}", tail[tail.len().saturating_sub(4)..].join(" · ")),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    workshop_status(s)
}

pub fn cmd_video_server_stop(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let state_path = dir.join(".video-server.json");
    if let Ok(raw) = std::fs::read_to_string(&state_path)
        && let Ok(saved) = serde_json::from_str::<Value>(&raw)
        && let Some(pid) = saved.get("pid").and_then(Value::as_u64)
        && is_managed_video_server_pid(pid)
    {
        // The studio runs in its own process group (npx and the node server it starts).
        let _ = std::process::Command::new("kill")
            .args(["-TERM", "--", &format!("-{pid}")])
            .status();
    }
    let _ = std::fs::remove_file(&state_path);
    workshop_status(s)
}

/// Puts the storyboard panels on the song's FilmCraft sequence as placeholders.
pub fn cmd_video_place_references(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let fcproj = match params.get("project").and_then(Value::as_str) {
        Some(p) => resolve_workspace_path(s, p),
        None => video_project_path(&dir).ok_or_else(|| {
            EngineError::InvalidParams("no FilmCraft project yet: run audio.transcribe first".into())
        })?,
    };
    let images: Vec<PathBuf> = match params.get("images").and_then(Value::as_array) {
        Some(list) => list.iter().filter_map(Value::as_str).map(|p| resolve_workspace_path(s, p)).collect(),
        None => panel_files(&dir),
    };
    if images.is_empty() {
        return Err(EngineError::InvalidParams("no storyboard panels to place (workshop.add kind=panels first)".into()));
    }
    let track = params.get("track").and_then(Value::as_str).unwrap_or("V1");
    let tool = "filmcraft-cli";
    let failed = |what: &str, detail: String| EngineError::ExternalTool(tool.into(), format!("{what}: {detail}"));
    let project_arg = fcproj.to_string_lossy().into_owned();
    let sequence: Value = serde_json::from_str(&run_tool_in(&dir, tool, &["--project".into(), project_arg.clone(), "inspect".into(), "sequence".into()])?)
        .map_err(|e| failed("inspect", e.to_string()))?;
    let duration = sequence
        .get("duration")
        .and_then(Value::as_u64)
        .filter(|d| *d > 0)
        .ok_or_else(|| failed("inspect", "the FilmCraft project has no sequence with a length".into()))?;
    let imported: Value = serde_json::from_str(&run_tool_in(
        &dir,
        tool,
        &["--project".into(), project_arg.clone(), "--save".into(), "exec".into(), "file.import".into(), json!({"paths": images}).to_string()],
    )?)
    .map_err(|e| failed("file.import", e.to_string()))?;
    let items: Vec<u64> = imported.get("items").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).collect();
    if items.len() != images.len() {
        return Err(failed("file.import", format!("imported {} of {} images: {imported}", items.len(), images.len())));
    }
    let (_, spans) = storyboard_timing(&dir, items.len()).ok_or_else(|| {
        EngineError::InvalidParams("transcribe the song first: panel times come from the transcript".into())
    })?;
    let ticks = |seconds: f64| ((seconds * FILMCRAFT_TICKS_PER_SECOND) as u64).min(duration);
    let lines: Vec<String> = items
        .iter()
        .zip(&spans)
        .flat_map(|(item, item_spans)| {
            item_spans.iter().map(move |(a, b)| (item, *a, *b))
        })
        .filter(|(_, a, b)| b > a)
        .map(|(item, a, b)| {
            json!({"id": "timeline.place", "params": {"item": item, "track": track, "time": ticks(a), "duration": ticks(b) - ticks(a), "insert": false}}).to_string()
        })
        .collect();
    if lines.is_empty() {
        return Err(EngineError::InvalidParams("no panel lines up with a sung line in the transcript".into()));
    }
    let script = dir.join(".place_references.jsonl");
    std::fs::write(&script, lines.join("\n"))?;
    let run = run_tool_in(&dir, tool, &["--project".into(), project_arg, "--save".into(), "--compact".into(), "run".into(), script.to_string_lossy().into_owned()]);
    let _ = std::fs::remove_file(&script);
    let mut clips = Vec::new();
    for line in run?.lines().filter(|l| l.trim_start().starts_with('{')) {
        let entry: Value = serde_json::from_str(line).map_err(|e| failed("run", e.to_string()))?;
        if entry.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(failed("timeline.place", entry.to_string()));
        }
        clips.extend(entry.pointer("/result/clips").and_then(Value::as_array).cloned().unwrap_or_default());
    }
    let timing: Vec<Value> = images.iter().zip(&spans).map(|(img, sp)| json!({"panel": img, "begins": sp.iter().map(|(a, _)| clock(*a)).collect::<Vec<_>>()})).collect();
    Ok(json!({"ok": true, "filmcraft_project": fcproj, "track": track, "clips": clips, "timing": timing}))
}

/// Place a rendered track in HyperFrames without recreating the composition.
pub fn cmd_video_place_track(s:&mut Session,params:&Value)->Result<Value>{
    let dir=project_dir(s)?;
    if s.video_target!="hyperframes"{return Err(EngineError::InvalidParams("Select HyperFrames before placing this track".into()));}
    let script=crate::workspace::workspace_root(s).join("pipeline/place_hyperframes_track.py");
    let output=run_tool_in(&dir,"python3",&[script.to_string_lossy().into_owned(),"--project".into(),dir.to_string_lossy().into_owned(),"--request".into(),params.to_string()])?;
    serde_json::from_str(&output).map_err(|e|EngineError::InvalidParams(e.to_string()))
}
