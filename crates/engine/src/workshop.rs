//! The song workshop: four stages (song, transcript, reference, animate) plus asset fix list,
//! audio transcription, and unified project search. Storyboard chapters/PDF live in [`crate::storyboard`]
//! and video target scaffolding/servers live in [`crate::video_target`].

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

use crate::control_bindings::CONTROLS;
use crate::project::cmd_project_add_asset;
use crate::storyboard::{
    SceneDoc, clock, panel_spans, read_scene_doc, scene_doc_path, scenes,
};
use crate::video_target::{
    ensure_video_target_project, target_project_chip, target_project_path,
    video_server_info,
};
use crate::workspace::{
    has_extension, resolve_workspace_path, run_tool_in, song_audio_path, workspace_root,
};

// Re-export storyboard and video target command handlers so `crate::workshop::*` callers continue to work.
pub use crate::storyboard::{
    cmd_storyboard_approve, cmd_storyboard_edit, cmd_storyboard_import_openmontage,
    cmd_storyboard_init, cmd_storyboard_read,
    cmd_storyboard_toc, cmd_workshop_build_storyboard, cmd_workshop_storyboard,
};
pub use crate::video_target::{
    cmd_video_init, cmd_video_place_track, cmd_video_render,
    cmd_video_server_start, cmd_video_server_stop, cmd_video_snapshot,
};

/// Image file extensions recognized by the workshop.
pub(crate) const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];
/// Storyboard template inside a song folder: `storyboard/panel-NN.png (+ .prompt.md)`, `storyboard.pdf`.
pub(crate) const STORYBOARD_DIR: &str = "storyboard";
pub(crate) const STORYBOARD_PDF: &str = "storyboard.pdf";
/// Static images for video tracks (characters, props, backgrounds), ideally transparent.
pub(crate) const ASSETS_DIR: &str = "assets";
/// Video targets the song can be edited in; Remotion is the default.
pub const VIDEO_TARGETS: &[&str] = &["remotion", "filmcraft", "hyperframes"];
/// FilmCraft time base: ticks per second.
pub(crate) const FILMCRAFT_TICKS_PER_SECOND: f64 = 254_016_000_000.0;
/// Asset fix list: one decision per problem asset ("clean" or "recreate").
pub(crate) const FIXLIST: &str = "asset-fixes.json";

pub(crate) fn song_seconds(dir: &Path) -> Option<f64> {
    if let Some(duration)=std::fs::read(dir.join("transcript-provenance.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).and_then(|v|v["recording_duration_sec"].as_f64()).filter(|v|v.is_finite()&&*v>0.0){return Some(duration);}
    let fcproj = video_project_path(dir)?;
    let out = run_tool_in(
        dir,
        "filmcraft-cli",
        &[
            "--project".into(),
            fcproj.to_string_lossy().into_owned(),
            "inspect".into(),
            "sequence".into(),
        ],
    )
    .ok()?;
    let seq: Value = serde_json::from_str(&out).ok()?;
    seq.get("duration")
        .and_then(Value::as_f64)
        .map(|t| t / FILMCRAFT_TICKS_PER_SECOND)
}

/// The studio's own words, kept in data the user edits (never in code).
#[derive(Clone, Default)]
pub struct Studio {
    pub name: Option<String>,
    pub intent: Option<String>,
    pub source: Option<PathBuf>,
}

/// Read the studio intent from `intent.md` in the song folder, else `STUDIO.md` beside projects.
pub fn studio(dir: &Path) -> Studio {
    let candidates = [
        Some(dir.join("intent.md")),
        dir.parent().map(|p| p.join("STUDIO.md")),
    ];
    for path in candidates.into_iter().flatten() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let mut name = None;
            let mut body = Vec::new();
            for line in text.lines() {
                match line.strip_prefix("# ") {
                    Some(h) if name.is_none() => name = Some(h.trim().to_string()),
                    _ => body.push(line),
                }
            }
            let intent = body.join("\n").trim().to_string();
            return Studio {
                name,
                intent: (!intent.is_empty()).then_some(intent),
                source: Some(path),
            };
        }
    }
    Studio {
        name: None,
        intent: None,
        source: None,
    }
}

/// Skill files an agent should read for a stage.
fn skill_hints(s: &Session, dir: &Path, names: &[&str]) -> String {
    let roots = [
        Some(workspace_root(s).join(".agents/skills")),
        dir.parent()
            .and_then(Path::parent)
            .map(|p| p.join(".agents/skills")),
    ];
    let found: Vec<String> = names
        .iter()
        .filter_map(|n| {
            roots
                .iter()
                .flatten()
                .map(|r| r.join(n).join("SKILL.md"))
                .find(|p| p.is_file())
        })
        .map(|p| p.display().to_string())
        .collect();
    if found.is_empty() {
        String::new()
    } else {
        format!("\nSkills: {}", found.join(", "))
    }
}

/// What an agent needs to start a stage.
fn stage_brief(s: &Session, dir: &Path, stage: &str) -> String {
    format!("{}\n{}", brief_header(dir), stage_body(s, dir, stage))
}

pub(crate) fn brief_header(dir: &Path) -> String {
    let project = dir.display();
    let studio = studio(dir);
    let intent = match (&studio.intent, &studio.source) {
        (Some(intent), Some(src)) => format!("{intent}\n(from {})\n", src.display()),
        _ => format!(
            "No studio intent yet: write it in {project}/intent.md or STUDIO.md beside the projects.\n"
        ),
    };
    format!(
        "{intent}Project folder: {project}\nWorkshop state: magic-cli exec workshop.status (run project.open path=<the .magic> first)\nStoryboard (read it first): {project}/{STORYBOARD_DIR}/{STORYBOARD_PDF} — pdfcraft-cli text <pdf> --page N\n"
    )
}

pub(crate) fn stage_body(s: &Session, dir: &Path, stage: &str) -> String {
    let target = s.video_target.as_str();
    match stage {
        "song" => "Stage 1 — Song. Point the project at the song (never copy audio) and add the artist's liner notes.\n  magic-cli exec workshop.set_audio path=<song>\n  magic-cli exec workshop.add kind=liner_notes paths='[\"<file>\"]'".to_string(),
        "transcript" => format!(
            "Stage 2 — Transcription & transcoding. FilmCraft speech-to-text, headless; it also writes <song>.fcproj with the song on a sequence.\n  magic-cli exec audio.transcribe\nAlready have a transcript? magic-cli exec workshop.import_transcript path=<file> (it is checked first).\nVideo target: {target}.{}",
            skill_hints(s, dir, &[match target { "hyperframes" => "hyperframes-cli", "filmcraft" => "filmcraft", _ => "remotion-best-practices" }])
        ),
        "reference" => format!(
            "Stage 3 — Reference art. Template: {STORYBOARD_DIR}/panel-NN.png (+ panel-NN.prompt.md with the exact prompt), {STORYBOARD_DIR}/{STORYBOARD_PDF}, {ASSETS_DIR}/<name>.png (static, transparent).\nPanel N illustrates line N of each verse. Follow the studio intent above for style and image rules.\n  magic-cli exec workshop.split_panels image=<grid.png> columns=3 rows=2 prompts=<prompt.md>\n  magic-cli exec workshop.build_storyboard\nCut-outs with PhotoCraft only: magic-cli exec intake.validate image=<panel> asset=<name>{}",
            skill_hints(s, dir, &["storyboard"])
        ),
        _ => format!(
            "Stage 4 — Animate assets. One animated subject at a time over its background; not every asset animates.\nExport layered files for the video target ({target}); never render or encode video.{}",
            skill_hints(s, dir, &["layered-motion", "character-rigging", "image-to-animation"])
        ),
    }
}

/// Files in `dir` (not recursive), sorted, optionally limited to some extensions.
pub(crate) fn files_in(dir: &Path, only: Option<&[&str]>) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
        })
        .filter(|p| only.is_none_or(|exts| has_extension(p, exts)))
        .collect();
    files.sort();
    files
}

pub(crate) fn transcript_files(dir: &Path) -> Vec<PathBuf> {
    let native = dir.join("hyperframes/transcript.json");
    if native.is_file() { return vec![native]; }
    files_in(dir, Some(&["json"]))
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".transcript.json"))
        })
        .collect()
}

pub(crate) fn transcript_document(path: &Path) -> Result<Value> {
    let mut doc: Value = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|e| EngineError::InvalidParams(format!("Invalid transcript {}: {e}", path.display())))?;
    if doc.is_array() { doc = json!({"words":doc}); }
    if !doc.is_object() { return Err(EngineError::InvalidParams("Transcript must be a word array or object with words".into())); }
    if !doc["paragraphs"].is_array() {
        let captions = path.with_file_name("transcript.srt");
        doc["paragraphs"] = if captions.is_file() {
            json!(import_caption_paragraphs(&std::fs::read_to_string(captions)?)?)
        } else { doc["words"].clone() };
    }
    Ok(doc)
}

/// Storyboard panels, in order: `storyboard/panel-NN.`.
pub(crate) fn panel_files(dir: &Path) -> Vec<PathBuf> {
    files_in(&dir.join(STORYBOARD_DIR), Some(IMAGE_EXTENSIONS))
        .into_iter()
        .filter(|p| {
            p.file_stem()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("panel-"))
        })
        .collect()
}

