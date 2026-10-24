//! Target video project scaffolding (Remotion, FilmCraft, HyperFrames), web dev server
//! management (`video.server_start` / `video.server_stop`), and reference placement.

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

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
            "url": if target=="hyperframes"{format!("http://localhost:{port}/#project/hyperframes")}else{format!("http://localhost:{port}")},
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

/// The HyperFrames version a video project pins (`hyperframes@X` in its package.json scripts,
/// or a `hyperframes` dependency), if it pins one.
pub(crate) fn pinned_hyperframes(project: &Path) -> Option<String> {
    let text=std::fs::read_to_string(project.join("package.json")).ok()?;
    let json:Value=serde_json::from_str(&text).ok()?;
    let scripts=json["scripts"].as_object().into_iter().flatten().filter_map(|(_,v)|v.as_str());
    scripts.filter_map(|cmd|cmd.split("hyperframes@").nth(1).map(|rest|rest.chars().take_while(|c|c.is_ascii_digit()||*c=='.').collect::<String>()))
        .find(|v|!v.is_empty())
        .or_else(||json["dependencies"]["hyperframes"].as_str().or(json["devDependencies"]["hyperframes"].as_str()).map(|v|v.trim_start_matches(['^','~','=']).to_string()))
}

/// An already-downloaded executable of exactly `version`: the project's own node_modules, then
/// the npm cache under `npx_root`. Nothing is installed or fetched.
pub(crate) fn pinned_cli(project: &Path, npx_root: &Path, version: &str) -> Option<PathBuf> {
    let matches=|modules:&Path|{
        let manifest=std::fs::read_to_string(modules.join("hyperframes/package.json")).ok()?;
        let found=serde_json::from_str::<Value>(&manifest).ok()?["version"].as_str()?.to_string();
        let bin=modules.join(".bin/hyperframes");
        (found==version && bin.is_file()).then_some(bin)
    };
    matches(&project.join("node_modules")).or_else(||{
        let mut caches:Vec<PathBuf>=std::fs::read_dir(npx_root).ok()?.filter_map(|e|e.ok()).map(|e|e.path().join("node_modules")).collect();
        caches.sort();
        caches.iter().find_map(|modules|matches(modules))
    })
}

