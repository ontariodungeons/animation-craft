//! Words: editable visible lyric animations built from the artist's canonical lines, timed by
//! the hidden transcript reference. The reference (heard words, their order and times) is never
//! edited or exported; mismatches against the artist's words are reported, never substituted.
//! Text and timing are not editable here: `words.save` changes only style, emphasis and motion.
use std::{fs, path::{Path, PathBuf}};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use crate::{EngineError, Result, Session};

pub const ACTIONS: &[&str] = &["none", "wave", "tiny", "swim", "no", "cry", "big", "float"];
const ENTRANCES: &[&str] = &["pop", "fade", "rise", "none"];
const EXITS: &[&str] = &["fade", "drop", "none"];
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

/// One animation per printed section occurrence; nothing styled or moving until edited.
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

/// `words.read`: the saved Words document, or the unsaved default built from the artist's sheet.
/// Reports when the hidden reference changed since the document was saved.
pub fn cmd_words_read(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let (mut doc, saved) = match load(&dir)? { Some(doc) => (doc, true), None => (default_document(&aligned(s)?), false) };
    let (_, current_sha, _) = reference(&dir)?;
    let stale = saved && doc["reference"]["sha256"].as_str() != Some(current_sha.as_str());
    if let Some(id) = p["animation_id"].as_str() {
        let anim = doc["animations"].as_array().and_then(|a| a.iter().find(|x| x["animation_id"] == id)).cloned()
            .ok_or_else(|| invalid(format!("No Words animation {id}")))?;
        doc["animations"] = json!([anim]);
    }
    Ok(json!({"ok":true,"saved":saved,"reference_changed":stale,"path":document_path(&dir),"document":doc}))
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
    let mut doc = match load(&dir)? { Some(doc) => doc, None => default_document(&aligned(s)?) };
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
    let anim = &read["document"]["animations"][0];
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
    let html = include_str!("../../../pipeline/templates/words-composition.html")
        .replace("__COMPOSITION_ID__", &composition_id).replace("__WIDTH__", &width.to_string()).replace("__HEIGHT__", &height.to_string())
        .replace("__DURATION__", &format!("{duration:.6}"))
        .replace("__DATA__", &serde_json::to_string(&data).map_err(|e| invalid(e.to_string()))?.replace("</", "<\\/"));
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
