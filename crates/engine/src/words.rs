//! Words: editable visible lyric animations built from the artist's canonical lines, timed by
//! the hidden transcript reference. The reference (heard words, their order and times) is never
//! edited or exported; mismatches against the artist's words are reported, never substituted.
//! Text and timing are not editable here: `words.save` changes only style, emphasis, motion and
//! each line's design (where its words sit, their lettering and moves, and drawn strokes).
use std::{fs, path::{Path, PathBuf}};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use crate::{EngineError, Result, Session};

pub const ACTIONS: &[&str] = &["none", "wave", "tiny", "swim", "no", "cry", "big", "float"];
const ENTRANCES: &[&str] = &["pop", "fade", "rise", "none"];
const EXITS: &[&str] = &["fade", "drop", "none"];
/// How a designed lyric item moves on its accent word.
const MOVES: &[&str] = &["quiet", "sleep", "settle", "fall", "pop", "rise", "east", "west", "loop"];
/// A designed item's lettering: the section's song font, or a serif or sans for contrast.
const FONTS: &[&str] = &["song", "serif", "sans"];
const LEAD_IN: f64 = 0.4;
const HOLD: f64 = 1.0;

fn invalid(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }

fn norm(text: &str) -> String { text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect() }

fn document_path(dir: &Path) -> PathBuf { dir.join("design/words.json") }

/// The hidden reference: the project transcript's heard words, by position.
fn reference(dir: &Path) -> Result<(PathBuf, String, Vec<Value>)> {
    let path = crate::workshop::transcript_files(dir).into_iter().next()
        .ok_or_else(|| invalid("No transcript in this project; the timing reference is missing"))?;
    let bytes = fs::read(&path)?;
    let doc: Value = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    let words = doc.as_array().or_else(|| doc["words"].as_array()).cloned().unwrap_or_default();
    Ok((path, format!("{:x}", Sha256::digest(&bytes)), words))
}

/// Global alignment of the artist's sung words to the heard words (edit distance on
/// normalized text). Returns, per canonical word, the matched reference index if any.
fn align(canonical: &[String], heard: &[String]) -> Vec<Option<usize>> {
    let (n, m) = (canonical.len(), heard.len());
    let mut cost = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..=n { cost[i][0] = i as u32; }
    for j in 0..=m { cost[0][j] = j as u32; }
    for i in 1..=n {
        for j in 1..=m {
            let sub = cost[i - 1][j - 1] + u32::from(canonical[i - 1] != heard[j - 1]);
            cost[i][j] = sub.min(cost[i - 1][j] + 1).min(cost[i][j - 1] + 1);
        }
    }
    let mut out = vec![None; n];
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let sub = cost[i - 1][j - 1] + u32::from(canonical[i - 1] != heard[j - 1]);
        if cost[i][j] == sub { out[i - 1] = Some(j - 1); i -= 1; j -= 1; }
        else if cost[i][j] == cost[i - 1][j] + 1 { i -= 1; }
        else { j -= 1; }
    }
    out
}

/// Canonical sung words in order, each mapped to the reference with its status and timing.
fn aligned(s: &mut Session) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let lyrics = crate::lyrics::cmd_song_lyrics(s, &json!({}))?;
    if lyrics["source"] != "canonical" { return Err(invalid("No artist lyric sheet; Words needs the artist's canonical lines")); }
    let (ref_path, ref_sha, heard) = reference(&dir)?;
    let mut words = Vec::new();
    for section in lyrics["sections"].as_array().into_iter().flatten() {
        for line in section["lines"].as_array().into_iter().flatten() {
            let sung = line["sung_id"].as_str().or(line["id"].as_str()).unwrap_or_default().to_owned();
            let timed: Vec<Value> = line["words"].as_array().cloned().unwrap_or_default();
            let texts: Vec<String> = if timed.is_empty() {
                line["text"].as_str().unwrap_or_default().split_whitespace().map(str::to_owned).collect()
            } else { timed.iter().filter_map(|w| w["text"].as_str().map(str::to_owned)).collect() };
            for (k, text) in texts.into_iter().enumerate() {
                words.push(json!({"word_id":format!("{sung}/w{k}"),"text":text,"line_id":line["id"],"sung_id":sung,
                    "section":section["section"],"occurrence":section["occurrence"],
                    "forced":timed.get(k).map(|w| json!([w["start"], w["end"]]))}));
            }
        }
    }
    let canonical: Vec<String> = words.iter().map(|w| norm(w["text"].as_str().unwrap_or_default())).collect();
    let heard_norm: Vec<String> = heard.iter().map(|w| norm(w["text"].as_str().unwrap_or_default())).collect();
    let map = align(&canonical, &heard_norm);
    for (word, found) in words.iter_mut().zip(&map) {
        let status = match found { Some(j) if canonical_eq(word, &heard_norm[*j]) => "match", Some(_) => "substituted", None => "missing" };
        let (start, end, source) = match (found, status) {
            (Some(j), "match") => (heard[*j]["start"].clone(), heard[*j]["end"].clone(), "reference"),
            _ => match word["forced"].as_array() { Some(f) => (f[0].clone(), f[1].clone(), "forced_alignment"), None => (Value::Null, Value::Null, "none") },
        };
        word["ref_index"] = json!(found);
        word["heard"] = found.map_or(Value::Null, |j| heard[j]["text"].clone());
        word["status"] = json!(status);
        word["start"] = start; word["end"] = end; word["timing_source"] = json!(source);
    }
    let mut mismatches = Vec::new();
    for w in words.iter().filter(|w| w["status"] != "match") {
        mismatches.push(json!({"word_id":w["word_id"],"line_id":w["line_id"],"canonical":w["text"],"heard":w["heard"],"kind":w["status"]}));
    }
    // Matched words keep the reference timing; where the measured alignment of the artist's text
    // disagrees by more than half a second, say so rather than choose silently.
    let disagreements: Vec<Value> = words.iter().filter(|w| w["timing_source"] == "reference").filter_map(|w| {
        let forced = w["forced"].as_array()?;
        let (rs, fs) = (w["start"].as_f64()?, forced[0].as_f64()?);
        ((rs - fs).abs() > 0.5).then(|| json!({"word_id":w["word_id"],"text":w["text"],"reference":[w["start"], w["end"]],"measured":forced}))
    }).collect();
    for word in &mut words { word.as_object_mut().unwrap().remove("forced"); }
    let heard_used: Vec<usize> = map.iter().flatten().copied().collect();
    let unmatched_heard: Vec<Value> = (0..heard.len()).filter(|j| !heard_used.contains(j))
        .map(|j| json!({"ref_index":j,"heard":heard[j]["text"]})).collect();
    Ok(json!({"reference":{"kind":"transcript","path":ref_path,"sha256":ref_sha,"word_count":heard.len()},
        "canonical":{"sheet":lyrics["sheet"],"original":lyrics["original"]["path"]},
        "words":words,"mismatches":mismatches,"timing_disagreements":disagreements,"unmatched_heard":unmatched_heard}))
}

fn canonical_eq(word: &Value, heard: &str) -> bool { norm(word["text"].as_str().unwrap_or_default()) == heard }

/// `words.align`: the artist's words against the hidden reference. Reports only; writes nothing.
pub fn cmd_words_align(s: &mut Session, _: &Value) -> Result<Value> {
    let mut report = aligned(s)?;
    let total = report["words"].as_array().map_or(0, Vec::len);
    let matched = report["words"].as_array().into_iter().flatten().filter(|w| w["status"] == "match").count();
    report["summary"] = json!({"canonical_words":total,"matched":matched,"mismatched":total - matched,
        "timing_disagreements":report["timing_disagreements"].as_array().map_or(0, Vec::len)});
    Ok(report)
}

/// Value of `name="..."` inside one start tag, matched as a whole attribute name.
fn tag_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let at = tag.find(&needle)? + needle.len();
    tag[at..].split('"').next()
}

