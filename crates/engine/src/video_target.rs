//! Official HyperFrames project creation, live Studio server
//! management (`video.server_start` / `video.server_stop`), and reference placement.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{EngineError, Result, Session};
use crate::workshop::{
    project_dir, workshop_status,
};
use crate::workspace::{resolve_workspace_path, run_tool_in};

pub(crate) fn target_project_path(_: &Session, dir: &Path, target: &str) -> Option<PathBuf> {
    if target != "hyperframes" {
        return None;
    }
    let hf = dir.join("hyperframes");
    let config = hf.join("hyperframes.json");
    let index = hf.join("index.html");
    if index.is_file() {
        Some(if config.is_file() { config } else { index })
    } else {
        None
    }
}

pub(crate) fn target_project_chip(s: &Session, dir: &Path, target: &str) -> Value {
    let found = target_project_path(s, dir, target);
    let path = found.clone().unwrap_or_else(|| dir.join("hyperframes/index.html"));
    let name = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().into_owned();
    json!({"name": name, "path": path, "exists": found.is_some(), "target": "hyperframes"})
}

fn native_preview(dir: &Path, flags: &[String], executable: Option<&str>) -> Result<Value> {
    let project=dir.join("hyperframes");
    let runtime=hyperframes_cli(executable,Some(&project))?;
    let mut args=vec!["preview".into(),project.to_string_lossy().into_owned(),"--json".into()];
    args.extend_from_slice(flags);
    let output=run_tool_in(dir,runtime.to_str().ok_or_else(||EngineError::InvalidParams("HyperFrames path must be UTF-8".into()))?,&args)?;
    let reply:Value=serde_json::from_str(&output).map_err(|e|EngineError::InvalidParams(format!("HyperFrames preview: {e}")))?;
    if reply["ok"]!=true {return Err(EngineError::ExternalTool("hyperframes preview".into(),reply.to_string()));}
    Ok(reply["result"].clone())
}

pub(crate) fn video_server_info(dir: &Path, _: &str) -> Value {
    if !dir.join("hyperframes/index.html").is_file(){return json!({"running":false,"target":"hyperframes","state":"not-created"});}
    match native_preview(dir,&["--status".into()],None) {
        Ok(native)=>json!({"running":native["state"]=="running" && native["ready"]==true,"target":"hyperframes","port":native["port"],"url":native["studioUrl"],"pid":native["pid"],"managed":true,"native":native}),
        Err(error)=>json!({"running":null,"target":"hyperframes","state":"unavailable","error":error.to_string()})
    }
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
    let path=configured.or_else(cached).or_else(on_path).ok_or_else(||EngineError::InvalidParams(match &pinned {
        Some((dir,version))=>format!("HyperFrames {version} not found. This project pins it in {}. Install it with `npm i -g hyperframes@{version}`, or set HYPERFRAMES_CLI=/absolute/path/to/hyperframes (or pass hyperframes_cli=...). Nothing was installed.",dir.join("package.json").display()),
        None=>"HyperFrames executable not found on PATH. Install it with `npm i -g hyperframes`, or set HYPERFRAMES_CLI=/absolute/path/to/hyperframes (or pass hyperframes_cli=...). Nothing was installed.".into(),
    }))?;
    if !path.is_absolute() || !path.is_file() {return Err(EngineError::InvalidParams(format!("HyperFrames executable must be an existing absolute file: {}",path.display())));}
    Ok(path)
}

fn init_hyperframes(s:&Session, dir:&Path, executable:Option<&str>) -> Result<PathBuf> {
    let project=dir.join("hyperframes");
    let cli=hyperframes_cli(executable,Some(&project))?;
    let mut args=vec!["init".to_string(),project.to_string_lossy().into_owned(),"--non-interactive".into(),"--example".into(),"blank".into(),"--skill".into(),"hyperframes".into(),"--skip-transcribe".into()];
    if let Some(audio)=&s.song_audio {args.extend(["--audio".into(),audio.to_string_lossy().into_owned()]);}
    let output=std::process::Command::new(&cli).args(&args).current_dir(dir).output()
        .map_err(|e|EngineError::ExternalTool(cli.display().to_string(),e.to_string()))?;
    if !output.status.success(){return Err(EngineError::ExternalTool(cli.display().to_string(),String::from_utf8_lossy(&output.stderr).into_owned()));}
    target_project_path(s,dir,"hyperframes").ok_or_else(||EngineError::InvalidParams("HyperFrames init did not produce its project files".into()))
}

/// Existing native project only; all creation goes through the official init command.
pub(crate) fn ensure_video_target_project(s: &Session, dir: &Path, target: &str) -> Result<PathBuf> {
    target_project_path(s,dir,target).ok_or_else(||EngineError::InvalidParams("No HyperFrames project; upload audio or run video.init".into()))
}