pub(crate) fn video_project_path(dir: &Path) -> Option<PathBuf> {
    transcript_files(dir)
        .iter()
        .find_map(|t| {
            let name = t.file_name()?.to_str()?.replace(".transcript.json", ".fcproj");
            let fcproj = t.with_file_name(name);
            fcproj.is_file().then_some(fcproj)
        })
        .or_else(|| files_in(dir, Some(&["fcproj"])).into_iter().next())
}

/// Static and cutout image assets in the song project folder.
pub(crate) fn workshop_asset_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut push_unique = |paths: Vec<PathBuf>| {
        for p in paths {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    };
    push_unique(files_in(&dir.join(ASSETS_DIR), Some(IMAGE_EXTENSIONS)));
    push_unique(files_in(
        &dir.join(ASSETS_DIR).join("images"),
        Some(IMAGE_EXTENSIONS),
    ));
    push_unique(files_in(
        &dir.join(ASSETS_DIR).join("characters/approved-crops"),
        Some(IMAGE_EXTENSIONS),
    ));
    if let Ok(chars) = std::fs::read_dir(dir.join(ASSETS_DIR).join("characters")) {
        let mut char_dirs: Vec<PathBuf> = chars
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        char_dirs.sort();
        for cdir in char_dirs {
            push_unique(files_in(&cdir.join("parts"), Some(IMAGE_EXTENSIONS)));
        }
    }
    if let Ok(bgs) = std::fs::read_dir(dir.join(ASSETS_DIR).join("backgrounds")) {
        let mut bg_dirs: Vec<PathBuf> = bgs
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        bg_dirs.sort();
        for bdir in bg_dirs {
            push_unique(files_in(&bdir, Some(IMAGE_EXTENSIONS)));
        }
    }
    push_unique(files_in(&dir.join("layer-sources"), Some(IMAGE_EXTENSIONS)));
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut subdirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("magicstudio-"))
            })
            .collect();
        subdirs.sort();
        for sub in subdirs {
            push_unique(files_in(&sub.join("parts"), Some(IMAGE_EXTENSIONS)));
            push_unique(files_in(&sub.join("reference"), Some(IMAGE_EXTENSIONS)));
        }
    }
    out
}

fn file_chip(path: &Path) -> Value {
    json!({"name": path.file_name().and_then(|n| n.to_str()), "path": path, "exists": path.exists()})
}

/// The open project's directory; errors when no project is open.
pub(crate) fn project_dir(s: &Session) -> Result<PathBuf> {
    s.project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            EngineError::InvalidParams("no project is open; use project.new or project.open".into())
        })
}

/// Workshop status: what each stage needs, whether complete, asset riggability, server state.
pub(crate) fn workshop_status(s: &Session) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = s.video_target.as_str();
    let audio_ok = s.song_audio.as_ref().is_some_and(|p| p.is_file());
    let liner = files_in(&dir.join("liner-notes"), None);
    let transcripts = transcript_files(&dir);
    let panels = panel_files(&dir);
    let assets = workshop_asset_files(&dir);
    let pdf = dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF);
    let scene_docs: Vec<SceneDoc> = (1..)
        .map(|n| (n, scene_doc_path(&dir, n)))
        .take_while(|(_, p)| p.is_file())
        .map(|(n, _)| read_scene_doc(&dir, n))
        .collect();
    let approved = pdf.is_file()
        && if scene_docs.is_empty() {
            s.draft_approved
        } else {
            scene_docs.iter().all(|d| d.status == "approved")
        };
    let panel_timing = transcript_files(&dir)
        .first()
        .and_then(|t| transcript_document(t).ok())
        .map(|t| panel_spans(&scenes(&t, None), panels.len()));

    let song_why = match (audio_ok, liner.is_empty()) {
        (false, _) => "Choose the song's audio.",
        (true, true) => "Song audio linked and ready (liner notes optional).",
        (true, false) => "Song and liner notes are in place.",
    };
    let active_target_path = target_project_path(s, &dir, target);
    let target_chip = target_project_chip(s, &dir, target);
    let target_project = json!({
        "target": target,
        "path": active_target_path,
        "ready": active_target_path.is_some(),
        "chip": target_chip,
    });
    let targets: Vec<Value> = VIDEO_TARGETS
        .iter()
        .map(|t| {
            let path = target_project_path(s, &dir, t);
            json!({"target": t, "selected": *t == target, "path": path, "exists": path.is_some()})
        })
        .collect();
    let server = video_server_info(&dir, target);
    let loaded_board = std::fs::read(dir.join("storyboard/movie-board.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or(Value::Null);
    let transcript_source = transcripts.first().cloned().or_else(|| {
        loaded_board["source_transcript"].as_str().map(PathBuf::from)
    });
    let supplied_transcript = transcript_source
        .as_ref()
        .and_then(|p| transcript_document(p).ok())
        .unwrap_or(Value::Null);
    let mut transcript_errors = transcript_problems(&supplied_transcript);
    if loaded_board["timing_valid"] == false {
        transcript_errors.push(loaded_board["timing_warning"].as_str()
            .unwrap_or("Storyboard timing has not passed validation").to_string());
    }
    let transcript_valid = transcript_errors.is_empty();
    let transcript_why = if transcript_source.is_none() {
        "Import existing timed lyrics, or transcribe the linked song.".to_string()
    } else if !transcript_valid {
        format!("Transcript needs repair: {}. Original inputs are retained.", transcript_errors.join("; "))
    } else if let Some(source) = loaded_board["source_transcript"].as_str() {
        format!(
            "Storyboard uses native output {source} directly ({} words); no transcription rerun. Archive label: {} (producer executable/version not recorded). {}",
            loaded_board["source_words"],
            Path::new(source)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            loaded_board["timing_warning"].as_str().unwrap_or("")
        )
    } else if let Some(tool) = supplied_transcript["source"].as_str() {
        format!(
            "Reused native transcript; recorded tool: {tool}; recorded model: {}. No transcription run in this import.",
            supplied_transcript["model"].as_str().unwrap_or("unknown")
        )
    } else {
        "Transcript ready; producer unknown because the supplied file did not record it."
            .to_string()
    };
    let reference_why = match (panels.is_empty(), pdf.is_file(), approved) {
        (true, _, _) => "Add storyboard panels (split a storyboard grid or add panel images).",
        (false, false, _) => "Build the storyboard PDF from the panels and transcript.",
        (false, true, false) => "Review the storyboard PDF, then approve it.",
        (false, true, true) => "Storyboard approved.",
    };
    let animated: Vec<Value> = s
        .project_assets
        .iter()
        .map(|a| {
            let loaded = s.assets.get(&a.id);
            let rig = loaded.and_then(|x| x.cutout_rig.as_ref());
            let parts = rig.map(|r| r.parts.len());
            let tracks = rig
                .and_then(|r| r.effectcraft.get("tracks"))
                .and_then(Value::as_array)
                .map(|t| t.len());
            let controls: Vec<&str> = rig
                .map(|r| {
                    CONTROLS
                        .iter()
                        .copied()
                        .filter(|c| r.control_bindings.supports(c))
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "id": a.id,
                "name": a.name,
                "kind": a.kind,
                "path": a.path,
                "parts": parts,
                "tracks": tracks,
                "controls": controls,
                "animatable": rig.is_some_and(|r| r.parts.len() >= 2),
            })
        })
        .collect();
    let mut transcript_outputs: Vec<Value> = transcripts.iter().map(|p| file_chip(p)).collect();
    // The target's own song analysis (audio.analyze), listed as plain files to open and review.
    for name in [
        "transcript.json",
        "transcript.srt",
        "audiomap.json",
        "STORYBOARD.md",
    ] {
        let file = dir.join(&s.video_target).join(name);
        if file.is_file() {
            transcript_outputs.push(file_chip(&file));
        }
    }
    transcript_outputs.push(target_chip.clone());
    let stages = vec![
        json!({"id": "song", "title": "Song", "done": audio_ok, "why": song_why,
            "inputs": s.song_audio.iter().map(|p| file_chip(p)).chain(liner.iter().map(|p| file_chip(p))).collect::<Vec<_>>(),
            "outputs": vec![target_chip],
            "brief": stage_brief(s, &dir, "song")}),
        json!({"id": "transcript", "title": "Transcription & transcoding", "done": transcript_valid, "why": transcript_why,
            "inputs": s.song_audio.iter().map(|p| file_chip(p)).collect::<Vec<_>>(),
            "outputs": transcript_outputs,
            "document": supplied_transcript, "problems": transcript_errors,
            "target": target_project, "brief": stage_brief(s, &dir, "transcript")}),
        json!({"id": "reference", "title": "Reference art", "done": !panels.is_empty() && approved, "why": reference_why,
            "inputs": panels.iter().enumerate().map(|(i, p)| {
                let mut chip = file_chip(p);
                if let Some(spans) = panel_timing.as_ref().and_then(|t| t.get(i)) {
                    chip["begins"] = json!(spans.iter().map(|(a, _)| clock(*a)).collect::<Vec<_>>());
                }
                chip
            }).collect::<Vec<_>>(),
            "outputs": std::iter::once(file_chip(&pdf)).chain(assets.iter().map(|p| file_chip(p))).collect::<Vec<_>>(),
            "approved": approved, "pdf": pdf, "brief": stage_brief(s, &dir, "reference")}),
        json!({"id": "animate", "title": "Animate assets", "done": !animated.is_empty(), "optional": true,
            "why": if animated.is_empty() { "Drag an asset into Stage 4 to create an animated subject (static scenes need none)." } else { "Pick an animated subject to rig or preview; static assets stay on video tracks." },
            "inputs": assets.iter().map(|p| file_chip(p)).collect::<Vec<_>>(), "assets": animated,
            "brief": stage_brief(s, &dir, "animate")}),
    ];
    let complete = stages.iter().take(3).all(|st| st["done"] == json!(true));
    let studio_info = studio(&dir);
    let effective_video_project = s.video_project_path.clone().or_else(|| active_target_path.clone());
    Ok(json!({
        "ok": true,
        "project_dir": dir,
        "complete": complete,
        "video_target": target,
        "targets": targets,
        "video_server": server,
        "asset_files": assets.iter().map(|p| file_chip(p)).collect::<Vec<_>>(),
        "studio": {"name": studio_info.name, "intent": studio_info.intent, "source": studio_info.source},
        "video_project_path": effective_video_project,
        "video_project_exists": effective_video_project.as_ref().is_some_and(|p| p.exists()),
        "stages": stages
    }))
}

/// numbered_prompts parses panel descriptions from a generating prompt.
fn numbered_prompts(text: &str) -> std::collections::BTreeMap<usize, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut current: Option<usize> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        let number = trimmed
            .split_once(". ")
            .and_then(|(n, rest)| n.parse::<usize>().ok().map(|n| (n, rest)));
        match number {
            Some((n, rest)) => {
                let rest = rest.trim_start();
                let rest = if rest.starts_with('(') {
                    rest.split_once(") ").map_or(rest, |(_, r)| r)
                } else {
                    rest
                };
                out.insert(n, rest.to_string());
                current = Some(n);
            }
            None if trimmed.is_empty() => current = None,
            None => {
                if let Some(n) = current
                    && let Some(entry) = out.get_mut(&n)
                {
                    entry.push(' ');
                    entry.push_str(trimmed);
                }
            }
        }
    }
    out
}

