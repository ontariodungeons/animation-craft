//! Storyboard chapters, scene packets (`storyboard/scene-N.md`), PDF builder (`pdfcraft-cli`),
//! and OpenMontage backlot mirror.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};
use crate::video_target::target_project_path;
use crate::workshop::{
    ASSETS_DIR, IMAGE_EXTENSIONS, STORYBOARD_DIR, STORYBOARD_PDF,
    brief_header, files_in, panel_files, project_dir, song_seconds, stage_body, studio,
    transcript_files, workshop_status,
};
use crate::workspace::{has_extension, resolve_workspace_path, run_tool_in};

/// A scene's packet sections, in the order they print.
pub(crate) const SCENE_SECTIONS: &[(&str, &str)] = &[
    ("skills", "Skills"),
    ("reference_prompt", "Reference prompt"),
    ("animation_prompt", "Animation prompt"),
    ("layer_stack", "Layer stack"),
    ("teaching_cues", "Teaching cues"),
    ("visual_review", "Visual review expectations"),
    ("build_steps", "Build steps"),
    ("acceptance_checks", "Acceptance checks"),
    ("plan_vs_actual", "Plan vs actual"),
];
/// Valid scene statuses.
pub(crate) const SCENE_STATUSES: &[&str] = &["draft", "needs review", "approved"];

/// One sung phrase per line with its start time.
pub(crate) fn lyric_lines(transcript: &Value) -> Vec<(f64, String)> {
    let words: Vec<(f64, &str)> = transcript
        .get("words")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|w| Some((w.get("start")?.as_f64()?, w.get("text")?.as_str()?.trim())))
        .collect();
    let mut lines: Vec<(f64, String)> = Vec::new();
    let mut sentence_end = false;
    for (start, text) in words {
        let capital = text.chars().next().is_some_and(char::is_uppercase)
            && !matches!(text.trim_end_matches([',', '.', '!', '?']), "I" | "I'm" | "I'll" | "I've" | "I'd");
        match lines.last_mut() {
            Some((_, line)) if !(capital || sentence_end) => {
                line.push(' ');
                line.push_str(text);
            }
            _ => lines.push((start, text.to_string())),
        }
        sentence_end = text.ends_with(['.', '!', '?']);
    }
    lines
}

/// Scenes from the transcript: each sung stanza is a scene, gaps are instrumental scenes.
pub(crate) fn scenes(transcript: &Value, song_seconds: Option<f64>) -> Vec<Value> {
    let lines = lyric_lines(transcript);
    let stanzas: Vec<(f64, f64)> = transcript
        .get("paragraphs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p.get("start")?.as_f64()?, p.get("end")?.as_f64()?)))
        .collect();
    let mut out = Vec::new();
    let mut cursor = 0.0;
    let mut verse = 0;
    let instrumental = |from: f64, to: f64| json!({"kind": "instrumental", "start": from, "end": to, "lines": []});
    for (start, end) in &stanzas {
        if start - cursor >= 2.0 {
            out.push(instrumental(cursor, *start));
        }
        verse += 1;
        let sung: Vec<Value> = lines
            .iter()
            .filter(|(t, _)| *t >= start - 0.01 && *t < end - 0.01)
            .map(|(t, text)| json!({"start": t, "text": text}))
            .collect();
        out.push(json!({"kind": "verse", "verse": verse, "start": start, "end": end, "lines": sung}));
        cursor = *end;
    }
    if let Some(total) = song_seconds
        && total - cursor >= 2.0
    {
        out.push(instrumental(cursor, total));
    }
    out
}

/// Seconds of song with no caption that make an instrumental scene.
const CAPTION_GAP: f64 = 1.0;

/// The video target's own caption file (`<target>/transcript.srt`), when it has one.
pub(crate) fn caption_file(dir: &Path) -> Option<PathBuf> {
    ["hyperframes", "remotion", "filmcraft"].iter().map(|t| dir.join(t).join("transcript.srt")).find(|p| p.is_file())
}

/// Scenes from the video target's own captions (`<target>/transcript.srt`): one scene per
/// caption (its number, time and words), plus an instrumental scene for each stretch of
/// `CAPTION_GAP` seconds or more without one. Scenes tile the song. None without a caption file.
pub(crate) fn caption_scenes(dir: &Path) -> Option<Vec<Value>> {
    let srt = caption_file(dir)?;
    let text = std::fs::read_to_string(&srt).ok()?;
    let secs = |s: &str| -> Option<f64> {
        let (hms, ms) = s.trim().split_once([',', '.'])?;
        let mut parts = hms.split(':').map(|v| v.parse::<f64>().ok());
        let (h, m, sec) = (parts.next()??, parts.next()??, parts.next()??);
        Some(h * 3600.0 + m * 60.0 + sec + ms.parse::<f64>().ok()? / 1000.0)
    };
    let cues: Vec<(usize, f64, f64, String)> = text
        .replace("\r\n", "\n")
        .split("\n\n")
        .filter_map(|block| {
            let lines: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            let at = lines.iter().position(|l| l.contains("-->"))?;
            let (a, b) = lines[at].split_once("-->")?;
            let number = lines.first().and_then(|n| n.parse().ok()).unwrap_or(0);
            Some((number, secs(a)?, secs(b)?, lines[at + 1..].join(" ")))
        })
        .collect();
    let audiomap: Option<Value> = std::fs::read_to_string(srt.with_file_name("audiomap.json")).ok().and_then(|t| serde_json::from_str(&t).ok());
    let total = audiomap
        .and_then(|m| m["audio"]["duration_sec"].as_f64())
        .or_else(|| song_seconds(dir))
        .or_else(|| cues.last().map(|c| c.2))?;
    let instrumental = |from: f64, to: f64| json!({"kind": "instrumental", "start": from, "end": to, "lines": []});
    let mut out = Vec::new();
    let mut cursor = 0.0;
    for (i, (number, start, end, words)) in cues.iter().enumerate() {
        if start - cursor >= CAPTION_GAP {
            out.push(instrumental(cursor, *start));
            cursor = *start;
        }
        let next = cues.get(i + 1).map_or(total, |c| c.1);
        let until = if next - end >= CAPTION_GAP { *end } else { next };
        out.push(json!({"kind": "verse", "verse": number, "caption": number, "start": cursor, "end": until,
            "lines": [{"start": start, "text": words}]}));
        cursor = until;
    }
    if total - cursor >= CAPTION_GAP {
        out.push(instrumental(cursor, total));
    } else if let Some(last) = out.last_mut() {
        last["end"] = json!(total);
    }
    Some(out)
}

/// When each storyboard panel is on screen: panel N covers line N of every verse.
pub(crate) fn panel_spans(scene_list: &[Value], panels: usize) -> Vec<Vec<(f64, f64)>> {
    let mut spans = vec![Vec::new(); panels];
    for scene in scene_list.iter().filter(|sc| sc["kind"] == json!("verse")) {
        let lines = scene["lines"].as_array().cloned().unwrap_or_default();
        let end = scene["end"].as_f64().unwrap_or(0.0);
        for (j, line) in lines.iter().enumerate().take(panels) {
            let start = line["start"].as_f64().unwrap_or(0.0);
            let next = lines.get(j + 1).and_then(|l| l["start"].as_f64()).unwrap_or(end);
            spans[j].push((start, next));
        }
    }
    spans
}

/// The transcript's scenes and each panel's spans, or None before transcription.
pub(crate) fn storyboard_timing(dir: &Path, panels: usize) -> Option<(Vec<Value>, Vec<Vec<(f64, f64)>>)> {
    let scene_list = match caption_scenes(dir) {
        Some(list) => list,
        None => {
            let transcript = transcript_files(dir).into_iter().next()?;
            let doc: Value = serde_json::from_str(&std::fs::read_to_string(transcript).ok()?).ok()?;
            scenes(&doc, song_seconds(dir))
        }
    };
    let spans = panel_spans(&scene_list, panels);
    Some((scene_list, spans))
}

/// A scene's source document (`storyboard/scene-N.md`).
#[derive(Default, Clone)]
pub(crate) struct SceneDoc {
    pub title: Option<String>,
    pub status: String,
    pub time: Option<(f64, f64)>,
    pub reference: Option<String>,
    pub teaching_reference: Option<String>,
    pub notes: Vec<String>,
    pub lyrics: std::collections::BTreeMap<usize, String>,
    pub cues: std::collections::BTreeMap<usize, String>,
    pub sections: std::collections::BTreeMap<String, String>,
}

pub(crate) fn scene_doc_path(dir: &Path, n: usize) -> PathBuf {
    dir.join(STORYBOARD_DIR).join(format!("scene-{n}.md"))
}

pub(crate) fn read_scene_doc(dir: &Path, n: usize) -> SceneDoc {
    let mut doc = SceneDoc { status: "draft".into(), ..Default::default() };
    let Ok(text) = std::fs::read_to_string(scene_doc_path(dir, n)) else { return doc };
    let mut section = String::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("# ") {
            if doc.title.is_none() {
                doc.title = Some(h.trim().to_string());
            }
        } else if let Some(h) = t.strip_prefix("## ") {
            section = SCENE_SECTIONS.iter().find(|(_, label)| label.eq_ignore_ascii_case(h.trim())).map(|(k, _)| (*k).to_string())
                .unwrap_or_else(|| h.trim().to_lowercase());
        } else if section.is_empty() {
            if let Some(st) = t.strip_prefix("status:") {
                doc.status = st.trim().to_string();
            } else if let Some(r) = t.strip_prefix("reference:") {
                doc.reference = Some(r.trim().to_string()).filter(|r| !r.is_empty());
            } else if let Some(tr) = t.strip_prefix("teaching_reference:") {
                doc.teaching_reference = Some(tr.trim().to_string()).filter(|r| !r.is_empty());
            } else if let Some(tm) = t.strip_prefix("time:")
                && let Some((a, b)) = tm.trim().split_once(['-', '–'])
                && let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>())
            {
                doc.time = Some((a, b));
            }
        } else if section == "notes" {
            if let Some(note) = t.strip_prefix("- ") {
                doc.notes.push(note.to_string());
            }
        } else if section == "lyrics" {
            if let Some((n, words)) = t.split_once(':')
                && let Ok(n) = n.trim().parse::<usize>()
            {
                doc.lyrics.insert(n, words.trim().to_string());
            }
        } else if section == "cues" {
            if let Some((n, cue)) = t.split_once(':')
                && let Ok(n) = n.trim().parse::<usize>()
            {
                doc.cues.insert(n, cue.trim().to_string());
            }
        } else if !t.starts_with("<!--") {
            let entry = doc.sections.entry(section.clone()).or_default();
            if !entry.is_empty() {
                entry.push('\n');
            }
            entry.push_str(line);
        }
    }
    for v in doc.sections.values_mut() {
        *v = v.trim().to_string();
    }
    doc.sections.retain(|_, v| !v.is_empty());
    doc
}