pub(crate) fn probe_audio_duration(audio: Option<&Path>) -> f64 {
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

/// The official caption component bound to timed words (`{text,start,end}`) shown in the given index groups.
pub(crate) fn write_caption_source(dest:&Path,words:&[Value],groups:&[[usize;2]],duration:f64)->Result<()>{
    let text=serde_json::to_string(words).map_err(|e|EngineError::InvalidParams(e.to_string()))?.replace('<',"\\u003c");
    let html=include_str!("../../../pipeline/templates/hyperframes-captions.html").replace("__AC_WORDS__",&text).replace("__AC_GROUPS__",&serde_json::to_string(groups).unwrap()).replace("__AC_DURATION__",&duration.to_string());
    std::fs::create_dir_all(dest.parent().unwrap())?;std::fs::write(dest,html)?;Ok(())
}

/// Deterministic binding of native timed words into the official reusable caption component.
/// Existing edited caption sources/visual overrides are never replaced on unchanged input.
pub(crate) fn ensure_native_captions(s:&Session, dir:&Path) -> Result<Value> {
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
    if host.contains("data-track-kind=\"captions\"") && !host.contains("data-composition-src=\"compositions/captions.html\"") {return Ok(json!({"ready":false,"preserved_existing_caption_track":true,"reason":"Existing authored caption track preserved; synchronization with imported timing has not been verified","word_count":words.len()}));}
    let duration=probe_audio_duration(s.song_audio.as_deref()).max(words.iter().filter_map(|w|w["end"].as_f64()).fold(0.0,f64::max));
    let reused=dest.is_file();
    let mut changed=!reused;
    if reused {
        let state:Value=std::fs::read(&receipt).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        if state["source_kind"]=="artist_lines" {return Ok(json!({"ready":state["input_sha256"]==digest,"preserved_existing_caption_track":true,"reason":"Authored lyric track retained; changed timing requires review","source":dest,"provenance":receipt}));}
        changed=state["input_sha256"]!=digest;
        let actual=format!("{:x}",Sha256::digest(std::fs::read(&dest)?));
        if changed && state["source_sha256"]!=actual {return Err(EngineError::InvalidParams(format!("Existing caption source {} was edited or has no source fingerprint; preserved without overwrite. Review it before replacing timing",dest.display())));}
    }
    if changed {
        let groups:Vec<[usize;2]>=(0..words.len()).step_by(5).map(|start|[start,(start+4).min(words.len()-1)]).collect();
        write_caption_source(&dest,words,&groups,duration)?;
        let source_hash=format!("{:x}",Sha256::digest(std::fs::read(&dest)?));
        std::fs::write(&receipt,serde_json::to_vec_pretty(&json!({"input_sha256":digest,"source_sha256":source_hash,"transcript":transcript,"word_count":words.len(),"duration":duration,"grouping":"provisional deterministic batches of five; no semantic or listening review claimed","source":"official caption-highlight; native caption-group classes"})).unwrap())?;
    }
    let marker="data-composition-src=\"compositions/captions.html\"";
    if !host.contains(marker) {
        if !host.contains("</body>"){return Err(EngineError::InvalidParams("Official host has no closing body; caption source retained but no host edit performed".into()));}
        let end=host.rfind("</div>").ok_or_else(||EngineError::InvalidParams("Official host has no composition container; no host edit performed".into()))?;
        host.insert_str(end,&format!("<div id=\"caption-highlight\" class=\"clip\" data-composition-id=\"caption-highlight\" {marker} data-track-kind=\"captions\" data-start=\"0\" data-duration=\"{duration}\" data-track-index=\"3\" data-width=\"1920\" data-height=\"1080\"></div>\n"));std::fs::write(&entry,host)?;
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
    if target!="hyperframes" {return Err(EngineError::InvalidParams("Magic Studio uses HyperFrames".into()));}
    if params["assemble"]==true {
        return Err(EngineError::InvalidParams("The installed HyperFrames workflow retired the music-to-video assembler. Existing project files were preserved. Author the contained HyperFrames compositions with the current /hyperframes skill; video.init only creates or reopens the project.".into()));
    }
    let existed=target_project_path(s,&dir,&target).is_some();
    if params["replace_scaffold"]==true {return Err(EngineError::InvalidParams("Existing projects are edited by HyperFrames; init does not replace them".into()));}
    let reused=existed;
    let created = if !reused {init_hyperframes(s,&dir,params["hyperframes_cli"].as_str())?}else{ensure_video_target_project(s, &dir, &target)?};
    if !created.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "video.init did not create an editable HyperFrames project: {}. Official initialization returned no editable project; existing audio/transcript files were preserved.", created.display()
        )));
    }
    let runtime=hyperframes_cli(params["hyperframes_cli"].as_str(),Some(&dir.join("hyperframes")))?;
    let native=run_tool_in(&dir,runtime.to_str().ok_or_else(||EngineError::InvalidParams("HyperFrames executable path must be UTF-8".into()))?,&["timeline".into(),dir.join("hyperframes").to_string_lossy().into_owned(),"--json".into()])?;
    let native:Value=serde_json::from_str(&native).map_err(|e|EngineError::InvalidParams(format!("HyperFrames timeline: {e}")))?;
    if s.song_audio.is_some() && !native["timeline"]["tracks"].as_array().into_iter().flatten().any(|t|t["kind"]=="audio" && t["rows"].as_array().is_some_and(|rows|!rows.is_empty())) {
        return Err(EngineError::InvalidParams("HyperFrames project has no audio track; upload was not completed".into()));
    }
    if !reused && s.song_audio.is_some() {
        let copied=native["timeline"]["tracks"].as_array().into_iter().flatten()
            .filter(|track|track["kind"]=="audio").flat_map(|track|track["rows"].as_array().into_iter().flatten())
            .filter_map(|row|row["src"].as_str()).map(|src|dir.join("hyperframes").join(src))
            .find(|path|path.is_file() && path.canonicalize().ok().is_some_and(|p|dir.join("hyperframes").canonicalize().ok().is_some_and(|root|p.starts_with(root))))
            .ok_or_else(||EngineError::InvalidParams("HyperFrames did not return a contained copy of the uploaded recording; project was not marked imported".into()))?;
        s.song_audio=Some(copied);
    }
    let mut status = workshop_status(s)?;
    status["native_timeline"]=native;
    status["music_assembly"]=Value::Null;
    status["captions"]=if params["captions"]!=false {ensure_native_captions(s,&dir)?}else{Value::Null};
    status["initialized"] = json!(created);
    status["reused_existing_project"]=json!(reused);
    status["video_complete"]=json!(false);
    status["song_imported"]=json!(!reused&&s.song_audio.is_some());
    s.video_project_path = target_project_path(s, &dir, &target);
    status["video_project_path"]=json!(s.video_project_path);
    if s.project_path.is_some() { crate::project::write_project_file(s)?; }
    status["initialization_scope"]=json!("Official editable project with linked song and supplied timing when available; authored visuals and rendered output are separate steps");
    if !reused {status["runtime_executable"]=json!(hyperframes_cli(params["hyperframes_cli"].as_str(),Some(&dir.join("hyperframes")))?);}
    Ok(status)
}