/// Checks a transcript has the shape audio.transcribe writes.
pub(crate) fn transcript_problems(doc: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let timed = |v: &Value| {
        v.get("start")
            .and_then(Value::as_f64)
            .zip(v.get("end").and_then(Value::as_f64))
    };
    match doc.get("words").and_then(Value::as_array) {
        None => problems.push("missing \"words\": [{text, start, end}]".into()),
        Some(words) if words.is_empty() => problems.push("\"words\" is empty".into()),
        Some(words) => {
            let mut last = f64::MIN;
            for (i, w) in words.iter().enumerate() {
                let text_ok = w
                    .get("text")
                    .and_then(Value::as_str)
                    .is_some_and(|t| !t.trim().is_empty());
                match (text_ok, timed(w)) {
                    (true, Some((a, b))) if a>=0.0 && a.is_finite() && b.is_finite() && b >= a && a >= last - 0.001 => last = a,
                    (true, Some(_)) => {
                        problems.push(format!("word {} is out of order or ends before it starts", i + 1))
                    }
                    _ => problems.push(format!("word {} needs text, start and end", i + 1)),
                }
                if problems.len() > 5 {
                    break;
                }
            }
        }
    }
    match doc.get("paragraphs").and_then(Value::as_array) {
        None => problems.push(
            "missing \"paragraphs\": [{text, start, end}] (the sung lines or stanzas)".into(),
        ),
        Some(lines) => {
            let mut previous=0.0;
            if lines.is_empty() || lines.iter().any(|p| match timed(p){Some((a,b)) if a.is_finite()&&b.is_finite()&&a>=previous&&a>=0.0&&b>=a&&p["text"].as_str().is_some_and(|t|!t.trim().is_empty())=>{previous=a;false},_=>true}){
                problems.push("paragraphs need ordered nonnegative start/end and nonempty text".into());
            }
        }
    }
    if let Some(duration)=doc["recording_duration_sec"].as_f64().filter(|v|v.is_finite()&&*v>=0.0){
        for key in ["words","paragraphs"]{
            for (i,v) in doc[key].as_array().into_iter().flatten().enumerate(){
                if v["end"].as_f64().is_some_and(|end|end>duration+0.001){problems.push(format!("{key} item {} ends after the recording ({duration:.6}s). Check matching audio/transcript; original received bytes retained.",i+1));}
            }
        }
    }
    problems
}

/// List every registered command, optionally filtered by id/label/params substring.
pub fn cmd_query_list(_s: &mut Session, params: &Value) -> Result<Value> {
    let f = params
        .get("filter")
        .or_else(|| params.get("query"))
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let list: Vec<Value> = crate::commands::command_specs()
        .iter()
        .filter(|c| {
            f.is_empty()
                || c.id.to_ascii_lowercase().contains(&f)
                || c.label.to_ascii_lowercase().contains(&f)
                || c.params.to_ascii_lowercase().contains(&f)
        })
        .map(|c| c.describe())
        .collect();
    Ok(json!({"count": list.len(), "commands": list}))
}