/// `words.studio_track`: the lyric word clips already in the HyperFrames video, grouped by the
/// artist's printed sections. A clip belongs to a sung line when its id ends in `<line id>-<n>`;
/// its word id is then `<line id>/w<n>`, the same id Words uses. Reads only.
pub fn cmd_words_studio_track(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let path = dir.join("hyperframes/index.html");
    let html = fs::read_to_string(&path).map_err(|_| invalid("This project has no HyperFrames video yet; run video.init first"))?;
    let mut clips = Vec::new();
    let mut from = 0;
    while let Some(open) = html[from..].find('<').map(|i| i + from) {
        let close = html[open..].find('>').map_or(html.len(), |i| i + open + 1);
        let tag = &html[open..close];
        from = close;
        if !tag_attr(tag, "class").is_some_and(|c| c.split_whitespace().any(|c| c == "lyric-word-clip")) { continue; }
        let (Some(id), Some(start), Some(duration)) = (tag_attr(tag, "id"), tag_attr(tag, "data-start").and_then(|v| v.parse::<f64>().ok()),
            tag_attr(tag, "data-duration").and_then(|v| v.parse::<f64>().ok())) else { continue };
        // The clip's own text, without its markup.
        let inner = html[close..].split("</div>").next().unwrap_or_default();
        let mut text = String::new();
        let mut in_tag = false;
        for c in inner.chars() { match c { '<' => in_tag = true, '>' => in_tag = false, c if !in_tag => text.push(c), _ => {} } }
        clips.push((id.to_owned(), text.trim().to_owned(), start, duration));
    }
    let lyrics = crate::lyrics::cmd_song_lyrics(s, &json!({}))?;
    let mut used = vec![false; clips.len()];
    let mut sections = Vec::new();
    for section in lyrics["sections"].as_array().into_iter().flatten() {
        let mut found = Vec::new();
        for line in section["lines"].as_array().into_iter().flatten() {
            let sung = line["sung_id"].as_str().or(line["id"].as_str()).unwrap_or_default();
            if sung.is_empty() { continue; }
            for (i, (id, text, start, duration)) in clips.iter().enumerate() {
                let Some(n) = id.rsplit_once('-').filter(|(head, _)| *head == sung || head.ends_with(&format!("-{sung}")))
                    .and_then(|(_, n)| n.parse::<usize>().ok()) else { continue };
                used[i] = true;
                found.push(json!({"clip_id":id,"word_id":format!("{sung}/w{n}"),"line_id":line["id"],"text":text,"start":start,"duration":duration}));
            }
        }
        if found.is_empty() { continue; }
        found.sort_by(|a, b| a["start"].as_f64().partial_cmp(&b["start"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
        let start = found.iter().filter_map(|c| c["start"].as_f64()).fold(f64::INFINITY, f64::min);
        let end = found.iter().filter_map(|c| Some(c["start"].as_f64()? + c["duration"].as_f64()?)).fold(0.0, f64::max);
        sections.push(json!({"section":section["section"],"label":section["label"],"occurrence":section["occurrence"],"start":start,"end":end,"clips":found}));
    }
    let unmatched: Vec<&str> = clips.iter().zip(&used).filter(|(_, u)| !**u).map(|((id, ..), _)| id.as_str()).collect();
    Ok(json!({"source":path,"clip_count":clips.len(),"sections":sections,"unmatched":unmatched}))
}

/// The style a lyrics-only view adds: everything in the video is hidden except its lyric word
/// clips, on a plain ground, so the lyrics can be judged on their own.
const LYRICS_ONLY_STYLE: &str = r#"<style data-magicstudio="lyrics-only">
[data-composition-id] * { visibility: hidden !important; }
.lyric-word-clip, .lyric-word-clip *, .lyric-design-clip, .lyric-design-clip * { visibility: visible !important; }
html, body, [data-composition-id] { background: #1b2430 !important; }
</style>"#;

/// The opposite view: the video's scene with its lyric word clips hidden, for designing new lyrics
/// over the picture they will sit in.
const SCENE_ONLY_STYLE: &str = r#"<style data-magicstudio="scene-only">
.lyric-word-clip, .lyric-word-clip *, .lyric-design-clip, .lyric-design-clip * { visibility: hidden !important; }
</style>"#;

/// `words.lyrics_view`: a derived copy of the video for the Design stage: only its lyric word clips
/// (`show: "lyrics"`, the default) or only its scene (`show: "scene"`). Written beside the Words
/// compositions and rewritten only when the video changes; the video itself is never touched.
pub fn cmd_words_lyrics_view(s: &mut Session, p: &Value) -> Result<Value> {
    let (style, name) = match p["show"].as_str().unwrap_or("lyrics") {
        "lyrics" => (LYRICS_ONLY_STYLE, "lyrics-only"),
        "scene" => (SCENE_ONLY_STYLE, "scene-only"),
        other => return Err(invalid(format!("show must be lyrics or scene, not {other}"))),
    };
    let dir = crate::workshop::project_dir(s)?;
    let hf = dir.join("hyperframes");
    let html = fs::read_to_string(hf.join("index.html")).map_err(|_| invalid("This project has no HyperFrames video yet; run video.init first"))?;
    if !html.contains("lyric-word-clip") { return Err(invalid("The video has no lyric word clips (class lyric-word-clip) to show on their own")); }
    let head = html.find("</head>").ok_or_else(|| invalid("The video's index.html has no </head>; nothing written"))?;
    let derived = format!("{}{style}\n{}", &html[..head], &html[head..]);
    let src = format!("compositions/words/{name}.html");
    let file = hf.join(&src);
    let written = fs::read_to_string(&file).ok().as_deref() != Some(derived.as_str());
    if written {
        fs::create_dir_all(file.parent().unwrap())?;
        fs::write(&file, &derived)?;
    }
    Ok(json!({"src":src,"file":file,"written":written,"video_sha256":format!("{:x}", Sha256::digest(html.as_bytes()))}))
}

/// Each track kind and the registry search that finds its effects.
const EFFECT_KINDS: &[(&str, &str)] = &[("text", "word by word text reveal")];

/// `words.effects`: ready-made effects from the HyperFrames registry for each track kind (or one,
/// with `kind`), searched with the project's pinned HyperFrames. Reads only; installs nothing.
pub fn cmd_words_effects(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let hf = dir.join("hyperframes");
    let cli = crate::video_target::hyperframes_cli(p["hyperframes_cli"].as_str(), Some(&hf))?;
    let wanted = p["kind"].as_str();
    if let Some(kind) = wanted && !EFFECT_KINDS.iter().any(|(k, _)| *k == kind) {
        return Err(invalid(format!("kind must be one of {}", EFFECT_KINDS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "))));
    }
    let mut kinds = serde_json::Map::new();
    for (kind, query) in EFFECT_KINDS.iter().filter(|(k, _)| wanted.is_none_or(|w| w == *k)) {
        let out = crate::workspace::run_tool_in(&hf, &cli.to_string_lossy(), &["catalog".into(), "--query".into(), (*query).into(), "--json".into()])?;
        let found: Value = serde_json::from_str(&out).map_err(|e| invalid(format!("HyperFrames catalog answered with something other than JSON: {e}")))?;
        let effects: Vec<Value> = found["results"].as_array().into_iter().flatten()
            .map(|r| json!({"name":r["name"],"type":r["type"],"title":r["title"],"description":r["description"],"tags":r["tags"]})).collect();
        kinds.insert((*kind).into(), json!({"query":query,"tier":found["tier"],"effects":effects}));
    }
    Ok(json!({"source":"hyperframes registry","kinds":kinds}))
}

/// One animation per printed section occurrence; nothing styled or moving until edited.
/// A saved document gains the sung section occurrences it lacks (a repeat added to the sheet later), in
/// sung order; its existing sections and designs stay as they are.
fn with_sung_sections(mut doc: Value, report: &Value) -> Value {
    let fresh = default_document(report)["animations"].as_array().cloned().unwrap_or_default();
    let ids: Vec<Value> = fresh.iter().map(|a| a["animation_id"].clone()).collect();
    if let Some(list) = doc["animations"].as_array_mut() {
        for (k, anim) in fresh.into_iter().enumerate() {
            if list.iter().any(|a| a["animation_id"] == anim["animation_id"]) { continue; }
            let at = list.iter().position(|a| ids.iter().position(|id| *id == a["animation_id"]).is_some_and(|j| j > k)).unwrap_or(list.len());
            list.insert(at, anim);
        }
    }
    doc
}

fn default_document(report: &Value) -> Value {
    let mut animations: Vec<Value> = Vec::new();
    for w in report["words"].as_array().into_iter().flatten() {
        let id = format!("words-{}-{}", w["section"].as_str().unwrap_or("lyrics").replace('_', "-"), w["occurrence"].as_u64().unwrap_or(1));
        if animations.last().is_none_or(|a| a["animation_id"] != json!(id)) {
            animations.push(json!({"animation_id":id,"scope":{"kind":"section","section":w["section"],"occurrence":w["occurrence"],"line_ids":[],"sung_ids":[]},
                "source_revision":0,"origin":null,"style":{},"words":[]}));
        }
        let anim = animations.last_mut().unwrap();
        for (key, field) in [("line_ids", "line_id"), ("sung_ids", "sung_id")] {
            if !anim["scope"][key].as_array().unwrap().contains(&w[field]) { anim["scope"][key].as_array_mut().unwrap().push(w[field].clone()); }
        }
        anim["words"].as_array_mut().unwrap().push(json!({"word_id":w["word_id"],"text":w["text"],"line_id":w["line_id"],"sung_id":w["sung_id"],
            "ref_index":w["ref_index"],"start":w["start"],"end":w["end"],"timing_source":w["timing_source"],
            "emphasis":false,"action":"none","entrance":"pop","exit":"fade"}));
    }
    for anim in &mut animations { timing(anim); }
    json!({"format":"magicstudio.words","version":1,"reference":report["reference"],"canonical":report["canonical"],
        "mismatches":report["mismatches"],"animations":animations})
}

/// Untrimmed authored span. Word times stay song seconds; the composition's local time zero sits
/// at `song_anchor = first_start - lead_in`, so
/// `authored_duration = (last_end + hold) - (first_start - lead_in)`. Never moves the audio.
/// Each word's time in the video's own lyric track (`words.studio_track`), by word id. Empty when
/// the project has no video or no lyric clips; then the reference timing stands.
fn studio_times(s: &mut Session) -> std::collections::HashMap<String, (f64, f64)> {
    let track = cmd_words_studio_track(s, &json!({})).unwrap_or(Value::Null);
    track["sections"].as_array().into_iter().flatten().flat_map(|sec| sec["clips"].as_array().cloned().unwrap_or_default())
        .filter_map(|c| Some((c["word_id"].as_str()?.to_owned(), (c["start"].as_f64()?, c["start"].as_f64()? + c["duration"].as_f64()?)))).collect()
}

/// Printed words the singer didn't sing as written have no time. Each run of them is spread evenly
/// between the timed words around it (marked `interpolated`), so a line still reads and moves in
/// place; a section with no timed word at all stays untimed and is refused by compose.
fn fill_untimed(anim: &mut Value) {
    let Some(words) = anim["words"].as_array_mut() else { return };
    let timed = |w: &Value| w["start"].is_number() && w["end"].is_number();
    let mut i = 0;
    while i < words.len() {
        if timed(&words[i]) { i += 1; continue; }
        let run_end = (i..words.len()).find(|&k| timed(&words[k])).unwrap_or(words.len());
        let before = (0..i).rev().find(|&k| timed(&words[k])).and_then(|k| words[k]["end"].as_f64());
        let after = words.get(run_end).and_then(|w| w["start"].as_f64());
        let n = (run_end - i) as f64;
        let (from, to) = match (before, after) {
            (Some(a), Some(b)) if b > a => (a, b),
            (Some(a), _) => (a, a + 0.35 * n),
            (None, Some(b)) => ((b - 0.35 * n).max(0.0), b),
            (None, None) => return,
        };
        let step = (to - from) / n;
        for (k, w) in words[i..run_end].iter_mut().enumerate() {
            let start = from + step * k as f64;
            w["start"] = json!((start * 1e6).round() / 1e6);
            w["end"] = json!(((start + step) * 1e6).round() / 1e6);
            w["timing_source"] = json!("interpolated");
        }
        i = run_end;
    }
}

fn timing(anim: &mut Value) {
    let times: Vec<(f64, f64)> = anim["words"].as_array().into_iter().flatten()
        .filter_map(|w| Some((w["start"].as_f64()?, w["end"].as_f64()?))).collect();
    if let (Some(first), Some(last)) = (times.iter().map(|t| t.0).reduce(f64::min), times.iter().map(|t| t.1).reduce(f64::max)) {
        let lead_in = LEAD_IN.min(first); // the song clock cannot start before zero
        anim["lead_in"] = json!(lead_in);
        anim["hold"] = json!(HOLD);
        anim["song_anchor"] = json!(first - lead_in);
        anim["authored_duration"] = json!((last + HOLD) - (first - lead_in));
    }
}

fn load(dir: &Path) -> Result<Option<Value>> {
    match fs::read(document_path(dir)) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|e| invalid(format!("design/words.json is not valid JSON: {e}")))?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// `words.captions`: subtitles for the recording grouped by the artist's sheet: one cue per section
/// (each verse or chorus as sung; the default, so storyboard scenes follow the song's sections) or per
/// line (`by: "line"`). Every heard word appears once, in order, in its own wording and time (the
/// transcript the workshop imports it with); a heard word aligned to no printed word stays with the
/// group it is sung in.
/// The artist's printed words with their sung timing, one caption group per printed line (a line over
/// six words shows in two halves). A printed word the singer left out is placed between its sung
/// neighbours; heard words outside the printed lines are not shown.
fn printed_caption_words(aligned: &Value) -> (Vec<Value>, Vec<[usize; 2]>) {
    let src: Vec<&Value> = aligned["words"].as_array().into_iter().flatten().collect();
    let mut words: Vec<Value> = src.iter().map(|w| json!({"text": w["text"], "start": w["start"], "end": w["end"], "word_id": w["word_id"], "line": w["sung_id"]})).collect();
    let mut i = 0;
    while i < words.len() {
        if words[i]["start"].is_f64() { i += 1; continue; }
        let j = (i..words.len()).find(|&k| words[k]["start"].is_f64()).unwrap_or(words.len());
        let before = if i > 0 { words[i - 1]["end"].as_f64().unwrap_or(0.0) } else { 0.0 };
        let next = words.get(j).and_then(|w| w["start"].as_f64()).unwrap_or(before + 0.3 * (j - i) as f64);
        let prev = before.max(next - 0.35 * (j - i) as f64);  // a skipped word sits just before its sung neighbour
        let step = (next - prev).max(0.0) / (j - i) as f64;
        for (n, w) in words[i..j].iter_mut().enumerate() {
            w["start"] = json!(prev + step * n as f64);
            w["end"] = json!(prev + step * (n + 1) as f64);
        }
        i = j;
    }
    let mut groups = Vec::new();
    let mut start = 0;
    for k in 0..src.len() {
        let line_ends = k + 1 == src.len() || src[k + 1]["sung_id"] != src[k]["sung_id"] || src[k + 1]["occurrence"] != src[k]["occurrence"];
        if line_ends {
            let n = k + 1 - start;
            if n > 6 { groups.push([start, start + n.div_ceil(2) - 1]); groups.push([start + n.div_ceil(2), k]); } else { groups.push([start, k]); }
            start = k + 1;
        }
    }
    (words, groups)
}

/// `words.captions to=video`: the video's caption track shows the artist's printed lines for every section
/// not designed in the video. The earlier caption source is kept beside it as `captions.vN.html`.
fn captions_to_video(s: &Session, dir: &Path, aligned: &Value) -> Result<Value> {
    // A section already designed in the video shows its own lyrics; the captions cover only the rest.
    let index = dir.join("hyperframes/index.html");
    let host = fs::read_to_string(&index).unwrap_or_default();
    let designed: std::collections::BTreeSet<&str> = host.split("data-magicstudio-words=\"").skip(1).filter_map(|r| r.split('"').next()).collect();
    let open: Vec<Value> = aligned["words"].as_array().into_iter().flatten().filter(|w| {
        let id = format!("words-{}-{}", w["section"].as_str().unwrap_or("lyrics").replace('_', "-"), w["occurrence"].as_u64().unwrap_or(1));
        !designed.contains(id.as_str())
    }).cloned().collect();
    let (words, groups) = printed_caption_words(&json!({"words": open}));
    let comp = dir.join("hyperframes/compositions");
    let dest = comp.join("captions.html");
    if words.is_empty() {  // every section is designed: the caption track leaves the video, its source stays
        let marker = "data-composition-src=\"compositions/captions.html\"";
        let kept: Vec<&str> = host.lines().filter(|l| !l.contains(marker)).collect();
        if kept.len() != host.lines().count() { fs::write(&index, kept.join("\n") + "\n")?; }
        return Ok(json!({"ok":true,"caption_track":"removed: every section is designed","source":dest,"designed":designed}));
    }
    let kept = if dest.is_file() {
        let version = (1..).map(|n| comp.join(format!("captions.v{n}.html"))).find(|p| !p.exists()).unwrap();
        fs::copy(&dest, &version)?;
        Some(version)
    } else { None };
    let duration = crate::video_target::probe_audio_duration(s.song_audio.as_deref())
        .max(words.iter().filter_map(|w| w["end"].as_f64()).fold(0.0, f64::max));
    crate::video_target::write_caption_source(&dest, &words, &groups, duration)?;
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&words).map_err(|e| invalid(e.to_string()))?));
    let receipt = dir.join("hyperframes/.magicstudio-captions.json");
    fs::write(&receipt, serde_json::to_vec_pretty(&json!({"source_kind":"artist_lines","input_sha256":digest,"word_count":words.len(),
        "groups":groups.len(),"duration":duration,"previous_source":kept,"grouping":"one group per printed line; long lines in two halves"}))
        .map_err(|e| invalid(e.to_string()))?)?;
    Ok(json!({"ok":true,"source":dest,"previous_source":kept,"provenance":receipt,"words":words.len(),"groups":groups.len(),
        "first":words.first(),"last":words.last(),"next":"hyperframes snapshot <project>/hyperframes --at <t> to check, then render"}))
}

pub fn cmd_words_captions(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let aligned = aligned(s)?;
    let (_, _, heard) = reference(&dir)?;
    let by_line = match p["by"].as_str().unwrap_or("section") { "section" => false, "line" => true, other => return Err(invalid(format!("by must be section or line, not {other}"))) };
    let line_of: std::collections::HashMap<u64, String> = aligned["words"].as_array().into_iter().flatten()
        .filter_map(|w| Some((w["ref_index"].as_u64()?, if by_line { w["sung_id"].as_str()?.to_owned() }
            else { format!("{}#{}", w["section"].as_str()?, w["occurrence"].as_u64()?) }))).collect();
    let mut cues: Vec<(String, Vec<&Value>)> = Vec::new();
    for (i, w) in heard.iter().enumerate() {
        let line = line_of.get(&(i as u64)).cloned().or_else(|| cues.last().map(|(id, _)| id.clone())).unwrap_or_default();
        if cues.last().is_none_or(|(id, _)| *id != line) { cues.push((line, Vec::new())); }
        cues.last_mut().unwrap().1.push(w);
    }
    let clock = |t: f64| { let ms = (t * 1000.0).round() as u64; format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000) };
    let mut srt = String::new();
    for (n, (_, words)) in cues.iter().enumerate() {
        let start = words.iter().filter_map(|w| w["start"].as_f64()).reduce(f64::min).unwrap_or(0.0);
        let end = words.iter().filter_map(|w| w["end"].as_f64()).reduce(f64::max).unwrap_or(start);
        let text = words.iter().filter_map(|w| w["text"].as_str().or_else(|| w["word"].as_str())).collect::<Vec<_>>().join(" ");
        srt.push_str(&format!("{}\n{} --> {}\n{text}\n\n", n + 1, clock(start), clock(end.max(start + 0.1))));
    }
    if p["to"] == "video" { return captions_to_video(s, &dir, &aligned); }
    let out = p["out"].as_str().map(PathBuf::from).unwrap_or_else(|| dir.join("lyrics.captions.srt"));
    if let Some(parent) = out.parent() { fs::create_dir_all(parent)?; }
    fs::write(&out, &srt)?;
    Ok(json!({"ok":true,"path":out,"cues":cues.len(),"words":heard.len()}))
}

/// `words.read`: the saved Words document, or the unsaved default built from the artist's sheet.
/// Reports when the hidden reference changed since the document was saved.
pub fn cmd_words_read(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let (mut doc, saved) = match load(&dir)? { Some(doc) => (with_sung_sections(doc, &aligned(s)?), true), None => (default_document(&aligned(s)?), false) };
    let (_, current_sha, _) = reference(&dir)?;
    let stale = saved && doc["reference"]["sha256"].as_str() != Some(current_sha.as_str());
    if let Some(id) = p["animation_id"].as_str() {
        let anim = doc["animations"].as_array().and_then(|a| a.iter().find(|x| x["animation_id"] == id)).cloned()
            .ok_or_else(|| invalid(format!("No Words animation {id}")))?;
        doc["animations"] = json!([anim]);
    }
    let video = fs::read_to_string(dir.join("hyperframes/index.html")).unwrap_or_default();
    let in_video: Vec<&str> = video.split("data-magicstudio-words=\"").skip(1).filter_map(|rest| rest.split('"').next()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    Ok(json!({"ok":true,"saved":saved,"reference_changed":stale,"path":document_path(&dir),"document":doc,"in_video":in_video,"fonts":video_fonts(&dir.join("hyperframes"), &video)}))
}

/// The fonts the video declares (`@font-face` family, file and weight), so the designer can letter
/// in the video's own type.
fn video_fonts(hf: &Path, video: &str) -> Vec<Value> {
    video.split("@font-face").skip(1).filter_map(|rule| {
        let rule = rule.split('}').next()?;
        let family = rule.split("font-family:").nth(1)?.split([';', '}']).next()?.trim().trim_matches(['"', '\'']).to_owned();
        let url = rule.split("url(").nth(1)?.split(')').next()?.trim().trim_matches(['"', '\'']);
        let weight = rule.split("font-weight:").nth(1).and_then(|w| w.split([';', '}']).next()).map(str::trim).unwrap_or("400");
        Some(json!({"family":family,"path":hf.join(url),"weight":weight}))
    }).collect()
}

fn one_of(value: &Value, allowed: &[&str], what: &str) -> Result<Value> {
    value.as_str().filter(|v| allowed.contains(v)).map(|v| json!(v))
        .ok_or_else(|| invalid(format!("{what} must be one of {}", allowed.join(", "))))
}

/// `words.save`: apply style/emphasis/motion edits to one animation. `expected_source_revision`
/// must match the stored revision, or nothing is written and the conflict is returned.
pub fn cmd_words_save(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let id = p["animation_id"].as_str().ok_or_else(|| invalid("animation_id is required"))?;
    let expected = p["expected_source_revision"].as_u64().ok_or_else(|| invalid("expected_source_revision is required"))?;
    let mut doc = match load(&dir)? { Some(doc) => with_sung_sections(doc, &aligned(s)?), None => default_document(&aligned(s)?) };
    let anim = doc["animations"].as_array_mut().and_then(|a| a.iter_mut().find(|x| x["animation_id"] == id))
        .ok_or_else(|| invalid(format!("No Words animation {id}")))?;
    let current = anim["source_revision"].as_u64().unwrap_or(0);
    if current != expected {
        return Ok(json!({"ok":false,"conflict":true,"animation_id":id,"source_revision":current,"expected_source_revision":expected,
            "reason":"The animation changed since you read it; reload and reapply your edit"}));
    }
    if let Some(style) = p.get("style").filter(|v| !v.is_null()) {
        let map = style.as_object().ok_or_else(|| invalid("style must be an object"))?;
        for (key, value) in map {
            match key.as_str() {
                "font" | "color" | "emphasis_color" | "outline_color" | "background" if value.is_string() => {}
                "size" if value.as_f64().is_some_and(|v| (40.0..=320.0).contains(&v)) => {}
                _ => return Err(invalid(format!("style.{key} is not an editable style field or has an invalid value"))),
            }
            anim["style"][key] = value.clone();
        }
    }
    for edit in p["words"].as_array().into_iter().flatten() {
        let word_id = edit["word_id"].as_str().ok_or_else(|| invalid("each word edit needs word_id"))?;
        let word = anim["words"].as_array_mut().and_then(|w| w.iter_mut().find(|w| w["word_id"] == word_id))
            .ok_or_else(|| invalid(format!("{word_id} is not in {id}")))?;
        for (key, value) in edit.as_object().into_iter().flatten() {
            let checked = match key.as_str() {
                "word_id" => continue,
                "emphasis" => value.as_bool().map(|b| json!(b)).ok_or_else(|| invalid("emphasis must be true or false"))?,
                "action" => one_of(value, ACTIONS, "action")?,
                "entrance" => one_of(value, ENTRANCES, "entrance")?,
                "exit" => one_of(value, EXITS, "exit")?,
                other => return Err(invalid(format!("{other} is not editable here; text and timing come from the artist's sheet and the reference"))),
            };
            word[key] = checked;
        }
    }
    if let Some(designs) = p.get("design").filter(|v| !v.is_null()) {
        for (line_id, design) in designs.as_object().ok_or_else(|| invalid("design must be an object of line_id: design"))? {
            let words: Vec<Value> = anim["words"].as_array().into_iter().flatten().filter(|w| w["line_id"] == json!(line_id)).cloned().collect();
            if words.is_empty() { return Err(invalid(format!("{line_id} is not a line of {id}"))); }
            anim["design"][line_id] = if design.is_null() { Value::Null } else { line_design(&words, design)? };
        }
    }
    if let Some(effects) = p.get("effects").filter(|v| !v.is_null()) {
        for (track, choice) in effects.as_object().ok_or_else(|| invalid("effects must be an object of track: effect"))? {
            if track != "text" { return Err(invalid(format!("effects.{track}: only the text track takes registry effects so far"))); }
            anim["effects"][track] = if choice.is_null() { Value::Null } else { text_effect(&dir.join("hyperframes"), choice)? };
        }
    }
    if let Some(k) = p.get("kinetic") {
        anim["kinetic"] = if k.is_null() { Value::Null } else { kinetic_design(&anim["words"], k)? };
    }
    anim["source_revision"] = json!(current + 1);
    anim["origin"] = p.get("origin").filter(|v| v.is_string()).cloned().unwrap_or(json!("magicstudio"));
    let saved = anim.clone();
    let path = document_path(&dir);
    fs::create_dir_all(path.parent().unwrap())?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&doc).map_err(|e| invalid(e.to_string()))?)?;
    fs::rename(&temporary, &path)?;
    Ok(json!({"ok":true,"conflict":false,"path":path,"animation":saved}))
}

/// Check one line's design. Items group the line's own words, all of them, in sung order; each
/// item's text is joined from those words, so lyrics never change here. Strokes are SVG path data.
fn line_design(words: &[Value], design: &Value) -> Result<Value> {
    let num = |v: &Value, what: &str, lo: f64, hi: f64| v.as_f64().filter(|n| (lo..=hi).contains(n)).map(|n| json!(n))
        .ok_or_else(|| invalid(format!("{what} must be a number from {lo} to {hi}")));
    let colour = |v: &Value| v.as_str().filter(|c| c.len() <= 32 && !c.contains(['<', '>', '"', ';'])).map(|c| json!(c))
        .ok_or_else(|| invalid("colour must be a short CSS colour"));
    let mut placed: Vec<&str> = Vec::new();
    let mut items = Vec::new();
    for item in design["items"].as_array().ok_or_else(|| invalid("a design needs items"))? {
        let ids: Vec<&str> = item["word_ids"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if ids.is_empty() { return Err(invalid("each item needs word_ids")); }
        let text: Vec<&str> = ids.iter().map(|id| words.iter().find(|w| w["word_id"] == json!(id)).and_then(|w| w["text"].as_str())
            .ok_or_else(|| invalid(format!("{id} is not a word of this line")))).collect::<Result<_>>()?;
        let accent = match &item["accent"] { Value::Null => Value::Null,
            a => a.as_str().filter(|a| ids.contains(a)).map(|a| json!(a)).ok_or_else(|| invalid("accent must be one of the item's words"))? };
        placed.extend(&ids);
        items.push(json!({"word_ids":ids,"text":text.join(" "),"x":num(&item["x"],"x",-2000.0,6000.0)?,"y":num(&item["y"],"y",-2000.0,6000.0)?,
            "size":num(&item["size"],"size",8.0,600.0)?,"weight":item["weight"].as_u64().filter(|w| [400, 700].contains(w)).ok_or_else(|| invalid("weight must be 400 or 700"))?,
            "italic":item["italic"].as_bool().ok_or_else(|| invalid("italic must be true or false"))?,"color":colour(&item["color"])?,
            "font":one_of(item.get("font").unwrap_or(&json!("song")),FONTS,"font")?,
            "outline":if item["outline"].is_null() { Value::Null } else { colour(&item["outline"])? },
            "rotate":num(&item["rotate"],"rotate",-45.0,45.0)?,"motion":one_of(&item["motion"],MOVES,"motion")?,"accent":accent}));
    }
    let line: Vec<&str> = words.iter().filter_map(|w| w["word_id"].as_str()).collect();
    if placed != line { return Err(invalid("the items must hold every word of the line once, in sung order")); }
    let mut draws = Vec::new();
    for draw in design["draws"].as_array().into_iter().flatten() {
        let mut paths = Vec::new();
        for path in draw["paths"].as_array().ok_or_else(|| invalid("a drawing needs paths"))? {
            let d = path["d"].as_str().filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit() || " ,.-eMmLlHhVvCcSsQqTtAaZz".contains(c)))
                .ok_or_else(|| invalid("a stroke's d must be SVG path data"))?;
            let (from, to) = (num(&path["from"],"from",0.0,600.0)?, num(&path["to"],"to",0.0,600.0)?);
            if from.as_f64() > to.as_f64() { return Err(invalid("a stroke finishes drawing after it starts")); }
            paths.push(json!({"d":d,"stroke":colour(&path["stroke"])?,"width":num(&path["width"],"width",0.5,60.0)?,"from":from,"to":to}));
        }
        let view_box = draw["view_box"].as_str().filter(|v| v.split_whitespace().count() == 4 && v.split_whitespace().all(|n| n.parse::<f64>().is_ok()))
            .ok_or_else(|| invalid("view_box must be four numbers"))?;
        draws.push(json!({"name":draw["name"].as_str().unwrap_or("Drawing"),"view_box":view_box,"x":num(&draw["x"],"x",-2000.0,6000.0)?,
            "y":num(&draw["y"],"y",-2000.0,6000.0)?,"w":num(&draw["w"],"w",1.0,6000.0)?,"h":num(&draw["h"],"h",1.0,6000.0)?,"paths":paths}));
    }
    Ok(json!({"items":items,"draws":draws}))
}

/// A registry component's declared variables, read from its `data-composition-variables`.
fn declared_variables(component: &str) -> Vec<Value> {
    let Some(at) = component.find("data-composition-variables='") else { return Vec::new() };
    let rest = &component[at + "data-composition-variables='".len()..];
    rest.split('\'').next().and_then(|raw| serde_json::from_str(raw).ok()).unwrap_or_default()
}

/// Check a text-track effect against the installed component: it must take the line as a string
/// content variable; `cues` (word landings) is used when it declares one. Only declared variables
/// may be set, and the text and cues are always the song's, never stored.
fn text_effect(hf: &Path, choice: &Value) -> Result<Value> {
    let name = choice["name"].as_str().filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .ok_or_else(|| invalid("effect name must be letters, digits or '-'"))?;
    let src = format!("compositions/components/{name}.html");
    let component = fs::read_to_string(hf.join(&src)).map_err(|_| invalid(format!("{name} is not installed in this project; run words.effect_install first")))?;
    let declared = declared_variables(&component);
    let text = declared.iter().find(|v| v["type"] == "string" && v["role"] == "content").and_then(|v| v["id"].as_str())
        .ok_or_else(|| invalid(format!("{name} takes no text, so it cannot show a lyric line")))?;
    let cues = declared.iter().any(|v| v["id"] == "cues");
    let mut variables = serde_json::Map::new();
    for (key, value) in choice["variables"].as_object().into_iter().flatten() {
        if key == text || key == "cues" || !declared.iter().any(|v| v["id"] == json!(key)) {
            return Err(invalid(format!("{name} has no settable variable {key}")));
        }
        variables.insert(key.clone(), value.clone());
    }
    Ok(json!({"name":name,"src":src,"text_variable":text,"cues":cues,"variables":variables}))
}

/// `words.effect_install`: install one HyperFrames registry effect into the project with the
/// pinned HyperFrames (`hyperframes add`). Needs the network; already-installed effects are left as they are.
pub fn cmd_words_effect_install(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let hf = dir.join("hyperframes");
    let name = p["name"].as_str().filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .ok_or_else(|| invalid("name must be a registry effect name (letters, digits or '-')"))?;
    let src = format!("compositions/components/{name}.html");
    if hf.join(&src).is_file() { return Ok(json!({"name":name,"src":src,"installed":false,"already":true})); }
    let cli = crate::video_target::hyperframes_cli(p["hyperframes_cli"].as_str(), Some(&hf))?;
    let out = crate::workspace::run_tool_in(&hf, &cli.to_string_lossy(), &["add".into(), name.into(), "--json".into()])?;
    if !hf.join(&src).is_file() { return Err(invalid(format!("hyperframes add {name} did not write {src}: {}", out.trim()))); }
    Ok(json!({"name":name,"src":src,"installed":true,"already":false,"receipt":serde_json::from_str::<Value>(&out).unwrap_or(json!(out.trim()))}))
}

const KINETIC_LIBRARY: &str = include_str!("../../../pipeline/templates/kinetic-library.json");

/// A section's kinetic design, checked against the kinetic library: layout, entrance for the other words,
/// an act per meaning word (by word id), the band, a seed and the lettering.
fn kinetic_design(words: &Value, k: &Value) -> Result<Value> {
    let lib: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let named = |kind: &str, v: &Value| v.as_str().filter(|n| lib[kind][*n].is_object()).map(|n| json!(n))
        .ok_or_else(|| invalid(format!("kinetic: {v} is not one of the library's {kind}")));
    let ids: Vec<&Value> = words.as_array().into_iter().flatten().map(|w| &w["word_id"]).collect();
    let mut acts = serde_json::Map::new();
    for (word, act) in k["acts"].as_object().into_iter().flatten() {
        if !ids.contains(&&json!(word)) { return Err(invalid(format!("kinetic.acts: {word} is not a word of this section"))); }
        acts.insert(word.clone(), named("acts", act)?);
    }
    let hex = |v: &Value| v.as_str().filter(|c| c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())).map(|c| json!(c));
    let st = &k["style"];
    let style = json!({"font":st["font"].as_str().filter(|f| f.len() <= 40 && f.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-')).unwrap_or("sans"),
        "hero_font":st["hero_font"].as_str().filter(|f| f.len() <= 40 && f.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-')),
        "weight":st["weight"].as_u64().filter(|w| (100..=900).contains(w)).unwrap_or(700),"hero_weight":st["hero_weight"].as_u64().filter(|w| (100..=900).contains(w)).unwrap_or(700),
        "hero_italic":st["hero_italic"].as_bool().unwrap_or(false),
        "size":st["size"].as_f64().filter(|v| (20.0..=300.0).contains(v)).unwrap_or(64.0),"hero_size":st["hero_size"].as_f64().filter(|v| (20.0..=400.0).contains(v)).unwrap_or(110.0),
        "color":hex(&st["color"]).unwrap_or(json!("#173F49")),"hero_color":hex(&st["hero_color"]).unwrap_or(json!("#B8442A")),"outline":hex(&st["outline"])});
    Ok(json!({"layout":named("layouts",&k["layout"])?,"entrance":named("entrances",&k["entrance"])?,"acts":acts,
        "band":if k["band"] == "bottom" { "bottom" } else { "top" },"seed":k["seed"].as_u64().unwrap_or(1),"style":style}))
}

/// The section as kinetic text: every word at its local time, laid out and performed in the browser.
fn kinetic_composition(anim: &Value, composition_id: &str, width: u64, height: u64, duration: f64, words: &[Value]) -> Result<String> {
    let lib: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let data = json!({"width":width,"height":height,"duration":duration,"words":words,"kinetic":anim["kinetic"],"style":anim["kinetic"]["style"],"library":lib});
    Ok(include_str!("../../../pipeline/templates/words-kinetic.html").replace("__COMPOSITION_ID__", composition_id)
        .replace("__WIDTH__", &width.to_string()).replace("__HEIGHT__", &height.to_string()).replace("__DURATION__", &format!("{duration:.6}"))
        .replace("__DATA__", &serde_json::to_string(&data).map_err(|e| invalid(e.to_string()))?.replace("</", "<\\/")))
}

/// The section composed with its text effect: one mount per sung line, at the line's time, with
/// the line's text and (when the effect takes them) each word's landing as cues from the mount start.
fn effect_composition(anim: &Value, composition_id: &str, width: u64, height: u64, duration: f64, local: &dyn Fn(f64) -> f64) -> String {
    let effect = &anim["effects"]["text"];
    let name = effect["name"].as_str().unwrap_or_default();
    let mut lines: Vec<(String, Vec<&Value>)> = Vec::new();
    for w in anim["words"].as_array().into_iter().flatten() {
        let line = w["line_id"].as_str().unwrap_or_default().to_owned();
        if lines.last().is_none_or(|(id, _)| *id != line) { lines.push((line, Vec::new())); }
        lines.last_mut().unwrap().1.push(w);
    }
    let attr = |v: &str| v.replace('&', "&amp;").replace('\'', "&#39;").replace('<', "&lt;");
    let mut mounts = String::new();
    for (i, (_, words)) in lines.iter().enumerate() {
        let start = local(words[0]["start"].as_f64().unwrap_or(0.0));
        let last = local(words.last().unwrap()["end"].as_f64().unwrap_or(0.0)) + HOLD;
        let end = lines.get(i + 1).map_or(last, |(_, next)| last.min(local(next[0]["start"].as_f64().unwrap_or(last))));
        let text = words.iter().filter_map(|w| w["text"].as_str()).collect::<Vec<_>>().join(" ");
        let mut values = effect["variables"].as_object().cloned().unwrap_or_default();
        values.insert(effect["text_variable"].as_str().unwrap_or("text").into(), json!(text));
        if effect["cues"] == true {
            let cues: Vec<String> = words.iter().filter_map(|w| w["start"].as_f64()).map(|t| format!("{:.3}", (local(t) - start).max(0.0))).collect();
            values.insert("cues".into(), json!(cues.join(",")));
        }
        mounts.push_str(&format!("  <div class=\"line\" data-composition-id=\"{name}\" data-composition-src=\"../components/{name}.html\" data-start=\"{start:.6}\" data-duration=\"{:.6}\" data-variable-values='{}'></div>\n",
            (end - start).max(0.1), attr(&Value::Object(values).to_string())));
    }
    format!("<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"UTF-8\" /></head>\n<body>\n<!-- Derived by `mc exec words.compose` from design/words.json with the {name} effect; regenerate rather than hand-edit. -->\n\
<div data-composition-id=\"{composition_id}\" data-width=\"{width}\" data-height=\"{height}\" data-duration=\"{duration:.6}\" class=\"words-track\">\n\
  <style>[data-composition-id=\"{composition_id}\"] {{ position: relative; width: {width}px; height: {height}px; overflow: hidden; }}\n\
  [data-composition-id=\"{composition_id}\"] > .line {{ position: absolute; left: 6%; right: 6%; top: 62%; height: 26%; }}</style>\n{mounts}</div>\n</body>\n</html>\n")
}

/// The section composed from its line designs: each designed line's items, moves and strokes on the
/// section's local clock, and undesigned lines as a plain row, so the whole section still reads.
fn design_composition(anim: &Value, composition_id: &str, width: u64, height: u64, duration: f64, local: &dyn Fn(f64) -> f64) -> Result<String> {
    let mut lines: Vec<(String, Vec<&Value>)> = Vec::new();
    for w in anim["words"].as_array().into_iter().flatten() {
        let line = w["line_id"].as_str().unwrap_or_default().to_owned();
        if lines.last().is_none_or(|(id, _)| *id != line) { lines.push((line, Vec::new())); }
        lines.last_mut().unwrap().1.push(w);
    }
    let start_of = |w: &Value| local(w["start"].as_f64().unwrap_or(0.0));
    let mut out = Vec::new();
    for (i, (line_id, words)) in lines.iter().enumerate() {
        let start = start_of(words[0]);
        let end = lines.get(i + 1).map_or(local(words.last().unwrap()["end"].as_f64().unwrap_or(0.0)) + HOLD, |(_, next)| start_of(next[0]));
        let design = &anim["design"][line_id];
        if !design.is_object() {
            let text = words.iter().filter_map(|w| w["text"].as_str()).collect::<Vec<_>>().join(" ");
            out.push(json!({"start":start,"end":end,"plain":true,"text":text}));
            continue;
        }
        let items: Vec<Value> = design["items"].as_array().into_iter().flatten().map(|it| {
            let accent = it["accent"].as_str().and_then(|id| words.iter().find(|w| w["word_id"] == json!(id))).map_or(start, |w| start_of(w));
            let mut it = it.clone();
            it["accent"] = json!(accent);
            it
        }).collect();
        out.push(json!({"start":start,"end":end,"items":items,"draws":design["draws"]}));
    }
    let style = &anim["style"];
    let data = json!({"composition_id":composition_id,"duration":duration,"font":style["font"],"size":style["size"].as_f64().unwrap_or(96.0),
        "color":style["color"].as_str().unwrap_or("#fff6e0"),"lines":out});
    Ok(include_str!("../../../pipeline/templates/words-design.html")
        .replace("__COMPOSITION_ID__", composition_id).replace("__WIDTH__", &width.to_string()).replace("__HEIGHT__", &height.to_string())
        .replace("__DURATION__", &format!("{duration:.6}"))
        .replace("__DATA__", &serde_json::to_string(&data).map_err(|e| invalid(e.to_string()))?.replace("</", "<\\/")))
}

/// Take one section's placement back out of the video: its mount and the rule hiding its lyric clips.
fn without_placement(html: &str, id: &str) -> String {
    let mark = format!("data-magicstudio-words=\"{id}\"");
    let mut out = html.to_owned();
    for (open, close) in [("<style ", "</style>\n"), ("<div ", "</div>\n")] {
        while let Some(at) = out.find(&mark) {
            let Some(begin) = out[..at].rfind(open) else { break };
            if out[begin..at].contains('>') { break; }
            let Some(end) = out[at..].find(close).map(|e| at + e + close.len()) else { break };
            out.replace_range(begin..end, "");
        }
    }
    out
}

/// `words.apply`: put one section's designed lyrics into the video (`use`, the default) or take
/// them back out (`use: false`). Using composes the section, mounts it at its song time and hides
/// the section's original lyric clips; restoring leaves the video exactly as it was before.
/// `words.design`: every sung line designed from the project's lyric recipe (`lyrics-design.json`: palette,
/// band, hero words and how they act), laid out by `pipeline/lyrics_layout.py`, saved per section and,
/// unless `apply=false`, used in the video. The recipe holds the art direction; nothing here is per song.
pub fn cmd_words_design(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let recipe = p["recipe"].as_str().map(PathBuf::from).unwrap_or_else(|| dir.join("lyrics-design.json"));
    if !recipe.is_file() { return Err(invalid(format!("No lyric recipe at {}; write one (see pipeline/lyrics_layout.py)", recipe.display()))); }
    let doc = cmd_words_read(s, &json!({}))?["document"].clone();
    let times: std::collections::HashMap<String, Value> = cmd_words_studio_track(s, &json!({}))?["sections"].as_array().into_iter().flatten()
        .flat_map(|sec| sec["clips"].as_array().cloned().unwrap_or_default())
        .filter_map(|c| Some((c["word_id"].as_str()?.to_owned(), c["start"].clone()))).collect();
    let animations: Vec<Value> = doc["animations"].as_array().into_iter().flatten().map(|a| json!({"animation_id":a["animation_id"],"section":a["scope"]["section"],
        "words":a["words"].as_array().into_iter().flatten().map(|w| { let mut w = w.clone();
            if let Some(t) = w["word_id"].as_str().and_then(|id| times.get(id)) { w["start"] = t.clone(); } w }).collect::<Vec<_>>()})).collect();
    let input = dir.join(".agent/words-design-input.json");
    if let Some(parent) = input.parent() { fs::create_dir_all(parent)?; }
    fs::write(&input, serde_json::to_vec(&json!({"animations":animations})).map_err(|e| invalid(e.to_string()))?)?;
    let root = crate::workspace::workspace_root(s);
    let out = crate::workspace::run_tool_in(&root, "python3", &[root.join("pipeline/lyrics_layout.py").to_string_lossy().into_owned(),
        "--recipe".into(), recipe.to_string_lossy().into_owned(), "--input".into(), input.to_string_lossy().into_owned()])?;
    let designs: Value = out.lines().rev().find_map(|l| serde_json::from_str::<Value>(l).ok()).map(|v| v["designs"].clone())
        .ok_or_else(|| EngineError::CommandFailed("words.design".into(), "lyrics_layout.py returned no designs".into()))?;
    let recipe_doc: Value = serde_json::from_slice(&fs::read(&recipe)?).map_err(|e| invalid(format!("lyric recipe: {e}")))?;
    let style = recipe_doc["font"].as_str().map(|f| json!({"font": f})).unwrap_or(json!({}));
    // Kinetic text when the recipe has a `kinetic` block: meaning words act, the rest arrive by the section's
    // entrance, each section in its own layout (chosen here or varied by seed through the recipe's lists).
    let kin = &recipe_doc["kinetic"];
    let pick = |list: &Value, key: &str, k: usize| p[key].as_str().map(|v| json!(v)).or_else(|| list.as_array().filter(|l| !l.is_empty())
        .map(|l| l[(p["seed"].as_u64().unwrap_or(0) as usize + k * 7) % l.len()].clone()));
    let only = p["section"].as_str();
    let mut sections = Vec::new();
    for (k, a) in doc["animations"].as_array().into_iter().flatten().enumerate() {
        let id = a["animation_id"].as_str().unwrap_or_default();
        if only.is_some_and(|o| o != id) { continue; }
        let mut save = json!({"animation_id":id,"expected_source_revision":a["source_revision"],"origin":"words.design","style":style,"design":designs[id]});
        if kin.is_object() && p["kinetic"] != false {
            let acts: serde_json::Map<String, Value> = a["words"].as_array().into_iter().flatten().filter_map(|w| {
                let key: String = w["text"].as_str()?.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).collect();
                kin["acts"][&key].as_str().map(|act| (w["word_id"].as_str().unwrap_or_default().to_owned(), json!(act)))
            }).collect();
            save["kinetic"] = json!({"layout":pick(&kin["layouts"], "layout", k).unwrap_or(json!("karaoke")),"entrance":pick(&kin["entrances"], "entrance", k).unwrap_or(json!("fade-up")),
                "acts":acts,"band":recipe_doc["band"],"seed":p["seed"].as_u64().unwrap_or(1) + k as u64,"style":kin["style"]});
        }
        let saved = cmd_words_save(s, &save)?;
        let applied = if p["apply"] == false { Value::Null } else { cmd_words_apply(s, &json!({"animation_id":id}))?["in_video"].clone() };
        sections.push(json!({"animation_id":id,"lines":designs[id].as_object().map_or(0, |m| m.len()),"revision":saved["animation"]["source_revision"],"in_video":applied,
            "kinetic":saved["animation"]["kinetic"].as_object().map(|k| json!({"layout":k["layout"],"entrance":k["entrance"],"acts":k["acts"].as_object().map_or(0, |a| a.len())}))}));
    }
    // The video's caption track keeps only the sections without a design, so no line shows twice.
    let captions = if p["apply"] != false && dir.join("hyperframes/compositions/captions.html").is_file() {
        let aligned = aligned(s)?; captions_to_video(s, &dir, &aligned)?
    } else { Value::Null };
    Ok(json!({"ok":true,"recipe":recipe,"captions":captions,"sections":sections,"lines":sections.iter().filter_map(|x| x["lines"].as_u64()).sum::<u64>(),
        "next":"open Design to adjust any line, or hyperframes snapshot <project>/hyperframes --at <t> to check the video"}))
}

/// `words.track`: the song's lyric line in the video, one clip per printed word at its sung time (from the
/// project's timed words), held to its line's end. The clips are invisible anchors: the Studio retimes them,
/// and designed or kinetic lyrics and their drawings take their times from them. Re-running replaces the line.
pub fn cmd_words_track(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let index = dir.join("hyperframes/index.html");
    let html = fs::read_to_string(&index).map_err(|_| invalid("This project has no HyperFrames video yet; run video.init first"))?;
    let (words, _) = printed_caption_words(&aligned(s)?);
    if words.is_empty() { return Err(invalid("No printed words with timing; check words.align")); }
    let mut line_end: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for w in &words { let e = line_end.entry(w["line"].as_str().unwrap_or_default().to_owned()).or_insert(0.0); *e = e.max(w["end"].as_f64().unwrap_or(0.0)); }
    let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
    let mut clips = String::new();
    for (i, w) in words.iter().enumerate() {
        let (id, text) = (w["word_id"].as_str().unwrap_or_default(), w["text"].as_str().unwrap_or_default());
        let start = w["start"].as_f64().unwrap_or(0.0);
        let end = line_end[w["line"].as_str().unwrap_or_default()] + HOLD;
        clips.push_str(&format!("<div id=\"kinetic-{}\" class=\"clip lyric-word-clip\" data-start=\"{start:.6}\" data-duration=\"{:.6}\" data-track-index=\"{}\" data-word-id=\"{}\" aria-label=\"Sung word: {}\" style=\"opacity:0\">{}</div>\n",
            esc(&id.replace("/w", "-")), (end - start).max(0.1), 200 + i, esc(id), esc(text), esc(text)));
    }
    let kept: Vec<&str> = html.lines().filter(|l| !l.contains("class=\"clip lyric-word-clip\"")).collect();
    let mut out = kept.join("\n") + "\n";
    let at = out.find("<audio").or_else(|| out.rfind("</div>")).ok_or_else(|| invalid("The video has no root composition to hold the lyric line"))?;
    let at = out[..at].rfind('\n').map_or(at, |n| n + 1);
    out.insert_str(at, &clips);
    fs::write(&index, out)?;
    Ok(json!({"ok":true,"clips":words.len(),"first":words.first(),"last":words.last(),"path":index,
        "next":"words.design (or the Design page) builds the lyric visuals on these anchors; retime words in the Studio"}))
}

pub fn cmd_words_apply(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let index = dir.join("hyperframes/index.html");
    let id = p["animation_id"].as_str().filter(|i| i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        .ok_or_else(|| invalid("animation_id is required (letters, digits, '-' or '_')"))?;
    let original = fs::read_to_string(&index).map_err(|_| invalid("This project has no HyperFrames video yet; run video.init first"))?;
    let mut html = without_placement(&original, id);
    let use_it = p["use"].as_bool().unwrap_or(true);
    let mut receipt = json!({"ok":true,"animation_id":id,"in_video":use_it});
    if use_it {
        let composed = cmd_words_compose(s, &json!({"animation_id":id}))?;
        let read = cmd_words_read(s, &json!({"animation_id":id}))?;
        let scope = &read["document"]["animations"][0]["scope"];
        let track = cmd_words_studio_track(s, &json!({}))?;
        let clips: Vec<String> = track["sections"].as_array().into_iter().flatten()
            .filter(|sec| sec["section"] == scope["section"] && sec["occurrence"] == scope["occurrence"])
            .flat_map(|sec| sec["clips"].as_array().cloned().unwrap_or_default()).filter_map(|c| c["clip_id"].as_str().map(|c| format!("#{c}"))).collect();
        // A video with lyric clips of its own has them hidden for this section; a video without them
        // (a new song with no lyric track yet) simply gains the designed lyrics.
        if !clips.is_empty() {
            let head = html.find("</head>").ok_or_else(|| invalid("The video's index.html has no </head>; nothing written"))?;
            html.insert_str(head, &format!("<style data-magicstudio-words=\"{id}\">{} {{ visibility: hidden !important; }}</style>\n", clips.join(", ")));
        }
        let root = html.find("data-composition-id=").and_then(|at| html[at..].find('>').map(|e| at + e + 1))
            .ok_or_else(|| invalid("The video has no root composition"))?;
        let track_index = html.split("data-track-index=\"").skip(1).filter_map(|r| r.split('"').next()?.parse::<u64>().ok()).max().unwrap_or(0) + 1;
        let (w, h) = (composed["width"].as_u64().unwrap_or(1920), composed["height"].as_u64().unwrap_or(1080));
        // HyperFrames needs the host's id, the scene root and the timeline key to be the same id.
        let composition_id = composed["composition_id"].as_str().unwrap_or(id);
        html.insert_str(root, &format!("<div id=\"{composition_id}\" class=\"clip lyric-design-clip\" data-magicstudio-words=\"{id}\" data-composition-id=\"{composition_id}\" data-composition-src=\"{}\" data-start=\"{:.6}\" data-duration=\"{:.6}\" data-track-index=\"{track_index}\" data-width=\"{w}\" data-height=\"{h}\" style=\"position:absolute;left:0;top:0;width:{w}px;height:{h}px;z-index:76\"></div>\n",
            composed["src"].as_str().unwrap_or_default(),
            composed["song_anchor"].as_f64().unwrap_or(0.0), composed["authored_duration"].as_f64().unwrap_or(0.0)));
        receipt["src"] = composed["src"].clone();
        receipt["start"] = composed["song_anchor"].clone();
        receipt["hidden_clips"] = json!(clips.len());
    }
    if html != original {
        let temporary = index.with_extension("html.tmp");
        fs::write(&temporary, &html)?;
        fs::rename(&temporary, &index)?;
    }
    receipt["changed"] = json!(html != original);
    Ok(receipt)
}

fn composition_size(hf: &Path) -> (u64, u64) {
    let index = fs::read_to_string(hf.join("index.html")).unwrap_or_default();
    let attr = |name: &str| index.split(&format!("{name}=\"")).nth(1).and_then(|v| v.split('"').next()).and_then(|v| v.parse().ok());
    (attr("data-width").unwrap_or(1920), attr("data-height").unwrap_or(1080))
}

/// `words.compose`: write the derived, untrimmed HyperFrames sub-composition for one animation and
/// return its receipt. Only that file is written; the hidden reference, the source document and
/// any placement are untouched. `instance_id` gives a repeated placement its own composition id.
pub fn cmd_words_compose(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let hf = dir.join("hyperframes");
    if !hf.join("index.html").is_file() { return Err(invalid("This project has no HyperFrames project yet; run video.init first")); }
    let id = p["animation_id"].as_str().ok_or_else(|| invalid("animation_id is required"))?;
    let read = cmd_words_read(s, &json!({"animation_id":id}))?;
    // The edit syncs to the lyric track already in the video, not to the hidden reference.
    let mut synced = read["document"]["animations"][0].clone();
    let times = studio_times(s);
    if !times.is_empty() {
        for w in synced["words"].as_array_mut().into_iter().flatten() {
            if let Some((a, b)) = w["word_id"].as_str().and_then(|id| times.get(id)) { w["start"] = json!(a); w["end"] = json!(b); w["timing_source"] = json!("studio"); }
        }
    }
    fill_untimed(&mut synced);
    timing(&mut synced);
    let anim = &synced;
    let instance = match p["instance_id"].as_str() {
        Some(i) if !i.is_empty() && i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') => Some(i),
        Some(_) => return Err(invalid("instance_id must be letters, digits, '-' or '_'")),
        None => None,
    };
    let composition_id = instance.map_or_else(|| id.to_owned(), |i| format!("{id}--{i}"));
    let untimed: Vec<&str> = anim["words"].as_array().into_iter().flatten()
        .filter(|w| !w["start"].is_number() || !w["end"].is_number()).filter_map(|w| w["word_id"].as_str()).collect();
    if !untimed.is_empty() { return Err(invalid(format!("Words without timing cannot be placed: {}; resolve them with words.align", untimed.join(", ")))); }
    let anchor = anim["song_anchor"].as_f64().ok_or_else(|| invalid("Animation has no song_anchor"))?;
    let duration = anim["authored_duration"].as_f64().ok_or_else(|| invalid("Animation has no authored_duration"))?;
    // Song seconds become local seconds exactly once, here (rounded to microseconds).
    let local = |t: f64| ((t - anchor) * 1e6).round() / 1e6;
    let words: Vec<Value> = anim["words"].as_array().into_iter().flatten().map(|w| json!({"word_id":w["word_id"],"line_id":w["line_id"],"text":w["text"],
        "start":local(w["start"].as_f64().unwrap()),"end":local(w["end"].as_f64().unwrap()),
        "emphasis":w["emphasis"],"action":w["action"],"entrance":w["entrance"],"exit":w["exit"]})).collect();
    let (width, height) = composition_size(&hf);
    let data = json!({"composition_id":composition_id,"animation_id":id,"source_revision":anim["source_revision"],"duration":duration,
        "hold":anim["hold"],"style":anim["style"],"words":words});
    let designed = anim["design"].as_object().is_some_and(|d| d.values().any(Value::is_object));
    let html = if anim["kinetic"].is_object() { kinetic_composition(anim, &composition_id, width, height, duration, &words)? }
        else if designed { design_composition(anim, &composition_id, width, height, duration, &local)? }
        else if anim["effects"]["text"].is_object() { effect_composition(anim, &composition_id, width, height, duration, &local) }
        else { include_str!("../../../pipeline/templates/words-composition.html")
        .replace("__COMPOSITION_ID__", &composition_id).replace("__WIDTH__", &width.to_string()).replace("__HEIGHT__", &height.to_string())
        .replace("__DURATION__", &format!("{duration:.6}"))
        .replace("__DATA__", &serde_json::to_string(&data).map_err(|e| invalid(e.to_string()))?.replace("</", "<\\/")) };
    let file = hf.join("compositions/words").join(format!("{composition_id}.html"));
    fs::create_dir_all(file.parent().unwrap())?;
    fs::write(&file, &html)?;
    Ok(json!({"ok":true,"animation_id":id,"instance_id":instance,"composition_id":composition_id,"file":file,
        "src":format!("compositions/words/{composition_id}.html"),"sha256":format!("{:x}", Sha256::digest(html.as_bytes())),
        "source_revision":anim["source_revision"],"authored_duration":duration,"song_anchor":anchor,"lead_in":anim["lead_in"],"hold":anim["hold"],
        "width":width,"height":height,"saved_source":read["saved"]}))
}

#[cfg(test)]
mod tests {

    #[test]
    fn kinetic_designs_use_the_library_and_the_section_words() {
        let words = json!([{"word_id":"v1-l1/w0","text":"around"},{"word_id":"v1-l1/w1","text":"sea"}]);
        let ok = kinetic_design(&words, &json!({"layout":"scatter","entrance":"pop","acts":{"v1-l1/w0":"spin"},"seed":3,"style":{"font":"C059","color":"#112233"}})).unwrap();
        assert_eq!((ok["layout"].clone(), ok["acts"]["v1-l1/w0"].clone(), ok["style"]["color"].clone(), ok["band"].clone()), (json!("scatter"), json!("spin"), json!("#112233"), json!("top")));
        assert!(kinetic_design(&words, &json!({"layout":"melt","entrance":"pop"})).is_err(), "unknown layout");
        assert!(kinetic_design(&words, &json!({"layout":"stack","entrance":"pop","acts":{"v9-l1/w0":"spin"}})).is_err(), "a word from another section");
        assert!(kinetic_design(&words, &json!({"layout":"stack","entrance":"pop","acts":{"v1-l1/w1":"explode"}})).is_err(), "unknown act");
        let lib: Value = serde_json::from_str(KINETIC_LIBRARY).unwrap();
        assert!(lib["acts"].as_object().unwrap().len() >= 12 && lib["layouts"].as_object().unwrap().len() >= 4);
    }

    #[test]
    fn a_saved_document_gains_a_sung_repeat_in_order_and_keeps_its_designs() {
        let w = |section: &str, occurrence: u64, id: &str| json!({"section":section,"occurrence":occurrence,"word_id":id,"text":"x","line_id":id,"sung_id":id});
        let report = json!({"words":[w("verse_1",1,"a"),w("verse_2",1,"b"),w("verse_1",2,"c")]});
        let mut saved = default_document(&json!({"words":[w("verse_1",1,"a"),w("verse_2",1,"b")]}));
        saved["animations"][0]["design"] = json!({"a":{"items":[],"draws":[]}});
        let doc = with_sung_sections(saved, &report);
        let ids: Vec<&str> = doc["animations"].as_array().unwrap().iter().map(|a| a["animation_id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["words-verse-1-1", "words-verse-2-1", "words-verse-1-2"]);
        assert!(doc["animations"][0]["design"]["a"].is_object(), "existing designs stay");
    }

    #[test]
    fn printed_captions_keep_artist_text_and_place_skipped_words() {
        let w = |text: &str, line: &str, t: Option<(f64, f64)>| json!({"text":text,"sung_id":line,"occurrence":1,
            "start":t.map(|t| t.0),"end":t.map(|t| t.1)});
        let aligned = json!({"words":[w("Oh","l1",None),w("the","l1",Some((1.0,1.2))),w("big","l1",Some((1.2,1.4))),
            w("one","l2",Some((2.0,2.2))),w("two","l2",Some((2.2,2.4))),w("three","l2",Some((2.4,2.6))),w("four","l2",Some((2.6,2.8))),
            w("five","l2",Some((2.8,3.0))),w("six","l2",Some((3.0,3.2))),w("seven","l2",Some((3.2,3.4)))]});
        let (words, groups) = printed_caption_words(&aligned);
        assert_eq!(words[0]["text"], "Oh");
        assert!((words[0]["start"].as_f64().unwrap() - 0.65).abs() < 1e-9 && words[0]["end"].as_f64() == Some(1.0), "a skipped word sits just before its sung neighbour");
        assert_eq!(groups, vec![[0, 2], [3, 6], [7, 9]]);
    }
    use super::*;

    fn project(name: &str) -> (PathBuf, Session) {
        let dir = std::env::temp_dir().join(format!("words-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("artifacts")).unwrap();
        fs::write(dir.join("artifacts/official_lyrics.json"), json!({"title":"Song","structure_as_printed":["verse_1","chorus"],
            "lines":[{"id":"v1-l1","section":"verse_1","text":"A song of the fish in the sea."},{"id":"ch-l1","section":"chorus","text":"Hey ho don't cry."}]}).to_string()).unwrap();
        fs::write(dir.join("song.transcript.json"), json!({"words":[
            {"text":"A","start":1.0,"end":1.1},{"text":"song","start":1.2,"end":1.5},{"text":"of","start":1.6,"end":1.7},{"text":"the","start":1.8,"end":1.9},
            {"text":"fishing","start":2.0,"end":2.4},{"text":"sea.","start":2.5,"end":3.0},
            {"text":"Hey,","start":4.0,"end":4.2},{"text":"ho,","start":4.3,"end":4.5},{"text":"don't","start":4.6,"end":4.8},{"text":"cry.","start":4.9,"end":5.3}]}).to_string()).unwrap();
        let mut s = Session::default();
        s.project_path = Some(dir.join("Song.magic"));
        (dir, s)
    }

    #[test]
    fn align_reports_mismatches_without_substituting() {
        let (_, mut s) = project("align");
        let report = cmd_words_align(&mut s, &json!({})).unwrap();
        let words = report["words"].as_array().unwrap();
        assert_eq!(words.iter().map(|w| w["text"].as_str().unwrap()).collect::<Vec<_>>().join(" "), "A song of the fish in the sea. Hey ho don't cry.");
        assert_eq!(words[0]["start"], 1.0);
        assert_eq!(words[0]["timing_source"], "reference");
        assert_eq!(report["summary"]["mismatched"], 3);
        let kinds: Vec<(&str, &str)> = report["mismatches"].as_array().unwrap().iter().map(|m| (m["canonical"].as_str().unwrap(), m["kind"].as_str().unwrap())).collect();
        assert!(kinds.contains(&("fish", "substituted")) || kinds.contains(&("in", "substituted")) || kinds.contains(&("the", "substituted")), "{kinds:?}");
        assert!(words.iter().filter(|w| w["status"] != "match").all(|w| w["timing_source"] != "reference"));
        assert_eq!(words[8]["word_id"], "ch-l1/w0");
    }

    #[test]
    fn timing_disagreements_are_reported_not_chosen() {
        let (dir, mut s) = project("disagree");
        // The measured alignment of the artist's chorus puts "ho" 0.7 s after the reference; "Hey" agrees.
        fs::write(dir.join("artifacts/lyric_timing_v1.json"), json!({"lines":[{"id":"ch-sung-1","section":"chorus","text":"Hey ho don't cry.",
            "words":[{"word":"Hey","start":4.05,"end":4.2},{"word":"ho","start":5.0,"end":5.2},{"word":"don't","start":4.6,"end":4.8},{"word":"cry.","start":4.9,"end":5.3}]}]}).to_string()).unwrap();
        let report = cmd_words_align(&mut s, &json!({})).unwrap();
        let found = report["timing_disagreements"].as_array().unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0]["text"].clone(), found[0]["reference"].clone(), found[0]["measured"].clone()), (json!("ho"), json!([4.3, 4.5]), json!([5.0, 5.2])));
        assert_eq!(report["summary"]["timing_disagreements"], 1);
        let ho = report["words"].as_array().unwrap().iter().find(|w| w["word_id"] == found[0]["word_id"]).unwrap();
        assert_eq!((ho["start"].clone(), ho["timing_source"].clone()), (json!(4.3), json!("reference")), "reference timing kept, not replaced");
        assert!(report["words"].as_array().unwrap().iter().all(|w| w.get("forced").is_none()));
    }

    #[test]
    fn studio_track_groups_the_videos_lyric_clips_by_section() {
        let (dir, mut s) = project("studio");
        assert!(cmd_words_studio_track(&mut s, &json!({})).is_err(), "needs a HyperFrames video");
        fs::create_dir_all(dir.join("hyperframes")).unwrap();
        fs::write(dir.join("hyperframes/index.html"), r#"<div data-composition-id="main">
<div id="kinetic-ch-l1-1" class="clip lyric-word-clip" data-start="4.3" data-duration="0.5"><span>ho</span></div>
<div id="kinetic-ch-l1-0" class="clip lyric-word-clip" data-start="4.0" data-duration="0.6"><span class="ink">Hey</span></div>
<div id="kinetic-v1-l1-0" class="clip lyric-word-clip" data-start="1.0" data-duration="0.4"><span>A</span></div>
<div id="kinetic-x-9" class="clip lyric-word-clip" data-start="9" data-duration="1"><span>stray</span></div>
<img id="backdrop" class="clip" data-start="0" data-duration="10"></div>"#).unwrap();
        let track = cmd_words_studio_track(&mut s, &json!({})).unwrap();
        assert_eq!(track["clip_count"], 4);
        let sections = track["sections"].as_array().unwrap();
        assert_eq!(sections.iter().map(|s| s["section"].as_str().unwrap()).collect::<Vec<_>>(), ["verse_1", "chorus"]);
        let chorus = sections[1]["clips"].as_array().unwrap();
        assert_eq!(chorus.iter().map(|c| c["word_id"].as_str().unwrap()).collect::<Vec<_>>(), ["ch-l1/w0", "ch-l1/w1"], "ordered by time");
        assert_eq!((chorus[0]["text"].clone(), chorus[0]["clip_id"].clone()), (json!("Hey"), json!("kinetic-ch-l1-0")));
        assert_eq!((sections[1]["start"].clone(), sections[1]["end"].clone()), (json!(4.0), json!(4.8)));
        assert_eq!(track["unmatched"], json!(["kinetic-x-9"]), "a clip outside the sheet is reported, not dropped");
    }

    #[test]
    fn lyrics_view_derives_a_lyrics_only_copy_and_leaves_the_video_alone() {
        let (dir, mut s) = project("lyrics-view");
        fs::create_dir_all(dir.join("hyperframes")).unwrap();
        let video = r#"<html><head><title>v</title></head><body><div data-composition-id="main"><img id="sea" class="clip" data-start="0" data-duration="9">
<div id="kinetic-ch-l1-0" class="clip lyric-word-clip" data-start="4" data-duration="1"><span>Hey</span></div></div></body></html>"#;
        fs::write(dir.join("hyperframes/index.html"), video).unwrap();
        let first = cmd_words_lyrics_view(&mut s, &json!({})).unwrap();
        assert_eq!((first["src"].clone(), first["written"].clone()), (json!("compositions/words/lyrics-only.html"), json!(true)));
        let derived = fs::read_to_string(dir.join("hyperframes/compositions/words/lyrics-only.html")).unwrap();
        assert!(derived.contains(r#"data-magicstudio="lyrics-only""#) && derived.contains(".lyric-word-clip, .lyric-word-clip *"));
        assert!(derived.find("lyrics-only").unwrap() < derived.find("</head>").unwrap(), "the style sits in the head");
        assert_eq!(fs::read_to_string(dir.join("hyperframes/index.html")).unwrap(), video, "the video is untouched");
        assert_eq!(cmd_words_lyrics_view(&mut s, &json!({})).unwrap()["written"], false, "unchanged video, nothing rewritten");
        let scene = cmd_words_lyrics_view(&mut s, &json!({"show":"scene"})).unwrap();
        assert_eq!(scene["src"], "compositions/words/scene-only.html");
        let scene_html = fs::read_to_string(dir.join("hyperframes/compositions/words/scene-only.html")).unwrap();
        assert!(scene_html.contains(r#"data-magicstudio="scene-only""#) && !scene_html.contains("[data-composition-id] * { visibility: hidden"), "only the lyric clips are hidden");
        assert!(cmd_words_lyrics_view(&mut s, &json!({"show":"credits"})).is_err());
        fs::write(dir.join("hyperframes/index.html"), "<html><head></head><body><div data-composition-id=\"main\"></div></body></html>").unwrap();
        assert!(cmd_words_lyrics_view(&mut s, &json!({})).is_err(), "no lyric clips, nothing to show");
    }

    #[test]
    fn effects_come_from_the_registry_per_track_kind() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, mut s) = project("effects");
        fs::create_dir_all(dir.join("hyperframes")).unwrap();
        // A stand-in for the HyperFrames CLI: answers a catalog search with one effect named after the query.
        let cli = dir.join("fake-hyperframes");
        fs::write(&cli, "#!/bin/sh\nprintf '{\"tier\":\"words\",\"results\":[{\"name\":\"%s\",\"type\":\"component\",\"title\":\"T\",\"description\":\"D\",\"tags\":[\"x\"]}]}' \"$3\"\n").unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
        let params = json!({"hyperframes_cli": cli});
        let all = cmd_words_effects(&mut s, &params).unwrap();
        assert_eq!(all["kinds"].as_object().unwrap().keys().collect::<Vec<_>>(), ["text"], "lyrics only: text effects");
        assert_eq!(all["kinds"]["text"]["effects"][0]["name"], "word by word text reveal", "the kind's search reaches the catalog");
        let one = cmd_words_effects(&mut s, &json!({"hyperframes_cli": cli, "kind": "text"})).unwrap();
        assert_eq!(one["kinds"].as_object().unwrap().len(), 1);
        assert!(cmd_words_effects(&mut s, &json!({"hyperframes_cli": cli, "kind": "sparkles"})).is_err());
    }

    #[test]
    fn a_registry_text_effect_mounts_once_per_line_with_text_and_cues() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, mut s) = project("effect");
        fs::create_dir_all(dir.join("hyperframes")).unwrap();
        fs::write(dir.join("hyperframes/index.html"), r#"<div data-composition-id="main" data-width="1280" data-height="720"></div>"#).unwrap();
        // A stand-in `hyperframes add`: writes the component the registry would.
        let cli = dir.join("fake-hyperframes");
        fs::write(&cli, "#!/bin/sh\nmkdir -p compositions/components\nprintf '%s' '<html data-composition-variables='\"'\"'[{\"id\":\"text\",\"type\":\"string\",\"role\":\"content\"},{\"id\":\"cues\",\"type\":\"string\",\"role\":\"timing\"},{\"id\":\"accent\",\"type\":\"enum\",\"role\":\"style\"}]'\"'\"'><template></template></html>' > compositions/components/$2.html\necho '{\"ok\":true}'\n").unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
        let effect = json!({"name":"rise","variables":{"accent":"blue"}});
        assert!(cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":0,"effects":{"text":effect}})).is_err(), "not installed yet");
        let installed = cmd_words_effect_install(&mut s, &json!({"name":"rise","hyperframes_cli":cli})).unwrap();
        assert_eq!((installed["installed"].clone(), installed["src"].clone()), (json!(true), json!("compositions/components/rise.html")));
        assert_eq!(cmd_words_effect_install(&mut s, &json!({"name":"rise","hyperframes_cli":cli})).unwrap()["already"], true);
        for bad in [json!({"name":"rise","variables":{"glow":1}}), json!({"name":"rise","variables":{"text":"x"}})] {
            assert!(cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":0,"effects":{"text":bad}})).is_err());
        }
        let saved = cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":0,"effects":{"text":effect}})).unwrap();
        assert_eq!((saved["animation"]["effects"]["text"]["text_variable"].clone(), saved["animation"]["effects"]["text"]["cues"].clone()), (json!("text"), json!(true)));
        let composed = cmd_words_compose(&mut s, &json!({"animation_id":"words-chorus-1"})).unwrap();
        let html = fs::read_to_string(composed["file"].as_str().unwrap()).unwrap();
        assert_eq!(html.matches("class=\"line\"").count(), 1, "one mount per sung line");
        assert!(html.contains(r#"data-composition-src="../components/rise.html""#) && html.contains(r#"data-start="0.400000""#));
        assert!(html.contains(r#""text":"Hey ho don&#39;t cry.""#), "the line's own words, escaped for the attribute: {html}");
        assert!(html.contains(r#""cues":"0.000,0.300,0.600,0.900""#) && html.contains(r#""accent":"blue""#), "{html}");
        let cleared = cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":1,"effects":{"text":null}})).unwrap();
        assert!(cleared["animation"]["effects"]["text"].is_null());
        assert!(fs::read_to_string(cmd_words_compose(&mut s, &json!({"animation_id":"words-chorus-1"})).unwrap()["file"].as_str().unwrap()).unwrap().contains("__DATA__") == false);
    }

    #[test]
    fn save_edits_motion_only_and_detects_stale_revisions() {
        let (dir, mut s) = project("save");
        let read = cmd_words_read(&mut s, &json!({})).unwrap();
        assert_eq!(read["saved"], false);
        let anims = read["document"]["animations"].as_array().unwrap();
        assert_eq!(anims.iter().map(|a| a["animation_id"].as_str().unwrap()).collect::<Vec<_>>(), ["words-verse-1-1", "words-chorus-1"]);
        assert!(anims[1]["song_anchor"].as_f64().unwrap() > 3.0);
        let edit = json!({"animation_id":"words-chorus-1","expected_source_revision":0,"style":{"size":150},
            "words":[{"word_id":"ch-l1/w3","action":"cry","emphasis":true}]});
        let saved = cmd_words_save(&mut s, &edit).unwrap();
        assert_eq!(saved["animation"]["source_revision"], 1);
        assert_eq!(saved["animation"]["words"][3]["action"], "cry");
        assert!(dir.join("design/words.json").is_file());
        let stale = cmd_words_save(&mut s, &edit).unwrap();
        assert_eq!((stale["ok"].clone(), stale["conflict"].clone(), stale["source_revision"].clone()), (json!(false), json!(true), json!(1)));
        for bad in [json!([{"word_id":"ch-l1/w0","text":"Hay"}]), json!([{"word_id":"ch-l1/w0","start":9}]), json!([{"word_id":"ch-l1/w0","action":"explode"}])] {
            assert!(cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":1,"words":bad})).is_err());
        }
        let reread = cmd_words_read(&mut s, &json!({"animation_id":"words-chorus-1"})).unwrap();
        assert_eq!((reread["saved"].clone(), reread["reference_changed"].clone()), (json!(true), json!(false)));
        fs::write(dir.join("song.transcript.json"), json!({"words":[{"text":"Hey","start":4.0,"end":4.2}]}).to_string()).unwrap();
        assert_eq!(cmd_words_read(&mut s, &json!({})).unwrap()["reference_changed"], true);
    }

    #[test]
    fn a_line_design_groups_the_lines_own_words_and_checks_every_field() {
        let (_dir, mut s) = project("design");
        let item = |ids: &[&str], motion: &str, accent: Option<&str>| json!({"word_ids":ids,"x":120,"y":80,"size":96,"weight":700,"italic":false,
            "color":"#173F49","font":"serif","outline":"#fff8e9","rotate":-3,"motion":motion,"accent":accent});
        let stroke = json!({"name":"Arrow","view_box":"0 0 100 100","x":10,"y":20,"w":100,"h":100,
            "paths":[{"d":"M 10 10 Q 50 90 90 10","stroke":"#8B3D26","width":6,"from":0.2,"to":1.1}]});
        let design = json!({"ch-l1":{"items":[item(&["ch-l1/w0","ch-l1/w1"],"pop",Some("ch-l1/w1")),item(&["ch-l1/w2","ch-l1/w3"],"quiet",None)],"draws":[stroke]}});
        let saved = cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":0,"design":design})).unwrap();
        let line = &saved["animation"]["design"]["ch-l1"];
        assert_eq!((line["items"][0]["text"].clone(), line["items"][1]["text"].clone()), (json!("Hey ho"), json!("don't cry.")), "text comes from the sheet");
        assert_eq!(line["draws"][0]["paths"][0]["to"], 1.1);
        assert_eq!((line["items"][0]["font"].clone(), line["items"][0]["outline"].clone()), (json!("serif"), json!("#fff8e9")));
        let mut bad = |design: Value| cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":1,"design":{"ch-l1":design}})).is_err();
        let whole = |a: Value, b: Value| json!({"items":[a,b],"draws":[]});
        assert!(bad(whole(item(&["ch-l1/w1","ch-l1/w0"],"pop",None), item(&["ch-l1/w2","ch-l1/w3"],"quiet",None))), "words keep the sung order");
        assert!(bad(json!({"items":[item(&["ch-l1/w0","ch-l1/w1"],"pop",None)],"draws":[]})), "every word of the line is placed");
        assert!(bad(whole(item(&["ch-l1/w0","ch-l1/w1"],"explode",None), item(&["ch-l1/w2","ch-l1/w3"],"quiet",None))), "only known moves");
        assert!(bad(whole(item(&["ch-l1/w0","ch-l1/w1"],"pop",Some("ch-l1/w3")), item(&["ch-l1/w2","ch-l1/w3"],"quiet",None))), "the accent is one of the item's words");
        let mut scribble = design["ch-l1"].clone();
        scribble["draws"][0]["paths"][0]["d"] = json!("M 0 0 <script>");
        assert!(bad(scribble), "a stroke is path data only");
        let cleared = cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":1,"design":{"ch-l1":null}})).unwrap();
        assert!(cleared["animation"]["design"]["ch-l1"].is_null());
        assert!(cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":2,"design":{"v1-l1":null}})).is_err(), "only this section's lines");
    }

    #[test]
    fn a_designed_section_goes_into_the_video_and_comes_back_out_exactly() {
        let (dir, mut s) = project("apply");
        fs::create_dir_all(dir.join("hyperframes")).unwrap();
        let video = r#"<html><head><style>@font-face{font-family:Andika;src:url("assets/vendor/Andika-Bold.ttf") format("truetype");font-weight:700}</style></head><body>
<div id="root" data-composition-id="main" data-width="1920" data-height="1080">
<div id="kinetic-ch-l1-0" class="clip lyric-word-clip" data-start="4.0" data-duration="0.3" data-track-index="200"><span>Hey</span></div>
<div id="kinetic-ch-l1-1" class="clip lyric-word-clip" data-start="4.3" data-duration="0.3" data-track-index="201"><span>ho</span></div>
<div id="kinetic-ch-l1-2" class="clip lyric-word-clip" data-start="4.6" data-duration="0.3" data-track-index="202"><span>don't</span></div>
<div id="kinetic-ch-l1-3" class="clip lyric-word-clip" data-start="4.9" data-duration="0.4" data-track-index="203"><span>cry.</span></div>
</div></body></html>
"#;
        fs::write(dir.join("hyperframes/index.html"), video).unwrap();
        let read = cmd_words_read(&mut s, &json!({})).unwrap();
        assert_eq!((read["fonts"][0]["family"].clone(), read["fonts"][0]["weight"].clone()), (json!("Andika"), json!("700")), "the video's own lettering");
        assert!(read["fonts"][0]["path"].as_str().unwrap().ends_with("hyperframes/assets/vendor/Andika-Bold.ttf"));
        let alone = cmd_words_apply(&mut s, &json!({"animation_id":"words-verse-1-1"})).unwrap();
        assert_eq!((alone["hidden_clips"].clone(), alone["changed"].clone()), (json!(0), json!(true)), "a section without lyric clips still goes in");
        cmd_words_apply(&mut s, &json!({"animation_id":"words-verse-1-1","use":false})).unwrap();
        assert_eq!(fs::read_to_string(dir.join("hyperframes/index.html")).unwrap(), video, "and comes back out exactly");
        let item = |ids: &[&str], motion: &str, accent: Option<&str>| json!({"word_ids":ids,"x":200,"y":120,"size":140,"weight":400,"italic":true,
            "color":"#173F49","font":"serif","outline":null,"rotate":0,"motion":motion,"accent":accent});
        let design = json!({"ch-l1":{"items":[item(&["ch-l1/w0","ch-l1/w1"],"pop",Some("ch-l1/w1")),item(&["ch-l1/w2","ch-l1/w3"],"settle",Some("ch-l1/w3"))],
            "draws":[{"name":"Underline","view_box":"0 0 280 30","x":200,"y":300,"w":280,"h":30,"paths":[{"d":"M 5 14 Q 61 5 125 13 T 269 10","stroke":"#8B3D26","width":6,"from":0.3,"to":0.9}]}]}});
        cmd_words_save(&mut s, &json!({"animation_id":"words-chorus-1","expected_source_revision":0,"design":design})).unwrap();
        let applied = cmd_words_apply(&mut s, &json!({"animation_id":"words-chorus-1"})).unwrap();
        assert_eq!((applied["changed"].clone(), applied["hidden_clips"].clone()), (json!(true), json!(4)));
        let html = fs::read_to_string(dir.join("hyperframes/index.html")).unwrap();
        assert_eq!(html.matches("lyric-design-clip").count(), 1);
        assert!(html.contains(r#"data-composition-src="compositions/words/words-chorus-1.html""#) && html.contains(r#"data-track-index="204""#), "{html}");
        assert!(html.contains("#kinetic-ch-l1-0, #kinetic-ch-l1-1, #kinetic-ch-l1-2, #kinetic-ch-l1-3 { visibility: hidden !important; }"), "{html}");
        let composed = fs::read_to_string(dir.join("hyperframes/compositions/words/words-chorus-1.html")).unwrap();
        assert!(composed.starts_with("<template>") && composed.trim_end().ends_with("</template>"), "a HyperFrames scene file");
        assert!(html.contains(r#"<div id="words-chorus-1" class="clip lyric-design-clip""#), "the host carries the scene's id");
        assert!(composed.contains("data-words-design") && composed.contains(r#""text":"Hey ho""#) && composed.contains(r#""accent":0.7"#), "accent on the local clock: {composed}");
        assert_eq!(cmd_words_read(&mut s, &json!({})).unwrap()["in_video"], json!(["words-chorus-1"]));
        let again = cmd_words_apply(&mut s, &json!({"animation_id":"words-chorus-1"})).unwrap();
        assert_eq!(again["changed"], false, "applying twice changes nothing");
        cmd_words_apply(&mut s, &json!({"animation_id":"words-chorus-1","use":false})).unwrap();
        assert_eq!(fs::read_to_string(dir.join("hyperframes/index.html")).unwrap(), video, "restored exactly");
    }

    #[test]
    fn captions_are_one_cue_per_sung_line_of_the_sheet() {
        let (dir, mut s) = project("captions");
        let lines = cmd_words_captions(&mut s, &json!({"by":"line"})).unwrap()["cues"].as_u64().unwrap();
        let made = cmd_words_captions(&mut s, &json!({})).unwrap();
        assert!(made["cues"].as_u64().unwrap() <= lines, "a section holds one or more lines");
        assert!(cmd_words_captions(&mut s, &json!({"by":"word"})).is_err());
        let srt = fs::read_to_string(dir.join("lyrics.captions.srt")).unwrap();
        assert_eq!(made["cues"].as_u64().unwrap() as usize, srt.matches(" --> ").count());
        assert!(srt.starts_with("1\n00:00:"), "{srt}");
        let words: usize = srt.split("\n\n").filter(|c| !c.trim().is_empty()).map(|c| c.lines().nth(2).unwrap_or("").split_whitespace().count()).sum();
        assert_eq!(words as u64, made["words"].as_u64().unwrap(), "every heard word once: {srt}");
    }

    #[test]
    fn compose_writes_one_isolated_deterministic_subcomposition() {
        let (dir, mut s) = project("compose");
        assert!(cmd_words_compose(&mut s, &json!({"animation_id":"words-chorus-1"})).is_err(), "needs a HyperFrames project");
        fs::create_dir_all(dir.join("hyperframes")).unwrap();
        fs::write(dir.join("hyperframes/index.html"), r#"<div data-composition-id="main" data-width="1280" data-height="720"></div>"#).unwrap();
        let reference = fs::read(dir.join("song.transcript.json")).unwrap();
        let a = cmd_words_compose(&mut s, &json!({"animation_id":"words-chorus-1"})).unwrap();
        let b = cmd_words_compose(&mut s, &json!({"animation_id":"words-chorus-1","instance_id":"take-2"})).unwrap();
        assert_eq!((a["width"].clone(), a["height"].clone(), a["composition_id"].clone()), (json!(1280), json!(720), json!("words-chorus-1")));
        assert_eq!(b["composition_id"], "words-chorus-1--take-2");
        assert_ne!(a["file"], b["file"]);
        let html = fs::read_to_string(a["file"].as_str().unwrap()).unwrap();
        assert!(html.contains(r#"data-composition-id="words-chorus-1""#) && !html.contains("__DATA__"));
        assert!(!html.contains(" id=\""), "no global DOM ids inside the composition");
        // Local times: song seconds minus song_anchor, once. Chorus starts at 4.0 with a 0.4 lead-in.
        assert_eq!(a["song_anchor"], 3.6);
        assert!(html.contains(r#""start":0.4"#), "first word at local lead-in");
        let duration = a["authored_duration"].as_f64().unwrap();
        assert!((duration - ((5.3 + HOLD) - (4.0 - LEAD_IN))).abs() < 1e-9);
        assert_eq!(cmd_words_compose(&mut s, &json!({"animation_id":"words-chorus-1"})).unwrap()["sha256"], a["sha256"]);
        assert_eq!(fs::read(dir.join("song.transcript.json")).unwrap(), reference, "reference untouched");
        assert!(!dir.join("design/words.json").exists(), "compose does not save the source");
        assert!(cmd_words_compose(&mut s, &json!({"animation_id":"words-chorus-1","instance_id":"../x"})).is_err());
    }
}