/// Resolve an explicitly selected executable, HYPERFRAMES_CLI, PATH, or an already-downloaded copy
/// of the version the project pins; never install with npx. A failure says exactly what to do.
pub(crate) fn hyperframes_cli(explicit: Option<&str>, project: Option<&Path>) -> Result<PathBuf> {
    let configured=explicit.map(PathBuf::from).or_else(||std::env::var_os("HYPERFRAMES_CLI").map(PathBuf::from));
    let on_path=||std::env::var_os("PATH").and_then(|v|std::env::split_paths(&v).map(|d|d.join("hyperframes")).find(|p|p.is_file()));
    let pinned=project.and_then(|dir|pinned_hyperframes(dir).map(|version|(dir,version)));
    let cached=||pinned.as_ref().and_then(|(dir,version)|{
        let npx=PathBuf::from(std::env::var_os("HOME")?).join(".npm/_npx");
        pinned_cli(dir,&npx,version)
    });
    let path=configured.or_else(on_path).or_else(cached).ok_or_else(||EngineError::InvalidParams(match &pinned {
        Some((dir,version))=>format!("HyperFrames {version} not found. This project pins it in {}. Install it with `npm i -g hyperframes@{version}`, or set HYPERFRAMES_CLI=/absolute/path/to/hyperframes (or pass hyperframes_cli=...). Nothing was installed.",dir.join("package.json").display()),
        None=>"HyperFrames executable not found on PATH. Install it with `npm i -g hyperframes`, or set HYPERFRAMES_CLI=/absolute/path/to/hyperframes (or pass hyperframes_cli=...). Nothing was installed.".into(),
    }))?;
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

fn init_hyperframes(s:&Session, dir:&Path, executable:Option<&str>, replace_scaffold:bool) -> Result<PathBuf> {
    let cli=hyperframes_cli(executable,Some(&dir.join("hyperframes")))?;
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|EngineError::InvalidParams(e.to_string()))?.as_nanos();
    let stage=dir.join(format!(".hyperframes-init-{}-{stamp}",std::process::id()));
    let mut args=vec!["init".to_string(),stage.to_string_lossy().into_owned(),"--non-interactive".into(),"--skip-transcribe".into()];
    if let Some(audio)=&s.song_audio {args.extend(["--audio".into(),audio.to_string_lossy().into_owned()]);}
    let output=std::process::Command::new(&cli).args(&args).current_dir(dir).env("HYPERFRAMES_SKIP_SKILLS","1").output()
        .map_err(|e|EngineError::ExternalTool(cli.display().to_string(),e.to_string()))?;
    if !output.status.success(){return Err(EngineError::ExternalTool(cli.display().to_string(),format!("official init exited {:?}: {}. Partial output retained at {}",output.status.code(),String::from_utf8_lossy(&output.stderr),stage.display())));}
    if let Some(transcript)=crate::workshop::transcript_files(dir).into_iter().next() {
        let imported=run_tool_in(dir,cli.to_str().ok_or_else(||EngineError::InvalidParams("Runtime path must be UTF-8".into()))?,&["transcribe".into(),transcript.to_string_lossy().into_owned(),"--dir".into(),stage.to_string_lossy().into_owned(),"--json".into(),"--no-runtime-install".into()])?;
        let result:Value=serde_json::from_str(&imported).map_err(|e|EngineError::InvalidParams(format!("Official existing-transcript import returned invalid JSON: {e}")))?;
        if result["ok"]!=true || !stage.join("transcript.json").is_file(){return Err(EngineError::InvalidParams(format!("Official existing-transcript import failed: {imported}; staged files retained at {}",stage.display())));}
    }
    let config=stage.join("hyperframes.json");let entry=stage.join("index.html");
    if !entry.is_file() || !std::fs::read(&config).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v|v.is_object()) {
        return Err(EngineError::InvalidParams(format!("Official HyperFrames init did not create a valid hyperframes.json and index.html; output retained at {}",stage.display())));
    }
    let dest=dir.join("hyperframes");let mut files=Vec::new();generated_files(&stage,Path::new(""),&mut files)?;
    // Replacement is explicit and backed up; captions, artwork and other files cannot be overwritten.
    let replaceable=|p:&Path|replace_scaffold&&matches!(p.to_str(),Some("index.html"|"hyperframes.json"|"package.json"|"meta.json"));
    for relative in &files {
        let target=dest.join(relative);
        let same=target.is_file()&&std::fs::read(&target)?==std::fs::read(stage.join(relative))?;
        if (target.exists()&&!same&&!replaceable(relative)) || target.ancestors().skip(1).take_while(|p|p.starts_with(&dest)).any(|p|p.exists()&&!p.is_dir()) {
            return Err(EngineError::InvalidParams(format!("Official init conflicts with existing {}; no existing file overwritten. Generated output retained at {}",target.display(),stage.display())));
        }
    }
    for relative in files {
        let target=dest.join(&relative);
        if target.is_file() {
            if std::fs::read(&target)?==std::fs::read(stage.join(&relative))?{continue;}
            let backup=dest.join(".magicstudio-history").join(format!("init-{stamp}")).join(&relative);
            std::fs::create_dir_all(backup.parent().ok_or_else(||EngineError::InvalidParams("Backup has no parent".into()))?)?;std::fs::copy(&target,backup)?;
        }
        std::fs::create_dir_all(target.parent().ok_or_else(||EngineError::InvalidParams("Generated file has no parent".into()))?)?;
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
                        "// Remotion project scaffolded by Magic Studio\n",
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

/// Deterministic binding of native timed words into the official reusable caption component.
/// Existing edited caption sources/visual overrides are never replaced on unchanged input.
fn ensure_native_captions(s:&Session, dir:&Path) -> Result<Value> {
    use sha2::{Digest,Sha256};
    let hf=dir.join("hyperframes");
    let Some(transcript)=crate::workshop::transcript_files(dir).into_iter().next() else {return Ok(json!({"ready":false,"reason":"No timed transcript available"}));};
    let raw=std::fs::read(&transcript)?;
    let input:Value=serde_json::from_slice(&raw).map_err(|e|EngineError::InvalidParams(e.to_string()))?;
    let words=input.as_array().or_else(||input["words"].as_array()).filter(|v|!v.is_empty()).ok_or_else(||EngineError::InvalidParams("Native captions require existing timed words".into()))?;
    if input.is_array() {
        if words.iter().any(|w|w["text"].as_str().is_none_or(|t|t.trim().is_empty()) || w["start"].as_f64().zip(w["end"].as_f64()).is_none_or(|(a,b)|!a.is_finite()||!b.is_finite()||a<0.0||b<a)) {return Err(EngineError::InvalidParams("Native caption words require text and valid start/end timestamps".into()));}
    } else {
        let problems=crate::workshop::transcript_problems(&input);if !problems.is_empty(){return Err(EngineError::InvalidParams(problems.join("; ")));}
    }
    let digest=format!("{:x}",Sha256::digest(serde_json::to_vec(words).map_err(|e|EngineError::InvalidParams(e.to_string()))?));
    let dest=hf.join("compositions/captions.html");let receipt=hf.join(".magicstudio-captions.json");let entry=hf.join("index.html");
    let mut host=std::fs::read_to_string(&entry)?;
    if host.contains("data-track-kind=\"captions\"") && !host.contains("data-composition-src=\"compositions/captions.html\"") {return Ok(json!({"ready":true,"preserved_existing_caption_track":true,"word_count":words.len()}));}
    let duration=probe_audio_duration(s.song_audio.as_deref()).max(words.iter().filter_map(|w|w["end"].as_f64()).fold(0.0,f64::max));
    let reused=dest.is_file();
    if reused {
        let state:Value=std::fs::read(&receipt).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        if state["input_sha256"]!=digest {return Err(EngineError::InvalidParams(format!("Existing caption source {} differs from this transcript or has no matching provenance; preserved without overwrite. Review that source before replacing timing",dest.display())));}
    } else {
        let groups:Vec<[usize;2]>=(0..words.len()).step_by(5).map(|start|[start,(start+4).min(words.len()-1)]).collect();
        let text=serde_json::to_string(words).map_err(|e|EngineError::InvalidParams(e.to_string()))?.replace('<',"\\u003c");
        let html=include_str!("../../../pipeline/templates/hyperframes-captions.html").replace("__AC_WORDS__",&text).replace("__AC_GROUPS__",&serde_json::to_string(&groups).unwrap()).replace("__AC_DURATION__",&duration.to_string());
        std::fs::create_dir_all(dest.parent().unwrap())?;std::fs::write(&dest,html)?;
        std::fs::write(&receipt,serde_json::to_vec_pretty(&json!({"input_sha256":digest,"transcript":transcript,"word_count":words.len(),"duration":duration,"grouping":"provisional deterministic batches of five; no semantic or listening review claimed","source":"official caption-highlight; native caption-group classes"})).unwrap())?;
    }
    let marker="data-composition-src=\"compositions/captions.html\"";
    if !host.contains(marker) {
        if !host.contains("</body>"){return Err(EngineError::InvalidParams("Official host has no closing body; caption source retained but no host edit performed".into()));}
        let end=host.rfind("</div>").ok_or_else(||EngineError::InvalidParams("Official host has no composition container; no host edit performed".into()))?;
        host.insert_str(end,&format!("<div data-composition-id=\"caption-highlight\" {marker} data-track-kind=\"captions\" data-start=\"0\" data-duration=\"{duration}\" data-track-index=\"3\" data-width=\"1920\" data-height=\"1080\"></div>\n"));std::fs::write(&entry,host)?;
    }
    Ok(json!({"ready":true,"source":dest,"provenance":receipt,"word_count":words.len(),"duration_sec":duration,"reused_existing_source":reused,"grouping_review":"provisional; native timing preserved; no semantic correction or auditory verification"}))
}

pub fn cmd_video_init(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or(&s.video_target)
        .to_string();
    let existed=target_project_path(s,&dir,&target).is_some();
    let replace=params["replace_scaffold"]==true;
    let reused=existed&&!replace;
    let created = if target=="hyperframes" && !reused {init_hyperframes(s,&dir,params["hyperframes_cli"].as_str(),replace)?}else{ensure_video_target_project(s, &dir, &target)?};
    if target == "hyperframes" && !created.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "video.init did not create an editable HyperFrames project: {}. Official initialization returned no editable project; existing audio/transcript files were preserved.", created.display()
        )));
    }
    let mut assembly=Value::Null;
    if params["assemble"]==true {
        if target!="hyperframes" {return Err(EngineError::InvalidParams("Music-to-video assembly requires target=hyperframes".into()));}
        let hf=dir.join("hyperframes");let plan=hf.join("STORYBOARD.md");
        if !plan.is_file(){return Err(EngineError::InvalidParams("Music-to-video assembly requires derived STORYBOARD.md and completed frame files from the native storyboard; no replacement plan was invented".into()));}
        let map=params["audiomap"].as_str().map(|p|resolve_workspace_path(s,p)).unwrap_or_else(||hf.join("audiomap.json"));
        let data:Value=serde_json::from_slice(&std::fs::read(&map)?).map_err(|e|EngineError::InvalidParams(e.to_string()))?;
        let audio=data["audio"]["path"].as_str().map(PathBuf::from).filter(|p|p.is_file()).ok_or_else(||EngineError::InvalidParams("Existing audiomap audio.path must identify the already processed BGM; no new analysis/transcode was attempted".into()))?;
        for (source,dest) in [(audio,hf.join("assets/bgm.mp3")),(map.clone(),hf.join("audiomap.json"))] {
            if dest.is_file()&&std::fs::read(&source)?!=std::fs::read(&dest)?{return Err(EngineError::InvalidParams(format!("Existing {} differs; no overwrite performed",dest.display())));}
            if !dest.exists(){std::fs::create_dir_all(dest.parent().ok_or_else(||EngineError::InvalidParams("Input destination has no parent".into()))?)?;std::fs::copy(source,dest)?;}
        }
        let skill=std::env::var_os("HOME").map(PathBuf::from).ok_or_else(||EngineError::InvalidParams("HOME unavailable for installed music-to-video skill".into()))?.join(".agents/skills/music-to-video");
        let arg=|p:&Path|p.to_string_lossy().into_owned();
        let validate=skill.join("scripts/validate-plan.mjs");let assemble=skill.join("scripts/assemble-index.mjs");
        let validation=run_tool_in(&hf,"node",&[arg(&validate),"--storyboard".into(),arg(&plan),"--audiomap".into(),arg(&hf.join("audiomap.json")),"--templates".into(),arg(&skill.join("references/templates"))])?;
        // Preserve the previous composition before the owning workflow's deterministic assembler writes it.
        let backup=hf.join(".magicstudio-history/assembled-before.html");
        if hf.join("index.html").is_file()&&!backup.exists(){std::fs::create_dir_all(backup.parent().ok_or_else(||EngineError::InvalidParams("Assembly backup has no parent".into()))?)?;std::fs::copy(hf.join("index.html"),&backup)?;}
        let result=run_tool_in(&hf,"node",&[arg(&assemble),"--storyboard".into(),arg(&plan),"--hyperframes".into(),arg(&hf),"--audiomap".into(),arg(&hf.join("audiomap.json"))])?;
        if result.contains("anomalies") {return Err(EngineError::InvalidParams(format!("Music-to-video assembler reported incomplete inputs: {result}")));}
        assembly=json!({"workflow":"music-to-video","validation_script":validate,"assembly_script":assemble,"validation":validation,"result":result,"storyboard":plan,"audiomap":hf.join("audiomap.json"),"bgm":hf.join("assets/bgm.mp3")});
    }
    let mut status = workshop_status(s)?;
    status["music_assembly"]=assembly;
    status["captions"]=if target=="hyperframes" && params["captions"]!=false {ensure_native_captions(s,&dir)?}else{Value::Null};
    status["initialized"] = json!(created);
    status["reused_existing_project"]=json!(reused);
    status["video_complete"]=json!(false);
    status["song_imported"]=json!(target=="hyperframes"&&!reused&&s.song_audio.is_some());
    status["existing_timing_imported"]=json!(target=="hyperframes"&&!reused&&!crate::workshop::transcript_files(&dir).is_empty());
    status["replaced_existing_scaffold"]=json!(existed&&replace);
    s.video_project_path = target_project_path(s, &dir, &target);
    status["video_project_path"]=json!(s.video_project_path);
    if s.project_path.is_some() { let _ = crate::project::write_project_file(s); }
    status["initialization_scope"]=json!("Official editable project with linked song and supplied timing when available; authored visuals and rendered output are separate steps");
    if target=="hyperframes" && !reused {status["runtime_executable"]=json!(hyperframes_cli(params["hyperframes_cli"].as_str(),Some(&dir.join("hyperframes")))?);}
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
    let _ = if target=="hyperframes" && target_project_path(s,&dir,&target).is_none(){init_hyperframes(s,&dir,params["hyperframes_cli"].as_str(),false)?}else{ensure_video_target_project(s, &dir, &target)?};
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
            std::process::Command::new(hyperframes_cli(params["hyperframes_cli"].as_str(),Some(&work_dir))?)
                .args(["preview", "--port", &port.to_string(), "--no-open", "--foreground", "--force-new", &work_dir.to_string_lossy()])
                .current_dir(&work_dir)
                .env("PWD", &work_dir)
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
        "url": if target=="hyperframes"{format!("http://localhost:{port}/#project/hyperframes")}else{format!("http://localhost:{port}")},
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
    let mut request=params.clone();
    if params.get("blender_job").is_some() && params.get("animation_job").is_some(){return Err(EngineError::InvalidParams("Pass only one animation job selector".into()));}
    if let Some(job)=params.get("animation_job").or_else(||params.get("blender_job")){
        request=animation_job_request(s,params,job)?;
    } else if params.get("character").is_some() || params.get("track").is_some(){
        let character=params["character"].as_str().ok_or_else(||EngineError::InvalidParams("Source motion needs character and track".into()))?;
        let track=params["track"].as_str().ok_or_else(||EngineError::InvalidParams("Source motion needs character and track".into()))?;
        // Inspect the saved rig without loading it into the live selection or rewriting project metadata.
        let asset=s.project_assets.iter().find(|a|a.id==character)
            .ok_or_else(||EngineError::InvalidParams("Source character is not registered".into()))?;
        let path=asset.path.as_ref().ok_or_else(||EngineError::InvalidParams("Source character has no saved rig".into()))?;
        let bytes=std::fs::read(path)?;
        let source:Value=serde_json::from_slice(&bytes).map_err(|_|EngineError::InvalidParams("Source motion needs a saved rig JSON".into()))?;
        let clip=source["tracks"].as_array().and_then(|tracks|tracks.iter().find(|t|t["id"]==track))
            .ok_or_else(||EngineError::InvalidParams("Save this named motion track in the rig before linking it to the video".into()))?;
        if !clip["duration"].as_f64().is_some_and(|d|d.is_finite()&&d>0.0){return Err(EngineError::InvalidParams("Saved source track has no valid duration".into()));}
        request["source_motion"]=json!({"character":character,"track":track,"duration":clip["duration"],
            "spec":path,"canvas":source["canvas"],"rig_sha256":format!("{:x}",Sha256::digest(&bytes)),"relationship":"declared source clip; rendered media is a separate artifact"});
    }
    let script=crate::workspace::workspace_root(s).join("pipeline/place_hyperframes_track.py");
    let output=run_tool_in(&dir,"python3",&[script.to_string_lossy().into_owned(),"--project".into(),dir.to_string_lossy().into_owned(),"--request".into(),request.to_string()])?;
    serde_json::from_str(&output).map_err(|e|EngineError::InvalidParams(e.to_string()))
}