fn write_scene_doc(dir: &Path, n: usize, label: &str, doc: &SceneDoc) -> Result<()> {
    let mut out = format!("# {}\nstatus: {}\n", doc.title.as_deref().unwrap_or(label), doc.status);
    if let Some((a, b)) = doc.time {
        out.push_str(&format!("time: {a}-{b}\n"));
    }
    if let Some(r) = &doc.reference {
        out.push_str(&format!("reference: {r}\n"));
    }
    if let Some(tr) = &doc.teaching_reference {
        out.push_str(&format!("teaching_reference: {tr}\n"));
    }
    out.push_str("\n## Notes\n");
    for note in &doc.notes {
        out.push_str(&format!("- {note}\n"));
    }
    out.push_str("\n## Lyrics\n<!-- corrections only: line number: words (the transcript keeps what was heard) -->\n");
    for (n, words) in &doc.lyrics {
        out.push_str(&format!("{n}: {words}\n"));
    }
    if !doc.cues.is_empty() {
        out.push_str("\n## Cues\n<!-- teaching / parent action cues per line: line number: instruction -->\n");
        for (n, cue) in &doc.cues {
            out.push_str(&format!("{n}: {cue}\n"));
        }
    }
    for (key, heading) in SCENE_SECTIONS {
        if let Some(text) = doc.sections.get(*key) {
            out.push_str(&format!("\n## {heading}\n{text}\n"));
        }
    }
    std::fs::create_dir_all(dir.join(STORYBOARD_DIR))?;
    std::fs::write(scene_doc_path(dir, n), out)?;
    Ok(())
}

/// The song's scenes: explicit intervals from scene files when every one has a `time:`,
/// otherwise transcript stanzas and gaps.
pub(crate) fn storyboard_scenes(dir: &Path) -> Result<Vec<Value>> {
    let docs: Vec<SceneDoc> = (1..).take_while(|n| scene_doc_path(dir, *n).is_file()).map(|n| read_scene_doc(dir, n)).collect();
    let explicit = !docs.is_empty() && docs.iter().all(|d| d.time.is_some());
    let captions = caption_scenes(dir);
    let doc: Value = match &captions {
        Some(_) if !explicit => Value::Null,
        _ => {
            let transcript = transcript_files(dir).into_iter().next().ok_or_else(|| {
                EngineError::InvalidParams("transcribe or import the transcript first: scenes come from it".into())
            })?;
            serde_json::from_str(&std::fs::read_to_string(transcript)?)
                .map_err(|e| EngineError::InvalidParams(format!("invalid transcript: {e}")))?
        }
    };
    let mut list = if let (Some(list), false) = (captions, explicit) {
        list
    } else if explicit {
        let lines = lyric_lines(&doc);
        docs.iter()
            .enumerate()
            .map(|(i, d)| {
                let (a, b) = d.time.unwrap_or_default();
                let sung: Vec<Value> = lines.iter().filter(|(t, _)| *t >= a - 0.01 && *t < b - 0.01).map(|(t, text)| json!({"start": t, "text": text})).collect();
                let kind = if sung.is_empty() { "instrumental" } else { "scene" };
                json!({"kind": kind, "verse": i + 1, "start": a, "end": b, "lines": sung})
            })
            .collect()
    } else {
        scenes(&doc, song_seconds(dir))
    };
    for (i, scene) in list.iter_mut().enumerate() {
        let sd = docs.get(i).cloned().unwrap_or_else(|| read_scene_doc(dir, i + 1));
        let start_sec = scene["start"].as_f64().unwrap_or(0.0);
        if let Some(lines) = scene["lines"].as_array_mut() {
            for (j, line) in lines.iter_mut().enumerate() {
                if let Some(words) = sd.lyrics.get(&(j + 1)) {
                    line["heard"] = line["text"].clone();
                    line["text"] = json!(words);
                }
                if let Some(cue) = sd.cues.get(&(j + 1)) {
                    line["cue"] = json!(cue);
                }
            }
            let max_idx = sd.lyrics.keys().chain(sd.cues.keys()).copied().max().unwrap_or(0);
            for k in (lines.len() + 1)..=max_idx {
                let mut entry = json!({
                    "start": start_sec,
                    "text": sd.lyrics.get(&k).cloned().unwrap_or_default()
                });
                if let Some(cue) = sd.cues.get(&k) {
                    entry["cue"] = json!(cue);
                }
                lines.push(entry);
            }
            if !lines.is_empty() {
                scene["kind"] = json!("scene");
            }
        }
        scene["status"] = json!(sd.status);
        scene["notes"] = json!(sd.notes);
        scene["title"] = json!(sd.title);
        scene["reference"] = json!(sd.reference.as_ref().map(|r| dir.join(r)));
        scene["teaching_reference"] = json!(sd.teaching_reference.as_ref().map(|r| dir.join(r)));
        scene["sections"] = json!(sd.sections);
    }
    Ok(list)
}

