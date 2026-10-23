//! `studio.cues`: read-only cue lookup over a project's HyperFrames composition
//! (`hyperframes/index.html`) and character action timeline (`artifacts/action_timeline.json`).
//!
//! A clip is an element with `class="clip"`, `id`, `data-start` and `data-duration`. Per-word
//! caption elements (`lyric-word-clip`), the root composition and the song audio are not clips.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::Path;

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

struct Clip {
    id: String,
    start: f64,
    end: f64,
    src: Option<String>,
}

struct Scene {
    id: String,
    start: f64,
    end: f64,
    actions: Vec<Value>,
}

/// Value of `name="..."` inside one start tag, matched as a whole attribute name.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let mut from = 0;
    while let Some(i) = tag[from..].find(&needle).map(|i| i + from) {
        if tag[..i].ends_with(char::is_whitespace) {
            let rest = &tag[i + needle.len()..];
            return rest.find('"').map(|end| &rest[..end]);
        }
        from = i + needle.len();
    }
    None
}

/// Start tags of `html`, quote-aware so `>` inside attribute values does not end a tag.
fn start_tags(html: &str) -> Vec<&str> {
    let mut tags = Vec::new();
    let bytes = html.as_bytes();
    let mut i = 0;
    while let Some(open) = html[i..].find('<').map(|o| o + i) {
        let mut quoted = false;
        let mut j = open + 1;
        while j < bytes.len() && (quoted || bytes[j] != b'>') {
            if bytes[j] == b'"' { quoted = !quoted; }
            j += 1;
        }
        tags.push(&html[open + 1..j]);
        i = j + 1;
        if i >= html.len() { break; }
    }
    tags
}

fn read_clips(html: &str) -> Vec<Clip> {
    start_tags(html).into_iter().filter_map(|tag| {
        let classes: Vec<&str> = attr(tag, "class")?.split_whitespace().collect();
        if !classes.contains(&"clip") || classes.contains(&"lyric-word-clip") { return None; }
        let start: f64 = attr(tag, "data-start")?.parse().ok()?;
        let duration: f64 = attr(tag, "data-duration")?.parse().ok()?;
        Some(Clip { id: attr(tag, "id")?.to_string(), start, end: start + duration, src: attr(tag, "src").map(str::to_string) })
    }).collect()
}

fn read_scenes(timeline: &Value) -> Vec<Scene> {
    timeline["scenes"].as_array().into_iter().flatten().map(|s| Scene {
        id: s["scene_id"].as_str().unwrap_or_default().to_string(),
        start: s["start_seconds"].as_f64().unwrap_or(0.0),
        end: s["end_seconds"].as_f64().unwrap_or(0.0),
        actions: s["actions"].as_array().cloned().unwrap_or_default(),
    }).collect()
}

/// Rounds composition seconds to milliseconds so float noise (`4.300000000000001`) stays out of reports.
fn ms(t: f64) -> f64 {
    (t * 1000.0).round() / 1000.0
}

fn active(start: f64, end: f64, at: f64) -> bool {
    start <= at && at < end
}