/// Unified CLI search across commands, connected subjects, workshop files, and storyboard scenes.
pub fn cmd_project_search(s: &mut Session, params: &Value) -> Result<Value> {
    let q = params
        .get("query")
        .or_else(|| params.get("filter"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let q_lower = q.to_ascii_lowercase();

    let commands: Vec<Value> = crate::commands::command_specs()
        .iter()
        .filter(|c| {
            q_lower.is_empty()
                || c.id.to_ascii_lowercase().contains(&q_lower)
                || c.label.to_ascii_lowercase().contains(&q_lower)
                || c.params.to_ascii_lowercase().contains(&q_lower)
        })
        .map(|c| json!({"id": c.id, "label": c.label, "params": c.params}))
        .collect();

    let subjects: Vec<Value> = s
        .project_assets
        .iter()
        .filter(|a| {
            q_lower.is_empty()
                || a.id.to_ascii_lowercase().contains(&q_lower)
                || a.name.to_ascii_lowercase().contains(&q_lower)
                || a.path
                    .as_ref()
                    .is_some_and(|p| p.display().to_string().to_ascii_lowercase().contains(&q_lower))
        })
        .map(|a| {
            let parts: Vec<String> = s
                .assets
                .get(&a.id)
                .map(|loaded| loaded.parts.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default();
            json!({
                "id": a.id,
                "name": a.name,
                "kind": a.kind,
                "path": a.path,
                "parts": parts,
            })
        })
        .collect();

    let files = cmd_workshop_files(s, &json!({"filter": q}))
        .ok()
        .and_then(|v| v.get("files").cloned())
        .unwrap_or_else(|| json!([]));

    let storyboard_hits = if !q.is_empty() {
        crate::storyboard_crud::search(s, q)
            .ok()
            .and_then(|v| v.get("hits").cloned())
            .unwrap_or_else(|| json!([]))
    } else {
        cmd_storyboard_toc(s, &json!({}))
            .ok()
            .and_then(|v| v.get("chapters").cloned())
            .unwrap_or_else(|| json!([]))
    };

    Ok(json!({
        "ok": true,
        "query": q,
        "project": s.project_path,
        "active_asset": s.active_asset,
        "subjects": subjects,
        "files": files,
        "storyboard": storyboard_hits,
        "commands": commands,
    }))
}

// ━━━ Workshop commands ━━━

pub fn cmd_workshop_status(s: &mut Session, _: &Value) -> Result<Value> {
    workshop_status(s)
}

pub fn cmd_workshop_set_audio(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing audio 'path'".into()))?;
    let audio = song_audio_path(s, raw)?;
    let exists=ensure_video_target_project(s, &dir, "hyperframes").is_ok();
    let connected=s.song_audio.clone().filter(|path|path.is_file()).or_else(||audio.file_name().map(|name|dir.join("hyperframes").join(name)).filter(|path|path.is_file()));
    let unchanged=connected.as_ref().is_some_and(|old|old==&audio || std::fs::read(old).ok().zip(std::fs::read(&audio).ok()).is_some_and(|(old,new)|old==new));
    if exists && !unchanged {
        return Err(EngineError::InvalidParams("This HyperFrames project already has a timeline. Update its audio in HyperFrames before relinking the recording; the existing project was preserved".into()));
    }
    let previous = (s.song_audio.clone(), s.video_target.clone(), s.video_project_path.clone());
    s.song_audio = Some(if exists && unchanged { connected.unwrap() } else { audio });
    s.video_target = "hyperframes".into();
    let setup = (|| {
        let video = crate::video_target::cmd_video_init(s, &json!({"target":"hyperframes","captions":false,"hyperframes_cli":params["hyperframes_cli"]}))?;
        let preview = crate::video_target::cmd_video_server_start(s, &json!({"hyperframes_cli":params["hyperframes_cli"]}))?;
        crate::project::write_project_file(s)?;
        Ok(json!({"ok":true,"video":video,"preview":preview}))
    })();
    if setup.is_err() {
        (s.song_audio, s.video_target, s.video_project_path) = previous;
        crate::project::write_project_file(s)?;
    }
    setup
}

pub fn cmd_workshop_set_target(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = params.get("target").and_then(Value::as_str).unwrap_or_default();
    if !VIDEO_TARGETS.contains(&target) {
        return Err(EngineError::InvalidParams(format!(
            "target must be one of {VIDEO_TARGETS:?}"
        )));
    }
    s.video_target = target.into();
    // The new target's project replaces the old one; failing to scaffold it leaves none, never the previous target's.
    s.video_project_path = ensure_video_target_project(s, &dir, target).ok();
    crate::project::write_project_file(s)?;
    workshop_status(s)
}

/// Copies liner notes, storyboard panels or static assets into the template folders.
pub fn cmd_workshop_add(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let (folder, only) = match params.get("kind").and_then(Value::as_str) {
        Some("liner_notes") => ("liner-notes", None),
        Some("panels") => (STORYBOARD_DIR, Some(IMAGE_EXTENSIONS)),
        Some("assets") => (ASSETS_DIR, Some(IMAGE_EXTENSIONS)),
        _ => {
            return Err(EngineError::InvalidParams(
                "kind must be liner_notes, panels or assets".into(),
            ))
        }
    };
    let paths: Vec<PathBuf> = params
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .collect();
    if paths.is_empty() {
        return Err(EngineError::InvalidParams("pass 'paths': [files]".into()));
    }
    let target = dir.join(folder);
    std::fs::create_dir_all(&target)?;
    let mut next_panel = panel_files(&dir).len() + 1;
    let mut added = Vec::new();
    for path in &paths {
        if !path.is_file() {
            return Err(EngineError::InvalidParams(format!(
                "file not found: {}",
                path.display()
            )));
        }
        if let Some(exts) = only
            && !has_extension(path, exts)
        {
            return Err(EngineError::InvalidParams(format!(
                "not an image: {}",
                path.display()
            )));
        }
        let dest = if folder == STORYBOARD_DIR {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let name = format!("panel-{next_panel:02}.{ext}");
            next_panel += 1;
            target.join(name)
        } else {
            target.join(
                path.file_name()
                    .ok_or_else(|| EngineError::InvalidParams("bad file name".into()))?,
            )
        };
        if dest != *path {
            if dest.is_file() && std::fs::read(&dest)?!=std::fs::read(path)? {
                return Err(EngineError::InvalidParams(format!("Material destination exists with different bytes: {}. Rename or explicitly resolve the collision; original retained.",dest.display())));
            }
            if !dest.exists(){std::fs::copy(path, &dest)?;}
        }
        added.push(dest);
    }
    let animate = params
        .get("animate")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || params.get("subject").and_then(Value::as_str).is_some();
    let mut connected_subjects = Vec::new();
    if folder == ASSETS_DIR && animate {
        let explicit_name = params.get("subject").and_then(Value::as_str);
        for dest in &added {
            let mut add_params = json!({
                "path": dest,
                "kind": "auto",
                "if_not_exists": true,
                "load": true,
            });
            if let Some(name) = explicit_name {
                add_params["name"] = json!(name);
            }
            if let Ok(res) = cmd_project_add_asset(s, &add_params)
                && let Some(id) = res.get("asset_id").and_then(Value::as_str)
            {
                connected_subjects.push(id.to_string());
            }
        }
        crate::project::write_project_file(s)?;
    }
    let mut status = workshop_status(s)?;
    status["added"] = json!(added);
    if !connected_subjects.is_empty() {
        status["connected_subjects"] = json!(connected_subjects);
    }
    Ok(status)
}

/// Cuts a storyboard grid into storyboard/panel-NN.png with PhotoCraft.
pub fn cmd_workshop_split_panels(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let image = params
        .get("image")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .ok_or_else(|| EngineError::InvalidParams("missing storyboard grid 'image'".into()))?;
    let columns = params.get("columns").and_then(Value::as_u64).unwrap_or(3).max(1) as u32;
    let rows = params.get("rows").and_then(Value::as_u64).unwrap_or(2).max(1) as u32;
    let inset = params.get("inset").and_then(Value::as_u64).unwrap_or(8) as u32;
    let (w, h) = image::image_dimensions(&image)?;
    let (cw, ch) = (w / columns, h / rows);
    if cw <= 2 * inset || ch <= 2 * inset {
        return Err(EngineError::InvalidParams(
            "panels too small for that grid and inset".into(),
        ));
    }
    let out_dir = dir.join(STORYBOARD_DIR);
    std::fs::create_dir_all(&out_dir)?;
    let mut panels = Vec::new();
    for r in 0..rows {
        for c in 0..columns {
            let n = r * columns + c + 1;
            let out = out_dir.join(format!("panel-{n:02}.png"));
            let rect = json!({"x": c * cw + inset, "y": r * ch + inset, "width": cw - 2 * inset, "height": ch - 2 * inset});
            run_tool_in(
                &dir,
                "photocraft-cli",
                &[
                    "run".into(),
                    image.to_string_lossy().into_owned(),
                    "--cmd".into(),
                    "image.crop".into(),
                    "--params".into(),
                    rect.to_string(),
                    "--out".into(),
                    out.to_string_lossy().into_owned(),
                ],
            )?;
            panels.push(out);
        }
    }
    let mut prompts_written = Vec::new();
    if let Some(prompt_file) = params
        .get("prompts")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
    {
        let numbered = numbered_prompts(&std::fs::read_to_string(&prompt_file)?);
        let source = prompt_file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        for (i, panel) in panels.iter().enumerate() {
            if let Some(text) = numbered.get(&(i + 1)) {
                let out = panel.with_extension("prompt.md");
                std::fs::write(
                    &out,
                    format!(
                        "{text}\n\nSource: {source} (panel {} of {})\n",
                        i + 1,
                        image.file_name().and_then(|n| n.to_str()).unwrap_or_default()
                    ),
                )?;
                prompts_written.push(out);
            }
        }
    }
    let mut status = workshop_status(s)?;
    status["panels"] = json!(panels);
    status["prompts"] = json!(prompts_written);
    Ok(status)
}

/// Preserve externally authored cue boundaries; never invent word timing from subtitle lines.
fn import_caption_paragraphs(text: &str) -> Result<Vec<Value>> {
    let time = |raw: &str| -> Option<f64> {
        let value = raw.trim().replace(',', ".");
        let p = value.split(':').map(str::parse::<f64>).collect::<std::result::Result<Vec<_>, _>>().ok()?;
        if p.len()!=3 || p.iter().any(|n|!n.is_finite() || *n<0.0) || p[1]>=60.0 || p[2]>=60.0 { return None; }
        Some(p[0]*3600.0+p[1]*60.0+p[2])
    };
    let mut cues=Vec::new(); let mut previous=0.0;
    for block in text.trim_start_matches('\u{feff}').replace("\r\n","\n").split("\n\n").filter(|b|!b.trim().is_empty()) {
        let lines=block.lines().collect::<Vec<_>>();
        let cue=lines.iter().position(|l|l.contains("-->"));
        let parsed=cue.and_then(|i| { let (a,b)=lines[i].split_once("-->")?; Some((time(a)?,time(b)?,lines[i+1..].join(" "))) });
        let Some((start,end,text))=parsed else {return Err(EngineError::InvalidParams("invalid SRT cue timing".into()));};
        if start<previous || end<start || text.trim().is_empty() {return Err(EngineError::InvalidParams("SRT cues must be ordered, timed and nonempty".into()));}
        previous=start; cues.push(json!({"text":text,"start":start,"end":end}));
    }
    if cues.is_empty() {return Err(EngineError::InvalidParams("caption file is empty".into()));}
    Ok(cues)
}

/// Supplies a transcript made earlier instead of transcribing again.
pub fn cmd_workshop_import_transcript(s: &mut Session, params:&Value)->Result<Value>{
 let dir=project_dir(s)?;let mut adjusted=params.clone();
 let mut received=std::fs::read(dir.join("intake-state.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).and_then(|v|v["received"].as_array().cloned()).unwrap_or_default();
 for (key,original) in [("path","original_source"),("captions","original_captions"),("audiomap","original_audiomap")] {
  if let Some(raw)=params[key].as_str(){
   let source=resolve_workspace_path(s,raw);
   let record=crate::command_center::receive(&dir,&source)?;
   adjusted[original]=json!(source);adjusted[key]=record["path"].clone();if !received.iter().any(|v|v["path"]==record["path"]){received.push(record);}
  }
 }
 let result=import_transcript_inner(s,&adjusted);
 let errors=match &result{Ok(_)=>vec![],Err(e)=>vec![e.to_string()]};
 crate::command_center::write_state(&dir,&json!({"received":received,"problems":errors,"valid":result.is_ok(),"command":"workshop.import_transcript","manual":"magic-cli commands workshop.import_transcript; crates/engine/src/workshop.rs"}))?;
 result
}

fn import_transcript_inner(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let source = params
        .get("path")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .ok_or_else(|| EngineError::InvalidParams("missing transcript 'path'".into()))?;
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&source)?)
        .map_err(|e| EngineError::InvalidParams(format!("not a JSON transcript: {e}")))?;
    let captions = params["captions"].as_str().map(|p| resolve_workspace_path(s,p));
    let mut caption_text = captions.as_ref().map(std::fs::read_to_string).transpose()?;
    if doc.is_array() { doc=json!({"words":doc}); }
    if let Some(text)=&caption_text {
        doc["paragraphs"]=json!(import_caption_paragraphs(text)?);
    }

    let mut grouping="provided paragraphs";
    if doc.get("paragraphs").and_then(Value::as_array).is_none() {
        if let Some(words)=doc["words"].as_array(){
            doc["paragraphs"]=json!(words.iter().map(|w|json!({"text":w["text"],"start":w["start"],"end":w["end"]})).collect::<Vec<_>>());
            grouping="provisional word fallback; original word timestamps";
        }
    }
    if caption_text.is_none(){
        let stamp=|v:f64|{let ms=(v*1000.0).round() as u64;format!("{:02}:{:02}:{:02},{:03}",ms/3600000,(ms/60000)%60,(ms/1000)%60,ms%1000)};
        caption_text=doc["paragraphs"].as_array().map(|ps|ps.iter().enumerate().map(|(i,p)|format!("{}\n{} --> {}\n{}\n\n",i+1,stamp(p["start"].as_f64().unwrap_or(0.0)),stamp(p["end"].as_f64().unwrap_or(0.0)),p["text"].as_str().unwrap_or_default())).collect::<String>());
    }
    doc["grouping"]=json!(grouping);

    let mut problems = transcript_problems(&doc);
    if let (Some(recorded), Some(song)) = (doc.get("audio").and_then(Value::as_str), &s.song_audio)
        && Path::new(recorded) != song.as_path()
    {
        problems.push(format!(
            "transcript is for {recorded}, but this project's song is {}",
            song.display()
        ));
    }
    let mapped_duration=params["audiomap"].as_str().map(|v|resolve_workspace_path(s,v)).and_then(|p|std::fs::read(p).ok()).and_then(|b|serde_json::from_slice::<Value>(&b).ok()).and_then(|v|v["audio"]["duration_sec"].as_f64()).filter(|v|v.is_finite()&&*v>=0.0);
    let probe_output=s.song_audio.as_ref().map(|p|std::process::Command::new("ffprobe").args(["-v","error","-show_entries","format=duration","-of","default=noprint_wrappers=1:nokey=1"]).arg(p).output());
    let probed_duration=probe_output.as_ref().and_then(|r|r.as_ref().ok()).filter(|o|o.status.success()).and_then(|o|std::str::from_utf8(&o.stdout).ok()).and_then(|v|v.trim().parse::<f64>().ok()).filter(|v|v.is_finite()&&*v>=0.0);
    doc["recording_duration_sec"]=json!(probed_duration);
    problems.extend(transcript_problems(&doc).into_iter().filter(|e|e.contains("recording")));
    if let (Some(recording),Some(mapped))=(probed_duration,mapped_duration){
        if (recording-mapped).abs()>0.001{problems.push(format!("Audiomap duration {mapped:.6}s disagrees with recording {recording:.6}s. Supply the matching map or omit it; recording duration will not be extended."));}
    }
    // The recording is authoritative. A supplied map is explicitly unverified when no probe succeeded.
    let duration=probed_duration.or(mapped_duration);
    let duration_warning=if probed_duration.is_none(){Some("Recording duration is unknown: provide readable matching audio and a working ffprobe. Supplied map duration, if any, is unverified; transcript endpoints are not recording-length evidence.")}else{None};
    if !problems.is_empty() {
        return Err(EngineError::InvalidParams(format!(
            "transcript does not match: {}",
            problems.join("; ")
        )));
    }
    let stem = crate::workspace::sanitize_asset_id(
        s.project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|n| n.to_str())
            .unwrap_or("song"),
    );
    for old in transcript_files(&dir) {
        if old != dir.join(format!("{stem}.transcript.json")) && old != dir.join("hyperframes/transcript.json") {
            return Err(EngineError::InvalidParams(format!(
                "project already has a transcript: {} (remove it first)",
                old.display()
            )));
        }
    }
    let dest = dir.join("hyperframes/transcript.json");
    let serialized=serde_json::to_vec_pretty(&doc["words"]).map_err(|e|EngineError::InvalidParams(e.to_string()))?;
    if dest.is_file() && !params["replace"].as_bool().unwrap_or(false) {
        let old=transcript_document(&dest)?;
        if old["words"]!=doc["words"] || old["paragraphs"]!=doc["paragraphs"] {
            return Err(EngineError::InvalidParams("Transcript differs from existing authored data. Inspect received files; pass replace=true only to replace explicitly.".into()));
        }
    }
    std::fs::create_dir_all(dest.parent().unwrap())?;
    std::fs::write(&dest, serialized)?;
    let mut copied = vec![dest];
    if let Some(text)=caption_text {
        let folder=dir.join(&s.video_target);
        std::fs::create_dir_all(&folder)?;
        let path=folder.join("transcript.srt");
        std::fs::write(&path,text)?;
        copied.push(path);
    }
    if let Some(fcproj) = params
        .get("filmcraft_project")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
    {
        if !fcproj.is_file() {
            return Err(EngineError::InvalidParams(format!(
                "FilmCraft project not found: {}",
                fcproj.display()
            )));
        }
        let to = dir.join(format!("{stem}.fcproj"));
        std::fs::copy(&fcproj, &to)?;
        copied.push(to);
    }
    let mut provenance=json!({"mode":"reused existing transcript; no ASR or transcoding","source":params["original_source"],"captions_source":params["original_captions"],"duration_sec":duration,"duration_source":if probed_duration.is_some(){"authoritative recording probe"}else{"unverified supplied audiomap or unknown"},"duration_warning":duration_warning, "recording_duration_sec":probed_duration,"audiomap_duration_sec":mapped_duration,"provider":"unknown; not recorded by source","model":"unknown; not recorded by source","manual":"~/.agents/skills/media-use/audio/references/transcribe.md","executable":std::env::current_exe().ok(),"version":env!("CARGO_PKG_VERSION"),"tool":"workshop.import_transcript (native Rust; ASR not called)","grouping":grouping});
    {
        use sha2::Digest;
        let raw_hash=std::fs::read(&source).ok().map(|bytes|format!("{:x}",sha2::Sha256::digest(bytes)));
        let prior=std::fs::read(dir.join("transcript-provenance.json")).ok().and_then(|bytes|serde_json::from_slice::<Value>(&bytes).ok());
        if let Some(prior)=prior.filter(|v|raw_hash.is_some()&&v["producer"]["transcript_sha256"]==json!(raw_hash)) {
            provenance["producer"]=prior["producer"].clone();
        }
    }
    std::fs::write(dir.join("transcript-provenance.json"),serde_json::to_vec_pretty(&provenance).map_err(|e|EngineError::InvalidParams(e.to_string()))?)?;
    let captions = if ensure_video_target_project(s, &dir, &s.video_target).is_ok() {
        crate::video_target::ensure_native_captions(s, &dir)?
    } else { json!({"ready":false,"reason":"Create the contained HyperFrames project to display these timed lyrics"}) };
    let mut status = workshop_status(s)?;
    status["captions"] = captions;
    status["imported"] = json!(copied);
    status["recording_duration_sec"]=json!(probed_duration);
    status["duration_warning"]=json!(duration_warning);
    Ok(status)
}

/// Approve or unapprove the workshop draft.
pub fn cmd_workshop_approve_draft(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let approved = params.get("approved").and_then(Value::as_bool).unwrap_or(true);
    if approved && !dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF).is_file() {
        return Err(EngineError::InvalidParams(
            "build the storyboard first (workshop.build_storyboard)".into(),
        ));
    }
    s.draft_approved = approved;
    crate::project::write_project_file(s)?;
    workshop_status(s)
}

/// Every file the workshop knows about, grouped by stage.
pub fn cmd_workshop_files(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let mut files = Vec::new();
    if let Some(audio) = &s.song_audio {
        files.push(file_entry("audio", audio, true));
    }
    for f in files_in(&dir.join("liner-notes"), None) {
        files.push(file_entry("liner_notes", &f, true));
    }
    for t in &transcript_files(&dir) {
        files.push(file_entry("transcript", t, true));
    }
    if let Some(fcproj) = video_project_path(&dir) {
        files.push(file_entry("transcript", &fcproj, true));
    }
    for f in files_in(&dir.join(STORYBOARD_DIR), None) {
        files.push(file_entry("storyboard", &f, true));
    }
    for f in workshop_asset_files(&dir) {
        files.push(file_entry("assets", &f, true));
    }
    let listed: Vec<PathBuf> = files
        .iter()
        .filter_map(|f| f["path"].as_str().map(PathBuf::from))
        .collect();
    for f in files_in(&dir, None) {
        if !listed.contains(&f) && Some(&f) != s.project_path.as_ref() {
            files.push(file_entry("other", &f, true));
        }
    }
    if let Some(group_filter) = params
        .get("group")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        && !group_filter.is_empty()
    {
        files.retain(|f| {
            f["group"]
                .as_str()
                .is_some_and(|g| g.to_ascii_lowercase() == group_filter)
        });
    }
    if let Some(q) = params
        .get("filter")
        .or_else(|| params.get("query"))
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        && !q.is_empty()
    {
        files.retain(|f| {
            f["name"]
                .as_str()
                .is_some_and(|n| n.to_ascii_lowercase().contains(&q))
                || f["path"]
                    .as_str()
                    .is_some_and(|p| p.to_ascii_lowercase().contains(&q))
                || f["group"]
                    .as_str()
                    .is_some_and(|g| g.to_ascii_lowercase().contains(&q))
        });
    }
    Ok(json!({"ok": true, "project_dir": dir, "count": files.len(), "files": files}))
}

/// Removes a workshop file: song audio is only unlinked; project-folder files go to trash.
pub fn cmd_workshop_remove(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing 'path'".into()))?;
    let path = resolve_workspace_path(s, raw);
    if s.song_audio.as_ref() == Some(&path) {
        s.song_audio = None;
        crate::project::write_project_file(s)?;
        let mut status = workshop_status(s)?;
        status["unlinked"] = json!(path);
        return Ok(status);
    }
    let inside = path
        .canonicalize()
        .ok()
        .zip(dir.canonicalize().ok())
        .is_some_and(|(p, d)| p.starts_with(&d));
    if !inside || Some(&path) == s.project_path.as_ref() || !path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "only files inside the project folder can be removed: {}",
            path.display()
        )));
    }
    run_tool_in(
        &dir,
        "gio",
        &["trash".into(), path.to_string_lossy().into_owned()],
    )?;
    if path == dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF) {
        s.draft_approved = false;
        crate::project::write_project_file(s)?;
    }
    let mut status = workshop_status(s)?;
    status["trashed"] = json!(path);
    Ok(status)
}

