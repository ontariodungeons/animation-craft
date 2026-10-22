//! Animated image sequences: images composed on a fixed canvas, each entering and leaving with a
//! kind, duration, delay and ease — the "animated image sequence" workflow.
//!
//! The command does three things, each with an existing tool:
//! 1. PhotoCraft places every image on a transparent canvas of the sequence size (`file.placeEmbedded`).
//! 2. The entry/exit choices become layered-motion terms (the same `keys`/`ease` vocabulary the
//!    engine already evaluates and the exporter already builds).
//! 3. `export.layered_motion` renders the track with EffectCraft and validates it.
//!
//! Nothing here invents an animation evaluator or an encoder.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::workspace::{resolve_craft_cli, resolve_workspace_path, run_tool_in, workspace_root};
use crate::{EngineError, Result, Session};

/// Where the sequence writes: the open project's folder, else the workspace root.
fn base_root(s: &Session) -> PathBuf {
    s.project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| workspace_root(s))
}

fn number(value: Option<&Value>, what: &str, fallback: f64) -> Result<f64> {
    match value {
        None | Some(Value::Null) => Ok(fallback),
        Some(v) => v
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| EngineError::InvalidParams(format!("{what} must be a finite number"))),
    }
}

fn pair(value: Option<&Value>, what: &str) -> Result<[f64; 2]> {
    let items = value
        .and_then(Value::as_array)
        .filter(|a| a.len() == 2)
        .ok_or_else(|| EngineError::InvalidParams(format!("{what} must be [x, y]")))?;
    let mut out = [0.0; 2];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| EngineError::InvalidParams(format!("{what} entries must be numbers")))?;
    }
    Ok(out)
}

/// The keys one side of an entrance or exit contributes, by kind, for one property.
/// Values are offsets from the resting state (position px, scale %, rotation deg, opacity %).
fn side_keys(kind: &str, start: f64, end: f64, canvas: [f64; 2], entering: bool) -> Vec<(String, Vec<Value>)> {
    let (w, h) = (canvas[0], canvas[1]);
    let travel = 0.12;
    // `from` is the state at `start`, `to` the resting state at `end` (reversed for an exit).
    let order = |a: Value, b: Value| {
        if entering {
            vec![json!([start, a]), json!([end, b])]
        } else {
            vec![json!([start, b]), json!([end, a])]
        }
    };
    match kind {
        "fade" => vec![("opacity".into(), order(json!(-100.0), json!(0.0)))],
        "rise" | "drop" | "left" | "right" => {
            let offset = match kind {
                "rise" => json!([0.0, h * travel]),
                "drop" => json!([0.0, -h * travel]),
                "left" => json!([w * travel, 0.0]),
                _ => json!([-w * travel, 0.0]),
            };
            vec![
                ("position".into(), order(offset, json!([0.0, 0.0]))),
                ("opacity".into(), order(json!(-100.0), json!(0.0))),
            ]
        }
        "zoom_in" | "zoom_out" => {
            let scale = if kind == "zoom_in" { -25.0 } else { 25.0 };
            vec![
                ("scale".into(), order(json!([scale, scale]), json!([0.0, 0.0]))),
                ("opacity".into(), order(json!(-100.0), json!(0.0))),
            ]
        }
        "turn" => vec![
            ("rotation".into(), order(json!(-18.0), json!(0.0))),
            ("opacity".into(), order(json!(-100.0), json!(0.0))),
        ],
        _ => Vec::new(),
    }
}

fn term(ease: &str, keys: Vec<Value>) -> Value {
    json!({"kind": "keys", "ease": ease, "keys": keys})
}

/// Compose the per-property motion terms of one layer from its entry and exit.
fn layer_motion(entry: &Value, exit: &Value, duration: f64, canvas: [f64; 2]) -> Value {
    let mut props: std::collections::BTreeMap<String, Vec<Value>> = std::collections::BTreeMap::new();
    let sides: [(&Value, bool); 2] = [(entry, true), (exit, false)];
    for (side, entering) in sides {
        let kind = side.get("kind").and_then(Value::as_str).unwrap_or("none");
        if kind == "none" {
            continue;
        }
        let secs = side.get("duration").and_then(Value::as_f64).unwrap_or(0.6).max(0.01);
        let delay = side.get("delay").and_then(Value::as_f64).unwrap_or(if entering { 0.0 } else { (duration - secs).max(0.0) });
        let start = if entering { delay } else { delay };
        let end = if entering { (delay + secs).min(duration) } else { (delay + secs).min(duration) };
        let ease = side.get("ease").and_then(Value::as_str).unwrap_or("smooth");
        for (prop, keys) in side_keys(kind, start, end, canvas, entering) {
            props.entry(prop).or_default().push(term(ease, keys));
        }
    }
    Value::Object(props.into_iter().map(|(k, v)| (k, Value::Array(v))).collect())
}