fn path_exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

fn unique_stage_dir(parent: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(parent)?;
    for _ in 0..8 {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?.as_nanos();
        let path = parent.join(format!(".magicstudio-video-{}-{stamp}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(EngineError::InvalidParams("Could not create a unique video staging directory".into()))
}

fn snapshot_times(params: &Value) -> Result<Vec<f64>> {
    let invalid = || EngineError::InvalidParams("video.snapshot requires at=<seconds> or at=[seconds, ...]".into());
    let values: Vec<f64> = match &params["at"] {
        Value::Number(n) => vec![n.as_f64().ok_or_else(invalid)?],
        Value::String(s) => s.split(',').map(|part| part.trim().parse::<f64>().map_err(|_| invalid())).collect::<Result<_>>()?,
        Value::Array(a) => a.iter().map(|v| v.as_f64().ok_or_else(invalid)).collect::<Result<_>>()?,
        _ => return Err(invalid()),
    };
    if values.is_empty() || values.iter().any(|t| !t.is_finite() || *t < 0.0) {
        return Err(EngineError::InvalidParams("Snapshot times must be a non-empty list of finite non-negative seconds".into()));
    }
    for pair in values.windows(2) {
        if (pair[1] - pair[0]).abs() < 0.001 {
            return Err(EngineError::InvalidParams("Snapshot times must be at least 1 ms apart".into()));
        }
    }
    Ok(values)
}

/// Snapshot exact requested times through the official CLI. A single `out` is a PNG path;
/// multiple times use `out` as a new directory containing the CLI's PNG names.
pub fn cmd_video_snapshot(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or(&s.video_target)
        .to_string();
    if target != "hyperframes" {
        return Err(EngineError::InvalidParams(
            "video.snapshot requires target=hyperframes".into(),
        ));
    }
    let hf = dir.join("hyperframes");
    if !hf.join("index.html").is_file() {
        return Err(EngineError::InvalidParams(
            "No HyperFrames project in this song. Run video.init with target=hyperframes first.".into(),
        ));
    }
    let times = snapshot_times(params)?;
    let out = params
        .get("out")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("video.snapshot requires out=<path>".into()))?;
    let out_path = resolve_workspace_path(s, out);
    if path_exists(&out_path) {
        return Err(EngineError::InvalidParams(format!(
            "Output already exists, refusing to overwrite: {}",
            out_path.display()
        )));
    }
    let parent = out_path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let cli = hyperframes_cli(params["hyperframes_cli"].as_str(), Some(&hf))?;
    let stage = unique_stage_dir(parent)?;
    let at_arg = times.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
    let output = std::process::Command::new(&cli)
        .args(["snapshot".into(), hf.to_string_lossy().into_owned(), "--at".into(), at_arg,
            "--no-end".into(), "--describe".into(), "false".into(),
            "--output".into(), stage.to_string_lossy().into_owned()])
        .current_dir(&hf).output()
        .map_err(|e| { let _ = std::fs::remove_dir_all(&stage); EngineError::ExternalTool(cli.display().to_string(), e.to_string()) })?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(EngineError::ExternalTool(cli.display().to_string(), format!("snapshot exited {:?}; stdout: {stdout}; stderr: {stderr}", output.status.code())));
    }
    let mut pngs = std::fs::read_dir(&stage)?.filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"))).collect::<Vec<_>>();
    pngs.sort();
    if pngs.len() != times.len() || pngs.iter().any(|p| std::fs::metadata(p).map_or(true, |m| m.len() == 0)) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(EngineError::ExternalTool(cli.display().to_string(), format!("snapshot produced {} non-empty PNGs for {} requested times; stdout: {stdout}; stderr: {stderr}", pngs.len(), times.len())));
    }
    let mut paths = Vec::with_capacity(pngs.len());
    if times.len() == 1 {
        let source = &pngs[0];
        if let Some(p) = out_path.parent() { std::fs::create_dir_all(p)?; }
        if let Err(e) = std::fs::hard_link(source, &out_path) {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(EngineError::ExternalTool(cli.display().to_string(), format!("could not publish snapshot without overwrite: {e}; stdout: {stdout}; stderr: {stderr}")));
        }
        paths.push(out_path.clone());
    } else {
        std::fs::create_dir(&out_path).map_err(|e| EngineError::ExternalTool(cli.display().to_string(), format!("could not create snapshot output directory without overwrite: {e}")))?;
        for source in &pngs {
            let target = out_path.join(source.file_name().unwrap_or_default());
            if let Err(e) = std::fs::hard_link(source, &target) {
                for published in &paths { let _ = std::fs::remove_file(published); }
                let _ = std::fs::remove_dir(&out_path);
                let _ = std::fs::remove_dir_all(&stage);
                return Err(EngineError::ExternalTool(cli.display().to_string(), format!("could not publish snapshot without overwrite: {e}; stdout: {stdout}; stderr: {stderr}")));
            }
            paths.push(target);
        }
    }
    let _ = std::fs::remove_dir_all(&stage);
    let at_result = if times.len() == 1 { json!(times[0]) } else { json!(times) };
    Ok(json!({
        "ok": true,
        "command": "video.snapshot",
        "project": hf,
        "target": "hyperframes",
        "at": at_result,
        "output": out_path,
        "outputs": paths,
        "stdout": stdout,
        "stderr": stderr,
        "provenance": "official HyperFrames CLI snapshot; no existing output overwritten",
    }))
}