fn load(s: &Session, params: &Value) -> Result<(std::path::PathBuf, Vec<Clip>, Vec<Scene>)> {
    let raw = params.get("project").and_then(Value::as_str).filter(|p| !p.trim().is_empty())
        .ok_or_else(|| invalid("missing 'project' (.acproj path)"))?;
    let acproj = crate::workspace::resolve_workspace_path(s, raw);
    if !acproj.is_file() { return Err(invalid(format!("project not found: {}", acproj.display()))); }
    let dir = acproj.parent().unwrap_or(Path::new("."));
    let html = std::fs::read_to_string(dir.join("hyperframes/index.html"))
        .map_err(|e| invalid(format!("cannot read {}: {e}", dir.join("hyperframes/index.html").display())))?;
    let timeline_path = dir.join("artifacts/action_timeline.json");
    let scenes = match std::fs::read(&timeline_path) {
        Ok(bytes) => read_scenes(&serde_json::from_slice(&bytes)
            .map_err(|e| invalid(format!("invalid {}: {e}", timeline_path.display())))?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    Ok((acproj, read_clips(&html), scenes))
}

pub fn cues(s: &mut Session, params: &Value) -> Result<Value> {
    let (project, clips, scenes) = load(s, params)?;
    match (params.get("at").and_then(Value::as_f64), params.get("clip").and_then(Value::as_str)) {
        (Some(at), None) => {
            let scene = scenes.iter().find(|sc| active(sc.start, sc.end, at));
            let scene_id = scene.map(|sc| sc.id.as_str());
            let mut cues: Vec<Value> = clips.iter().filter(|c| active(c.start, c.end, at)).map(|c| json!({
                "kind": "clip", "id": c.id, "scene": scene_id,
                "start": ms(c.start), "end": ms(c.end), "local": ms(at - c.start),
            })).collect();
            for sc in &scenes {
                for a in &sc.actions {
                    let start = a["at_seconds"].as_f64().unwrap_or(0.0);
                    let end = start + a["duration_seconds"].as_f64().unwrap_or(0.0);
                    if !active(start, end, at) { continue; }
                    cues.push(json!({
                        "kind": "action", "character": a["character_id"], "action": a["action"], "scene": sc.id,
                        "start": ms(start), "end": ms(end), "local": ms(at - start),
                    }));
                }
            }
            Ok(json!({
                "ok": true, "project": project, "at": at,
                "scene": scene.map(|sc| json!({"id": sc.id, "start": ms(sc.start), "end": ms(sc.end), "local": ms(at - sc.start)})),
                "cues": cues,
            }))
        }
        (None, Some(id)) => {
            let clip = clips.iter().find(|c| c.id == id).ok_or_else(|| invalid(format!("no clip with id '{id}' in the composition")))?;
            // The same source placed under other ids counts as another use of that clip.
            let uses: Vec<Value> = clips.iter()
                .filter(|c| c.id == id || (clip.src.is_some() && c.src == clip.src))
                .map(|c| json!({
                    "id": c.id, "start": ms(c.start), "end": ms(c.end),
                    "scenes": scenes.iter().filter(|sc| sc.start < c.end && c.start < sc.end).map(|sc| sc.id.as_str()).collect::<Vec<_>>(),
                })).collect();
            Ok(json!({"ok": true, "project": project, "clip": id, "src": clip.src, "uses": uses}))
        }
        _ => Err(invalid("give exactly one of 'at' (song seconds) or 'clip' (clip id)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_at_time_and_clip_uses_read_composition_and_timeline() {
        let dir = std::env::temp_dir().join(format!("ac-studio-cues-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
        std::fs::create_dir_all(dir.join("artifacts")).unwrap();
        std::fs::write(dir.join("Song.acproj"), "{\"assets\":[]}").unwrap();
        std::fs::write(dir.join("hyperframes/index.html"), concat!(
            "<div id=\"root\" data-composition-id=\"main\" data-start=\"0\" data-duration=\"20\">",
            "<img id=\"sea\" class=\"clip\" data-start=\"0\" data-duration=\"10\" src=\"sea.png\" aria-label=\"a > b\">",
            "<img id=\"sea-again\" class=\"clip\" data-start=\"12\" data-duration=\"4\" src=\"sea.png\">",
            "<div id=\"word\" class=\"clip lyric-word-clip\" data-start=\"4\" data-duration=\"1\"></div>",
            "<audio id=\"song\" data-start=\"0\" data-duration=\"20\" src=\"song.wav\"></audio></div>",
        )).unwrap();
        std::fs::write(dir.join("artifacts/action_timeline.json"), r#"{"scenes":[
            {"scene_id":"s01","start_seconds":0,"end_seconds":12,"actions":[{"at_seconds":3,"duration_seconds":2,"character_id":"fish","action":"swim"}]},
            {"scene_id":"s02","start_seconds":12,"end_seconds":20,"actions":[]}]}"#).unwrap();
        let mut s = Session::default();
        let project = dir.join("Song.acproj");

        let at = s.execute("studio.cues", json!({"project": &project, "at": 4.5})).unwrap();
        assert_eq!(at["scene"]["id"], "s01");
        let cues = at["cues"].as_array().unwrap();
        assert_eq!(cues.len(), 2, "word captions, root and audio are not clips: {cues:?}");
        assert_eq!((cues[0]["id"].as_str(), cues[0]["local"].as_f64()), (Some("sea"), Some(4.5)));
        assert_eq!((cues[1]["character"].as_str(), cues[1]["local"].as_f64()), (Some("fish"), Some(1.5)));

        let uses = s.execute("studio.cues", json!({"project": &project, "clip": "sea"})).unwrap();
        assert_eq!(uses["uses"].as_array().unwrap().len(), 2);
        assert_eq!(uses["uses"][1]["scenes"], json!(["s02"]));
        assert!(s.execute("studio.cues", json!({"project": &project, "clip": "missing"})).is_err());
        assert!(s.execute("studio.cues", json!({"project": &project})).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