/// Place one image on a transparent canvas of the sequence size with PhotoCraft.
fn prepare_layer(root: &Path, bin: &str, canvas: [u32; 2], layer: &Value, out: &Path) -> Result<()> {
    let file = layer
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("each layer needs `file`".into()))?;
    let source = Path::new(file);
    if !source.is_file() {
        return Err(EngineError::InvalidParams(format!("layer image not found: {file}")));
    }
    let center = layer
        .get("center")
        .and_then(Value::as_array)
        .filter(|a| a.len() == 2)
        .cloned()
        .unwrap_or_else(|| vec![json!(canvas[0] as f64 / 2.0), json!(canvas[1] as f64 / 2.0)]);
    let scale = layer.get("scale").and_then(Value::as_f64).unwrap_or(100.0);
    let new_doc = json!({"width": canvas[0], "height": canvas[1], "background": "transparent"}).to_string();
    let place = json!({"path": source.to_string_lossy(), "center": center, "scale": scale}).to_string();
    let args = vec![
        "run".to_string(),
        "--new".to_string(),
        new_doc,
        "--cmd".to_string(),
        "file.placeEmbedded".to_string(),
        "--params".to_string(),
        place,
        "--out".to_string(),
        out.to_string_lossy().into_owned(),
    ];
    run_tool_in(root, bin, &args)?;
    if !out.is_file() {
        return Err(EngineError::InvalidParams(format!("PhotoCraft wrote no layer: {}", out.display())));
    }
    Ok(())
}