fn ffprobe_duration(ffprobe: &std::ffi::OsStr, path: &Path) -> Result<f64> {
    let result = std::process::Command::new(ffprobe).args(["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(path).output().map_err(|e| EngineError::ExternalTool("ffprobe".into(), e.to_string()))?;
    if !result.status.success() {
        return Err(EngineError::ExternalTool("ffprobe".into(), format!("could not inspect rendered video: {}", String::from_utf8_lossy(&result.stderr))));
    }
    let value = String::from_utf8_lossy(&result.stdout).trim().parse::<f64>()
        .map_err(|e| EngineError::ExternalTool("ffprobe".into(), format!("invalid duration: {e}")))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(EngineError::ExternalTool("ffprobe".into(), "rendered video has no positive duration".into()));
    }
    Ok(value)
}

fn sha256_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Render a complete HyperFrames composition through the official CLI. The CLI has no range
/// arguments; callers requesting a slice receive an explicit error instead of a full-film render.
pub fn cmd_video_render(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or(&s.video_target)
        .to_string();
    if target != "hyperframes" {
        return Err(EngineError::InvalidParams(
            "video.render requires target=hyperframes".into(),
        ));
    }
    let hf = dir.join("hyperframes");
    if !hf.join("index.html").is_file() {
        return Err(EngineError::InvalidParams(
            "No HyperFrames project in this song. Run video.init with target=hyperframes first.".into(),
        ));
    }
    let out = params
        .get("out")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("video.render requires out=<path>".into()))?;
    let out_path = resolve_workspace_path(s, out);
    if path_exists(&out_path) {
        return Err(EngineError::InvalidParams(format!(
            "Output already exists, refusing to overwrite: {}",
            out_path.display()
        )));
    }
    if params.get("start").is_some() || params.get("end").is_some() {
        return Err(EngineError::InvalidParams("The official HyperFrames render command does not support start/end slices; omit both to render the complete composition".into()));
    }
    let fps = params
        .get("fps")
        .and_then(Value::as_f64)
        .unwrap_or(24.0);
    if !fps.is_finite() || !(1.0..=240.0).contains(&fps) || fps.fract() != 0.0 {
        return Err(EngineError::InvalidParams("fps must be a whole number from 1 through 240".into()));
    }
    let workers = params
        .get("workers")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(4)
        .clamp(1, 16);
    let hf = dir.join("hyperframes");
    render_video(params, std::ffi::OsStr::new("ffprobe"), &hf, &out_path, fps, workers)
}

