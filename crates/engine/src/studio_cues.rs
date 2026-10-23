//! `studio.cues`: read-only cue lookup over a project's HyperFrames composition
//! (`hyperframes/index.html`) and character action timeline (`artifacts/action_timeline.json`).
//!
//! A clip is an element with `class="clip"`, `id`, `data-start` and `data-duration`. Per-word
//! caption elements (`lyric-word-clip`), the root composition and the song audio are not clips.
//!
//! Real on/off windows come from running the composition's own per-frame script read-only in
//! `pipeline/sample_cutout_windows.mjs` (Bun). When Bun or the sampler is unavailable, or a clip is
//! also tweened by GSAP, the clip keeps its composition times and is reported as unresolved.
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
    /// Composition `data-start`..`data-start + data-duration`, before any script window.
    composition: (f64, f64),
    timing: String,
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
        Some(Clip {
            id: attr(tag, "id")?.to_string(), start, end: start + duration, src: attr(tag, "src").map(str::to_string),
            composition: (start, start + duration), timing: String::new(),
        })
    }).collect()
}

/// Runs the read-only window sampler on the composition folder; `Err` says why it could not.
fn sample_windows(s: &Session, hyperframes: &Path) -> std::result::Result<Value, String> {
    let root = crate::workspace::workspace_root(s);
    let script = root.join("pipeline/sample_cutout_windows.mjs");
    let args = [script.to_string_lossy().into_owned(), hyperframes.to_string_lossy().into_owned()];
    let out = crate::workspace::run_tool_in(&root, "bun", &args).map_err(|e| e.to_string())?;
    serde_json::from_str(&out).map_err(|e| format!("sampler output is not JSON: {e}"))
}

/// One entry per visible window: a clip shown twice becomes two entries with the same id.
fn apply_windows(clips: Vec<Clip>, sampled: &std::result::Result<Value, String>) -> Vec<Clip> {
    let mut out = Vec::new();
    for mut c in clips {
        let (lo, hi) = c.composition;
        let Ok(v) = sampled else {
            c.timing = "unresolved (sampler unavailable; composition times)".into();
            out.push(c);
            continue;
        };
        if let Some(reason) = v["unresolved"][&c.id].as_str() {
            c.timing = format!("unresolved ({reason}; composition times)");
            out.push(c);
            continue;
        }
        match v["windows"][&c.id].as_array() {
            None => { c.timing = "unresolved (not sampled; composition times)".into(); out.push(c); }
            Some(windows) if windows.is_empty() => { c.timing = "never shown".into(); c.end = c.start; out.push(c); }
            Some(windows) => for w in windows {
                let (Some(a), Some(b)) = (w[0].as_f64(), w[1].as_f64()) else { continue };
                out.push(Clip { id: c.id.clone(), start: a.max(lo), end: b.min(hi), src: c.src.clone(), composition: c.composition, timing: "sampled window".into() });
            },
        }
    }
    out
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

/// How a clip sits against the scene it starts in; scene times are the truth and nothing is changed.
fn fit(c: &Clip, scenes: &[Scene]) -> Value {
    let (Some(first), Some(last)) = (scenes.first(), scenes.last()) else { return Value::Null };
    if c.start <= first.start && c.end >= last.end { return json!("whole song"); }
    if c.start >= c.end { return json!("never shown"); }
    let Some(sc) = scenes.iter().find(|sc| active(sc.start, sc.end, c.start)) else { return json!("outside every scene") };
    if c.end > sc.end { json!({"over_run": ms(c.end - sc.end), "scene": sc.id}) }
    else if c.end < sc.end { json!({"under_run": ms(sc.end - c.end), "scene": sc.id}) }
    else { json!("fits scene") }
}

fn load(s: &Session, params: &Value) -> Result<(std::path::PathBuf, Vec<Clip>, Vec<Scene>, Value)> {
    let raw = params.get("project").and_then(Value::as_str).filter(|p| !p.trim().is_empty())
        .ok_or_else(|| invalid("missing 'project' (.magic path)"))?;
    let project_path = crate::workspace::resolve_workspace_path(s, raw);
    if !project_path.is_file() { return Err(invalid(format!("project not found: {}", project_path.display()))); }
    let dir = project_path.parent().unwrap_or(Path::new("."));
    let html = std::fs::read_to_string(dir.join("hyperframes/index.html"))
        .map_err(|e| invalid(format!("cannot read {}: {e}", dir.join("hyperframes/index.html").display())))?;
    let timeline_path = dir.join("artifacts/action_timeline.json");
    let scenes = match std::fs::read(&timeline_path) {
        Ok(bytes) => read_scenes(&serde_json::from_slice(&bytes)
            .map_err(|e| invalid(format!("invalid {}: {e}", timeline_path.display())))?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let sampled = sample_windows(s, &dir.join("hyperframes"));
    let source = match &sampled {
        Ok(v) => json!({"ok": true, "sampler": "pipeline/sample_cutout_windows.mjs", "scripts": v["scripts"]}),
        Err(e) => json!({"ok": false, "sampler": "pipeline/sample_cutout_windows.mjs", "error": e}),
    };
    Ok((project_path, apply_windows(read_clips(&html), &sampled), scenes, source))
}

pub fn cues(s: &mut Session, params: &Value) -> Result<Value> {
    let (project, clips, scenes, timing_source) = load(s, params)?;
    match (params.get("at").and_then(Value::as_f64), params.get("clip").and_then(Value::as_str)) {
        (Some(at), None) => {
            let scene = scenes.iter().find(|sc| active(sc.start, sc.end, at));
            let scene_id = scene.map(|sc| sc.id.as_str());
            let mut cues: Vec<Value> = clips.iter().filter(|c| active(c.start, c.end, at)).map(|c| json!({
                "kind": "clip", "id": c.id, "scene": scene_id,
                "start": ms(c.start), "end": ms(c.end), "local": ms(at - c.start), "fit": fit(c, &scenes),
                "timing": c.timing, "composition": [ms(c.composition.0), ms(c.composition.1)],
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
                "ok": true, "project": project, "at": at, "timing_source": timing_source,
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
                    "id": c.id, "start": ms(c.start), "end": ms(c.end), "fit": fit(c, &scenes),
                    "timing": c.timing, "composition": [ms(c.composition.0), ms(c.composition.1)],
                    "scenes": scenes.iter().filter(|sc| c.start < c.end && sc.start < c.end && c.start < sc.end).map(|sc| sc.id.as_str()).collect::<Vec<_>>(),
                })).collect();
            Ok(json!({"ok": true, "project": project, "clip": id, "src": clip.src, "timing_source": timing_source, "uses": uses}))
        }
        _ => Err(invalid("give exactly one of 'at' (song seconds) or 'clip' (clip id)")),
    }
}