/// `sequence.render`: compose images on a canvas, animate each in and out, render a transparent
/// video track plus review stills.
pub fn cmd_sequence_render(s: &mut Session, params: &Value) -> Result<Value> {
    let canvas = pair(params.get("canvas"), "canvas")?;
    let (w, h) = (canvas[0].round(), canvas[1].round());
    if !(16.0..=8192.0).contains(&w) || !(16.0..=8192.0).contains(&h) {
        return Err(EngineError::InvalidParams("canvas sides must be between 16 and 8192 pixels".into()));
    }
    let canvas_f = [w, h];
    let fps = number(params.get("fps"), "fps", 30.0)?.clamp(1.0, 120.0);
    let duration = number(params.get("duration"), "duration", 4.0)?;
    if duration <= 0.0 || duration > 600.0 {
        return Err(EngineError::InvalidParams("duration must be between 0 and 600 seconds".into()));
    }
    let name = params.get("name").and_then(Value::as_str).unwrap_or("sequence").to_string();
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(EngineError::InvalidParams("name must be letters, digits, - or _".into()));
    }
    let layers = params
        .get("layers")
        .and_then(Value::as_array)
        .filter(|l| !l.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("layers must be a non-empty array".into()))?;

    let root = base_root(s);
    let out_dir = match params.get("out_dir").and_then(Value::as_str).filter(|d| !d.trim().is_empty()) {
        Some(dir) => resolve_workspace_path(s, dir),
        _ => root.join("sequence").join(&name),
    };
    let prep_dir = out_dir.join("work").join("layers");
    std::fs::create_dir_all(&prep_dir)?;
    let bin = resolve_craft_cli("photocraft-cli");
    let canvas_px = [w as u32, h as u32];

    // The spec addresses layers by id, bottom to top.
    let mut spec_layers: Vec<Value> = Vec::new();
    let mut spec_motion = serde_json::Map::new();
    let mut prepared: Vec<Value> = Vec::new();

    if let Some(colour) = params.get("background").and_then(Value::as_str).filter(|c| !c.trim().is_empty()) {
        let bg = prep_dir.join("background.png");
        let new_doc = json!({"width": canvas_px[0], "height": canvas_px[1], "background": colour}).to_string();
        let args = vec![
            "run".to_string(),
            "--new".to_string(),
            new_doc,
            "--out".to_string(),
            bg.to_string_lossy().into_owned(),
        ];
        run_tool_in(&root, &bin, &args)?;
        if !bg.is_file() {
            return Err(EngineError::InvalidParams("PhotoCraft wrote no background layer".into()));
        }
        spec_layers.push(json!({"id": "background", "file": "work/layers/background.png", "pivot": [w / 2.0, h / 2.0]}));
        prepared.push(json!({"id": "background", "file": bg.to_string_lossy(), "width": canvas_px[0], "height": canvas_px[1]}));
    }

    for (index, layer) in layers.iter().enumerate() {
        let raw_id = layer.get("id").and_then(Value::as_str).unwrap_or("layer");
        let id: String = raw_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let id = if id.is_empty() { format!("layer{index}") } else { id };
        if prepared.iter().any(|p| p["id"] == json!(id)) {
            return Err(EngineError::InvalidParams(format!("duplicate layer id `{id}`")));
        }
        let out = prep_dir.join(format!("{id}.png"));
        prepare_layer(&root, &bin, canvas_px, layer, &out)?;
        spec_layers.push(json!({
            "id": id.clone(),
            "file": format!("work/layers/{id}.png"),
            "pivot": [w / 2.0, h / 2.0],
        }));
        let motion = layer_motion(
            &layer.get("entry").cloned().unwrap_or(Value::Null),
            &layer.get("exit").cloned().unwrap_or(Value::Null),
            duration,
            canvas_f,
        );
        if motion.as_object().is_some_and(|m| !m.is_empty()) {
            spec_motion.insert(id.clone(), motion);
        }
        prepared.push(json!({"id": id, "file": out.to_string_lossy(), "width": canvas_px[0], "height": canvas_px[1]}));
    }

    let spec = json!({
        "canvas": [canvas_px[0], canvas_px[1]],
        "fps": fps,
        "layers": spec_layers,
        "tracks": [{"id": name, "duration": duration, "motion": spec_motion}],
    });
    let spec_path = out_dir.join("sequence.json");
    std::fs::write(&spec_path, serde_json::to_string_pretty(&spec).map_err(|e| EngineError::InvalidParams(e.to_string()))?)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    let stills = params.get("stills").and_then(Value::as_u64).unwrap_or(0);
    let video = params.get("video").and_then(Value::as_bool).unwrap_or(true);
    let mut export = crate::exports::cmd_export_layered_motion(
        s,
        &json!({
            "spec": spec_path.to_string_lossy(),
            "out_dir": out_dir.to_string_lossy(),
            "tracks": name,
            "stills": stills,
            "video": video,
            "gpu": params.get("gpu").cloned().unwrap_or(json!(true)),
            "crf": params.get("crf").cloned().unwrap_or(json!(24)),
        }),
    )?;

    // A background fills the canvas, so the transparent-clip checks do not describe this result.
    // Keep the verdict honest: report which checks do not apply and judge the rest.
    if let Some(colour) = params.get("background").and_then(Value::as_str).filter(|c| !c.trim().is_empty()) {
        let skipped = ["alpha_channel", "not_clipped_by_canvas"];
        let mut all_ok = true;
        if let Some(outputs) = export.get_mut("outputs").and_then(Value::as_array_mut) {
            for out in outputs.iter_mut() {
                let Some(validation) = out.get_mut("validation").and_then(Value::as_object_mut) else { continue };
                let failed = validation
                    .remove("failed")
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|check| !check["name"].as_str().is_some_and(|n| skipped.contains(&n)))
                    .collect::<Vec<_>>();
                let ok = failed.is_empty();
                validation.insert("failed".into(), Value::Array(failed));
                validation.insert("ok".into(), json!(ok));
                validation.insert("not_applicable".into(), json!(skipped));
                validation.insert(
                    "note".into(),
                    json!(format!("opaque background {colour}: the alpha and edge-clipping checks do not apply")),
                );
                all_ok &= ok;
            }
        }
        export["validation_ok"] = json!(all_ok);
    }

    Ok(json!({
        "ok": true,
        "command": "sequence.render",
        "name": name,
        "canvas": canvas_px,
        "fps": fps,
        "duration": duration,
        "spec": spec_path.to_string_lossy(),
        "out_dir": out_dir.to_string_lossy(),
        "layers": prepared,
        "export": export,
    }))
}
