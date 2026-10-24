//! Storyboard chapters, scene packets (`storyboard/scene-N.md`), PDF builder (`pdfcraft-cli`),
//! and OpenMontage backlot mirror.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};
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
        .or_else(|| std::fs::read(dir.join("transcript-provenance.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).and_then(|v|v["duration_sec"].as_f64()))
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

/// The project's backgrounds, characters and props: the workshop's asset files plus connected
/// project images (the backlot mirror lists them all).
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

/// What a Cast & sets card shows.
pub(crate) enum CardArt {
    /// One picture (a reference image or a background).
    Image(PathBuf),
    /// A character's parts, assembled (they share one canvas).
    Parts(Vec<PathBuf>),
    /// An empty slot to fill in.
    Empty,
}

/// One card on the Cast & sets board: its section, its name and its art.
pub(crate) struct Card {
    pub section: &'static str,
    pub name: String,
    pub art: CardArt,
}

/// Cards on the board: a character per `assets/characters/<name>/` with a `parts/` or
/// `reference/` folder, a location per `assets/backgrounds/<name>/` (or image directly in it).
/// Each section has at least one row of three, padded with empty slots to fill in.
pub(crate) fn board_cards(dir: &Path) -> Vec<Card> {
    let images = |d: &Path| {
        let mut f = files_in(d, Some(IMAGE_EXTENSIONS));
        f.sort();
        f
    };
    let subdirs = |d: &Path| {
        let mut v: Vec<PathBuf> = std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        v.sort();
        v
    };
    let name = |p: &Path| p.file_stem().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    let assets = dir.join(ASSETS_DIR);
    let mut cast: Vec<Card> = subdirs(&assets.join("characters"))
        .into_iter()
        .filter_map(|c| {
            let art = match (images(&c.join("reference")).into_iter().next(), images(&c.join("parts"))) {
                (Some(r), _) => CardArt::Image(r),
                (None, parts) if !parts.is_empty() => CardArt::Parts(parts),
                _ => return None,
            };
            Some(Card { section: "CAST", name: name(&c), art })
        })
        .collect();
    let backgrounds = assets.join("backgrounds");
    let mut sets: Vec<Card> = subdirs(&backgrounds)
        .into_iter()
        .filter_map(|b| {
            let files = images(&b);
            let own = files.iter().find(|f| name(f) == name(&b)).or(files.first())?.clone();
            Some(Card { section: "LOCATIONS", name: name(&b), art: CardArt::Image(own) })
        })
        .chain(images(&backgrounds).into_iter().map(|f| Card { section: "LOCATIONS", name: name(&f), art: CardArt::Image(f) }))
        .collect();
    for (list, section) in [(&mut cast, "CAST"), (&mut sets, "LOCATIONS")] {
        while list.len() < 3 || list.len() % 3 != 0 {
            list.push(Card { section, name: String::new(), art: CardArt::Empty });
        }
    }
    cast.into_iter().chain(sets).collect()
}

/// Board geometry (portrait A4): where each card goes, as (page offset, x, y), and how many
/// pages the board takes. The first page also holds the master prompt and the locks.
pub(crate) const BOARD_TOP: f64 = 236.0;
pub(crate) fn board_layout(cards: &[Card]) -> (Vec<(usize, f64, f64)>, usize) {
    let (pw, ph, m, gap) = (595.0, 842.0, 32.0, 12.0);
    let cw = (pw - 2.0 * m - 2.0 * gap) / 3.0;
    let row_h = cw * 9.0 / 16.0 + 48.0;
    let (mut page, mut y, mut col) = (0usize, BOARD_TOP, 0usize);
    let mut last_section = "";
    let mut out = Vec::new();
    for card in cards {
        if card.section != last_section {
            if col != 0 {
                y += row_h;
                col = 0;
            }
            y += 22.0;
            last_section = card.section;
        }
        if col == 0 && y + row_h > ph - 40.0 {
            page += 1;
            y = 96.0;
        }
        out.push((page, m + col as f64 * (cw + gap), y));
        col += 1;
        if col == 3 {
            col = 0;
            y += row_h;
        }
    }
    (out, page + 1)
}

/// A character's parts stacked in order (they share one canvas), cropped to what is drawn,
/// written to `out`. None when they don't share a canvas.
pub fn assembled_parts(parts: &[PathBuf], out: &Path) -> Option<PathBuf> {
    let mut canvas: Option<image::RgbaImage> = None;
    for p in parts {
        let layer = image::open(p).ok()?.to_rgba8();
        match &mut canvas {
            None => canvas = Some(layer),
            Some(c) if c.dimensions() == layer.dimensions() => image::imageops::overlay(c, &layer, 0, 0),
            Some(_) => return None,
        }
    }
    let c = canvas?;
    let (w, h) = c.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, px) in c.enumerate_pixels() {
        if px[3] > 8 {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    if x1 < x0 {
        return None;
    }
    image::imageops::crop_imm(&c, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image().save(out).ok()?;
    Some(out.to_path_buf())
}

/// The PDF page of scene 1: after the lyrics page and the Cast & sets board.
fn first_scene_page(dir: &Path) -> usize {
    if let Some(page)=std::fs::read(dir.join("storyboard/movie-board.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).and_then(|v|v["first_scene_page"].as_u64()) {return page as usize;}
    2 + board_layout(&board_cards(dir)).1
}

/// Text typed into the existing storyboard.pdf's fields, by field name, so a rebuild keeps it.
fn saved_field_values(dir: &Path, pdf: &Path) -> std::collections::HashMap<String, String> {
    let script = dir.join(".storyboard.read.pdfcraft.json");
    let steps = json!([{"tool": "doc_open", "args": {"path": pdf}}, {"tool": "form_fields", "args": {"doc": 1}}]);
    if !pdf.is_file() || std::fs::write(&script, steps.to_string()).is_err() {
        return Default::default();
    }
    let out = run_tool_in(dir, "pdfcraft-cli", &["run".into(), "--script".into(), script.to_string_lossy().into_owned(), "--report".into()]);
    let _ = std::fs::remove_file(&script);
    let report: Value = out.ok().and_then(|o| serde_json::from_str(&o).ok()).unwrap_or_default();
    report["steps"][1]["result"][0]["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["type"] == json!("text"))
        .filter_map(|f| Some((f["name"].as_str()?.to_string(), f["value"].as_str()?.to_string())))
        .filter(|(_, v)| !v.trim().is_empty())
        .collect()
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
pub fn cmd_workshop_build_storyboard(s: &mut Session, params: &Value) -> Result<Value> {
    if params["template"]=="backlot" {
        let dir=project_dir(s)?;
        let active_board=std::fs::read(dir.join("storyboard/movie-board.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
        let transcript=if let Some(path)=params["transcript"].as_str(){if params["draft"]!=true{return Err(EngineError::InvalidParams("An explicit source transcript requires draft=true; active data is preserved".into()));}resolve_workspace_path(s,path).canonicalize()?}else{std::fs::read(dir.join("storyboard/movie-board.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).and_then(|v|v["source_transcript"].as_str().map(PathBuf::from)).filter(|p|p.is_file()).or_else(||transcript_files(&dir).into_iter().next()).ok_or_else(||EngineError::InvalidParams("Import the existing transcript first".into()))?};
        let title=s.project_path.as_ref().and_then(|p|p.file_stem()).and_then(|n|n.to_str()).unwrap_or("Movie board");
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        let out=if let Some(resume)=params["resume"].as_str(){
            let manifest=resolve_workspace_path(s,resume).canonicalize()?;
            if !manifest.starts_with(dir.join("storyboard").canonicalize()?) || manifest.file_name().and_then(|n|n.to_str())!=Some("manifest.json"){return Err(EngineError::InvalidParams("Resume must be this project's storyboard draft manifest".into()));}
            manifest.parent().ok_or_else(||EngineError::InvalidParams("Draft has no folder".into()))?.to_path_buf()
        }else{dir.join("storyboard").join(format!("movie-board-{stamp}"))};
        let title=title.to_owned();
        let count=storyboard_scenes(&dir)?.len();
        let scenes=(1..=count).map(|scene|cmd_storyboard_read(s,&json!({"scene":scene}))).collect::<Result<Vec<_>>>()?;
        std::fs::create_dir_all(&out)?;
        let scene_map=out.join("scene-map.json");
        let provenance=std::fs::read(dir.join("transcript-provenance.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
        let recording_duration=song_seconds(&dir);
        let timing_extent=recording_duration.or_else(||provenance["duration_sec"].as_f64()).or_else(||scenes.last().and_then(|v|v["end"].as_f64()));
        std::fs::write(&scene_map,serde_json::to_vec_pretty(&json!({"scenes":scenes,"source":"canonical CLI scene spans","transcript":transcript,"captions":if params["transcript"].is_string(){Value::Null}else if active_board["source_transcript"]==json!(transcript){active_board["source_captions"].clone()}else{json!(caption_file(&dir))},"duration_sec":timing_extent,"recording_duration_verified":recording_duration.is_some(),"audio":s.song_audio,"project":s.project_path,"assets":s.project_assets})).map_err(|e|EngineError::InvalidParams(e.to_string()))?)?;
        let script=crate::workspace::workspace_root(s).join("pipeline/movie_board.py");
        let mut args=vec![script.to_string_lossy().into_owned(),"--transcript".into(),transcript.to_string_lossy().into_owned(),"--scene-map".into(),scene_map.to_string_lossy().into_owned(),"--out".into(),out.to_string_lossy().into_owned(),"--title".into(),title];
        if params["allow_timing_overrun"]==true{if params["draft"]!=true{return Err(EngineError::InvalidParams("Timing-overrun source may only produce an explicitly warned draft".into()));}args.push("--allow-timing-overrun".into());}
        if let Some(source)=params["source"].as_str(){args.extend(["--source".into(),resolve_workspace_path(s,source).to_string_lossy().into_owned()]);}
        if let Some(panels)=params["panels"].as_str(){args.extend(["--panels".into(),panels.into()]);}
        let output=run_tool_in(&dir,"python3",&args)?;
        let mut result:Value=serde_json::from_str(&output).map_err(|e|EngineError::InvalidParams(e.to_string()))?;
        result["project"]=json!(s.project_path);result["manifest"]=json!(out.join("manifest.json"));
        result["draft"]=json!(params["draft"]==true || result["mapping"]["needs_correction"]==true);
        result["status"]=json!(if result["ok"]==false{"draft generation failed; active storyboard unchanged"}else if result["timing_valid"]==false{"editable draft with raw timing warning; not validated"}else if result["mapping"]["needs_correction"]==true{"draft needs source mapping correction"}else if result["draft"]==true{"draft ready for review; active storyboard unchanged"}else{"saved"});
        let executable=crate::command_center::cli_program();
        result["next_commands"]=json!([format!("{} --bridge auto exec storyboard.read scene=1 project={}",crate::command_center::shell_quote(&executable),crate::command_center::shell_quote(&s.project_path.as_ref().map(|p|p.to_string_lossy().into_owned()).unwrap_or_default()))]);
        result["resume_command"]=json!(format!("{} --bridge auto exec workshop.build_storyboard template=backlot draft=true resume={} source={} project={}",crate::command_center::shell_quote(&executable),crate::command_center::shell_quote(&out.join("manifest.json").to_string_lossy()),crate::command_center::shell_quote(params["source"].as_str().unwrap_or(&dir.to_string_lossy())),crate::command_center::shell_quote(&s.project_path.as_ref().map(|p|p.to_string_lossy().into_owned()).unwrap_or_default())));
        if let Some(transcript)=params["transcript"].as_str(){let command=format!("{} transcript={} allow_timing_overrun={}",result["resume_command"].as_str().unwrap_or_default(),crate::command_center::shell_quote(transcript),params["allow_timing_overrun"]==true);result["resume_command"]=json!(command);}
        std::fs::write(out.join("manifest.json"),result.to_string())?;
        if result["ok"]==true && result["draft"]!=true{std::fs::write(dir.join("storyboard/movie-board.json"),result.to_string())?;}
        return Ok(result);
    }
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
    let out_dir = dir.join(STORYBOARD_DIR);
    let pdf = out_dir.join(STORYBOARD_PDF);
    // Text typed into the current PDF's fields wins over everything else on a rebuild.
    let saved = saved_field_values(&dir, &pdf);
    let theme = dir.join(STORYBOARD_DIR).join(".theme");
    std::fs::create_dir_all(&theme)?;
    // The Old MacDonald template's palette: cream page, rust kickers, slate bars.
    let swatch = |name: &str, rgb: [u8; 3]| -> Result<PathBuf> {
        let p = theme.join(format!("{name}.png"));
        // pdfcraft-cli 0.9 refuses tiny images ("the image is too small"); a solid swatch scales freely.
        image::RgbaImage::from_pixel(32, 32, image::Rgba([rgb[0], rgb[1], rgb[2], 255])).save(&p)?;
        Ok(p)
    };
    let card_fill = swatch("card", [0xFF, 0xF8, 0xEC])?;
    let rust_fill = swatch("rust", [0x9A, 0x3B, 0x2B])?;
    let slate_fill = swatch("slate", [0x2F, 0x5D, 0x73])?;
    let line_fill = swatch("line", [0xE2, 0xD3, 0xBA])?;
    const PAGE: &str = "#F4E9D7";
    const INK: &str = "#292A25";
    const MUTE: &str = "#7A7062";
    const RUST: &str = "#9A3B2B";
    const ON_SLATE: &str = "#F4E9D7";
    const SLATE_LABEL: &str = "#A9C7D6";

    let asset_images = overview_images(s, &dir);
    let cards = board_cards(&dir);
    let (card_spots, board_pages) = board_layout(&cards);
    let first_scene_page = 2 + board_pages;
    let pages = 1 + board_pages + scene_list.len() + 1;
    let mut layout: Vec<Value> = Vec::new();

    // Portrait A4, to fill the tall storyboard pane.
    let (pw, ph, m) = (595.0_f64, 842.0_f64, 32.0_f64);
    let mut steps = vec![
        json!({"tool": "doc_create", "args": {"from": "blank", "pages": pages, "width": pw, "height": ph, "name": title}}),
        json!({"tool": "doc_background", "args": {"doc": 1, "color": PAGE}}),
    ];
    let text = |page: usize, rect: [f64; 4], t: String, size: f64, color: &str, bold: bool| {
        json!({"tool": "page_add_text", "args": {"doc": 1, "page": page, "rect": rect, "text": t, "size": size, "font": "helvetica", "color": color, "bold": bold}})
    };
    let image = |page: usize, path: &Path, rect: [f64; 4]| json!({"tool": "page_add_image", "args": {"doc": 1, "page": page, "path": path, "rect": rect}});
    let header = |page: usize, kicker: String, heading: String| {
        vec![
            image(page, &rust_fill, [0.0, 0.0, pw, 6.0]),
            text(page, [m, 22.0, pw - m - 120.0, 36.0], kicker, 8.5, RUST, true),
            text(page, [m, 38.0, pw - m - 120.0, 86.0], heading, 19.0, INK, true),
        ]
    };
    // Fillable fields: (name, page, rect, multiline, style), their starting values, and images.
    let mut fields: Vec<(String, usize, [f64; 4], bool, &str)> = Vec::new();
    let mut values = serde_json::Map::new();
    let mut field_images: Vec<(String, PathBuf)> = Vec::new();
    let mut field = |name: String, page: usize, rect: [f64; 4], multiline: bool, style: &'static str, start: Option<String>| {
        let value = saved.get(&name).cloned().or(start).filter(|v| !v.trim().is_empty());
        if let Some(v) = value {
            values.insert(name.clone(), json!(v));
        }
        fields.push((name, page, rect, multiline, style));
    };
    let label = |page: usize, at: [f64; 2], t: &str, color: &str| text(page, [at[0], at[1], at[0] + 240.0, at[1] + 12.0], t.into(), 7.0, color, true);

    // Page 1: the lyrics, one numbered row per scene, each a link to its page.
    let studio_info = studio(&dir);
    let kicker = format!("{} · STORYBOARD", studio_info.name.clone().unwrap_or_else(|| "SONG".into()).to_uppercase());
    steps.extend(header(1, kicker, title.clone()));
    let liner = files_in(&dir.join("liner-notes"), None);
    let facts = [
        format!("Song  {}", s.song_audio.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("not set")),
        format!("Captions  {}", transcript_path.strip_prefix(&dir).unwrap_or(&transcript_path).display()),
        format!("Liner notes  {}", if liner.is_empty() { "none attached".to_string() } else { liner.iter().filter_map(|p| p.file_name()?.to_str()).collect::<Vec<_>>().join(", ") }),
        format!("Edit in  {}", s.video_target),
        format!("Status  {}", if s.draft_approved { "approved" } else { "draft · needs review" }),
    ];
    steps.push(image(1, &card_fill, [m, 92.0, pw - m, 150.0]));
    steps.push(text(1, [m + 14.0, 100.0, pw - m - 14.0, 146.0], facts.join("     "), 8.5, MUTE, false));
    if let Some(intent) = &studio_info.intent {
        let first_line = intent.lines().find(|l| !l.trim().is_empty()).unwrap_or_default();
        steps.push(text(1, [m, 156.0, pw - m, 176.0], first_line.chars().take(160).collect(), 8.0, MUTE, false));
    }
    steps.push(label(1, [m, 182.0], "LYRICS · TAP A LINE TO OPEN ITS SCENE", RUST));
    let row_step = ((ph - 200.0 - 30.0) / scene_list.len().max(1) as f64).min(34.0);
    for (i, scene) in scene_list.iter().enumerate() {
        let y = 198.0 + i as f64 * row_step;
        let row = [m, y, pw - m, y + row_step - 4.0];
        steps.push(image(1, &card_fill, row));
        let range = format!("{}–{}", clock(scene["start"].as_f64().unwrap_or(0.0)), clock(scene["end"].as_f64().unwrap_or(0.0)));
        let lyric = scene["lines"][0]["text"].as_str().map(str::to_owned);
        let mid = y + (row_step - 4.0) / 2.0 - 7.0;
        steps.push(text(1, [m + 12.0, mid, m + 40.0, mid + 16.0], format!("{:02}", i + 1), 11.0, RUST, true));
        steps.push(text(1, [m + 44.0, mid + 1.0, m + 110.0, mid + 15.0], range, 8.5, MUTE, false));
        steps.push(match lyric {
            Some(words) => text(1, [m + 112.0, mid, pw - m - 10.0, mid + 16.0], words, 11.0, INK, true),
            None => text(1, [m + 112.0, mid, pw - m - 10.0, mid + 16.0], "Instrumental · design this stretch".into(), 11.0, MUTE, false),
        });
        steps.push(json!({"tool": "link_add", "args": {"doc": 1, "page": 1, "rect": row, "to_page": first_scene_page + i, "visible": false}}));
    }

    // Cast & sets: the instruction board. Master prompt and locks, then a card per character
    // and per location; every box is a field to type or drop an image into.
    for k in 0..board_pages {
        let title = if board_pages > 1 { format!("Cast & sets {}/{board_pages}", k + 1) } else { "Cast & sets".into() };
        steps.extend(header(2 + k, "INSTRUCTION BOARD".into(), title));
    }
    steps.push(label(2, [m, 92.0], "MASTER PROMPT · STYLE, MATERIALS, WHAT NEVER APPEARS", RUST));
    field("board_master_prompt".into(), 2, [m, 106.0, pw - m, 160.0], true, "body", None);
    steps.push(label(2, [m, 168.0], "LOCKS · WHAT MUST STAY THE SAME IN EVERY SCENE", RUST));
    field("board_locks".into(), 2, [m, 182.0, pw - m, 220.0], true, "body", None);
    let cw = (pw - 2.0 * m - 24.0) / 3.0;
    let mut last_section = "";
    for (n, (card, (pg, x, y))) in cards.iter().zip(&card_spots).enumerate() {
        let page = 2 + pg;
        if card.section != last_section {
            steps.push(label(page, [m, y - 18.0], card.section, RUST));
            last_section = card.section;
        }
        let img_box = [*x, *y, x + cw, y + cw * 9.0 / 16.0];
        steps.push(image(page, &card_fill, [img_box[0], img_box[1], img_box[2], img_box[3] + 30.0]));
        let slug = format!("{}_{n}", card.section.to_lowercase());
        // Transparent art is cropped to what is drawn, so a character fills its card.
        let art = match &card.art {
            CardArt::Image(p) => assembled_parts(std::slice::from_ref(p), &theme.join(format!("{slug}.png"))).or(Some(p.clone())),
            CardArt::Parts(parts) => assembled_parts(parts, &theme.join(format!("{slug}.png"))),
            CardArt::Empty => None,
        };
        if let Some(p) = art {
            layout.push(json!({"page": page, "path": p, "rect": img_box}));
            field_images.push((format!("{slug}_image"), p));
        }
        field(format!("{slug}_image"), page, img_box, false, "image", None);
        field(format!("{slug}_name"), page, [img_box[0] + 6.0, img_box[3] + 6.0, img_box[2] - 6.0, img_box[3] + 24.0], false, "name",
            (!card.name.is_empty()).then(|| card.name.clone()));
    }

    // One page per scene: the still, its fields, and the motion/caption bar.
    let image_box = [m, 96.0, pw - m, 96.0 + (pw - 2.0 * m) * 9.0 / 16.0];
    let card_top = image_box[3] + 12.0;
    let bar_top = 646.0;
    for (i, scene) in scene_list.iter().enumerate() {
        let page = first_scene_page + i;
        let n = i + 1;
        let range = format!("{} – {}", clock(scene["start"].as_f64().unwrap_or(0.0)), clock(scene["end"].as_f64().unwrap_or(0.0)));
        let lines = scene["lines"].as_array().cloned().unwrap_or_default();
        let lyric = lines.iter().filter_map(|l| l["text"].as_str()).collect::<Vec<_>>().join(" / ");
        let heading = match scene["title"].as_str().filter(|t| !t.starts_with("Scene ")) {
            Some(t) => t.to_string(),
            None if lyric.is_empty() => "Instrumental".into(),
            None => lyric.clone(),
        };
        steps.extend(header(page, format!("SCENE {n} OF {} · {range}", scene_list.len()), heading));
        field(format!("scene{n}_status"), page, [pw - m - 110.0, 24.0, pw - m, 42.0], false, "status",
            Some(scene["status"].as_str().unwrap_or("draft").to_string()));
        steps.push(image(page, &card_fill, image_box));
        match scene["reference"].as_str().map(PathBuf::from).filter(|p| p.is_file()) {
            Some(reference) => {
                layout.push(json!({"page": page, "path": reference, "rect": image_box}));
                field_images.push((format!("scene{n}_image"), reference));
            }
            None => {
                let mid = (image_box[1] + image_box[3]) / 2.0;
                steps.push(json!({"tool": "page_add_text", "args": {"doc": 1, "page": page, "rect": [image_box[0], mid - 8.0, image_box[2], mid + 8.0],
                    "text": "IMAGE · drop the still here (16:9)", "size": 10.0, "font": "helvetica", "color": MUTE, "align": "center"}}));
            }
        }
        field(format!("scene{n}_image"), page, image_box, false, "image", None);

        let sections = scene["sections"].as_object().cloned().unwrap_or_default();
        let section = |k: &str| sections.get(k).and_then(Value::as_str).map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "));
        let notes = scene["notes"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(" ");
        let cues: Vec<String> = lines.iter().filter_map(|l| l["cue"].as_str()).filter(|c| !c.trim().is_empty()).map(|c| format!("Action: {c}")).collect();
        let hold = scene["end"].as_f64().unwrap_or(0.0) - scene["start"].as_f64().unwrap_or(0.0);
        steps.push(image(page, &card_fill, [m, card_top, pw - m, bar_top - 12.0]));
        let (lx, rx, col_w) = (m + 14.0, pw / 2.0 + 6.0, pw / 2.0 - m - 20.0);
        let cell = |x: f64, y: f64, h: f64| [x, y + 12.0, x + col_w, y + 12.0 + h];
        let mut y = card_top + 12.0;
        for (key, title, h, start) in [
            ("lyric", "LYRIC CUE", 40.0, Some(if lyric.is_empty() { "Instrumental · no sung words".to_string() } else { [vec![lyric.clone()], cues.clone()].concat().join("\n") })),
            ("hold", "PLANNED HOLD", 18.0, Some(format!("{hold:.1} s"))),
            ("cast", "VISIBLE CAST", 50.0, section("layer_stack")),
        ] {
            steps.push(label(page, [lx, y], title, RUST));
            field(format!("scene{n}_{key}"), page, cell(lx, y, h), h > 20.0, "body", start);
            y += h + 22.0;
        }
        let mut y = card_top + 12.0;
        for (key, title, h, start) in [
            ("beat", "STORY BEAT · WHAT HAPPENS AND WHY", 70.0, section("reference_prompt").or_else(|| (!notes.is_empty()).then(|| notes.clone()))),
            ("lock", "CONTINUITY LOCK · WHAT MUST NOT CHANGE", 64.0, section("visual_review")),
        ] {
            steps.push(label(page, [rx, y], title, RUST));
            field(format!("scene{n}_{key}"), page, cell(rx, y, h), true, "body", start);
            y += h + 22.0;
        }
        steps.push(image(page, &slate_fill, [m, bar_top, pw - m, ph - 34.0]));
        steps.push(label(page, [lx, bar_top + 10.0], "MOTION · CAMERA MOVE AND ACCENTS", SLATE_LABEL));
        field(format!("scene{n}_motion"), page, [lx, bar_top + 22.0, pw - m - 14.0, bar_top + 82.0], true, "slate", section("animation_prompt"));
        steps.push(label(page, [lx, bar_top + 90.0], "EDITABLE CAPTION · ON-SCREEN WORDS, NEVER BAKED INTO ART", SLATE_LABEL));
        field(format!("scene{n}_caption"), page, [lx, bar_top + 102.0, pw - m - 14.0, bar_top + 124.0], false, "slate", (!lyric.is_empty()).then(|| lyric.clone()));
        steps.push(image(page, &line_fill, [m, ph - 30.0, pw - m, ph - 29.0]));
        steps.push(text(page, [m, ph - 26.0, pw - m, ph - 12.0],
            format!("ASSET scene-{n:02}.png  ·  STORYBOARD PAGE {page} OF {pages}  ·  source storyboard/scene-{n}.md"), 7.0, MUTE, false));
    }

    // Last page: the hand-off brief for agents.
    steps.extend(header(pages, "HANDOFF".into(), "Agent brief".into()));
    let brief = std::iter::once(brief_header(&dir))
        .chain(["song", "transcript", "reference", "animate"].iter().map(|st| stage_body(s, &dir, st)))
        .collect::<Vec<_>>()
        .join("\n");
    steps.push(image(pages, &card_fill, [m, 92.0, pw - m, ph - 32.0]));
    steps.push(json!({"tool": "page_add_text", "args": {"doc": 1, "page": pages, "rect": [m + 14.0, 104.0, pw - m - 14.0, ph - 40.0],
        "text": brief.replace(['—', '–'], "-"), "size": 7.5, "font": "courier", "color": INK}}));

    // The fields themselves, styled per kind, then their values and images.
    for (name, page, rect, multiline, kind) in &fields {
        let kind_type = if *kind == "image" { "image" } else { "text" };
        steps.push(json!({"tool": "form_add_field", "args": {"doc": 1, "page": page, "rect": rect, "name": name, "type": kind_type, "multiline": multiline}}));
    }
    for (kind, props) in [
        ("body", json!({"font_size": 9, "appearance": {"border": "#E2D3BA", "fill": "#FFFFFF", "text_color": INK, "width": 1, "font": "helvetica"}})),
        ("name", json!({"font_size": 9, "align": "center", "appearance": {"border": "none", "fill": "none", "text_color": INK, "font": "helvetica"}})),
        ("status", json!({"font_size": 8, "align": "center", "appearance": {"border": "none", "fill": "#5A7D3A", "text_color": "#FFFFFF", "font": "helvetica"}})),
        ("slate", json!({"font_size": 9, "appearance": {"border": "#5C8093", "fill": "none", "text_color": ON_SLATE, "width": 1, "font": "helvetica"}})),
        ("image", json!({"appearance": {"border": RUST, "style": "dashed", "width": 1, "fill": "none"}})),
    ] {
        let names: Vec<&String> = fields.iter().filter(|f| f.4 == kind).map(|f| &f.0).collect();
        if !names.is_empty() {
            let mut args = props;
            args["doc"] = json!(1);
            args["fields"] = json!(names);
            steps.push(json!({"tool": "form_set_props", "args": args}));
        }
    }
    if !values.is_empty() {
        steps.push(json!({"tool": "form_fill", "args": {"doc": 1, "values": values}}));
    }
    for (name, path) in &field_images {
        steps.push(json!({"tool": "form_set_image", "args": {"doc": 1, "field": name, "path": path}}));
    }
    steps.push(json!({"tool": "bookmark_add", "args": {"doc": 1, "title": "Lyrics", "page": 1}}));
    steps.push(json!({"tool": "bookmark_add", "args": {"doc": 1, "title": "Cast & sets", "page": 2}}));
    for (i, scene) in scene_list.iter().enumerate() {
        steps.push(json!({"tool": "bookmark_add", "args": {"doc": 1, "title": scene_label(i, scene), "page": first_scene_page + i}}));
    }
    steps.push(json!({"tool": "bookmark_add", "args": {"doc": 1, "title": "Agent brief", "page": pages}}));
    std::fs::create_dir_all(&out_dir)?;
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
    if let Ok(text)=std::fs::read_to_string(dir.join("storyboard/movie-board.json")) {
        if let Ok(mut board)=serde_json::from_str::<Value>(&text) {
            if board["design"]["placements"].is_array(){let base=crate::storyboard_design::base(&board);board["previews"]=base["previews"].clone();}
            board["layout"]=json!({"page_size":board["page_size"],"links":board["links"]});
            board["exists"]=json!(true);board["ok"]=json!(true);
            return Ok(board);
        }
    }
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
    Ok(json!({"scene_packets_available":scene_doc_path(&dir,1).is_file(),"editable_slide_integration":"Build the DeckCraft deck; Studio displays previews only","ok": true, "pdf": pdf, "exists": pdf.is_file(), "approved": s.draft_approved, "previews": previews,
        "scene_titles": layout.get("scene_titles").cloned().unwrap_or_else(|| json!([])),
        "first_scene_page": layout.get("first_scene_page").cloned().unwrap_or_else(|| json!(2)), "layout": layout, "stale": stale}))
}

pub fn cmd_storyboard_init(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    // The editable DeckCraft document is authoritative once created. Reopening never regenerates it.
    let active=dir.join("storyboard/movie-board.json");
    if let Ok(bytes)=std::fs::read(&active) {
        if let Ok(board)=serde_json::from_slice::<Value>(&bytes) {
            if board["deck"].as_str().is_some_and(|p|Path::new(p).is_file()) {
                let mut result=cmd_workshop_storyboard(s,&json!({}))?;
                result["reused_existing_document"]=json!(true);return Ok(result);
            }
        }
    }
    for folder in [STORYBOARD_DIR, ASSETS_DIR, "liner-notes"] {std::fs::create_dir_all(dir.join(folder))?;}
    // Canonical scene spans are parsed data, not another editable Markdown storyboard.
    let mut result=cmd_workshop_build_storyboard(s,&json!({"template":"backlot"}))?;
    result["reused_existing_document"]=json!(false);Ok(result)
}

pub fn cmd_storyboard_toc(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let list = storyboard_scenes(&dir)?;
    let panels = panel_files(&dir).len();
    let first_scene_page = first_scene_page(&dir);
    let chapters: Vec<Value> = list
        .iter()
        .enumerate()
        .map(|(i, sc)| {
            let lines = sc["lines"].as_array().map_or(0, Vec::len);
            json!({"target":format!("scene-{}",i+1),"scene": i + 1, "title": scene_label(i, sc), "page": first_scene_page + i, "status": sc["status"], "lines": lines,
                "panels": lines.min(panels), "notes": sc["notes"].as_array().map_or(0, Vec::len), "source": scene_doc_path(&dir, i + 1)})
        })
        .collect();
    let pages = first_scene_page + list.len();
    Ok(json!({"ok": true, "pdf": dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF), "lyrics_page": 1, "chapters": chapters,
        "overview_page": 2, "brief_page": pages}))
}

/// Saved input fingerprint only; polling this never runs a diagnosis.
pub(crate) fn doctor_revision(dir:&Path)->Option<String>{
    use sha2::{Digest,Sha256};
    let bytes=std::fs::read(dir.join("storyboard/movie-board.json")).ok()?;
    let board:Value=serde_json::from_slice(&bytes).ok()?;let mut hash=Sha256::new();hash.update(&bytes);
    let mut paths=vec![dir.join("artifacts/scene_plan.json"),dir.join("artifacts/script.json")];
    if let Some(path)=board["source_transcript"].as_str(){paths.push(path.into());}
    for path in paths{hash.update(path.to_string_lossy().as_bytes());if let Ok(bytes)=std::fs::read(&path){hash.update(bytes);}else{hash.update(b"missing");}}
    Some(format!("{:x}",hash.finalize()))
}

/// Explicit read-only diagnosis of the loaded project's single storyboard.
pub fn cmd_storyboard_doctor(s:&mut Session,p:&Value)->Result<Value>{
    let dir=project_dir(s)?;let manifest=dir.join("storyboard/movie-board.json");
    if !manifest.is_file(){return Err(EngineError::InvalidParams("No saved storyboard for this project".into()));}
    crate::activity::command(s.project_path.as_deref(),"storyboard.doctor",p,"dispatch_running",None);
    let start=std::time::Instant::now();
    let result=(|| -> Result<Value>{
        let script=crate::workspace::workspace_root(s).join("pipeline/movie_board.py");
        let output=run_tool_in(&dir,"python3",&[script.to_string_lossy().into_owned(),"--doctor".into(),manifest.to_string_lossy().into_owned(),"--source".into(),dir.to_string_lossy().into_owned()])?;
        let mut value:Value=serde_json::from_str(&output).map_err(|e|EngineError::InvalidParams(e.to_string()))?;
        value["project"]=json!(s.project_path);value["storyboard_revision"]=json!(doctor_revision(&dir));
        value["assessed_at_unix_ms"]=json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_millis()).unwrap_or(0));
        let report=dir.join(".agent/storyboard-doctor.json");std::fs::create_dir_all(report.parent().ok_or_else(||EngineError::InvalidParams("Missing report folder".into()))?)?;
        value["output"]=json!(report);std::fs::write(&report,serde_json::to_vec_pretty(&value).map_err(|e|EngineError::InvalidParams(e.to_string()))?)?;
        Ok(value)
    })();
    if let Ok(value)=&result{crate::activity::result(s.project_path.as_deref(),"storyboard.doctor",value,start.elapsed().as_millis());}
    crate::activity::command(s.project_path.as_deref(),"storyboard.doctor",p,if result.is_err(){"engine_error"}else{"engine_success"},result.as_ref().err().map(|e|e.to_string()).as_deref());
    result
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
    let mut result=json!({"target":format!("scene-{n}"),"ok": true, "scene": n, "title": scene_label(n - 1, scene), "kind": scene["kind"], "start": scene["start"], "end": scene["end"],
        "status": scene["status"], "notes": scene["notes"], "reference": scene["reference"], "teaching_reference": scene["teaching_reference"],
        "sections": scene["sections"], "lines": lines, "source": scene_doc_path(&dir, n), "page": first_scene_page(&dir) + n - 1});
    if let Ok(text)=std::fs::read_to_string(dir.join("storyboard/movie-board.json")) {
        if let Ok(board)=serde_json::from_str::<Value>(&text) {
            let page=board["first_scene_page"].as_u64().unwrap_or(1)+n as u64-1;
            if let Some(group)=board["page_plan"].as_array().and_then(|pages|pages.iter().filter_map(|p|p.get("group")).find(|g|g["id"]==format!("scene-{n}"))) {
                result["start"]=group["span_start"].clone();result["end"]=group["span_end"].clone();result["kind"]=group["kind"].clone();
                result["panels"]=group["panels"].clone();result["section_label"]=group["section_label"].clone();
                if let Some(label)=group["section_label"].as_str(){result["title"]=json!(format!("{label} / Scene {n:02}"));}
            }
            result["page"]=json!(page); result["deck"]=board["deck"].clone();
            result["preview"]=board["previews"][page as usize-1].clone();result["page_size"]=board["page_size"].clone();
            result["saved_placements"]=json!(board["design"]["placements"].as_array().map(|xs|xs.iter().filter(|p|p["page"].as_u64()==Some(page)).cloned().collect::<Vec<_>>()).unwrap_or_default());
            let design=crate::storyboard_design::inspect(s,&json!({}))?;
            result["text_fields"]=json!(design["text_fields"].as_array().map(|xs|xs.iter().filter(|f|f["page"].as_u64()==Some(page)).cloned().collect::<Vec<_>>()).unwrap_or_default());
            result["draft_placements"]=json!(design["placements"].as_array().map(|xs|xs.iter().filter(|p|p["page"].as_u64()==Some(page)).cloned().collect::<Vec<_>>()).unwrap_or_default());
        }
    }
    Ok(result)
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
    if let Ok(bytes)=std::fs::read(dir.join("storyboard/movie-board.json")) {
        if serde_json::from_slice::<Value>(&bytes).ok().and_then(|b|b["deck"].as_str().map(str::to_owned)).is_some() {
            return crate::storyboard_design::edit_text(s,params);
        }
    }
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
    if params["preserve_document"]==true{return Err(EngineError::InvalidParams("Static document copying was removed. Import scene content normally, then use workshop.build_storyboard template=backlot draft=true to populate the editable PDF template.".into()));}
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
        "workshop.build_storyboard template=backlot draft=true to populate the editable PDF from native transcript words and imported creative fields",
        "Review the rendered draft and mapping report, then storyboard.save_version draft=<returned manifest>"]);
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
                    "prompt": prompts.get(j).cloned().flatten(), "source_tool": "magicstudio"}));
            }
        }
    }
    for a in assets {
        let stem = a.file_stem().and_then(|n| n.to_str()).unwrap_or("asset");
        manifest.push(json!({"id": stem, "type": "image", "scene_id": null, "path": rel(a),
            "prompt": std::fs::read_to_string(a.with_extension("prompt.md")).ok(), "source_tool": "magicstudio"}));
    }
    let write = |name: &str, v: Value| -> Result<()> {
        let text = serde_json::to_string_pretty(&v).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        std::fs::write(art.join(name), text)?;
        Ok(())
    };
    write("scene_plan.json", json!({"scenes": scenes, "metadata": {"total_duration_seconds": total, "source": "magicstudio workshop.build_storyboard"}}))?;
    write("script.json", json!({"title": title, "sections": sections}))?;
    write("asset_manifest.json", json!({"assets": manifest}))?;
    let project_json = dir.join("project.json");
    if !project_json.is_file() {
        let id = dir.file_name().and_then(|n| n.to_str()).unwrap_or("project");
        std::fs::write(&project_json, json!({"version": "1.0", "project_id": id, "title": title, "pipeline_type": "animation"}).to_string())?;
    }
    Ok(())
}