// ━━━ Asset fix list commands ━━━

fn fixlist_path(s: &Session) -> Result<PathBuf> {
    Ok(project_dir(s)?.join(FIXLIST))
}

fn read_fixlist(s: &Session) -> Result<Vec<Value>> {
    let path = fixlist_path(s)?;
    Ok(std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("assets").and_then(Value::as_array).cloned())
        .unwrap_or_default())
}

fn fixlist_next(items: &[Value]) -> Vec<String> {
    let clean = items
        .iter()
        .filter(|i| i["decision"] == json!("clean") && i["status"] != json!("done"))
        .count();
    let recreate = items
        .iter()
        .filter(|i| i["decision"] == json!("recreate") && i["status"] != json!("done"))
        .count();
    let mut next = Vec::new();
    if recreate > 0 {
        next.push(format!(
            "{recreate} asset(s) to recreate: run assets.request_sheet, then give the brief it writes to the image agent (one asset sheet, one pass)"
        ));
    }
    if clean > 0 {
        next.push(format!(
            "{clean} asset(s) to clean: run each item's recorded `tool` command, check the result at full size, then assets.fixlist_add asset=<id> status=done"
        ));
    }
    if next.is_empty() {
        next.push("Nothing open. Assess new or changed assets with assets.assess path=<png>".into());
    }
    next
}