pub fn cmd_video_server_start(s: &mut Session, params: &Value) -> Result<Value> {
    let dir=project_dir(s)?;
    ensure_video_target_project(s,&dir,"hyperframes")?;
    let mut flags=vec!["--background".into(),"--no-open".into()];
    if let Some(port)=params["port"].as_u64(){flags.extend(["--port".into(),port.to_string()]);}
    let native=native_preview(&dir,&flags,params["hyperframes_cli"].as_str())?;
    if native["ready"]!=true {return Err(EngineError::ExternalTool("HyperFrames Studio".into(),native.to_string()));}
    let mut status=workshop_status(s)?;status["native_preview"]=native;Ok(status)
}

pub fn cmd_video_server_stop(s: &mut Session, _: &Value) -> Result<Value> {
    let dir=project_dir(s)?;
    native_preview(&dir,&["--stop".into()],None)?;
    workshop_status(s)
}

pub fn read_placed_tracks(dir: &Path) -> Vec<Value> {
    let folder = dir.join("hyperframes/assets/animation");
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut tracks = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(bytes) = std::fs::read(&path)
            && let Ok(val) = serde_json::from_slice::<Value>(&bytes)
        {
            tracks.push(val);
        }
    }
    tracks.sort_by_key(|v| v["track_index"].as_i64().unwrap_or(0));
    tracks
}

pub fn sync_placed_tracks_media_and_duration(
    s: &mut Session,
    character: &str,
    track: &str,
    new_media: Option<&str>,
    new_duration: Option<f64>,
) -> Result<Value> {
    let dir = project_dir(s)?;
    let existing = read_placed_tracks(&dir);
    let mut updated = Vec::new();
    let clean_track = track.trim_start_matches('@');
    for item in existing {
        let item_char = item["character"].as_str().unwrap_or("");
        let item_track = item["track"].as_str().unwrap_or("").trim_start_matches('@');
        if item_char == character && item_track == clean_track {
            if let Some(id) = item["id"].as_str() {
                let mut req = json!({
                    "id": id,
                    "start": item["start"].as_f64().unwrap_or(0.0),
                    "duration": new_duration.unwrap_or_else(|| item["duration"].as_f64().unwrap_or(1.0)),
                    "playback_rate": item["playback_rate"].as_f64().unwrap_or(1.0),
                    "rect": item["rect"].clone(),
                    "character": character,
                    "track": track,
                });
                if let Some(p) = new_media.or_else(|| item["media"].as_str()).or_else(|| item["path"].as_str()) {
                    req["path"] = json!(p);
                }
                if let Ok(placed) = cmd_video_place_track(s, &req) {
                    updated.push(placed["id"].clone());
                }
            }
        }
    }
    Ok(json!(updated))
}

pub fn sync_placed_tracks_duration(
    s: &mut Session,
    character: &str,
    track: &str,
    new_duration: f64,
) -> Result<Value> {
    sync_placed_tracks_media_and_duration(s, character, track, None, Some(new_duration))
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
