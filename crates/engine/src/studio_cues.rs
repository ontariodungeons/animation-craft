//! `studio.cues`: read-only cue lookup over a project's HyperFrames composition
//! (`hyperframes/index.html`) and character action timeline (`artifacts/action_timeline.json`).
//!
//! A clip is an element with `class="clip"`, `id`, `data-start` and `data-duration`. Per-word
//! caption elements (`lyric-word-clip`), the root composition and the song audio are not clips.
//!
//! The composition's own scripts can narrow a clip's window: a single `alpha('<id>', <expr>)` call whose
//! expression is a literal opacity, `range(t,a,b)`, `1-range(t,a,b)` or `pulse(t,a,b,c,d)` (directly or
//! through one binding of that form) gives the clip's real on/off times, provided the id is quoted
//! nowhere else in the scripts (data tables and loops may set its opacity under a computed id).
//! Anything else keeps the composition times and is reported as unresolved.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::collections::HashMap;
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
    timing: &'static str,
}

/// What a script says about when a clip is visible.
enum Shown {
    Unresolved,
    Never,
    Window(f64, f64),
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
            composition: (start, start + duration), timing: "unresolved (composition times)",
        })
    }).collect()
}

/// Text after `open` up to its matching `)`, or `None` when unbalanced.
fn until_close(text: &str) -> Option<&str> {
    let mut depth = 0;
    for (i, c) in text.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => return Some(&text[..i]),
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Every `alpha('<id>', <expr>)` call in `js`, by id.
fn alpha_calls(js: &str) -> HashMap<String, Vec<String>> {
    let mut calls: HashMap<String, Vec<String>> = HashMap::new();
    for (i, _) in js.match_indices("alpha('") {
        let rest = &js[i + 7..];
        let Some(q) = rest.find('\'') else { continue };
        let Some(expr) = rest[q + 1..].strip_prefix(',').and_then(until_close) else { continue };
        calls.entry(rest[..q].to_string()).or_default().push(expr.trim().to_string());
    }
    calls
}

/// Right-hand sides of every `name=...` binding in `js` (not `==`, `=>`, or a longer identifier).
fn bindings<'a>(js: &'a str, name: &str) -> Vec<&'a str> {
    js.match_indices(&format!("{name}=")).filter_map(|(i, m)| {
        let before = js[..i].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '$') { return None; }
        let rest = &js[i + m.len()..];
        if rest.starts_with('=') || rest.starts_with('>') { return None; }
        let mut depth = 0;
        let end = rest.char_indices().find(|&(_, c)| {
            match c { '(' | '[' | '{' => depth += 1, ')' | ']' | '}' => depth -= 1, _ => {} }
            depth < 0 || (depth == 0 && (c == ',' || c == ';' || c == '\n'))
        }).map_or(rest.len(), |(j, _)| j);
        Some(rest[..end].trim())
    }).collect()
}

/// Whether `id` is quoted in `js` anywhere other than its own `alpha(...)` / `move(...)` calls.
fn quoted_elsewhere(js: &str, id: &str) -> bool {
    [format!("'{id}'"), format!("\"{id}\"")].iter().any(|quoted| {
        js.match_indices(quoted.as_str()).any(|(i, _)| !(js[..i].ends_with("alpha(") || js[..i].ends_with("move(")))
    })
}

/// Literal numeric arguments of `name(t,...)` when `expr` is exactly that call.
fn literal_call(expr: &str, name: &str, arity: usize) -> Option<Vec<f64>> {
    let args = expr.strip_prefix(name)?.strip_prefix("(t,")?.strip_suffix(')')?;
    let values: Vec<f64> = args.split(',').map(|a| a.trim().parse().ok()).collect::<Option<_>>()?;
    (values.len() == arity).then_some(values)
}

fn shown(expr: &str, js: &str, follow: bool) -> Shown {
    if let Ok(opacity) = expr.parse::<f64>() {
        return if opacity > 0.0 { Shown::Window(f64::NEG_INFINITY, f64::INFINITY) } else { Shown::Never };
    }
    if let Some(v) = literal_call(expr, "range", 2) { return Shown::Window(v[0], f64::INFINITY); }
    if let Some(v) = expr.strip_prefix("1-").and_then(|e| literal_call(e, "range", 2)) { return Shown::Window(f64::NEG_INFINITY, v[1]); }
    if let Some(v) = literal_call(expr, "pulse", 4) { return Shown::Window(v[0], v[3]); }
    if follow && !expr.is_empty() && expr.chars().all(|c| c.is_alphanumeric() || c == '_')
        && let [rhs] = bindings(js, expr)[..] {
        return shown(rhs, js, false);
    }
    Shown::Unresolved
}

