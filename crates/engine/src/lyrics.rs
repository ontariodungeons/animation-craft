//! Song lyrics for display. The artist's canonical lyric sheet wins when the project has one;
//! otherwise callers show the transcript, labelled as such. Read-only: originals are never
//! rewritten, and aligned timing is supporting data attached to the artist's own lines.
use std::{fs, path::{Path, PathBuf}};
use serde_json::{Value, json};
use crate::{Result, Session};

fn read_json(path: &Path) -> Option<Value> { fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()) }

fn is_sheet(v: &Value) -> bool {
    v["lines"].as_array().is_some_and(|l| !l.is_empty() && l.iter().all(|x| x["id"].is_string() && x["text"].is_string()))
}

/// Liner-notes JSON sheets first (user-attached), then the earlier pipeline's location.
fn canonical(dir: &Path) -> Option<(PathBuf, Value)> {
    let mut candidates: Vec<PathBuf> = fs::read_dir(dir.join("liner-notes")).into_iter().flatten()
        .filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    candidates.sort();
    candidates.push(dir.join("artifacts/official_lyrics.json"));
    candidates.into_iter().find_map(|p| read_json(&p).filter(is_sheet).map(|v| (p, v)))
}

fn key(text: &str) -> String { text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect() }

/// The newest `artifacts/lyric_timing_vN.json` whose every line aligns one of the artist's lines.
fn timing(dir: &Path, sheet: &Value) -> Option<(PathBuf, Value)> {
    let texts: Vec<String> = sheet["lines"].as_array()?.iter().filter_map(|l| l["text"].as_str().map(key)).collect();
    let mut files: Vec<(u64, PathBuf)> = fs::read_dir(dir.join("artifacts")).into_iter().flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|p| p.file_name()?.to_str()?.strip_prefix("lyric_timing_v")?.strip_suffix(".json")?.parse().ok().map(|n| (n, p)))
        .collect();
    files.sort();
    files.into_iter().rev().find_map(|(_, p)| read_json(&p).filter(|v| v["lines"].as_array().is_some_and(|l| {
        !l.is_empty() && l.iter().all(|x| x["text"].as_str().is_some_and(|t| texts.contains(&key(t))))
    })).map(|v| (p, v)))
}

fn label(section: &str) -> String {
    let (name, number) = section.rsplit_once('_').filter(|(_, n)| n.parse::<u32>().is_ok()).unwrap_or((section, ""));
    let mut name = name.replace('_', " ");
    if let Some(first) = name.get(..1) { name = first.to_uppercase() + &name[1..]; }
    if number.is_empty() { name } else { format!("{name} {number}") }
}

/// The sheet as printed: each printed section occurrence with its lines, and the next matching
/// sung line's timing (its own id kept) attached when the alignment covers it.
fn sections(sheet: &Value, timed: Option<&Value>) -> Vec<Value> {
    let lines = sheet["lines"].as_array().cloned().unwrap_or_default();
    let mut order: Vec<String> = sheet["structure_as_printed"].as_array().into_iter().flatten()
        .filter_map(|s| s.as_str().map(str::to_owned)).collect();
    if order.is_empty() {
        for l in &lines { let s = l["section"].as_str().unwrap_or("lyrics").to_owned(); if order.last() != Some(&s) { order.push(s); } }
    }
    let mut sung: Vec<Value> = timed.and_then(|t| t["lines"].as_array()).cloned().unwrap_or_default();
    let mut seen = std::collections::HashMap::<String, usize>::new();
    order.iter().map(|section| {
        let count = seen.entry(section.clone()).or_default();
        *count += 1;
        let out: Vec<Value> = lines.iter().filter(|l| l["section"].as_str().unwrap_or("lyrics") == section).map(|line| {
            let text = line["text"].as_str().unwrap_or_default();
            let found = sung.iter().position(|s| s["section"] == line["section"] && s["text"].as_str().is_some_and(|t| key(t) == key(text)));
            let cue = found.map(|i| sung.remove(i));
            let words: Vec<Value> = cue.as_ref().and_then(|c| c["words"].as_array()).into_iter().flatten()
                .map(|w| json!({"text":w["word"].as_str().or(w["text"].as_str()),"start":w["start"],"end":w["end"]})).collect();
            json!({"id":line["id"],"sung_id":cue.as_ref().map(|c| c["id"].clone()),"text":text,
                "start":words.first().map(|w| w["start"].clone()),"end":words.last().map(|w| w["end"].clone()),"words":words})
        }).collect();
        json!({"section":section,"label":label(section),"occurrence":*count,"lines":out})
    }).collect()
}