/// Images on the overview pages ("Cast & sets"): a 3 x 3 grid per page.
const PER_ASSET_PAGE: usize = 9;

/// The project's backgrounds, characters and props for the overview: the workshop's asset
/// files plus connected project images.
fn overview_images(s: &Session, dir: &Path) -> Vec<PathBuf> {
    let mut images = crate::workshop::workshop_asset_files(dir);
    for a in &s.project_assets {
        if let Some(p) = &a.path
            && p.is_file()
            && has_extension(p, IMAGE_EXTENSIONS)
            && !images.contains(p)
        {
            images.push(p.clone());
        }
    }
    images
}

/// "<character or background> · <file>" for an image under assets/characters/<name>/ or
/// assets/backgrounds/<name>/, else the file name.
fn overview_label(dir: &Path, img: &Path) -> String {
    let file = img.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let rel = img.strip_prefix(dir.join(ASSETS_DIR)).unwrap_or(img);
    let parts: Vec<&str> = rel.iter().filter_map(|c| c.to_str()).collect();
    match parts.as_slice() {
        ["characters" | "backgrounds", name, ..] if parts.len() > 2 => format!("{name} · {file}"),
        _ => file.to_string(),
    }
}

/// The PDF page of scene 1: after the lyrics page and the overview pages.
fn first_scene_page(s: &Session, dir: &Path) -> usize {
    2 + overview_images(s, dir).len().div_ceil(PER_ASSET_PAGE).max(1)
}

pub(crate) fn scene_label(i: usize, scene: &Value) -> String {
    let range = format!("{}–{}", clock(scene["start"].as_f64().unwrap_or(0.0)), clock(scene["end"].as_f64().unwrap_or(0.0)));
    if let Some(title) = scene["title"].as_str().filter(|t| !t.starts_with("Scene ")) {
        return format!("Scene {} · {title} · {range}", i + 1);
    }
    if scene.get("caption").is_some() {
        let words = scene["lines"][0]["text"].as_str().unwrap_or_default();
        return format!("Scene {} · {words} · {range}", i + 1);
    }
    match scene["kind"].as_str() {
        Some("verse") => format!("Scene {} · Verse {} · {range}", i + 1, scene["verse"]),
        _ => format!("Scene {} · Instrumental · {range}", i + 1),
    }
}

fn scene_number(params: &Value, count: usize) -> Result<usize> {
    let n = params.get("scene").and_then(Value::as_u64).map(|n| n as usize).ok_or_else(|| EngineError::InvalidParams("missing 'scene' (1-based, see storyboard.toc)".into()))?;
    if n == 0 || n > count {
        return Err(EngineError::InvalidParams(format!("scene must be 1..={count}")));
    }
    Ok(n)
}

pub(crate) fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Newest change among the files the storyboard is built from.
fn storyboard_inputs_changed(dir: &Path) -> Option<std::time::SystemTime> {
    let mut files: Vec<PathBuf> = files_in(&dir.join(STORYBOARD_DIR), None)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("panel-") || n.starts_with("scene-"))
        })
        .collect();
    files.extend(files_in(&dir.join(ASSETS_DIR), None));
    files.extend(files_in(&dir.join("assets/references"), None));
    files.extend(files_in(&dir.join("liner-notes"), None));
    files.extend(transcript_files(dir));
    files.push(dir.join("intent.md"));
    if let Some(parent) = dir.parent() {
        files.push(parent.join("STUDIO.md"));
    }
    files.iter().filter_map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).max()
}