fn render_video(params: &Value, ffprobe: &std::ffi::OsStr, hf: &Path, out_path: &Path, fps: f64, workers: usize) -> Result<Value> {
    let cli = hyperframes_cli(params["hyperframes_cli"].as_str(), Some(hf))?;
    let parent = out_path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let stage = unique_stage_dir(parent)?;
    let staged = stage.join(out_path.file_name().unwrap_or_default());
    let args = ["render".to_string(), hf.to_string_lossy().into_owned(), "--output".into(), staged.to_string_lossy().into_owned(), "--fps".into(), fps.to_string(), "--workers".into(), workers.to_string()];
    let output = std::process::Command::new(&cli).args(&args).current_dir(hf).output()
        .map_err(|e| { let _ = std::fs::remove_dir_all(&stage); EngineError::ExternalTool(cli.display().to_string(), e.to_string()) })?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(EngineError::ExternalTool(cli.display().to_string(), format!("render exited {:?}; stdout: {stdout}; stderr: {stderr}", output.status.code())));
    }
    if std::fs::metadata(&staged).map_or(true, |m| m.len() == 0) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(EngineError::ExternalTool(cli.display().to_string(), format!("render reported success without a non-empty movie; stdout: {stdout}; stderr: {stderr}")));
    }
    let duration = match ffprobe_duration(ffprobe, &staged) {
        Ok(value) => value,
        Err(e) => { let _ = std::fs::remove_dir_all(&stage); return Err(EngineError::ExternalTool(cli.display().to_string(), format!("{e}; render stdout: {stdout}; render stderr: {stderr}"))); }
    };
    let bytes = std::fs::metadata(&staged)?.len();
    let digest = sha256_file(&staged)?;
    if let Err(e) = std::fs::hard_link(&staged, out_path) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(EngineError::ExternalTool(cli.display().to_string(), format!("could not publish render without overwrite: {e}; stdout: {stdout}; stderr: {stderr}")));
    }
    let _ = std::fs::remove_dir_all(&stage);
    Ok(json!({
        "ok": true,
        "command": "video.render",
        "project": hf,
        "target": "hyperframes",
        "output": out_path,
        "fps": fps,
        "workers": workers,
        "duration_sec": duration,
        "bytes": bytes,
        "sha256": digest,
        "stdout": stdout,
        "stderr": stderr,
        "receipt": {"cli": cli.clone(), "project": hf, "output": out_path, "duration_sec": duration, "fps": fps, "workers": workers, "bytes": bytes, "sha256": digest.clone()},
        "provenance": "official HyperFrames CLI render; no existing output overwritten",
    }))
}