/// `song.lyrics`: what to show as the song's words, with honest provenance. No writes.
pub fn cmd_song_lyrics(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let liner: Vec<PathBuf> = {
        let mut files: Vec<PathBuf> = fs::read_dir(dir.join("liner-notes")).into_iter().flatten().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect();
        files.sort();
        files
    };
    let Some((sheet_path, sheet)) = canonical(&dir) else {
        return Ok(json!({"ok":true,"source":"transcript","note":"No artist lyric sheet in this project; showing the transcript as heard, uncorrected.","liner_notes":liner,"original":null}));
    };
    let original = sheet["source"]["path"].as_str().map(PathBuf::from).map(|p| {
        let bytes = fs::metadata(&p).ok().map(|m| m.len());
        json!({"path":p,"exists":bytes.is_some(),"bytes":bytes,"description":sheet["source"]["description"]})
    });
    let timed = timing(&dir, &sheet);
    let sections = sections(&sheet, timed.as_ref().map(|(_, v)| v));
    Ok(json!({"ok":true,"source":"canonical","sheet":sheet_path,"title":sheet["title"],"artist":sheet["artist"],"album":sheet["album"],
        "artist_note":sheet["artist_note_verbatim"],"notes":sheet["notes"],"original":original,
        "line_count":sheet["lines"].as_array().map_or(0, Vec::len),"sections":sections,
        "timing":timed.as_ref().map(|(p, v)| json!({"file":p,"status":v["status"],"method":v["method"]})),"liner_notes":liner}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("song-lyrics-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("artifacts")).unwrap();
        fs::create_dir_all(dir.join("liner-notes")).unwrap();
        dir
    }

    const SHEET: &str = r#"{"title":"Song","artist":"Artist","artist_note_verbatim":"A favourite.","source":{"path":"/nonexistent/handout.doc"},
      "structure_as_printed":["verse_1","chorus","verse_2","chorus"],
      "lines":[{"id":"v1-l1","section":"verse_1","text":"Fish in the sea,"},{"id":"ch-l1","section":"chorus","text":"Hey ho, don't cry."},
               {"id":"v2-l1","section":"verse_2","text":"Swim around."}]}"#;
    const TIMING: &str = r#"{"status":"measured","lines":[
      {"id":"v1-l1","section":"verse_1","text":"Fish in the sea,","words":[{"word":"Fish","start":1.0,"end":1.4},{"word":"sea","start":2.0,"end":2.5}]},
      {"id":"ch1-l1","section":"chorus","text":"Hey ho, don't cry.","words":[{"word":"Hey","start":3.0,"end":3.3}]},
      {"id":"v2-l1","section":"verse_2","text":"Swim around.","words":[{"word":"Swim","start":5.0,"end":5.5}]},
      {"id":"ch2-l1","section":"chorus","text":"Hey ho, don't cry.","words":[{"word":"Hey","start":7.0,"end":7.3}]}]}"#;

    #[test]
    fn printed_structure_keeps_artist_text_and_sung_ids() {
        let dir = project("sheet");
        fs::write(dir.join("artifacts/official_lyrics.json"), SHEET).unwrap();
        fs::write(dir.join("artifacts/lyric_timing_v1.json"), r#"{"lines":[{"id":"x","section":"verse_1","text":"Fishing sea"}]}"#).unwrap();
        fs::write(dir.join("artifacts/lyric_timing_v3.json"), TIMING).unwrap();
        let sheet = read_json(&dir.join("artifacts/official_lyrics.json")).unwrap();
        let (file, timed) = timing(&dir, &sheet).unwrap();
        assert!(file.ends_with("lyric_timing_v3.json"));
        let out = sections(&sheet, Some(&timed));
        assert_eq!(out.iter().map(|s| s["label"].as_str().unwrap()).collect::<Vec<_>>(), ["Verse 1", "Chorus", "Verse 2", "Chorus"]);
        assert_eq!(out[1]["lines"][0]["sung_id"], "ch1-l1");
        assert_eq!(out[3]["lines"][0]["sung_id"], "ch2-l1");
        assert_eq!(out[3]["lines"][0]["id"], "ch-l1");
        assert_eq!(out[0]["lines"][0]["text"], "Fish in the sea,");
        assert_eq!((out[0]["lines"][0]["start"].clone(), out[0]["lines"][0]["end"].clone()), (json!(1.0), json!(2.5)));
    }

    #[test]
    fn liner_notes_sheet_wins_and_missing_timing_stays_untimed() {
        let dir = project("liner");
        fs::write(dir.join("artifacts/official_lyrics.json"), SHEET).unwrap();
        fs::write(dir.join("liner-notes/song.lyrics.json"), SHEET.replace("Fish in the sea,", "Fish in the deep sea,")).unwrap();
        fs::write(dir.join("liner-notes/notes.txt"), "not a sheet").unwrap();
        let (path, sheet) = canonical(&dir).unwrap();
        assert!(path.ends_with("liner-notes/song.lyrics.json"));
        let out = sections(&sheet, None);
        assert_eq!(out[0]["lines"][0]["text"], "Fish in the deep sea,");
        assert!(out[0]["lines"][0]["start"].is_null());
        assert_eq!(label("verse_12"), "Verse 12");
        assert_eq!(label("chorus"), "Chorus");
    }

    #[test]
    fn no_sheet_means_transcript_is_labelled() {
        let dir = project("none");
        fs::write(dir.join("liner-notes/notes.json"), r#"{"lines":[]}"#).unwrap();
        assert!(canonical(&dir).is_none());
    }
}