/// Builds storyboard/storyboard.pdf with pdfcraft-cli.
pub fn cmd_workshop_build_storyboard(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let title = s.project_path.as_ref().and_then(|p| p.file_stem()).and_then(|n| n.to_str()).unwrap_or("Untitled").to_string();
    let transcript_path = match caption_file(&dir) {
        Some(srt) => srt,
        None => {
            let path = transcript_files(&dir).into_iter().next().ok_or_else(|| {
                EngineError::InvalidParams("transcribe or import the transcript first: the storyboard lines scenes up with it".into())
            })?;
            let _transcript: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)
                .map_err(|e| EngineError::InvalidParams(format!("invalid transcript: {e}")))?;
            path
        }
    };
    let total = song_seconds(&dir);
    let scene_list = storyboard_scenes(&dir)?;
    let panels = panel_files(&dir);
    let prompt_texts: Vec<Option<String>> = panels
        .iter()
        .map(|p| {
            std::fs::read_to_string(p.with_extension("prompt.md")).ok().map(|t| {
                t.lines().filter(|l| !l.trim_start().starts_with('#') && !l.starts_with("Source:")).collect::<Vec<_>>().join(" ")
                    .split_whitespace().collect::<Vec<_>>().join(" ")
            })
        })
        .collect();
    let shared_words = {
        let lists: Vec<Vec<&str>> = prompt_texts.iter().flatten().map(|t| t.split(' ').collect()).collect();
        if lists.len() < 2 { 0 } else {
            let first = &lists[0];
            (0..first.len()).take_while(|&i| lists.iter().all(|l| l.get(i) == first.get(i))).count()
        }
    };
    let prompt_of = |i: usize| -> Option<String> {
        let text = prompt_texts.get(i)?.as_ref()?;
        let words: Vec<&str> = text.split(' ').collect();
        let own = if shared_words > 8 { words[shared_words.min(words.len())..].join(" ") } else { text.clone() };
        let own = if own.trim().is_empty() { text.clone() } else { own };
        Some(if own.chars().count() > 240 { format!("{}…", own.chars().take(240).collect::<String>()) } else { own })
    };
    let theme = dir.join(STORYBOARD_DIR).join(".theme");
    std::fs::create_dir_all(&theme)?;
    let piece = |name: &str, bytes: &[u8]| -> Result<PathBuf> {
        let p = theme.join(format!("{name}.png"));
        std::fs::write(&p, bytes)?;
        Ok(p)
    };
    let clapper = piece("clapper", include_bytes!("../../../assets/storyboard-theme/clapper.png"))?;
    let card_cream = piece("card-cream", include_bytes!("../../../assets/storyboard-theme/card-cream.png"))?;
    let card_dark = piece("card-dark", include_bytes!("../../../assets/storyboard-theme/card-dark.png"))?;
    let filmstrip = piece("filmstrip", include_bytes!("../../../assets/storyboard-theme/filmstrip.png"))?;
    const CREAM: &str = "#f2e9d5";
    const AMBER: &str = "#f0a83c";
    const INK: &str = "#29231a";
    const INK_TIME: &str = "#b8522d";
    const MUTE: &str = "#9a9aa3";

    let asset_images = overview_images(s, &dir);
    let asset_pages = asset_images.len().div_ceil(PER_ASSET_PAGE).max(1);
    let first_scene_page = 2 + asset_pages;
    let pages = 1 + asset_pages + scene_list.len() + 1;
    let mut layout: Vec<Value> = Vec::new();

    // Portrait A4, to fill the tall storyboard pane.
    let (pw, ph, m) = (595.0_f64, 842.0_f64, 36.0_f64);
    let mut steps = vec![
        json!({"tool": "doc_create", "args": {"from": "blank", "pages": pages, "width": pw, "height": ph, "name": title}}),
        json!({"tool": "doc_background", "args": {"doc": 1, "color": "#0a0a0c"}}),
    ];
    let glyphs = |t: String, font: &str| if font == "courier" { t.replace(['—', '–'], "-") } else { t };
    let text = |page: usize, at: [f64; 2], right: f64, t: String, size: f64, font: &str, color: &str, bold: bool| {
        json!({"tool": "page_add_text", "args": {"doc": 1, "page": page, "rect": [at[0], at[1], right, at[1] + size * 1.7],
            "text": glyphs(t, font), "size": size, "font": font, "color": color, "bold": bold}})
    };
    let block = |page: usize, rect: [f64; 4], t: String, size: f64, font: &str, color: &str| {
        json!({"tool": "page_add_text", "args": {"doc": 1, "page": page, "rect": rect, "text": glyphs(t, font), "size": size, "font": font, "color": color}})
    };
    let image = |page: usize, path: &Path, rect: [f64; 4]| json!({"tool": "page_add_image", "args": {"doc": 1, "page": page, "path": path, "rect": rect}});
    let header = |page: usize, kicker: String, heading: String| {
        vec![
            image(page, &clapper, [0.0, 0.0, pw, 16.0]),
            text(page, [m, 26.0], pw - m, kicker, 9.0, "courier", AMBER, true),
            text(page, [m, 40.0], pw - m, heading, 20.0, "helvetica", CREAM, true),
        ]
    };
    let fit = |path: &Path, b: [f64; 4]| -> Result<[f64; 4]> {
        let (iw, ih) = image::image_dimensions(path)?;
        let scale = ((b[2] - b[0]) / f64::from(iw)).min((b[3] - b[1]) / f64::from(ih.max(1)));
        let (w, h) = (f64::from(iw) * scale, f64::from(ih) * scale);
        let (x, y) = (b[0] + (b[2] - b[0] - w) / 2.0, b[1] + (b[3] - b[1] - h) / 2.0);
        Ok([x, y, x + w, y + h])
    };

    let studio_info = studio(&dir);
    let kicker = format!("{} · STORYBOARD", studio_info.name.clone().unwrap_or_else(|| "WORKSHOP".into()).to_uppercase());
    steps.extend(header(1, kicker, title.clone()));
    if let Some(intent) = &studio_info.intent {
        let first_line = intent.lines().find(|l| !l.trim().is_empty()).unwrap_or_default();
        let short_intent = if first_line.chars().count() > 145 {
            format!("{}…", first_line.chars().take(145).collect::<String>())
        } else {
            first_line.to_string()
        };
        steps.push(block(1, [m, 70.0, pw - m, 92.0], short_intent, 8.5, "helvetica", MUTE));
    }
    let liner = files_in(&dir.join("liner-notes"), None);
    let facts = [
        format!("SONG        {}", s.song_audio.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "not set".into())),
        format!("TRANSCRIPT  {}", transcript_path.strip_prefix(&dir).unwrap_or(&transcript_path).display()),
        format!("LINER NOTES {}", if liner.is_empty() { "optional (none attached)".to_string() } else { liner.iter().filter_map(|p| p.file_name()?.to_str()).collect::<Vec<_>>().join(", ") }),
        format!("EDIT IN     {} ({})", s.video_target, target_project_path(s, &dir, &s.video_target).map(|p| p.display().to_string()).unwrap_or_else(|| "ready".into())),
        format!("STATUS      {}", if s.draft_approved { "APPROVED" } else { "DRAFT — NEEDS REVIEW" }),
    ];
    let grid_sheet = [
        dir.join("assets/references/storyboard-grid-v1.png"),
        dir.join("storyboard/storyboard-grid.png"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    let teach_sheet = [
        dir.join("assets/references/teaching-cues-v1.png"),
        dir.join("storyboard/teaching-cues.png"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    if let Some(ref g) = grid_sheet {
        let box_r = if teach_sheet.is_some() {
            [pw - m - 260.0, 96.0, pw - m - 134.0, 168.0]
        } else {
            [pw - m - 260.0, 96.0, pw - m, 168.0]
        };
        if let Ok(r) = fit(g, box_r) {
            steps.push(image(1, g, r));
            layout.push(json!({"page": 1, "path": g, "rect": r}));
            steps.push(text(1, [box_r[0], 170.0], box_r[2], "STORYBOARD 3x2".into(), 7.5, "courier", AMBER, true));
        }
    }
    if let Some(ref tc) = teach_sheet {
        let box_r = [pw - m - 126.0, 96.0, pw - m, 168.0];
        if let Ok(r) = fit(tc, box_r) {
            steps.push(image(1, tc, r));
            layout.push(json!({"page": 1, "path": tc, "rect": r}));
            steps.push(text(1, [box_r[0], 170.0], box_r[2], "TEACHING CUES 3x2".into(), 7.5, "courier", AMBER, true));
        }
    }
    // The song facts use the full width unless reference sheets sit at the right.
    let facts_right = if grid_sheet.is_some() || teach_sheet.is_some() { pw - m - 270.0 } else { pw - m };
    steps.push(block(1, [m, 100.0, facts_right, 172.0], facts.join("\n"), 8.5, "courier", CREAM));
    steps.push(text(1, [m, 184.0], pw - m, "SCENES · CLICK A SCENE ROW TO JUMP TO ITS PACKET".into(), 9.0, "courier", AMBER, true));
    let row_step = ((ph - 206.0) / scene_list.len().max(1) as f64).min(44.0);
    let card_h = (row_step - 5.0).max(26.0);
    for (i, scene) in scene_list.iter().enumerate() {
        let y = 202.0 + i as f64 * row_step;
        steps.push(image(1, &card_dark, [m, y, pw - m, y + card_h]));
        let reference = scene["reference"].as_str().map(PathBuf::from).filter(|p| p.is_file());
        let first_panel = reference.as_ref().or_else(|| scene["lines"].as_array().filter(|l| !l.is_empty()).and_then(|_| panels.first()));
        let x = m + 14.0 + 56.0;
        if let Some(p) = first_panel {
            let r = fit(p, [m + 14.0, y + 3.0, m + 14.0 + 50.0, y + card_h - 3.0])?;
            steps.push(image(1, p, r));
        }
        steps.push(text(1, [x, y + (card_h - 14.0) * 0.45], pw - m - 120.0, scene_label(i, scene), 11.5, "helvetica", CREAM, true));
        let lines = scene["lines"].as_array().map_or(0, Vec::len);
        let status = scene["status"].as_str().unwrap_or("draft");
        let note = if lines == 0 { format!("instrumental · {status}") } else { format!("{lines} lines · {status}") };
        steps.push(text(1, [pw - m - 110.0, y + (card_h - 12.0) * 0.5], pw - m - 8.0, note, 9.0, "courier", AMBER, false));
        steps.push(json!({"tool": "link_add", "args": {"doc": 1, "page": 1, "rect": [m, y, pw - m, y + card_h], "to_page": first_scene_page + i, "visible": false}}));
    }
    for (i, scene) in scene_list.iter().enumerate() {
        let page = first_scene_page + i;
        let range = format!("{} – {}", clock(scene["start"].as_f64().unwrap_or(0.0)), clock(scene["end"].as_f64().unwrap_or(0.0)));
        let heading = match (scene["title"].as_str().filter(|t| !t.starts_with("Scene ")), scene["kind"].as_str()) {
            (Some(title), _) => title.to_string(),
            (None, _) if scene.get("caption").is_some() => scene["lines"][0]["text"].as_str().unwrap_or_default().to_string(),
            (None, Some("verse")) => format!("Verse {}", scene["verse"]),
            _ => "Instrumental".into(),
        };
        let status = scene["status"].as_str().unwrap_or("draft").to_uppercase();
        steps.extend(header(page, format!("SCENE {} · {range} · {status}", i + 1), heading));
        let notes: Vec<String> = scene["notes"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|n| format!("• {n}")).collect();
        let has_reference = scene["reference"].as_str().is_some_and(|p| Path::new(p).is_file());
        if !notes.is_empty() && !has_reference {
            steps.push(block(page, [m, ph - 18.0 - 12.0 * notes.len().min(3) as f64, pw - m, ph - 6.0], format!("NOTES  {}", notes.join("   ")), 8.0, "courier", AMBER));
        }
        let lines = scene["lines"].as_array().cloned().unwrap_or_default();
        if let Some(reference) = scene["reference"].as_str().map(PathBuf::from).filter(|p| p.is_file()) {
            let teach_ref = scene["teaching_reference"].as_str().map(PathBuf::from).filter(|p| p.is_file());
            let images_bottom = if let Some(ref teach) = teach_ref {
                let story_box = [m, 74.0, m + 216.0, 74.0 + 212.0];
                let r1 = fit(&reference, [story_box[0] + 6.0, story_box[1] + 6.0, story_box[2] - 6.0, story_box[3] - 18.0])?;
                steps.push(image(page, &reference, r1));
                layout.push(json!({"page": page, "path": reference, "rect": r1}));
                steps.push(text(page, [story_box[0] + 6.0, story_box[3] - 14.0], story_box[2], "STORY PANEL".into(), 7.5, "courier", AMBER, true));
                let teach_box = [m + 222.0, 74.0, m + 412.0, 74.0 + 212.0];
                let r2 = fit(teach, [teach_box[0] + 6.0, teach_box[1] + 6.0, teach_box[2] - 6.0, teach_box[3] - 18.0])?;
                steps.push(image(page, teach, r2));
                layout.push(json!({"page": page, "path": teach, "rect": r2}));
                steps.push(text(page, [teach_box[0] + 6.0, teach_box[3] - 14.0], teach_box[2], "TEACHING CUE".into(), 7.5, "courier", AMBER, true));
                teach_box[3]
            } else {
                let frame = [m, 74.0, m + 360.0, 74.0 + 230.0];
                let window = [frame[0] + 18.0, frame[1] + 22.0, frame[2] - 18.0, frame[3] - 22.0];
                let r = fit(&reference, window)?;
                steps.push(image(page, &reference, r));
                layout.push(json!({"page": page, "path": reference, "rect": r}));
                frame[3]
            };
            let lyric_text = if lines.is_empty() {
                "Instrumental — no sung words.".to_string()
            } else {
                lines
                    .iter()
                    .map(|l| {
                        let base = format!("{}  {}", clock(l["start"].as_f64().unwrap_or(0.0)), l["text"].as_str().unwrap_or(""));
                        match l["cue"].as_str().filter(|c| !c.trim().is_empty()) {
                            Some(cue) => format!("{base}\n       ACTION -> {cue}"),
                            None => base,
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            // The lyric card sits under the images, full width.
            let card_top = images_bottom + 10.0;
            let card_bottom = card_top + 24.0 + 14.0 * lyric_text.lines().count() as f64;
            steps.push(image(page, &card_cream, [m, card_top, pw - m, card_bottom]));
            steps.push(block(page, [m + 16.0, card_top + 8.0, pw - m - 10.0, card_bottom - 6.0], lyric_text, 8.5, "courier", INK));
            let sections = scene["sections"].as_object().cloned().unwrap_or_default();
            let short = |t: &str| {
                let flat = t.split_whitespace().collect::<Vec<_>>().join(" ");
                if flat.chars().count() > 330 { format!("{}…", flat.chars().take(330).collect::<String>()) } else { flat }
            };
            let mut packet: Vec<String> = Vec::new();
            if !notes.is_empty() {
                packet.push(format!("NOTES: {}", short(&notes.join("  "))));
            }
            packet.extend(SCENE_SECTIONS.iter().filter_map(|(k, label)| {
                sections.get(*k).and_then(Value::as_str).map(|t| format!("{}: {}", label.to_uppercase(), short(t)))
            }));
            if !packet.is_empty() {
                packet.push(format!("Full packet: storyboard/scene-{}.md", i + 1));
                let packet_top = card_bottom + 10.0;
                steps.push(block(page, [m, packet_top, pw - m, ph - 16.0], packet.join("\n\n"), 7.0, "courier", CREAM));
            }
            continue;
        }
        if lines.is_empty() {
            steps.push(image(page, &card_cream, [m, 90.0, pw - m, 150.0]));
            steps.push(text(page, [m + 20.0, 104.0], pw - m, format!("INSTRUMENTAL · {range}"), 12.0, "courier", INK, true));
            steps.push(text(page, [m + 20.0, 124.0], pw - m, "No sung words. Keep the action moving with the music; do not invent lyrics.".into(), 9.0, "courier", INK, false));
            continue;
        }
        let top = 76.0;
        let row_h = ((ph - top - 20.0) / lines.len() as f64).min(84.0);
        for (j, line) in lines.iter().enumerate() {
            let y = top + j as f64 * row_h;
            let card = [m, y, pw - m, y + row_h - 6.0];
            steps.push(image(page, &card_cream, card));
            let time = clock(line["start"].as_f64().unwrap_or(0.0));
            let lyric = line["text"].as_str().unwrap_or("").to_string();
            let cue = line["cue"].as_str().filter(|c| !c.trim().is_empty());
            let mut x = m + 16.0;
            if let Some(panel) = panels.get(j) {
                let frame_h = row_h - 14.0;
                let frame = [x, y + 4.0, x + frame_h, y + 4.0 + frame_h];
                let window = [frame[0] + frame_h * 0.07, frame[1] + frame_h * 0.13, frame[2] - frame_h * 0.07, frame[3] - frame_h * 0.13];
                let r = fit(panel, window)?;
                steps.push(image(page, panel, r));
                steps.push(image(page, &filmstrip, frame));
                layout.push(json!({"page": page, "path": panel, "rect": r}));
                x = frame[2] + 14.0;
                let name = panel.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                let prompt = prompt_of(j).unwrap_or_else(|| "(no prompt recorded for this panel)".into());
                let detail = match cue {
                    Some(c) => format!("ACTION -> {c}\n{name} — {prompt}"),
                    None => format!("{name} — {prompt}"),
                };
                steps.push(block(page, [x, y + 28.0, pw - m - 12.0, y + row_h - 10.0], detail, 7.5, "courier", INK));
            } else {
                let detail = match cue {
                    Some(c) => format!("ACTION -> {c}"),
                    None => format!("(no panel {} yet)", j + 1),
                };
                steps.push(text(page, [x, y + 30.0], pw - m - 12.0, detail, 8.0, "courier", INK_TIME, cue.is_some()));
            }
            steps.push(text(page, [x, y + 8.0], x + 44.0, time, 11.0, "courier", INK_TIME, true));
            steps.push(text(page, [x + 48.0, y + 7.0], pw - m - 12.0, lyric, 13.0, "courier", INK, true));
        }
    }
    let first_asset_page = 2;
    for k in 0..asset_pages {
        let page = first_asset_page + k;
        let title = if asset_pages > 1 { format!("Cast & sets {}/{asset_pages}", k + 1) } else { "Cast & sets".into() };
        steps.extend(header(page, "OVERVIEW · BACKGROUNDS, CHARACTERS, PROPS".into(), title));
        let chunk: Vec<&PathBuf> = asset_images.iter().skip(k * PER_ASSET_PAGE).take(PER_ASSET_PAGE).collect();
        if chunk.is_empty() {
            steps.push(text(page, [m, 90.0], pw - m, "Nothing yet. Add backgrounds and characters in AnimationCraft; they appear here.".into(), 10.0, "courier", MUTE, false));
        }
        let (cols, cell_w, cell_h) = (3usize, (pw - 2.0 * m) / 3.0, 236.0);
        for (n, img) in chunk.iter().enumerate() {
            let (cx, cy) = (m + (n % cols) as f64 * cell_w, 84.0 + (n / cols) as f64 * cell_h);
            let side = (cell_w - 16.0).min(cell_h - 40.0);
            let frame = [cx + 8.0, cy, cx + 8.0 + side, cy + side];
            let window = [frame[0] + side * 0.07, frame[1] + side * 0.13, frame[2] - side * 0.07, frame[3] - side * 0.13];
            let r = fit(img, window)?;
            steps.push(image(page, img, r));
            steps.push(image(page, &filmstrip, frame));
            layout.push(json!({"page": page, "path": img, "rect": r}));
            steps.push(text(page, [cx + 8.0, cy + side + 6.0], cx + cell_w - 4.0, overview_label(&dir, img), 8.5, "courier", CREAM, false));
        }
    }
    steps.extend(header(pages, "FOR THE AGENT".into(), "Agent brief".into()));
    let brief = std::iter::once(brief_header(&dir))
        .chain(["song", "transcript", "reference", "animate"].iter().map(|st| stage_body(s, &dir, st)))
        .collect::<Vec<_>>()
        .join("\n");
    steps.push(block(pages, [m, 76.0, pw - m, ph - 20.0], brief, 7.5, "courier", CREAM));
    steps.push(json!({"tool": "bookmark_add", "args": {"doc": 1, "title": "Lyrics", "page": 1}}));
    steps.push(json!({"tool": "bookmark_add", "args": {"doc": 1, "title": "Cast & sets", "page": first_asset_page}}));
    for (i, scene) in scene_list.iter().enumerate() {
        steps.push(json!({"tool": "bookmark_add", "args": {"doc": 1, "title": scene_label(i, scene), "page": first_scene_page + i}}));
    }
    steps.push(json!({"tool": "bookmark_add", "args": {"doc": 1, "title": "Agent brief", "page": pages}}));
    let out_dir = dir.join(STORYBOARD_DIR);
    std::fs::create_dir_all(&out_dir)?;
    let pdf = out_dir.join(STORYBOARD_PDF);
    steps.push(json!({"tool": "doc_save", "args": {"doc": 1, "path": pdf}}));
    let script = dir.join(".storyboard.pdfcraft.json");
    std::fs::write(&script, Value::Array(steps).to_string())?;
    let run = run_tool_in(&dir, "pdfcraft-cli", &["run".into(), "--script".into(), script.to_string_lossy().into_owned(), "--report".into()]);
    let _ = std::fs::remove_file(&script);
    let report: Value = serde_json::from_str(&run?).map_err(|e| EngineError::ExternalTool("pdfcraft-cli".into(), e.to_string()))?;
    if report.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(EngineError::ExternalTool("pdfcraft-cli".into(), report.to_string()));
    }
    let previews = out_dir.join(".preview");
    std::fs::create_dir_all(&previews)?;
    for old in files_in(&previews, Some(&["png"])) {
        std::fs::remove_file(old)?;
    }
    let mut preview_files = Vec::new();
    for page in 1..=pages {
        let png = previews.join(format!("page-{page:02}.png"));
        run_tool_in(&dir, "pdfcraft-cli", &["render".into(), pdf.to_string_lossy().into_owned(), "--page".into(), page.to_string(), "--dpi".into(), "110".into(), "--out".into(), png.to_string_lossy().into_owned()])?;
        preview_files.push(png);
    }
    let titles: Vec<String> = scene_list.iter().enumerate().map(|(i, sc)| scene_label(i, sc)).collect();
    let full_prompts: Vec<Option<String>> = panels
        .iter()
        .map(|p| std::fs::read_to_string(p.with_extension("prompt.md")).ok().map(|t| t.trim().to_string()))
        .collect();
    write_backlot_mirror(&dir, &title, &scene_list, &panels, &full_prompts, &asset_images, total)?;
    std::fs::write(previews.join("layout.json"), json!({"page_size": [pw, ph], "images": layout, "scene_titles": titles, "first_scene_page": first_scene_page}).to_string())?;
    s.draft_approved = false;
    crate::project::write_project_file(s)?;
    let mut status = workshop_status(s)?;
    status["storyboard"] = json!(pdf);
    status["scenes"] = json!(scene_list);
    status["previews"] = json!(preview_files);
    Ok(status)
}

/// Current storyboard build state: PDF exists, previews, staleness.
pub fn cmd_workshop_storyboard(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let pdf = dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF);
    let preview_dir = dir.join(STORYBOARD_DIR).join(".preview");
    let previews = files_in(&preview_dir, Some(&["png"]));
    let layout = std::fs::read_to_string(preview_dir.join("layout.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or_else(|| json!({}));
    let built = std::fs::metadata(&pdf).and_then(|m| m.modified()).ok();
    let can_build = !transcript_files(&dir).is_empty();
    let stale = can_build && match (built, storyboard_inputs_changed(&dir)) {
        (None, _) => !panel_files(&dir).is_empty(),
        (Some(b), Some(changed)) => changed > b,
        (Some(_), None) => false,
    };
    Ok(json!({"ok": true, "pdf": pdf, "exists": pdf.is_file(), "approved": s.draft_approved, "previews": previews,
        "scene_titles": layout.get("scene_titles").cloned().unwrap_or_else(|| json!([])),
        "first_scene_page": layout.get("first_scene_page").cloned().unwrap_or_else(|| json!(2)), "layout": layout, "stale": stale}))
}

pub fn cmd_storyboard_init(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    for folder in [STORYBOARD_DIR, ASSETS_DIR, "liner-notes"] {
        std::fs::create_dir_all(dir.join(folder))?;
    }
    let list = storyboard_scenes(&dir)?;
    let mut created = Vec::new();
    for (i, scene) in list.iter().enumerate() {
        if !scene_doc_path(&dir, i + 1).is_file() {
            write_scene_doc(&dir, i + 1, &scene_label(i, scene), &read_scene_doc(&dir, i + 1))?;
            created.push(scene_doc_path(&dir, i + 1));
        }
    }
    let mut toc = cmd_storyboard_toc(s, &json!({}))?;
    toc["created"] = json!(created);
    Ok(toc)
}

pub fn cmd_storyboard_toc(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let list = storyboard_scenes(&dir)?;
    let panels = panel_files(&dir).len();
    let first_scene_page = first_scene_page(s, &dir);
    let chapters: Vec<Value> = list
        .iter()
        .enumerate()
        .map(|(i, sc)| {
            let lines = sc["lines"].as_array().map_or(0, Vec::len);
            json!({"scene": i + 1, "title": scene_label(i, sc), "page": first_scene_page + i, "status": sc["status"], "lines": lines,
                "panels": lines.min(panels), "notes": sc["notes"].as_array().map_or(0, Vec::len), "source": scene_doc_path(&dir, i + 1)})
        })
        .collect();
    let pages = first_scene_page + list.len();
    Ok(json!({"ok": true, "pdf": dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF), "lyrics_page": 1, "chapters": chapters,
        "overview_page": 2, "brief_page": pages}))
}

pub fn cmd_storyboard_read(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let list = storyboard_scenes(&dir)?;
    let n = scene_number(params, list.len())?;
    let scene = &list[n - 1];
    let panels = panel_files(&dir);
    let lines: Vec<Value> = scene["lines"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(j, l)| {
            let panel = panels.get(j);
            json!({"line": j + 1, "start": l["start"], "time": clock(l["start"].as_f64().unwrap_or(0.0)), "lyric": l["text"], "heard": l.get("heard"),
                "cue": l.get("cue"), "panel": panel, "prompt": panel.and_then(|p| std::fs::read_to_string(p.with_extension("prompt.md")).ok())})
        })
        .collect();
    Ok(json!({"ok": true, "scene": n, "title": scene_label(n - 1, scene), "kind": scene["kind"], "start": scene["start"], "end": scene["end"],
        "status": scene["status"], "notes": scene["notes"], "reference": scene["reference"], "teaching_reference": scene["teaching_reference"],
        "sections": scene["sections"], "lines": lines, "source": scene_doc_path(&dir, n), "page": first_scene_page(s, &dir) + n - 1}))
}

pub fn cmd_storyboard_search(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let query = params.get("query").and_then(Value::as_str).filter(|q| !q.trim().is_empty()).ok_or_else(|| EngineError::InvalidParams("missing 'query'".into()))?.to_lowercase();
    let list = storyboard_scenes(&dir)?;
    let panels = panel_files(&dir);
    let mut hits = Vec::new();
    for (i, scene) in list.iter().enumerate() {
        for (j, l) in scene["lines"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let lyric = l["text"].as_str().unwrap_or_default();
            if lyric.to_lowercase().contains(&query) {
                hits.push(json!({"scene": i + 1, "line": j + 1, "where": "lyric", "text": lyric}));
            }
            if let Some(prompt) = panels.get(j).and_then(|p| std::fs::read_to_string(p.with_extension("prompt.md")).ok())
                && prompt.to_lowercase().contains(&query)
            {
                hits.push(json!({"scene": i + 1, "line": j + 1, "where": "prompt", "text": prompt.chars().take(200).collect::<String>()}));
            }
        }
        for note in scene["notes"].as_array().cloned().unwrap_or_default() {
            if note.as_str().is_some_and(|t| t.to_lowercase().contains(&query)) {
                hits.push(json!({"scene": i + 1, "where": "note", "text": note}));
            }
        }
    }
    Ok(json!({"ok": true, "query": query, "hits": hits}))
}

/// Changes one scene's source: a lyric correction, a line's panel prompt or image, a note, or the status.
pub fn cmd_storyboard_edit(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let list = storyboard_scenes(&dir)?;
    let n = scene_number(params, list.len())?;
    let field = params.get("field").and_then(Value::as_str).unwrap_or_default();
    let value = params.get("value").and_then(Value::as_str).unwrap_or_default().to_string();
    let line = params.get("line").and_then(Value::as_u64).map(|l| l as usize);
    let lines = list[n - 1]["lines"].as_array().map_or(0, Vec::len);
    let need_line = || -> Result<usize> {
        line.filter(|l| *l >= 1 && *l <= lines).ok_or_else(|| EngineError::InvalidParams(format!("'line' must be 1..={lines} for this scene")))
    };
    let mut doc = read_scene_doc(&dir, n);
    match field {
        "lyric" => {
            doc.lyrics.insert(need_line()?, value.clone());
        }
        "cue" => {
            doc.cues.insert(need_line()?, value.clone());
        }
        "note" => doc.notes.push(value.clone()),
        "prompt" => {
            let l = need_line()?;
            let panel = dir.join(STORYBOARD_DIR).join(format!("panel-{l:02}.png"));
            std::fs::write(panel.with_extension("prompt.md"), format!("{value}\n"))?;
        }
        "panel" => {
            let l = need_line()?;
            let src = resolve_workspace_path(s, &value);
            if !src.is_file() || !has_extension(&src, IMAGE_EXTENSIONS) {
                return Err(EngineError::InvalidParams(format!("not an image: {}", src.display())));
            }
            std::fs::copy(&src, dir.join(STORYBOARD_DIR).join(format!("panel-{l:02}.png")))?;
        }
        "time" => {
            let (a, b) = value.split_once(['-', '–']).and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?)))
                .filter(|(a, b)| b > a).ok_or_else(|| EngineError::InvalidParams("time is start-end in seconds, e.g. 0-21.3".into()))?;
            doc.time = Some((a, b));
        }
        "reference" => {
            let src = resolve_workspace_path(s, &value);
            if !src.is_file() || !has_extension(&src, IMAGE_EXTENSIONS) {
                return Err(EngineError::InvalidParams(format!("not an image: {}", src.display())));
            }
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let rel = format!("{STORYBOARD_DIR}/scene-{n}-reference.{ext}");
            if src != dir.join(&rel) {
                std::fs::copy(&src, dir.join(&rel))?;
            }
            doc.reference = Some(rel);
        }
        "teaching_reference" => {
            let src = resolve_workspace_path(s, &value);
            if !src.is_file() || !has_extension(&src, IMAGE_EXTENSIONS) {
                return Err(EngineError::InvalidParams(format!("not an image: {}", src.display())));
            }
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let rel = format!("{STORYBOARD_DIR}/scene-{n}-teaching.{ext}");
            if src != dir.join(&rel) {
                std::fs::copy(&src, dir.join(&rel))?;
            }
            doc.teaching_reference = Some(rel);
        }
        key if SCENE_SECTIONS.iter().any(|(k, _)| *k == key) => {
            doc.sections.insert(key.to_string(), value.clone());
        }
        "status" => {
            if !SCENE_STATUSES.contains(&value.as_str()) {
                return Err(EngineError::InvalidParams(format!("status must be one of {SCENE_STATUSES:?}")));
            }
        }
        _ => return Err(EngineError::InvalidParams(format!(
            "field must be lyric, cue, prompt, panel, note, status, time, reference, teaching_reference or a section: {}",
            SCENE_SECTIONS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ")
        ))),
    }
    doc.status = if field == "status" { value } else { "needs review".into() };
    write_scene_doc(&dir, n, &scene_label(n - 1, &list[n - 1]), &doc)?;
    cmd_workshop_build_storyboard(s, &json!({}))?;
    cmd_storyboard_read(s, &json!({"scene": n}))
}

/// Imports an existing OpenMontage-style project's scenes into this storyboard template.
pub fn cmd_storyboard_import_openmontage(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let src = params.get("path").and_then(Value::as_str).map(|p| resolve_workspace_path(s, p)).ok_or_else(|| EngineError::InvalidParams("missing 'path' (the existing project folder)".into()))?;
    let overwrite = params.get("overwrite").and_then(Value::as_bool).unwrap_or(false);
    let read = |rel: &str| -> Option<Value> { serde_json::from_str(&std::fs::read_to_string(src.join(rel)).ok()?).ok() };
    let plan = read("artifacts/scene_plan.json").ok_or_else(|| EngineError::InvalidParams(format!("no artifacts/scene_plan.json in {}", src.display())))?;
    let packets = read("review/scene-packets.json");
    let plan_scenes = plan.get("scenes").and_then(Value::as_array).cloned().unwrap_or_default();
    if plan_scenes.is_empty() {
        return Err(EngineError::InvalidParams("scene_plan.json has no scenes".into()));
    }
    if !overwrite && scene_doc_path(&dir, 1).is_file() {
        return Err(EngineError::InvalidParams("this project already has scene files (pass \"overwrite\": true to replace them)".into()));
    }
    let text_of = |v: &Value| -> Option<String> {
        match v {
            Value::String(t) => Some(t.clone()),
            Value::Array(items) => Some(items.iter().map(|i| format!("- {}", i.as_str().map(str::to_owned).unwrap_or_else(|| i.to_string()))).collect::<Vec<_>>().join("\n")),
            Value::Null => None,
            other => Some(other.to_string()),
        }
    };
    std::fs::create_dir_all(dir.join(STORYBOARD_DIR))?;
    let mut written = Vec::new();
    for (i, sc) in plan_scenes.iter().enumerate() {
        let n = i + 1;
        let id = sc.get("id").and_then(Value::as_str).unwrap_or_default();
        let packet = packets.as_ref().and_then(|p| p.get("scenes")).and_then(Value::as_array).and_then(|list| list.iter().find(|p| p["id"].as_str() == Some(id))).cloned().unwrap_or(Value::Null);
        let mut doc = SceneDoc { status: "draft".into(), ..Default::default() };
        doc.title = packet["title"].as_str().map(str::to_owned).or_else(|| sc.get("description").and_then(Value::as_str).map(|d| d.chars().take(60).collect()));
        doc.time = sc.get("start_seconds").and_then(Value::as_f64).zip(sc.get("end_seconds").and_then(Value::as_f64));
        if let Some(reference) = packet["reference"].as_str().map(|r| src.join(r)).filter(|p| p.is_file()) {
            let ext = reference.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let rel = format!("{STORYBOARD_DIR}/scene-{n}-reference.{ext}");
            std::fs::copy(&reference, dir.join(&rel))?;
            doc.reference = Some(rel);
        }
        let skills = packet["skills"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "));
        let skills = match (skills, packet["combination"].as_str()) {
            (Some(k), Some(c)) => Some(format!("{k}\n{c}")),
            (k, c) => k.or(c.map(str::to_owned)),
        };
        for (key, value) in [
            ("skills", skills),
            ("reference_prompt", text_of(&packet["reference_prompt"])),
            ("animation_prompt", text_of(&packet["animation_prompt"])),
            ("layer_stack", text_of(&packet["layer_stack_proposal"])),
            ("build_steps", text_of(&packet["build_steps"])),
            ("acceptance_checks", text_of(&packet["output_checks"])),
            ("plan_vs_actual", text_of(&packet["plan_vs_actual"])),
        ] {
            if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
                doc.sections.insert(key.into(), v);
            }
        }
        if let Some(desc) = sc.get("description").and_then(Value::as_str) {
            doc.notes.push(format!("Scene plan: {desc}"));
        }
        if let Some(status) = packet["status"].as_str() {
            doc.notes.push(format!("Imported status: {status}"));
        }
        write_scene_doc(&dir, n, &format!("Scene {n}"), &doc)?;
        written.push(scene_doc_path(&dir, n));
    }
    for n in plan_scenes.len() + 1.. {
        let extra = scene_doc_path(&dir, n);
        if !extra.is_file() {
            break;
        }
        std::fs::remove_file(extra)?;
    }
    let mut toc = cmd_storyboard_toc(s, &json!({}))?;
    toc["imported"] = json!(written);
    toc["next"] = json!([
        "workshop.build_storyboard to rebuild the PDF from the imported scenes",
        "storyboard.read scene=N for each scene: check lyrics against the official lyrics and fill gaps with storyboard.edit",
        "the user approves each scene with storyboard.approve"]);
    Ok(toc)
}

pub fn cmd_storyboard_approve(s: &mut Session, params: &Value) -> Result<Value> {
    let approved = params.get("approved").and_then(Value::as_bool).unwrap_or(true);
    let status = if approved { "approved" } else { "needs review" };
    cmd_storyboard_edit(s, &json!({"scene": params.get("scene"), "field": "status", "value": status}))
}

/// Write the storyboard backlot mirror for OpenMontage.
fn write_backlot_mirror(
    dir: &Path,
    title: &str,
    scene_list: &[Value],
    panels: &[PathBuf],
    prompts: &[Option<String>],
    assets: &[PathBuf],
    total: Option<f64>,
) -> Result<()> {
    let art = dir.join("artifacts");
    std::fs::create_dir_all(&art)?;
    let rel = |p: &Path| p.strip_prefix(dir).map(|r| r.display().to_string()).unwrap_or_else(|_| p.display().to_string());
    let mut scenes = Vec::new();
    let mut sections = Vec::new();
    let mut manifest = Vec::new();
    for (i, scene) in scene_list.iter().enumerate() {
        let (id, section) = (format!("scene-{}", i + 1), format!("section-{}", i + 1));
        let lines = scene["lines"].as_array().cloned().unwrap_or_default();
        let text = lines.iter().map(|l| format!("{}  {}", clock(l["start"].as_f64().unwrap_or(0.0)), l["text"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n");
        scenes.push(json!({"id": id, "type": scene["kind"], "description": scene_label(i, scene),
            "start_seconds": scene["start"], "end_seconds": scene["end"], "script_section_id": section}));
        sections.push(json!({"id": section, "start_seconds": scene["start"], "end_seconds": scene["end"],
            "text": if text.is_empty() { "(instrumental)".to_string() } else { text }}));
        for (j, _) in lines.iter().enumerate() {
            if let Some(panel) = panels.get(j) {
                let stem = panel.file_stem().and_then(|n| n.to_str()).unwrap_or("panel");
                manifest.push(json!({"id": format!("{stem}@{id}"), "type": "image", "scene_id": id, "path": rel(panel),
                    "prompt": prompts.get(j).cloned().flatten(), "source_tool": "animationcraft"}));
            }
        }
    }
    for a in assets {
        let stem = a.file_stem().and_then(|n| n.to_str()).unwrap_or("asset");
        manifest.push(json!({"id": stem, "type": "image", "scene_id": null, "path": rel(a),
            "prompt": std::fs::read_to_string(a.with_extension("prompt.md")).ok(), "source_tool": "animationcraft"}));
    }
    let write = |name: &str, v: Value| -> Result<()> {
        let text = serde_json::to_string_pretty(&v).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        std::fs::write(art.join(name), text)?;
        Ok(())
    };
    write("scene_plan.json", json!({"scenes": scenes, "metadata": {"total_duration_seconds": total, "source": "animationcraft workshop.build_storyboard"}}))?;
    write("script.json", json!({"title": title, "sections": sections}))?;
    write("asset_manifest.json", json!({"assets": manifest}))?;
    let project_json = dir.join("project.json");
    if !project_json.is_file() {
        let id = dir.file_name().and_then(|n| n.to_str()).unwrap_or("project");
        std::fs::write(&project_json, json!({"version": "1.0", "project_id": id, "title": title, "pipeline_type": "animation"}).to_string())?;
    }
    Ok(())
}