/// Measures one image layer and suggests clean vs recreate with the evidence.
pub fn cmd_assets_assess(s: &mut Session, params: &Value) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .ok_or_else(|| EngineError::InvalidParams("missing 'path'".into()))?;
    let img = image::open(&path)?.to_rgba8();
    let (w, h) = img.dimensions();
    let alpha = |x: u32, y: u32| img.get_pixel(x, y)[3];
    let opaque = img.pixels().filter(|p| p[3] > 128).count() as f64;
    let partial = img.pixels().filter(|p| p[3] > 8 && p[3] < 247).count() as f64;
    let border: Vec<(u32, u32)> = (0..w)
        .flat_map(|x| [(x, 0), (x, h - 1)])
        .chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]))
        .collect();
    let touching = border.iter().filter(|(x, y)| alpha(*x, *y) > 128).count() as f64
        / border.len().max(1) as f64;
    let mut edge = 0usize;
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            if alpha(x, y) > 128
                && [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                    .iter()
                    .any(|(a, b)| alpha(*a, *b) <= 128)
            {
                edge += 1;
            }
        }
    }
    let raggedness = edge as f64 / opaque.max(1.0).sqrt();
    let mut lum: Vec<f64> = img
        .pixels()
        .filter(|p| p[3] > 128)
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect();
    lum.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = lum.get(lum.len() / 2).copied().unwrap_or(0.0);
    let (smear, smear_aspect) = smear_band(&img, median);
    let (measured_cuts, full_bleed) = straight_cuts(&img);
    // A joint part (an arm, a wing) ends flat on the side where it joins; only the sides named
    // in `joints` are excused. Any other straight cut (a clipped hand) is still missing artwork.
    let sides = ["left", "right", "top", "bottom"];
    let joints: Vec<&str> = match params.get("joints") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) => list.iter().filter_map(Value::as_str).collect(),
        Some(_) => return Err(EngineError::InvalidParams("joints is a list of sides: left, right, top, bottom".into())),
    };
    if let Some(bad) = joints.iter().find(|j| !sides.contains(j)) {
        return Err(EngineError::InvalidParams(format!("unknown joint side '{bad}' (left, right, top, bottom)")));
    }
    let mut cuts = measured_cuts;
    for (i, side) in sides.iter().enumerate() {
        if joints.contains(side) {
            cuts[i] = 0;
        }
    }
    let mut reasons = Vec::new();
    for (side, run) in sides.iter().zip(cuts) {
        if run > 0 {
            reasons.push(format!(
                "artwork cut off in a straight line along the {side} edge ({run} px): it was cropped through, not drawn to end there"
            ));
        }
    }
    // A full-bleed layer (a page, a background) fills its frame on purpose.
    let touching = if full_bleed { 0.0 } else { touching };
    if touching > 0.15 {
        reasons.push(format!(
            "{:.0}% of the outline runs into the box edge (clipped)",
            touching * 100.0
        ));
    }
    if raggedness > 9.0 {
        reasons.push(format!("ragged edge ({raggedness:.1} edge px per sqrt(area))"));
    }
    if smear >= 0.02 && smear_aspect >= 3.0 {
        reasons.push(format!(
            "{:.0}% of the asset is a flat dark band (smeared hidden overlap?)",
            smear * 100.0
        ));
    }
    if opaque == 0.0 {
        reasons.push("no opaque pixels".into());
    }
    // Missing artwork cannot be cleaned back: a cut means a new, complete drawing.
    let suggestion = if opaque == 0.0 || touching > 0.35 || cuts.iter().any(|&run| run > 0) {
        "recreate"
    } else if reasons.is_empty() {
        "ok"
    } else {
        "clean"
    };
    Ok(
        json!({"ok": true, "path": path, "size": [w, h], "suggestion": suggestion, "reasons": reasons,
        "measures": {"clipped_border": touching, "full_bleed": full_bleed, "straight_cut_px": {"left": measured_cuts[0], "right": measured_cuts[1], "top": measured_cuts[2], "bottom": measured_cuts[3]}, "raggedness": raggedness, "smear_band": smear, "smear_aspect": smear_aspect, "soft_edge_pixels": partial / opaque.max(1.0)},
        "next": [match suggestion {
            "recreate" => "Record it: assets.fixlist_add asset=<id> path=<png> decision=recreate problem=<what is wrong>".to_string(),
            "clean" => "Record it: assets.fixlist_add asset=<id> path=<png> decision=clean problem=<what> tool=<exact command that fixes it>".to_string(),
            _ => "Looks fine by measure; still check it at full size before using it.".to_string(),
        }]}),
    )
}