/// Resolve a completed project-local animation receipt: verified clip, canvas rect mapped to the
/// rendered world frame, and the provenance frozen at render time (the current rig is not rehashed).
fn animation_job_request(s:&mut Session,params:&Value,job:&Value)->Result<Value>{
    let invalid=|m:String|EngineError::InvalidParams(m);
    if params.get("path").is_some(){return Err(invalid("Pass animation_job/blender_job or path, not both".into()));}
    let receipt=crate::blender::cmd_blender_status(s,&json!({"job":job}))?;
    if receipt["state"]!="completed"{return Err(invalid(format!("Animation job is {}, not completed",receipt["state"])));}
    if receipt["partial"]!=false{return Err(invalid("Animation job is a shortened preview; render the full track before placing it".into()));}
    let jobs=s.project_path.as_ref().and_then(|p|p.parent())
        .map(|p|p.join("animation/blender-jobs").join(receipt["job"].as_str().unwrap_or_default()))
        .ok_or_else(||invalid("Open a .magic project first".into()))?;
    let clip=PathBuf::from(receipt["clip"].as_str().ok_or_else(||invalid("Receipt has no clip".into()))?);
    if clip.parent()!=Some(jobs.as_path()){return Err(invalid("Receipt clip is outside its job folder".into()));}
    // Stream the hash: a long clip must not be loaded into Studio memory.
    let mut hasher=Sha256::new();
    std::io::copy(&mut std::fs::File::open(&clip)?,&mut hasher)?;
    if receipt["clip_sha256"].as_str()!=Some(format!("{:x}",hasher.finalize()).as_str()){
        return Err(invalid("Clip does not match its receipt clip_sha256".into()));
    }
    for key in ["character","track"]{
        if let Some(v)=params.get(key) && *v!=receipt[key]{return Err(invalid(format!("{key} does not match the rendered job ({})",receipt[key])));}
    }
    let number=|v:&Value,what:&str|v.as_f64().filter(|n|n.is_finite()).ok_or_else(||invalid(format!("{what} is missing or invalid")));
    let positive=|v:&Value,what:&str|number(v,what).and_then(|n|if n>0.0{Ok(n)}else{Err(invalid(format!("{what} must be positive")))});
    let rate=params.get("playback_rate").map_or(Ok(1.0),|v|positive(v,"playback_rate"))?;
    if rate>16.0{return Err(invalid("playback_rate must be in (0,16]".into()));}
    let duration=positive(&params["duration"],"duration")?;
    let (seconds,fps)=(positive(&receipt["clip_seconds"],"Receipt clip_seconds")?,positive(&receipt["fps"],"Receipt fps")?);
    let (width,height)=(positive(&receipt["width"],"Receipt width")?,positive(&receipt["height"],"Receipt height")?);
    // The receipt must describe the actual media: dimensions exact, rate and duration within a frame.
    let probe=run_tool_in(&jobs,"ffprobe",&["-v","error","-select_streams","v:0","-show_entries","stream=width,height,r_frame_rate:format=duration","-of","json"].map(String::from).into_iter().chain([clip.to_string_lossy().into_owned()]).collect::<Vec<_>>())?;
    let probe:Value=serde_json::from_str(&probe).map_err(|e|invalid(format!("ffprobe output: {e}")))?;
    let stream=&probe["streams"][0];
    let media_fps=stream["r_frame_rate"].as_str().and_then(|r|r.split_once('/'))
        .and_then(|(n,d)|Some(n.parse::<f64>().ok()?/d.parse::<f64>().ok()?)).filter(|f|f.is_finite()&&*f>0.0);
    let media_seconds=probe["format"]["duration"].as_str().and_then(|d|d.parse::<f64>().ok());
    let (Some(media_fps),Some(media_seconds),Some(media_w),Some(media_h))=(media_fps,media_seconds,stream["width"].as_f64(),stream["height"].as_f64())
        else{return Err(invalid("ffprobe could not read the clip's frame rate, duration or dimensions".into()));};
    if media_w!=width||media_h!=height||(media_fps-fps).abs()>fps*1e-3||(media_seconds-seconds).abs()>1.0/fps{
        return Err(invalid(format!("Clip media ({media_w}x{media_h}, {media_fps:.3}fps, {media_seconds:.3}s) does not match its receipt ({width}x{height}, {fps:.3}fps, {seconds:.3}s)")));
    }
    if duration>seconds/rate+1e-6{
        return Err(invalid(format!("Clip covers {:.3}s at playback_rate {rate}; requested duration is {duration}s",seconds/rate)));
    }
    // rect frames the original canvas with object-fit: contain; the clip itself shows world_rect.
    let values=|v:&Value,n:usize,what:&str|->Result<Vec<f64>>{
        let a=v.as_array().filter(|a|a.len()==n).ok_or_else(||invalid(format!("{what} must have {n} numbers")))?;
        a.iter().map(|x|number(x,what)).collect()
    };
    let (c,world,rect)=(values(&receipt["canvas"],2,"Receipt canvas")?,values(&receipt["world_rect"],4,"Receipt world_rect")?,values(&params["rect"],4,"rect")?);
    let (ww,wh)=(world[2]-world[0],world[3]-world[1]);
    if !(c[0]>0.0&&c[1]>0.0&&ww>0.0&&wh>0.0&&rect[2]>0.0&&rect[3]>0.0) || (ww/wh*height-width).abs()>1.0{
        return Err(invalid("rect, canvas or world_rect is invalid, or world_rect does not match the clip aspect".into()));
    }
    let scale=(rect[2]/c[0]).min(rect[3]/c[1]);
    let (x,y)=(rect[0]+(rect[2]-c[0]*scale)/2.0,rect[1]+(rect[3]-c[1]*scale)/2.0);
    let mut request=params.clone();
    request["path"]=json!(clip);
    request["canvas_rect"]=params["rect"].clone();
    request["rect"]=json!([x+world[0]*scale,y+world[1]*scale,ww*scale,wh*scale]);
    for key in ["character","track"]{
        if receipt[key].is_string(){request[key]=receipt[key].clone();}
    }
    let mut frozen=json!({"receipt":jobs.join("result.json"),"relationship":"provenance frozen when this clip was rendered; later source edits do not change it"});
    for key in ["job","source_job","backend","character","track","spec","rig_sha256","artwork","source_blend","source_blend_sha256","clip_sha256","clip_seconds","fps","first_frame","last_frame","canvas","world_rect","source_bundle","tool","selected_range","authored_range","range_note","partial","frame_mapping","alpha_evidence","native_project","native_project_sha256","audio_mode","fps_fraction"]{
        if let Some(value)=receipt.get(key){frozen[key]=value.clone();}
    }
    request["rendered_source"]=frozen;
    Ok(request)
}

