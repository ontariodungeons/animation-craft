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

/// The sheet in sung order: each section occurrence with its printed lines, and the next matching
/// sung line's timing (its own id kept) attached when the alignment covers it. `structure_as_sung`
/// (sections, or `{section, lines: [ids]}` for a partial repeat) gives the order the artist sings
/// when it differs from `structure_as_printed`; the printed text is never changed.
fn sections(sheet: &Value, timed: Option<&Value>) -> Vec<Value> {
    let lines = sheet["lines"].as_array().cloned().unwrap_or_default();
    let structure = if sheet["structure_as_sung"].is_array() { &sheet["structure_as_sung"] } else { &sheet["structure_as_printed"] };
    let mut order: Vec<(String, Option<Vec<Value>>)> = structure.as_array().into_iter().flatten()
        .filter_map(|s| match s.as_str() {
            Some(name) => Some((name.to_owned(), None)),
            None => Some((s["section"].as_str()?.to_owned(), s["lines"].as_array().cloned())),
        }).collect();
    if order.is_empty() {
        for l in &lines { let s = l["section"].as_str().unwrap_or("lyrics").to_owned(); if order.last().map(|o| &o.0) != Some(&s) { order.push((s, None)); } }
    }
    let mut sung: Vec<Value> = timed.and_then(|t| t["lines"].as_array()).cloned().unwrap_or_default();
    let mut seen = std::collections::HashMap::<String, usize>::new();
    order.iter().map(|(section, only)| {
        let count = seen.entry(section.clone()).or_default();
        *count += 1;
        let out: Vec<Value> = lines.iter().filter(|l| l["section"].as_str().unwrap_or("lyrics") == section)
            .filter(|l| only.as_ref().is_none_or(|ids| ids.contains(&l["id"]))).map(|line| {
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