/// Narrows clips to the windows their scripts give, where a script states one unambiguously.
fn apply_script_windows(clips: &mut [Clip], js: &str) {
    let calls = alpha_calls(js);
    for c in clips {
        let Some([expr]) = calls.get(&c.id).map(Vec::as_slice) else { continue };
        if quoted_elsewhere(js, &c.id) { continue; }
        match shown(expr, js, true) {
            Shown::Unresolved => {}
            Shown::Never => { c.timing = "never shown (script opacity 0)"; c.end = c.start; }
            Shown::Window(a, b) => {
                c.start = c.start.max(a);
                c.end = c.end.min(b).max(c.start);
                c.timing = if c.start < c.end { "script window" } else { "never shown (script window outside clip)" };
            }
        }
    }
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
    let Some(sc) = scenes.iter().find(|sc| active(sc.start, sc.end, c.start)) else { return json!("outside every scene") };
    if c.end > sc.end { json!({"over_run": ms(c.end - sc.end), "scene": sc.id}) }
    else if c.end < sc.end { json!({"under_run": ms(sc.end - c.end), "scene": sc.id}) }
    else { json!("fits scene") }
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
    let mut clips = read_clips(&html);
    let mut js = String::new();
    for tag in start_tags(&html).into_iter().filter(|t| t.starts_with("script")) {
        let Some(src) = attr(tag, "src").filter(|s| !s.contains("://")) else { continue };
        let path = dir.join("hyperframes").join(src);
        js += &std::fs::read_to_string(&path).map_err(|e| invalid(format!("composition script {}: {e}", path.display())))?;
        js.push('\n');
    }
    apply_script_windows(&mut clips, &js);
    Ok((acproj, clips, scenes))
}

pub fn cues(s: &mut Session, params: &Value) -> Result<Value> {
    let (project, clips, scenes) = load(s, params)?;
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
                    "id": c.id, "start": ms(c.start), "end": ms(c.end), "fit": fit(c, &scenes),
                    "timing": c.timing, "composition": [ms(c.composition.0), ms(c.composition.1)],
                    "scenes": scenes.iter().filter(|sc| c.start < c.end && sc.start < c.end && c.start < sc.end).map(|sc| sc.id.as_str()).collect::<Vec<_>>(),
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
            "<img id=\"boat\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"boat.png\">",
            "<img id=\"bed\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"bed.png\">",
            "<img id=\"hat\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"hat.png\">",
            "<img id=\"rug\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"rug.png\">",
            "<img id=\"cat\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"cat.png\">",
            "<script src=\"film.js\"></script>",
            "<div id=\"word\" class=\"clip lyric-word-clip\" data-start=\"4\" data-duration=\"1\"></div>",
            "<audio id=\"song\" data-start=\"0\" data-duration=\"20\" src=\"song.wav\"></audio></div>",
        )).unwrap();
        std::fs::write(dir.join("hyperframes/film.js"), concat!(
            "const boatOn=pulse(t,2,3,5,6);alpha('boat',boatOn);alpha('bed',0);\n",
            "alpha('hat',range(t,1,2));alpha('hat',1);alpha('rug',Math.max(a,b));\n",
            "alpha('cat',range(t,8,9));['cat'].forEach(id=>alpha(id,1));",
        )).unwrap();
        std::fs::write(dir.join("artifacts/action_timeline.json"), r#"{"scenes":[
            {"scene_id":"s01","start_seconds":0,"end_seconds":12,"actions":[{"at_seconds":3,"duration_seconds":2,"character_id":"fish","action":"swim"}]},
            {"scene_id":"s02","start_seconds":12,"end_seconds":20,"actions":[]}]}"#).unwrap();
        let mut s = Session::default();
        let project = dir.join("Song.acproj");

        let at = s.execute("studio.cues", json!({"project": &project, "at": 4.5})).unwrap();
        assert_eq!(at["scene"]["id"], "s01");
        let cues = at["cues"].as_array().unwrap();
        let ids: Vec<_> = cues.iter().map(|c| c["id"].as_str().or(c["character"].as_str()).unwrap()).collect();
        // Word captions, root and audio are not clips; bed is never shown.
        assert_eq!(ids, ["sea", "boat", "hat", "rug", "cat", "fish"]);
        assert_eq!(cues[0]["local"], json!(4.5));
        assert_eq!((cues[1]["start"].as_f64(), cues[1]["end"].as_f64(), cues[1]["timing"].as_str()), (Some(2.0), Some(6.0), Some("script window")));
        assert_eq!(cues[1]["local"], json!(2.5));
        assert_eq!(cues[2]["timing"], "unresolved (composition times)", "two alpha calls are not guessed");
        assert_eq!(cues[3]["timing"], "unresolved (composition times)");
        assert_eq!(cues[4]["timing"], "unresolved (composition times)", "an id also set through a loop is not guessed");
        assert_eq!(cues[5]["local"], json!(1.5));
        let at7 = s.execute("studio.cues", json!({"project": &project, "at": 7})).unwrap();
        assert!(at7["cues"].as_array().unwrap().iter().all(|c| c["id"] != "boat"));

        let uses = s.execute("studio.cues", json!({"project": &project, "clip": "sea"})).unwrap();
        assert_eq!(uses["uses"].as_array().unwrap().len(), 2);
        assert_eq!(uses["uses"][1]["scenes"], json!(["s02"]));
        assert_eq!(uses["uses"][0]["fit"], json!({"under_run": 2.0, "scene": "s01"}));
        assert_eq!(uses["uses"][1]["fit"], json!({"under_run": 4.0, "scene": "s02"}));
        assert!(s.execute("studio.cues", json!({"project": &project, "clip": "missing"})).is_err());
        assert!(s.execute("studio.cues", json!({"project": &project})).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