#[cfg(test)]
mod hyperframes_cli_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn temp(name:&str)->PathBuf{let d=std::env::temp_dir().join(format!("hf-cli-{name}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);std::fs::create_dir_all(&d).unwrap();d}
    fn install(modules:&Path,version:&str){
        std::fs::create_dir_all(modules.join("hyperframes")).unwrap();std::fs::create_dir_all(modules.join(".bin")).unwrap();
        std::fs::write(modules.join("hyperframes/package.json"),format!("{{\"version\":\"{version}\"}}")).unwrap();
        std::fs::write(modules.join(".bin/hyperframes"),"#!/bin/sh\n").unwrap();
    }
    fn fake_cli(root:&Path)->(PathBuf,PathBuf,PathBuf){
        let cli=root.join("hyperframes-fake");
        let log=root.join("args.json");
        let source=r##"#!/usr/bin/env python3
import json, pathlib, sys
args=sys.argv[1:]
pathlib.Path(__FAKE_LOG__).write_text(json.dumps(args))
if args[0] == 'snapshot':
    folder=pathlib.Path(args[args.index('--output')+1]); folder.mkdir(parents=True, exist_ok=True)
    count=len(args[args.index('--at')+1].split(','))
    for n in range(count): (folder / f'frame-{n}.png').write_bytes(b'png-fixture')
else:
    pathlib.Path(args[args.index('--output')+1]).write_bytes(b'movie-fixture')
print('fake stdout')
print('fake stderr', file=sys.stderr)
"##.replace("__FAKE_LOG__",&serde_json::to_string(&log.to_string_lossy().into_owned()).unwrap());
        std::fs::write(&cli, source).unwrap();
        let mut mode=std::fs::metadata(&cli).unwrap().permissions();mode.set_mode(0o755);std::fs::set_permissions(&cli,mode).unwrap();
        let ffprobe=root.join("ffprobe-fake");
        std::fs::write(&ffprobe,"#!/bin/sh\nprintf '1.25\\n'\n").unwrap();
        let mut mode=std::fs::metadata(&ffprobe).unwrap().permissions();mode.set_mode(0o755);std::fs::set_permissions(&ffprobe,mode).unwrap();
        (cli,ffprobe,log)
    }
    fn video_session(root:&Path)->Session{
        let project=root.join("song.magic");std::fs::write(&project,"{}").unwrap();
        let hf=root.join("hyperframes");std::fs::create_dir_all(&hf).unwrap();std::fs::write(hf.join("index.html"),"<html></html>").unwrap();
        let mut s=Session::default();s.cwd=root.to_path_buf();s.project_path=Some(project);s.video_target="hyperframes".into();s
    }
    #[test]
    fn the_pinned_version_comes_from_the_project_scripts(){
        let p=temp("pin");
        std::fs::write(p.join("package.json"),r#"{"scripts":{"dev":"npx --yes hyperframes@0.8.166 preview"}}"#).unwrap();
        assert_eq!(pinned_hyperframes(&p).as_deref(),Some("0.8.166"));
        std::fs::write(p.join("package.json"),r#"{"dependencies":{"hyperframes":"^0.8.181"}}"#).unwrap();
        assert_eq!(pinned_hyperframes(&p).as_deref(),Some("0.8.181"));
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn only_a_downloaded_copy_of_exactly_the_pinned_version_is_used(){
        let p=temp("proj");let npx=temp("npx");
        install(&npx.join("aaa/node_modules"),"0.8.189");
        assert_eq!(pinned_cli(&p,&npx,"0.8.166"),None,"another version is never used");
        install(&npx.join("bbb/node_modules"),"0.8.166");
        assert_eq!(pinned_cli(&p,&npx,"0.8.166"),Some(npx.join("bbb/node_modules/.bin/hyperframes")));
        install(&p.join("node_modules"),"0.8.166");
        assert_eq!(pinned_cli(&p,&npx,"0.8.166"),Some(p.join("node_modules/.bin/hyperframes")),"the project's own copy wins");
        std::fs::remove_dir_all(p).unwrap();std::fs::remove_dir_all(npx).unwrap();
    }
    #[test]
    fn official_snapshot_uses_project_positional_and_publishes_requested_pngs(){
        let root=temp("snapshot-adapter");let (cli,_,log)=fake_cli(&root);
        let mut s=video_session(&root);let out=root.join("stills");
        let result=cmd_video_snapshot(&mut s,&json!({"at":[1.0,2.5],"out":out,"hyperframes_cli":cli})).unwrap();
        let args:Vec<String>=serde_json::from_slice(&std::fs::read(&log).unwrap()).unwrap();
        assert_eq!(args[0],"snapshot");assert_eq!(args[1],root.join("hyperframes").to_string_lossy().to_string());
        assert!(args.contains(&"--output".into()));assert!(!args.iter().any(|a|a=="--project"||a=="--out"));
        assert_eq!(result["outputs"].as_array().unwrap().len(),2);
        assert!(result["outputs"].as_array().unwrap().iter().all(|p|Path::new(p.as_str().unwrap()).is_file()));
        assert_eq!(result["stdout"],"fake stdout\n");assert_eq!(result["stderr"],"fake stderr\n");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn official_render_uses_supported_args_and_returns_verified_receipt(){
        let root=temp("render-adapter");let (cli,ffprobe,log)=fake_cli(&root);
        let hf=root.join("hyperframes");std::fs::create_dir_all(&hf).unwrap();
        let out=root.join("renders/song.mp4");let params=json!({"hyperframes_cli":cli});
        let result=render_video(&params,ffprobe.as_os_str(),&hf,&out,30.0,3).unwrap();
        let args:Vec<String>=serde_json::from_slice(&std::fs::read(&log).unwrap()).unwrap();
        assert_eq!(args[0],"render");assert_eq!(args[1],hf.to_string_lossy().to_string());
        assert!(args.contains(&"--output".into()));assert!(args.contains(&"--fps".into()));assert!(args.contains(&"--workers".into()));
        assert!(!args.iter().any(|a|a=="--project"||a=="--out"||a=="--start"||a=="--end"));
        assert_eq!(result["duration_sec"],1.25);assert_eq!(result["bytes"],13);assert!(out.is_file());
        assert_eq!(result["stdout"],"fake stdout\n");assert_eq!(result["stderr"],"fake stderr\n");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn render_rejects_unsupported_slice_before_invoking_cli(){
        let root=temp("render-slice");let (cli,_,_)=fake_cli(&root);let mut s=video_session(&root);
        let error=cmd_video_render(&mut s,&json!({"out":root.join("out.mp4"),"start":1,"hyperframes_cli":cli})).unwrap_err().to_string();
        assert!(error.contains("does not support start/end slices"));assert!(!root.join("args.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