/// Straight cuts along each side of the artwork (left, right, top, bottom), in px; 0 = none.
///
/// Artwork cropped through ends in a straight run of solid pixels with nothing solid beyond
/// them, and the outline pulls away from that line past both ends of the run. A complete
/// drawing can also have a long, nearly straight edge (a wing, a chin, a hull), but its outline
/// keeps running along the line, so a run counts as a cut only when, within 7 px past its ends,
/// the outline recedes at least 3 px at one end and 2 px at the other (measured on cropped
/// cast layers and complete parts). Runs are looked for on the four lines just inside the bounds,
/// because a soft fringe can sit beyond a cut. A side lying on the canvas edge and solid for 90%
/// of the frame is the edge of a full-bleed layer (a page, a background), not a cut; such a layer
/// also continues past the frame on its other sides that touch the canvas, so those are not cuts
/// either. A long edge inside the canvas is never full-bleed evidence.
fn straight_cuts(img: &image::RgbaImage) -> ([u32; 4], bool) {
    let (w, h) = (i64::from(img.width()), i64::from(img.height()));
    let solid = |x: i64, y: i64| x >= 0 && y >= 0 && x < w && y < h && img.get_pixel(x as u32, y as u32)[3] > 128;
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, -1, -1);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 128 {
            let (x, y) = (i64::from(x), i64::from(y));
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    if x1 < 0 {
        return ([0; 4], false);
    }
    // Each side as (lines inward from the bounds, outward step, span along the side, is the
    // side vertical). Points are (across, along) and map back to (x, y) through `at`.
    let sides = [
        ((x0..=(x0 + 3).min(x1)).collect::<Vec<_>>(), -1, (y0, y1), true),
        (((x1 - 3).max(x0)..=x1).rev().collect(), 1, (y0, y1), true),
        ((y0..=(y0 + 3).min(y1)).collect(), -1, (x0, x1), false),
        (((y1 - 3).max(y0)..=y1).rev().collect(), 1, (x0, x1), false),
    ];
    let at = |vertical: bool, across: i64, along: i64| if vertical { (across, along) } else { (along, across) };
    // A full-bleed side has to be the frame itself: on the canvas edge, solid for 90% of it.
    let on_canvas_edge = [x0 == 0, x1 == w - 1, y0 == 0, y1 == h - 1];
    let mut runs = [0u32; 4];
    let mut bleed = [false; 4];
    for (i, (lines, out, (a0, a1), vertical)) in sides.iter().enumerate() {
        let length = (a1 - a0 + 1) as f64;
        // How far the outline sits inward of `line` at position `along` (None: nothing there).
        let recede = |line: i64, along: i64| {
            (0..=12).find(|d| {
                let (x, y) = at(*vertical, line - out * d, along);
                solid(x, y)
            })
        };
        for &line in lines {
            let mut start = None;
            for along in *a0..=a1 + 1 {
                let (x, y) = at(*vertical, line, along);
                let (ox, oy) = at(*vertical, line + out, along);
                let edge = along <= *a1 && solid(x, y) && !solid(ox, oy);
                match (edge, start) {
                    (true, None) => start = Some(along),
                    (false, Some(s)) => {
                        let run = (along - s) as u32;
                        let frame = if *vertical { h } else { w } as f64;
                        if on_canvas_edge[i] && line == lines[0] && f64::from(run) >= frame * 0.9 {
                            bleed[i] = true;
                        } else {
                            // Furthest the outline pulls away within 7 px past an end (None: gone).
                            let away = |end: i64, step: i64| {
                                (1..=7).map(|k| recede(line, end + step * k).unwrap_or(13)).max().unwrap_or(0)
                            };
                            let (before, after) = (away(s, -1), away(along - 1, 1));
                            let pulls_away = (before >= 3 && after >= 2) || (before >= 2 && after >= 3);
                            if run > 16 && f64::from(run) > length * 0.02 && pulls_away {
                                runs[i] = runs[i].max(run);
                            }
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
        }
    }
    let full_bleed = bleed.iter().any(|&b| b);
    if full_bleed {
        for i in 0..4 {
            if bleed[i] || on_canvas_edge[i] {
                runs[i] = 0;
            }
        }
    }
    (runs, full_bleed)
}

/// Largest flat dark band: (share of visible pixels, long/short side of its box).
/// A painted hidden-overlap strip is dark, blurred (5x5 brightness spread < 4) and elongated;
/// dark stripes keep the material's texture and pupils are round, so neither counts.
fn smear_band(img: &image::RgbaImage, median: f64) -> (f64, f64) {
    let (w, h) = img.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let lum: Vec<f64> = img
        .pixels()
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect();
    let visible: Vec<bool> = img.pixels().map(|p| p[3] > 16).collect();
    let spread = |x: usize, y: usize| {
        let (mut n, mut sum, mut sq) = (0.0, 0.0, 0.0);
        for yy in y.saturating_sub(2)..(y + 3).min(hu) {
            for xx in x.saturating_sub(2)..(x + 3).min(wu) {
                let l = lum[yy * wu + xx];
                n += 1.0;
                sum += l;
                sq += l * l;
            }
        }
        (sq / n - (sum / n) * (sum / n)).max(0.0).sqrt()
    };
    let mut flat: Vec<bool> = (0..wu * hu)
        .map(|i| visible[i] && lum[i] < median * 0.45 && spread(i % wu, i / wu) < 4.0)
        .collect();
    let (mut best, mut best_aspect) = (0usize, 0.0);
    for start in 0..flat.len() {
        if !flat[start] {
            continue;
        }
        flat[start] = false;
        let (mut stack, mut size) = (vec![start], 0usize);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % wu, i / wu);
            size += 1;
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            let mut push = |j: usize| {
                if flat[j] {
                    flat[j] = false;
                    stack.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < wu {
                push(i + 1);
            }
            if y > 0 {
                push(i - wu);
            }
            if y + 1 < hu {
                push(i + wu);
            }
        }
        if size > best {
            let (bw, bh) = ((x1 - x0 + 1) as f64, (y1 - y0 + 1) as f64);
            (best, best_aspect) = (size, bw.max(bh) / bw.min(bh));
        }
    }
    let total = visible.iter().filter(|v| **v).count().max(1);
    (best as f64 / total as f64, best_aspect)
}

pub fn cmd_assets_fixlist(s: &mut Session, _: &Value) -> Result<Value> {
    let items = read_fixlist(s)?;
    Ok(json!({"ok": true, "file": fixlist_path(s)?, "assets": items, "next": fixlist_next(&items)}))
}

/// Adds or updates one asset on the fix list (keyed by `asset`).
pub fn cmd_assets_fixlist_add(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = params
        .get("asset")
        .and_then(Value::as_str)
        .filter(|a| !a.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing 'asset' (a short id)".into()))?;
    let mut items = read_fixlist(s)?;
    let idx = items.iter().position(|i| i["asset"] == json!(asset));
    let mut item = idx
        .map(|i| items[i].clone())
        .unwrap_or_else(|| json!({"asset": asset, "status": "open"}));
    for key in ["path", "problem", "tool", "evidence", "reference", "status"] {
        if let Some(v) = params.get(key) {
            item[key] = v.clone();
        }
    }
    if let Some(d) = params.get("decision").and_then(Value::as_str) {
        if !["clean", "recreate"].contains(&d) {
            return Err(EngineError::InvalidParams(
                "decision must be clean or recreate".into(),
            ));
        }
        item["decision"] = json!(d);
    }
    if item.get("decision").is_none() {
        return Err(EngineError::InvalidParams(
            "new entries need decision=clean|recreate".into(),
        ));
    }
    if item["decision"] == json!("clean") && item.get("tool").is_none() {
        return Err(EngineError::InvalidParams(
            "decision=clean needs tool=<the exact command that fixes it>".into(),
        ));
    }
    match idx {
        Some(i) => items[i] = item,
        None => items.push(item),
    }
    let text = serde_json::to_string_pretty(&json!({"assets": items}))
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    std::fs::write(fixlist_path(s)?, text)?;
    cmd_assets_fixlist(s, &json!({}))
}

/// Writes ONE asset-sheet brief for the image agent covering every open "recreate" item.
pub fn cmd_assets_request_sheet(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let items: Vec<Value> = read_fixlist(s)?
        .into_iter()
        .filter(|i| i["decision"] == json!("recreate") && i["status"] != json!("done"))
        .collect();
    if items.is_empty() {
        return Err(EngineError::InvalidParams(
            "no open 'recreate' items on the fix list".into(),
        ));
    }
    let studio = studio(&dir);
    let mut brief = format!("# Asset sheet request — {} item(s), ONE pass\n\n", items.len());
    if let Some(intent) = studio.intent {
        brief.push_str(&format!("Studio intent:\n{intent}\n\n"));
    }
    brief.push_str("Make ONE asset sheet image containing every asset below, each separated on a plain background with space around it, so each can be cut out cleanly (Magic Studio cuts them with PhotoCraft). Match each asset's reference exactly in character, colour and material. Keep every part whole: nothing touching the sheet edge or another asset. No text on the sheet.\n\n## Assets\n");
    for (n, i) in items.iter().enumerate() {
        brief.push_str(&format!(
            "{}. **{}** — problem: {}. Current file: {}. Reference: {}\n",
            n + 1,
            i["asset"].as_str().unwrap_or_default(),
            i["problem"].as_str().unwrap_or("(not given)"),
            i["path"].as_str().unwrap_or("(none)"),
            i["reference"]
                .as_str()
                .unwrap_or("match the current file")
        ));
    }
    brief.push_str("\nSave the sheet as `assets/asset-sheet-<date>.png` with its exact prompt as `.prompt.md` beside it, then stop and report.\n");
    let out = dir.join("briefs");
    std::fs::create_dir_all(&out)?;
    let path = out.join("ASSET-SHEET-request.md");
    std::fs::write(&path, &brief)?;
    Ok(
        json!({"ok": true, "brief": path, "items": items.len(), "next": [
        format!("Send the image agent: follow {} exactly", path.display()),
        "When the sheet exists: cut each asset out (PhotoCraft via mc), check at full size, assets.fixlist_add asset=<id> status=done".to_string()]}),
    )
}

/// One row in the workshop file listing: group, name, path, size, exists, removable.
fn file_entry(group: &str, path: &Path, removable: bool) -> Value {
    let size = std::fs::metadata(path).map(|m| m.len()).ok();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    json!({"group": group, "name": name, "path": path, "size": size, "exists": path.exists(), "removable": removable})
}

// ━━━ Audio transcription ━━━

/// Use HyperFrames transcription and music analysis on the selected original recording.
pub fn cmd_audio_analyze(s: &mut Session, params: &Value) -> Result<Value> {
    if s.video_target != "hyperframes" {
        return Err(EngineError::InvalidParams("Magic Studio uses HyperFrames for song analysis".into()));
    }
    let audio = params["audio"].as_str().map(|p| resolve_workspace_path(s, p))
        .or_else(|| s.song_audio.clone()).filter(|p| p.is_file())
        .ok_or_else(|| EngineError::InvalidParams("no song audio: pass audio=<file> or workshop.set_audio".into()))?;
    let dir = project_dir(s)?;
    let hf = dir.join("hyperframes");
    let runtime = crate::video_target::hyperframes_cli(params["hyperframes_cli"].as_str(), Some(&hf))?;
    let runtime = runtime.to_str().ok_or_else(|| EngineError::InvalidParams("HyperFrames executable path must be UTF-8".into()))?;
    let overwrite = params["overwrite"].as_bool().unwrap_or(false);
    let model = params["model"].as_str().unwrap_or("large-v3");
    std::fs::create_dir_all(&hf)?;
    let transcript = hf.join("transcript.json");
    let srt = hf.join("transcript.srt");
    let arg = |p: &Path| p.to_string_lossy().into_owned();
    let mut trace = crate::audio_trace::AudioTrace::new(&dir, &audio, params)?;
    let mut ran = Vec::new();
    if trace.run("transcription", overwrite || !transcript.is_file(), &hf, runtime,
        &["transcribe".into(), arg(&audio), "-d".into(), arg(&hf), "--json".into(), "--model".into(), model.into()], &[transcript.clone()])?.is_some() {
        ran.push("hyperframes transcribe original recording → transcript.json");
    }
    if trace.run("caption_export", overwrite || !srt.is_file(), &hf, runtime,
        &["transcribe".into(), arg(&transcript), "--to".into(), "srt".into(), "-o".into(), arg(&srt)], &[srt.clone()])?.is_some() {
        ran.push("hyperframes transcribe --to srt → transcript.srt");
    }
    let words = trace.read_json("read_transcript", &transcript)?;
    let word_count = words.as_array().or_else(|| words["words"].as_array()).map_or(0, Vec::len);
    let mut analysis = Value::Null;
    if params["analyze_beats"] != false {
        let output = trace.run("music_analysis", true, &hf, runtime,
            &["audio".into(), arg(&audio), "--only".into(), "sections,onsets,moments".into(), "--json".into()], &[])?;
        analysis = serde_json::from_str(output.as_deref().unwrap_or_default())
            .map_err(|e| EngineError::InvalidParams(format!("HyperFrames audio returned invalid JSON: {e}")))?;
        if analysis["items"].as_array().is_none_or(Vec::is_empty) {
            return Err(EngineError::InvalidParams(format!("HyperFrames audio returned no recording analysis: {analysis}")));
        }
        std::fs::write(hf.join("audio-analysis.json"), serde_json::to_vec_pretty(&analysis).map_err(|e| EngineError::InvalidParams(e.to_string()))?)?;
        ran.push("hyperframes audio → audio-analysis.json");
    }
    trace.import(s, json!({"path":transcript,"captions":srt,"replace":overwrite}))?;
    Ok(json!({"ok":true,"target":"hyperframes","ran":ran,"analysis_attempt":trace.data,
        "provenance_receipt":dir.join("transcript-provenance.json"),"words":word_count,
        "files":{"audio":audio,"transcript":transcript,"captions":srt},"music_analysis":analysis,
        "next":["Review the words against the recording, then create scene pages from the selected storyboard template"]}))
}

#[cfg(test)]
mod tests {
    use super::smear_band;
    #[test]
    fn imported_captions_preserve_words_and_timing() {
        let cues=super::import_caption_paragraphs("1\r\n00:00:02,240 --> 00:00:04,160\r\nNelly go cross the ocean,\r\n").unwrap();
        assert_eq!(cues[0], serde_json::json!({"text":"Nelly go cross the ocean,","start":2.24,"end":4.16}));
        assert!(super::import_caption_paragraphs("1\n00:00:05,000 --> 00:00:02,000\nwrong").is_err());
        assert!(super::import_caption_paragraphs("1\n00:00:NaN --> 00:00:02,000\nwrong").is_err());
    }

    /// Light part with either a flat dark strip across the top (a painted overlap smear) or
    /// textured dark stripes (material colour).
    fn part(smeared: bool) -> image::RgbaImage {
        image::RgbaImage::from_fn(120, 60, |x, y| {
            let dark = if smeared { y < 8 } else { (x / 10) % 2 == 0 };
            let base = if dark { 30 } else { 200 };
            let grain = if smeared && dark { 0 } else { ((x * 7 + y * 13) % 5) as u8 * 6 };
            image::Rgba([base + grain, base + grain, base, 255])
        })
    }

    /// A disc of radius 100 in a 220px canvas, optionally sliced flat at x = `cut` with a
    /// soft fringe `fringe` px wide beyond the slice.
    fn disc(cut: Option<u32>, fringe: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(220, 220, |x, y| {
            let (dx, dy) = (x as f64 - 110.0, y as f64 - 110.0);
            let inside = dx * dx + dy * dy <= 100.0 * 100.0;
            let a = match cut {
                Some(c) if x > c + fringe => 0,
                Some(c) if x > c => 60,
                _ => 255,
            };
            image::Rgba([180, 160, 200, if inside { a } else { 0 }])
        })
    }

    #[test]
    fn a_whole_drawing_has_no_straight_cut() {
        assert_eq!(super::straight_cuts(&disc(None, 0)).0, [0, 0, 0, 0]);
    }

    #[test]
    fn artwork_cropped_through_is_a_straight_cut() {
        let cuts = super::straight_cuts(&disc(Some(170), 0)).0;
        assert!(cuts[1] > 100, "right cut {cuts:?}");
        assert_eq!([cuts[0], cuts[2], cuts[3]], [0, 0, 0]);
    }

    #[test]
    fn the_flat_stretch_of_a_long_curve_is_not_a_cut() {
        // A long, shallow ellipse (a wing): its top and bottom are nearly straight for a while.
        let wing = image::RgbaImage::from_fn(300, 120, |x, y| {
            let (dx, dy) = ((x as f64 - 150.0) / 140.0, (y as f64 - 60.0) / 40.0);
            image::Rgba([220, 200, 170, if dx * dx + dy * dy <= 1.0 { 255 } else { 0 }])
        });
        assert_eq!(super::straight_cuts(&wing).0, [0, 0, 0, 0]);
    }

    #[test]
    fn a_full_bleed_layer_is_not_cut() {
        // A ground band: full width along the bottom of the canvas, rolling hills on top.
        let band = image::RgbaImage::from_fn(400, 200, |x, y| {
            let top = 80.0 + 20.0 * (x as f64 / 40.0).sin();
            image::Rgba([90, 160, 80, if y as f64 >= top { 255 } else { 0 }])
        });
        assert_eq!(super::straight_cuts(&band), ([0, 0, 0, 0], true));
    }

    #[test]
    fn a_long_flat_edge_inside_the_canvas_is_not_full_bleed() {
        // Codex's review case: a D-shaped body with a flat right edge at x80 (not the canvas
        // edge, x99), curved left edge, and a protrusion cut off at the canvas's left edge.
        let d = image::RgbaImage::from_fn(100, 120, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let body = (20.0..=100.0).contains(&y) && x <= 80.0 && {
                let k = (y - 60.0) / 40.0;
                x >= 80.0 - 50.0 * (1.0 - k * k).max(0.0).sqrt()
            };
            let arm = (40.0..=70.0).contains(&y) && x <= 40.0;
            image::Rgba([200, 180, 160, if body || arm { 255 } else { 0 }])
        });
        let (cuts, bleed) = super::straight_cuts(&d);
        assert!(!bleed, "flat edge inside the canvas taken for a frame edge");
        assert!(cuts[0] > 16, "left cut lost: {cuts:?}");
    }

    /// An arm part drawn as a soft stroke (discs along a bent path, like a limb), cut flat at the
    /// shoulder (top) where it joins; the hand is a round paw, or sliced flat on the right.
    fn arm(clipped_hand: bool) -> std::path::PathBuf {
        let path: Vec<(f64, f64)> = (0..=40)
            .map(|i| {
                let t = f64::from(i) / 40.0;
                (70.0 + 130.0 * t * t, 10.0 + 110.0 * t)
            })
            .collect();
        let img = image::RgbaImage::from_fn(240, 170, |x, y| {
            let (x, y) = (f64::from(x), f64::from(y));
            let limb = path.iter().any(|(px, py)| (x - px).powi(2) + (y - py).powi(2) <= 22.0 * 22.0);
            let paw = (x - 200.0).powi(2) + (y - 125.0).powi(2) <= 30.0 * 30.0;
            let inside = (limb || paw) && y >= 12.0 && !(clipped_hand && x > 196.0);
            image::Rgba([150, 160, 210, if inside { 255 } else { 0 }])
        });
        let file = std::env::temp_dir().join(format!(
            "magicstudio_arm_{clipped_hand}_{}_{:?}.png",
            std::process::id(),
            std::thread::current().id()
        ));
        img.save(&file).unwrap();
        file
    }

    fn assess(path: &std::path::Path, joints: serde_json::Value) -> serde_json::Value {
        let mut s = crate::Session::default();
        super::cmd_assets_assess(&mut s, &serde_json::json!({"path": path, "joints": joints})).unwrap()
    }

    #[test]
    fn a_named_joint_excuses_only_its_own_side() {
        let whole = arm(false);
        let r = assess(&whole, serde_json::json!(["top"]));
        assert_eq!(r["suggestion"], "ok", "{r}");
        // Codex's review case: the shoulder joint is allowed, the clipped hand still blocks.
        let clipped = arm(true);
        let r = assess(&clipped, serde_json::json!(["top"]));
        assert_eq!(r["suggestion"], "recreate", "{r}");
        assert!(r["reasons"].to_string().contains("right edge"), "{r}");
        for p in [whole, clipped] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn a_cut_behind_a_soft_fringe_is_still_found() {
        assert!(super::straight_cuts(&disc(Some(170), 3)).0[1] > 100);
    }

    #[test]
    fn flat_elongated_dark_band_counts_as_smear() {
        let (share, aspect) = smear_band(&part(true), 200.0);
        assert!(share >= 0.02 && aspect >= 3.0, "share {share}, aspect {aspect}");
    }

    #[test]
    fn textured_dark_stripes_are_not_a_smear() {
        let (share, aspect) = smear_band(&part(false), 200.0);
        assert!(share < 0.02 || aspect < 3.0, "share {share}, aspect {aspect}");
    }
    #[test]
    fn recording_bounds_validate_every_word_and_paragraph_without_extending_duration() {
        let mut v=serde_json::json!({"recording_duration_sec":2.0,"words":[{"text":"a","start":0.0,"end":3.0},{"text":"b","start":1.0,"end":1.5}],"paragraphs":[{"text":"a b","start":0.0,"end":1.5}]});
        assert!(super::transcript_problems(&v).iter().any(|e|e.contains("recording")));
        v["words"][0]["end"]=serde_json::json!(0.5);v["paragraphs"][0]["end"]=serde_json::json!(3.0);
        assert!(super::transcript_problems(&v).iter().any(|e|e.contains("recording")));
        v["recording_duration_sec"]=serde_json::Value::Null;
        assert!(super::transcript_problems(&v).is_empty());
    }

}
