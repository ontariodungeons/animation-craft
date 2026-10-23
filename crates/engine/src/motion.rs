//! Motion evaluation extracted from the Rust stage. Clients only draw engine samples.
use crate::session::LoadedCutoutRigPlan;
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Default)]
struct EvaluatedRigMotion {
    angles_rad: BTreeMap<String, f64>,
    angles_deg: BTreeMap<String, f64>,
    pos_offset: BTreeMap<String, [f64; 2]>,
    scale_xy: BTreeMap<String, [f64; 2]>,
    opacity: BTreeMap<String, f64>,
}
fn eval_ease(ease_name: &str, u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    match ease_name {
        "linear" => u,
        "in" => u * u * u,
        "out" => {
            let inv = 1.0 - u;
            1.0 - inv * inv * inv
        }
        _ => u * u * (3.0 - 2.0 * u),
    }
}

fn eval_envelope(term: &Value, t: f64) -> f64 {
    let Some(env_arr) = term.get("envelope").and_then(Value::as_array) else {
        return 1.0;
    };
    let mut pts: Vec<(f64, f64)> = env_arr
        .iter()
        .filter_map(|p| {
            let a = p.as_array()?;
            Some((a.first()?.as_f64()? as f64, a.get(1)?.as_f64()? as f64))
        })
        .collect();
    if pts.is_empty() {
        return 1.0;
    }
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if t <= pts[0].0 {
        return pts[0].1;
    }
    for w in pts.windows(2) {
        let (t0, v0) = w[0];
        let (t1, v1) = w[1];
        if t <= t1 {
            let u = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return v0 + (v1 - v0) * u;
        }
    }
    pts.last().map(|p| p.1).unwrap_or(1.0)
}

fn eval_vec_at(val: &Value, dim: usize) -> Vec<f64> {
    if let Some(arr) = val.as_array() {
        (0..dim)
            .map(|i| arr.get(i).and_then(Value::as_f64).unwrap_or(0.0) as f64)
            .collect()
    } else if let Some(n) = val.as_f64() {
        vec![n as f64; dim]
    } else {
        vec![0.0; dim]
    }
}

fn eval_keys_term(term: &Value, t: f64, dim: usize) -> Vec<f64> {
    let Some(keys_arr) = term.get("keys").and_then(Value::as_array) else {
        return vec![0.0; dim];
    };
    let ease_name = term.get("ease").and_then(Value::as_str).unwrap_or("smooth");
    let mut pairs: Vec<(f64, Vec<f64>)> = keys_arr
        .iter()
        .filter_map(|k| {
            let a = k.as_array()?;
            let kt = a.first()?.as_f64()? as f64;
            let kv = eval_vec_at(a.get(1)?, dim);
            Some((kt, kv))
        })
        .collect();
    if pairs.is_empty() {
        return vec![0.0; dim];
    }
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if t <= pairs[0].0 {
        return pairs[0].1.clone();
    }
    for w in pairs.windows(2) {
        let (t0, ref v0) = w[0];
        let (t1, ref v1) = w[1];
        if t <= t1 {
            let u = if t1 > t0 {
                eval_ease(ease_name, (t - t0) / (t1 - t0))
            } else {
                1.0
            };
            return (0..dim).map(|i| v0[i] + (v1[i] - v0[i]) * u).collect();
        }
    }
    pairs
        .last()
        .map(|p| p.1.clone())
        .unwrap_or_else(|| vec![0.0; dim])
}

fn eval_ellipse_term(term: &Value, t: f64) -> [f64; 2] {
    let t0 = term.get("from").and_then(Value::as_f64).unwrap_or(0.0) as f64;
    let t1 = term.get("to").and_then(Value::as_f64).unwrap_or(1.0) as f64;
    let ease_name = term.get("ease").and_then(Value::as_str).unwrap_or("smooth");
    let u = if t1 > t0 {
        eval_ease(ease_name, (t - t0) / (t1 - t0))
    } else {
        1.0
    };
    let start = term.get("start").and_then(Value::as_f64).unwrap_or(0.0) as f64;
    let turns = term.get("turns").and_then(Value::as_f64).unwrap_or(1.0) as f64;
    let ang = start + std::f64::consts::TAU * turns * u;
    let centre = term
        .get("centre")
        .map(|v| eval_vec_at(v, 2))
        .unwrap_or_else(|| vec![0.0, 0.0]);
    let rx = term.get("rx").and_then(Value::as_f64).unwrap_or(0.0) as f64;
    let ry = term.get("ry").and_then(Value::as_f64).unwrap_or(0.0) as f64;
    let def_orig = [centre[0] + rx * start.cos(), centre[1] + ry * start.sin()];
    let orig = term
        .get("relativeTo")
        .map(|v| {
            let o = eval_vec_at(v, 2);
            [o[0], o[1]]
        })
        .unwrap_or(def_orig);
    [
        centre[0] + rx * ang.cos() - orig[0],
        centre[1] + ry * ang.sin() - orig[1],
    ]
}

fn eval_oscillate_term(term: &Value, t: f64, dim: usize) -> Vec<f64> {
    let mut out = vec![0.0; dim];
    let axis = term.get("axis").and_then(Value::as_u64).unwrap_or(0) as usize;
    if axis >= dim {
        return out;
    }
    let amp = term.get("amplitude").and_then(Value::as_f64).unwrap_or(0.0) as f64;
    let hz = term.get("hz").and_then(Value::as_f64).unwrap_or(1.0) as f64;
    let phase = term.get("phase").and_then(Value::as_f64).unwrap_or(0.0) as f64;
    out[axis] = amp * (std::f64::consts::TAU * hz * t + phase).sin() * eval_envelope(term, t);
    out
}

fn eval_layer_pos_at(motion_obj: &Value, layer_id: &str, t: f64) -> [f64; 2] {
    let Some(terms) = motion_obj
        .get(layer_id)
        .and_then(|m| m.get("position"))
        .and_then(Value::as_array)
    else {
        return [0.0, 0.0];
    };
    let mut dx = 0.0;
    let mut dy = 0.0;
    for term in terms {
        match term.get("kind").and_then(Value::as_str).unwrap_or("") {
            "keys" => {
                let v = eval_keys_term(term, t, 2);
                dx += v[0];
                dy += v[1];
            }
            "ellipse" => {
                let v = eval_ellipse_term(term, t);
                dx += v[0];
                dy += v[1];
            }
            "oscillate" => {
                let v = eval_oscillate_term(term, t, 2);
                dx += v[0];
                dy += v[1];
            }
            _ => {}
        }
    }
    [dx, dy]
}

fn eval_follow_lean_at(
    motion_obj: &Value,
    target_lid: &str,
    gain: f64,
    limit: f64,
    smooth: f64,
    fps: f64,
    duration: f64,
    t: f64,
) -> f64 {
    let fps = fps.max(1.0);
    let frames = ((duration * fps).round() as usize).max(1) + 1;
    let mut xs = Vec::with_capacity(frames);
    for i in 0..frames {
        let ti = (i as f64) / fps;
        xs.push(eval_layer_pos_at(motion_obj, target_lid, ti)[0]);
    }
    let alpha = if smooth <= 0.0 {
        1.0
    } else {
        1.0 - (-1.0 / (fps * smooth)).exp()
    };
    let mut leans = Vec::with_capacity(frames);
    let mut lean = 0.0f64;
    for i in 0..frames {
        let i_next = (i + 1).min(frames - 1);
        let i_prev = i.saturating_sub(1);
        let span = (i_next - i_prev).max(1) as f64;
        let v = (xs[i_next] - xs[i_prev]) * fps / span;
        let target = (gain * v).clamp(-limit, limit);
        lean += (target - lean) * alpha;
        leans.push(lean);
    }
    let f_idx = (t * fps).clamp(0.0, (frames - 1) as f64);
    let i0 = (f_idx.floor() as usize).min(frames - 1);
    let i1 = (i0 + 1).min(frames - 1);
    let frac = f_idx - (i0 as f64);
    leans[i0] + (leans[i1] - leans[i0]) * frac
}

fn evaluate_layered_motion_spec(
    rig: &LoadedCutoutRigPlan,
    track: &Value,
    t: f64,
    rest: bool,
    range_factor: f64,
) -> EvaluatedRigMotion {
    let mut angles_rad = BTreeMap::new();
    let mut angles_deg = BTreeMap::new();
    let mut pos_offset = BTreeMap::new();
    let mut scale_xy = BTreeMap::new();
    let mut opacity = BTreeMap::new();
    let fps = rig.effectcraft["fps"].as_f64().unwrap_or(30.0);
    let duration = track["duration"].as_f64().unwrap_or(1.0);
    let track_layers: Option<Vec<&str>> = track
        .get("layers")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().filter_map(Value::as_str).collect());
    let motion_active = !rest;
    let motion_obj = track.get("motion").unwrap_or(&Value::Null);
    for part in &rig.parts {
        let lid = part.id.as_str();
        // Match pipeline/layered_motion.py:389 — layers omitted from track.layers are not rendered.
        if let Some(ref allowed) = track_layers
            && !allowed.contains(&lid)
        {
            opacity.insert(part.id.clone(), 0.0);
            pos_offset.insert(part.id.clone(), [0.0, 0.0]);
            scale_xy.insert(part.id.clone(), [1.0, 1.0]);
            angles_deg.insert(part.id.clone(), 0.0);
            angles_rad.insert(part.id.clone(), 0.0);
            continue;
        }

        let layer_m = motion_obj.get(lid).unwrap_or(&Value::Null);

        // 1. Opacity (evaluated even at t=0 so hidden blink/viseme layers stay hidden at rest)
        let mut op_delta = 0.0f64;
        if let Some(op_terms) = layer_m.get("opacity").and_then(Value::as_array) {
            for term in op_terms {
                match term.get("kind").and_then(Value::as_str).unwrap_or("") {
                    "keys" => op_delta += eval_keys_term(term, t, 1)[0],
                    "oscillate" => op_delta += eval_oscillate_term(term, t, 1)[0],
                    _ => {}
                }
            }
        }
        let op = ((100.0 + op_delta).clamp(0.0, 100.0)) / 100.0;
        opacity.insert(part.id.clone(), op);

        if !motion_active {
            pos_offset.insert(part.id.clone(), [0.0, 0.0]);
            scale_xy.insert(part.id.clone(), [1.0, 1.0]);
            angles_deg.insert(part.id.clone(), 0.0);
            angles_rad.insert(part.id.clone(), 0.0);
            continue;
        }

        // 2. Position offset
        let raw_pos = eval_layer_pos_at(motion_obj, lid, t);
        pos_offset.insert(
            part.id.clone(),
            [raw_pos[0] * range_factor, raw_pos[1] * range_factor],
        );

        // 3. Scale offset
        let mut sx_pct = 0.0f64;
        let mut sy_pct = 0.0f64;
        if let Some(sc_terms) = layer_m.get("scale").and_then(Value::as_array) {
            for term in sc_terms {
                match term.get("kind").and_then(Value::as_str).unwrap_or("") {
                    "keys" => {
                        let v = eval_keys_term(term, t, 2);
                        sx_pct += v[0];
                        sy_pct += v[1];
                    }
                    "oscillate" => {
                        let v = eval_oscillate_term(term, t, 2);
                        sx_pct += v[0];
                        sy_pct += v[1];
                    }
                    _ => {}
                }
            }
        }
        scale_xy.insert(
            part.id.clone(),
            [
                (100.0 + sx_pct * range_factor) / 100.0,
                (100.0 + sy_pct * range_factor) / 100.0,
            ],
        );

        // 4. Rotation offset (keys, oscillate, follow with frame-stepped smoothing matching layered_motion.py)
        let mut rot_deg = 0.0f64;
        if let Some(rot_terms) = layer_m.get("rotation").and_then(Value::as_array) {
            for term in rot_terms {
                match term.get("kind").and_then(Value::as_str).unwrap_or("") {
                    "keys" => rot_deg += eval_keys_term(term, t, 1)[0],
                    "oscillate" => rot_deg += eval_oscillate_term(term, t, 1)[0],
                    "follow" => {
                        let target_lid = term.get("layer").and_then(Value::as_str).unwrap_or(lid);
                        let gain = term.get("gain").and_then(Value::as_f64).unwrap_or(0.0) as f64;
                        let limit = term
                            .get("limit")
                            .and_then(Value::as_f64)
                            .unwrap_or(45.0)
                            .abs() as f64;
                        let smooth = term
                            .get("smooth")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.15)
                            .max(0.0) as f64;
                        rot_deg += eval_follow_lean_at(
                            motion_obj, target_lid, gain, limit, smooth, fps, duration, t,
                        );
                    }
                    _ => {}
                }
            }
        }
        let final_deg = rot_deg * range_factor;
        angles_deg.insert(part.id.clone(), final_deg);
        angles_rad.insert(part.id.clone(), final_deg.to_radians());
    }

    EvaluatedRigMotion {
        angles_rad,
        angles_deg,
        pos_offset,
        scale_xy,
        opacity,
    }
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn load_character(s: &mut Session, params: &Value) -> Result<String> {
    let id = params["character"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| s.active_asset.clone())
        .or_else(|| s.project_assets.first().map(|a| a.id.clone()))
        .ok_or_else(|| invalid("Import artwork to animate"))?;
    crate::project::cmd_project_load_asset(s, &json!({"id": id}))?;
    Ok(id)
}

fn validate(rig: &LoadedCutoutRigPlan) -> Result<()> {
    if rig.rig_type != "layered_motion" {
        return Err(invalid("This artwork needs a layered motion rig"));
    }
    let fps = rig.effectcraft["fps"].as_f64().unwrap_or(30.0);
    if !(1.0..=120.0).contains(&fps) {
        return Err(invalid("Spec fps must be 1–120"));
    }
    let ids: BTreeSet<_> = rig.parts.iter().map(|p| p.id.as_str()).collect();
    if ids.len() != rig.parts.len() {
        return Err(invalid("Layer IDs must be unique"));
    }
    for part in &rig.parts {
        if !part.cropped_path.is_file() {
            return Err(invalid(format!("Missing artwork for {}", part.id)));
        }
        let dimensions = image::image_dimensions(&part.full_layer_path)?;
        if [dimensions.0, dimensions.1] != rig.canvas_size {
            return Err(invalid(format!(
                "{} does not match the rig canvas",
                part.id
            )));
        }
        let mut seen = BTreeSet::new();
        let mut node = Some(part);
        while let Some(p) = node {
            if !seen.insert(&p.id) {
                return Err(invalid("Layer parents contain a cycle"));
            }
            node = match p.parent.as_deref() {
                Some(id) => Some(
                    rig.parts
                        .iter()
                        .find(|x| x.id == id)
                        .ok_or_else(|| invalid(format!("Unknown parent {id}")))?,
                ),
                None => None,
            };
        }
    }
    for track in rig.effectcraft["tracks"]
        .as_array()
        .ok_or_else(|| invalid("No motion tracks"))?
    {
        if track["duration"]
            .as_f64()
            .is_none_or(|v| v <= 0.0 || v > 600.0)
        {
            return Err(invalid("Track duration must be between 0 and 600 seconds"));
        }
        if let Some(layers) = track["layers"].as_array() {
            for l in layers {
                if !ids.contains(l.as_str().unwrap_or("")) {
                    return Err(invalid("Unknown track layer"));
                }
            }
        }
        if let Some(motion) = track["motion"].as_object() {
            for (layer, props) in motion {
                if !ids.contains(layer.as_str()) {
                    return Err(invalid(format!("Unknown motion layer {layer}")));
                }
                for (prop, terms) in props
                    .as_object()
                    .ok_or_else(|| invalid("Motion properties must be an object"))?
                {
                    let dim = match prop.as_str() {
                        "position" | "scale" => 2,
                        "rotation" | "opacity" => 1,
                        _ => return Err(invalid("Unknown motion property")),
                    };
                    for term in terms
                        .as_array()
                        .ok_or_else(|| invalid("Motion terms must be an array"))?
                    {
                        let kind = term["kind"].as_str().unwrap_or("");
                        if !["keys", "ellipse", "oscillate", "follow"].contains(&kind) {
                            return Err(invalid("Unknown motion term"));
                        }
                        if let Some(ease) = term["ease"].as_str() {
                            if !["smooth", "linear", "in", "out"].contains(&ease) {
                                return Err(invalid("Unknown easing"));
                            }
                        }
                        if kind == "ellipse" && prop != "position" {
                            return Err(invalid("Ellipse applies to position only"));
                        }
                        if kind == "follow"
                            && (prop != "rotation"
                                || !ids.contains(term["layer"].as_str().unwrap_or(layer)))
                        {
                            return Err(invalid("Invalid follow target/property"));
                        }
                        if kind == "oscillate"
                            && (term["axis"].as_i64().unwrap_or(0) < 0
                                || term["axis"].as_i64().unwrap_or(0) >= dim)
                        {
                            return Err(invalid("Oscillation axis out of range"));
                        }
                        if kind == "keys" && term["keys"].as_array().is_none_or(Vec::is_empty) {
                            return Err(invalid("Keys cannot be empty"));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Standard actions are scripts in the existing exporter vocabulary, not another evaluator.
/// Root motion follows the stage's idle/walk/run bounce and jump anticipation/lift/settle pattern.
fn standard_tracks(rig: &LoadedCutoutRigPlan) -> Vec<Value> {
    let b = rig.character_bbox;
    let width = (b[2] - b[0]).max(1) as f64;
    let height = (b[3] - b[1]).max(1) as f64;
    let first = rig.effectcraft["tracks"]
        .as_array()
        .and_then(|ts| ts.first())
        .cloned()
        .unwrap_or(json!({"motion":{}}));
    let rest = evaluate_layered_motion_spec(rig, &first, 0.0, true, 1.0);
    let definitions = [
        ("idle", "Idle", 4.0, true),
        ("move", "Move", 4.0, false),
        ("jump", "Jump", 2.0, false),
        ("walk", "Walk", 4.0, true),
        ("run", "Run", 2.0, true),
        ("wave", "Wave", 3.0, true),
        ("talk", "Talk", 3.0, true),
        ("sway", "Sway", 4.0, true),
    ];
    // Sway: the largest part (by pixels, never by name) only bobs; every other part also turns
    // on its own pivot, phased by where that pivot sits so the motion ripples across the rig.
    let anchor = rig.parts.iter().max_by_key(|p| u64::from(p.crop_size[0]) * u64::from(p.crop_size[1])).map(|p| p.id.as_str());
    definitions.into_iter().map(|(id,label,duration,repeat)|{
        let mut motion=json!({});
        for part in &rig.parts {
            // Preserve the actual rest visibility, never infer anatomy from a name.
            motion[&part.id]=json!({"opacity":[{"kind":"keys","keys":[[0.0,100.0*(rest.opacity.get(&part.id).copied().unwrap_or(1.0)-1.0)]]}]});
            let bounce=|amplitude:f64,hz:f64|json!([{"kind":"oscillate","amplitude":amplitude,"hz":hz,"axis":1}]);
            if id=="sway" {
                if part.parent.is_none() {motion[&part.id]["position"]=bounce(height*0.015,0.5);}
                if Some(part.id.as_str())!=anchor {
                    let phase=std::f64::consts::TAU*(f64::from(part.pivot[0])-f64::from(b[0]))/width;
                    motion[&part.id]["rotation"]=json!([{"kind":"oscillate","amplitude":8.0,"hz":1.0,"phase":phase}]);
                }
                continue;
            }
            if part.parent.is_some() {continue;}
            match id {
                "idle"=>motion[&part.id]["position"]=bounce(height*0.012,0.25),
                "move"=>motion[&part.id]["position"]=json!([{"kind":"keys","ease":"smooth","keys":[[0,[0,0]],[duration,[width*1.2,0]]]}]),
                "jump"=>{
                    motion[&part.id]["position"]=json!([{"kind":"keys","ease":"smooth","keys":[[0,[0,0]],[duration*0.18,[0,height*0.035]],[duration*0.5,[0,-height*0.45]],[duration*0.82,[0,0]],[duration*0.9,[0,height*0.02]],[duration,[0,0]]]}]);
                    motion[&part.id]["scale"]=json!([{"kind":"keys","ease":"smooth","keys":[[0,[0,0]],[duration*0.18,[4,-5]],[duration*0.5,[-2,3]],[duration*0.82,[3,-4]],[duration,[0,0]]]}]);
                },
                "walk"=>motion[&part.id]["position"]=bounce(height*0.028,1.0),
                "run"=>motion[&part.id]["position"]=bounce(height*0.045,2.0),
                "wave"=>motion[&part.id]["rotation"]=json!([{"kind":"oscillate","amplitude":5,"hz":1.0}]),
                "talk"=>motion[&part.id]["position"]=bounce(height*0.008,2.0),
                _=>{},
            }
        }
        json!({"id":format!("@{id}"),"label":label,"duration":duration,"loop":repeat,"standard":true,"motion":motion})
    }).collect()
}

fn preview_rig(asset: &crate::Asset) -> Result<LoadedCutoutRigPlan> {
    let mut rig = if let Some(rig) = &asset.cutout_rig {
        rig.clone()
    } else {
        let size = image::image_dimensions(&asset.path)?;
        let rgba = image::open(&asset.path)?.into_rgba8();
        let bounds =
            crate::session::compute_alpha_bounds(&rgba, 16).unwrap_or([0, 0, size.0, size.1]);
        LoadedCutoutRigPlan {
            character_id: asset.id.clone(),
            rig_type: "layered_motion".into(),
            source_image: asset.path.clone(),
            canvas_size: [size.0, size.1],
            character_bbox: bounds,
            parts: vec![crate::session::CutoutRigPart {
                id: "artwork".into(),
                label: asset.id.clone(),
                pivot: [
                    (bounds[0] + bounds[2]) as f32 / 2.0,
                    (bounds[1] + bounds[3]) as f32 / 2.0,
                ],
                crop_box: [0, 0, size.0, size.1],
                crop_size: [size.0, size.1],
                cropped_path: asset.path.clone(),
                full_layer_path: asset.path.clone(),
                ..Default::default()
            }],
            effectcraft: json!({"fps":30,"tracks":[]}),
            ..Default::default()
        }
    };
    if rig.effectcraft["tracks"].is_null() {
        rig.effectcraft["tracks"] = json!([]);
    }
    rig.rig_type = "layered_motion".into();
    // The standard scripts are runtime choices; source tracks are unchanged.
    let standards = standard_tracks(&rig);
    rig.effectcraft["tracks"]
        .as_array_mut()
        .ok_or_else(|| invalid("Invalid track list"))?
        .extend(standards);
    Ok(rig)
}

fn summarize_track_channels(motion_obj: &Value) -> Vec<Value> {
    let Some(map) = motion_obj.as_object() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (layer, props_val) in map {
        let Some(props_obj) = props_val.as_object() else {
            continue;
        };
        let mut properties = Vec::new();
        let mut hz_list = Vec::new();
        let mut has_follow = false;
        let mut has_keys = false;
        for (prop_name, terms_val) in props_obj {
            properties.push(prop_name.clone());
            if let Some(terms) = terms_val.as_array() {
                for term in terms {
                    match term.get("kind").and_then(Value::as_str).unwrap_or("") {
                        "oscillate" => {
                            if let Some(hz) = term.get("hz").and_then(Value::as_f64) {
                                let label = format!("{:.1} Hz", hz).replace(".0 Hz", " Hz");
                                if !hz_list.contains(&label) {
                                    hz_list.push(label);
                                }
                            }
                        }
                        "follow" => has_follow = true,
                        "keys" | "ellipse" => has_keys = true,
                        _ => {}
                    }
                }
            }
        }
        let mut parts = Vec::new();
        if has_keys {
            parts.push("keys".to_string());
        }
        if let Some(first_hz) = hz_list.first() {
            parts.push(first_hz.clone());
        }
        if has_follow {
            parts.push("follow".to_string());
        }
        let summary = if parts.is_empty() {
            "static".to_string()
        } else {
            parts.join(" + ")
        };
        out.push(json!({
            "layer": layer,
            "properties": properties,
            "summary": summary,
            "rotation_bound": props_obj.contains_key("rotation"),
        }));
    }
    out
}

fn short_theatre_id(seed: &str) -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let digest = Sha256::digest(seed.as_bytes());
    (0..10)
        .map(|i| ALPHABET[(digest[i] as usize) % ALPHABET.len()] as char)
        .collect()
}

/// Convert authored rig rotation tracks into Theatre `tracksByObject` entries per clip.
pub(crate) fn convert_clip_rotation_tracks(
    s: &mut Session,
    params: &Value,
) -> Result<BTreeMap<String, BTreeMap<String, Value>>> {
    let id = load_character(s, params)?;
    let asset = s
        .assets
        .get(&id)
        .ok_or_else(|| invalid("Asset not loaded"))?;
    let rig = preview_rig(asset)?;
    let rig = &rig;
    validate(rig)?;
    let mut by_clip = BTreeMap::new();
    for track in rig.effectcraft["tracks"].as_array().into_iter().flatten() {
        let Some(clip_id) = track["id"].as_str() else {
            continue;
        };
        let duration = track["duration"].as_f64().unwrap_or(1.0).max(0.1);
        let motion_obj = track.get("motion").unwrap_or(&Value::Null);
        let mut objects = BTreeMap::new();
        for part in &rig.parts {
            let lid = part.id.as_str();
            let Some(rot_terms) = motion_obj
                .get(lid)
                .and_then(|m| m.get("rotation"))
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty())
            else {
                continue;
            };
            let mut max_hz = 0.0_f64;
            let mut key_times = vec![0.0_f64, (duration * 1000.0).round() / 1000.0];
            for term in rot_terms {
                match term.get("kind").and_then(Value::as_str).unwrap_or("") {
                    "oscillate" => {
                        let hz = term.get("hz").and_then(Value::as_f64).unwrap_or(1.0);
                        max_hz = max_hz.max(hz);
                    }
                    "follow" => {
                        max_hz = max_hz.max(1.2);
                        let target_lid = term.get("layer").and_then(Value::as_str).unwrap_or(lid);
                        if let Some(pos_terms) = motion_obj
                            .get(target_lid)
                            .and_then(|m| m.get("position"))
                            .and_then(Value::as_array)
                        {
                            for pt in pos_terms {
                                if let Some(ks) = pt.get("keys").and_then(Value::as_array) {
                                    for k in ks {
                                        if let Some(kt) = k.as_array().and_then(|a| a.first()).and_then(Value::as_f64) {
                                            key_times.push(kt.clamp(0.0, duration));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "keys" => {
                        if let Some(ks) = term.get("keys").and_then(Value::as_array) {
                            for k in ks {
                                if let Some(kt) = k.as_array().and_then(|a| a.first()).and_then(Value::as_f64) {
                                    key_times.push(kt.clamp(0.0, duration));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            if max_hz > 0.0 {
                let raw_step = (1.0 / (max_hz * 2.0)).clamp(0.18, 0.45);
                let steps = ((duration / raw_step).ceil() as usize).clamp(4, 20);
                for i in 1..steps {
                    let t = duration * (i as f64) / (steps as f64);
                    key_times.push((t * 1000.0).round() / 1000.0);
                }
            }
            key_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            key_times.dedup_by(|a, b| (*a - *b).abs() < 0.02);

            let mut keyframes = Vec::with_capacity(key_times.len());
            for (idx, &t) in key_times.iter().enumerate() {
                let eval = evaluate_layered_motion_spec(rig, track, t, false, 1.0);
                let deg = eval
                    .angles_deg
                    .get(lid)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(-45.0, 45.0);
                let deg_rounded = (deg * 100.0).round() / 100.0;
                let kf_id = short_theatre_id(&format!("kf:{clip_id}:{lid}:{idx}"));
                keyframes.push(json!({
                    "id": kf_id,
                    "position": (t * 1000.0).round() / 1000.0,
                    "value": deg_rounded,
                    "type": "bezier",
                    "handles": [0.5, 1.0, 0.5, 0.0],
                    "connectedRight": idx + 1 < key_times.len(),
                }));
            }
            let track_id = short_theatre_id(&format!("track:{clip_id}:{lid}:rotation"));
            objects.insert(
                lid.to_string(),
                json!({
                    "trackData": {
                        track_id.clone(): {
                            "type": "BasicKeyframedTrack",
                            "__debugName": format!("{lid}:[\"rotation\"]"),
                            "keyframes": keyframes,
                        }
                    },
                    "trackIdByPropPath": {
                        "[\"rotation\"]": track_id,
                    }
                }),
            );
        }
        by_clip.insert(clip_id.to_string(), objects);
    }
    Ok(by_clip)
}

pub fn cmd_motion_tracks(s: &mut Session, params: &Value) -> Result<Value> {
    let id = load_character(s, params)?;
    let asset = s
        .assets
        .get(&id)
        .ok_or_else(|| invalid("Asset not loaded"))?;
    let rig = preview_rig(asset)?;
    let rig = &rig;
    validate(rig)?;
    let tracks: Vec<_> = rig.effectcraft["tracks"].as_array().into_iter().flatten().map(|t| json!({
        "id":t["id"],
        "duration":t["duration"],
        "loop":t["loop"].as_bool().unwrap_or(false),
        "path":t["path"],
        "standard":t["standard"].as_bool().unwrap_or(false),
        "label":t["label"],
        "channels":summarize_track_channels(&t["motion"]),
    })).collect();
    // Plain image previews use a synthesized rig with no manifest path. Keep
    // replacement capture metadata absent there; normal animation inspection
    // must continue to work without a layered-rig source document.
    let metadata = if !rig.plan_path.as_os_str().is_empty() && rig.plan_path.is_file() {
        let spec_bytes = std::fs::read(&rig.plan_path)?;
        let rig_sha256 = format!("{:x}", Sha256::digest(&spec_bytes));
        let state_path = rig.plan_path.parent().map(|parent| {
            parent
                .join(".part-image-history")
                .join(crate::workspace::sanitize_asset_id(&id))
                .join("state.json")
        });
        let image_revision = state_path
            .as_ref()
            .map(|path| match std::fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(0),
                Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => std::fs::read(path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    .and_then(|state| {
                        let revision = state.get("revision").and_then(Value::as_u64)?;
                        let token = state.get("last_token")?;
                        (token.is_null() || token.as_str().is_some_and(|v| !v.is_empty()))
                            .then_some(revision)
                    }),
                _ => None,
            })
            .unwrap_or(None);
        let rig_raw: Value = serde_json::from_slice(&spec_bytes)
            .map_err(|e| invalid(format!("Invalid rig JSON: {e}")))?;
        Some((rig_raw, rig_sha256, image_revision))
    } else {
        None
    };
    let project_root = s
        .project_path
        .as_deref()
        .and_then(std::path::Path::parent)
        .and_then(|path| std::fs::canonicalize(path).ok());
    let original_reference = metadata
        .as_ref()
        .and_then(|(raw, _, _)| {
            let value = raw
                .get("source")
                .or_else(|| raw.get("source_image"))?
                .as_str()?;
            let raw_path = std::path::PathBuf::from(value);
            let path = if raw_path.is_absolute() {
                raw_path
            } else {
                project_root.as_ref()?.join(raw_path)
            };
            validate_local_png(path, project_root.as_deref())
        })
        .or_else(|| {
            if !rig.plan_path.as_os_str().is_empty() {
                return None;
            }
            validate_local_png(asset.path.clone(), project_root.as_deref())
        });
    let layers: Vec<_> = rig.parts.iter().map(|p| {
        let source_file = metadata.as_ref().and_then(|(raw,_,_)|crate::part_replacement::layer_file(raw, &rig.plan_path, &p.id).ok());
        let image_sha256 = source_file.as_ref().and_then(|path| std::fs::read(path).ok()).map(|bytes| format!("{:x}", Sha256::digest(bytes)));
        let source_size = source_file.as_ref().and_then(|path| image::image_dimensions(path).ok());
        json!({"id":p.id,"label":p.label,"file":p.cropped_path,"source_file":source_file,"source_size":source_size,"crop":p.crop_box,"pivot":p.pivot,"parent":p.parent,"order":p.layer,"image_sha256":image_sha256,
        "state":asset.parts.iter().find(|x| x.name == p.id)})
    }).collect();
    Ok(
        json!({"ok":true,"character":id,"canvas":rig.canvas_size,"bounds":rig.character_bbox,"source_image":rig.source_image,"original_reference":original_reference,"layers":layers,"tracks":tracks,"fps":rig.effectcraft["fps"],"spec":rig.plan_path,
        "rig_sha256":metadata.as_ref().map(|(_,hash,_)|hash),"image_revision":metadata.as_ref().and_then(|(_,_,revision)|*revision),
        "controls":rig.control_bindings,"audio":s.song_audio}),
    )
}

fn validate_local_png(path: std::path::PathBuf, root: Option<&std::path::Path>) -> Option<String> {
    let path = std::fs::canonicalize(path).ok()?;
    if !path.is_file()
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        || !root.is_some_and(|root| path.starts_with(root))
    {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    if image::guess_format(&bytes).ok() != Some(image::ImageFormat::Png)
        || image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).is_err()
    {
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

// Compose canvas-space parent transforms; opacity is deliberately independent of ancestors.
fn matrix(
    rig: &LoadedCutoutRigPlan,
    motion: &EvaluatedRigMotion,
    id: &str,
    names: Option<&Vec<Value>>,
) -> [f64; 6] {
    let mut m = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut current = Some(id);
    while let Some(part) = current.and_then(|id| rig.parts.iter().find(|p| p.id == id)) {
        let a = motion.angles_rad.get(&part.id).copied().unwrap_or(0.0);
        let sc = motion.scale_xy.get(&part.id).copied().unwrap_or([1.0, 1.0]);
        let pos = motion
            .pos_offset
            .get(&part.id)
            .copied()
            .unwrap_or([0.0, 0.0]);
        let (sin, cos) = a.sin_cos();
        let (px, py) = (part.pivot[0] as f64, part.pivot[1] as f64);
        let n = [
            cos * sc[0],
            sin * sc[0],
            -sin * sc[1],
            cos * sc[1],
            px + pos[0] - cos * sc[0] * px + sin * sc[1] * py,
            py + pos[1] - sin * sc[0] * px - cos * sc[1] * py,
        ];
        m = [
            n[0] * m[0] + n[2] * m[1],
            n[1] * m[0] + n[3] * m[1],
            n[0] * m[2] + n[2] * m[3],
            n[1] * m[2] + n[3] * m[3],
            n[0] * m[4] + n[2] * m[5] + n[4],
            n[1] * m[4] + n[3] * m[5] + n[5],
        ];
        current = part.parent.as_deref().filter(|p| {
            names.is_none_or(|ns| ns.is_empty() || ns.iter().any(|n| n.as_str() == Some(*p)))
        });
    }
    m
}

pub fn cmd_motion_sample(s: &mut Session, params: &Value) -> Result<Value> {
    let id = load_character(s, params)?;
    let asset = s
        .assets
        .get(&id)
        .ok_or_else(|| invalid("Asset not loaded"))?;
    let owned_rig = preview_rig(asset)?;
    let rig = &owned_rig;
    validate(rig)?;
    let tracks = rig.effectcraft["tracks"]
        .as_array()
        .ok_or_else(|| invalid("No motion tracks"))?;
    let track = match params["track"].as_str().filter(|x| !x.is_empty()) {
        Some(id) => tracks.iter().find(|t| t["id"].as_str() == Some(id)),
        None => tracks.first(),
    }
    .ok_or_else(|| invalid("Motion track not found"))?;
    let rest = params["rest"].as_bool().unwrap_or(false);
    let fps = params["fps"]
        .as_f64()
        .unwrap_or(rig.effectcraft["fps"].as_f64().unwrap_or(30.0));
    let amount = params["amount"].as_f64().unwrap_or(100.0);
    if !(1.0..=120.0).contains(&fps) || !(0.0..=100.0).contains(&amount) {
        return Err(invalid("fps must be 1–120 and amount 0–100"));
    }
    let duration = track["duration"].as_f64().unwrap_or(1.0);
    let count = if rest {
        1
    } else {
        (duration * fps).ceil() as usize + 1
    };
    if count * rig.parts.len() > 200_000 {
        return Err(invalid("Sample range too large; lower fps"));
    }
    let solo = asset.parts.iter().any(|p| p.solo);
    let mut frames = Vec::with_capacity(count);
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for i in 0..count {
        let t = (i as f64 / fps).min(duration);
        let motion = evaluate_layered_motion_spec(rig, track, t, rest, amount / 100.0);
        let layers: Vec<_> = rig.parts.iter().map(|p| {
            let shown = asset.parts.iter().find(|x| x.name==p.id).is_none_or(|x| x.visible && (!solo || x.solo));
            json!({"matrix":matrix(rig,&motion,&p.id,track["layers"].as_array()),"opacity":if shown {motion.opacity.get(&p.id).copied().unwrap_or(1.0)} else {0.0},
                "position":motion.pos_offset.get(&p.id),"rotation":motion.angles_deg.get(&p.id),"scale":motion.scale_xy.get(&p.id)})
        }).collect();
        for (part, layer) in rig.parts.iter().zip(&layers) {
            if layer["opacity"].as_f64().unwrap_or(0.0) <= 0.0 {
                continue;
            }
            let m = matrix(rig, &motion, &part.id, track["layers"].as_array());
            for (x, y) in [
                (part.crop_box[0], part.crop_box[1]),
                (part.crop_box[2], part.crop_box[1]),
                (part.crop_box[2], part.crop_box[3]),
                (part.crop_box[0], part.crop_box[3]),
            ] {
                let px = m[0] * x as f64 + m[2] * y as f64 + m[4];
                let py = m[1] * x as f64 + m[3] * y as f64 + m[5];
                bounds = [
                    bounds[0].min(px),
                    bounds[1].min(py),
                    bounds[2].max(px),
                    bounds[3].max(py),
                ];
            }
        }
        frames.push(json!({"time":t,"layers":layers}));
    }
    if !bounds[0].is_finite() {
        bounds = [
            0.0,
            0.0,
            rig.canvas_size[0] as f64,
            rig.canvas_size[1] as f64,
        ];
    }
    Ok(
        json!({"bounds":bounds,"ok":true,"character":id,"track":track["id"],"fps":fps,"duration":duration,"rest":rest,"loop":track["loop"].as_bool().unwrap_or(false),"frames":frames}),
    )
}

/// Author a drawn route using the existing position/scale key vocabulary. No new evaluator.
pub fn cmd_motion_path(s: &mut Session, params: &Value) -> Result<Value> {
    let id = load_character(s, params)?;
    let rig = s
        .assets
        .get(&id)
        .and_then(|a| a.cutout_rig.clone())
        .ok_or_else(|| invalid("Load a layered rig first"))?;
    validate(&rig)?;
    let points: Vec<[f64; 2]> = serde_json::from_value(params["points"].clone())
        .map_err(|_| invalid("Path points must be [x,y] pairs"))?;
    if !(2..=512).contains(&points.len()) || points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(invalid("Draw 2–512 finite path points"));
    }
    let duration = params["duration"].as_f64().unwrap_or(3.0);
    let start_size = params["start_size"].as_f64().unwrap_or(100.0);
    let end_size = params["end_size"].as_f64().unwrap_or(100.0);
    if !(0.1..=600.0).contains(&duration)
        || !(1.0..=1000.0).contains(&start_size)
        || !(1.0..=1000.0).contains(&end_size)
    {
        return Err(invalid("Duration must be 0.1–600s; sizes 1–1000%"));
    }
    let roots: Vec<_> = rig.parts.iter().filter(|p| p.parent.is_none()).collect();
    let part = if let Some(layer) = params["layer"].as_str() {
        roots.iter().copied().find(|p| p.id == layer)
    } else if roots.len() == 1 {
        roots.first().copied()
    } else {
        None
    }
    .ok_or_else(|| invalid("Choose one root layer for the path"))?;
    let before_text = std::fs::read_to_string(&rig.plan_path)?;
    let before: Value = serde_json::from_str(&before_text).map_err(|e| invalid(e.to_string()))?;
    // A file changed outside the engine must be reloaded before editing it.
    if before != rig.effectcraft {
        return Err(invalid(
            "Motion changed on disk; reload the object before drawing a path",
        ));
    }
    let base_id = params["track"].as_str();
    let mut track = before["tracks"]
        .as_array()
        .and_then(|ts| {
            base_id
                .and_then(|id| ts.iter().find(|t| t["id"].as_str() == Some(id)))
                .or_else(|| ts.first())
        })
        .cloned()
        .ok_or_else(|| invalid("Choose a source motion track"))?;
    let mut lengths = vec![0.0];
    for pair in points.windows(2) {
        let d = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
        lengths.push(lengths.last().copied().unwrap_or(0.0) + d);
    }
    let total = lengths.last().copied().unwrap_or(0.0);
    if total < 0.01 {
        return Err(invalid("Path needs two different points"));
    }
    let keys: Vec<_> = points
        .iter()
        .zip(&lengths)
        .map(|(p, d)| {
            json!([
                duration * d / total,
                [p[0] - part.pivot[0] as f64, p[1] - part.pivot[1] as f64]
            ])
        })
        .collect();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| invalid(e.to_string()))?
        .as_millis();
    let track_id = format!("drawn-path-{stamp}");
    track["id"] = json!(track_id);
    track["duration"] = json!(duration);
    track["loop"] = json!(false);
    if !track["motion"].is_object() {
        track["motion"] = json!({});
    }
    if !track["motion"][&part.id].is_object() {
        track["motion"][&part.id] = json!({});
    }
    track["motion"][&part.id]["position"] = json!([{"kind":"keys","ease":"linear","keys":keys}]);
    track["motion"][&part.id]["scale"] = json!([{"kind":"keys","ease":"smooth","keys":[[0,[start_size-100.0,start_size-100.0]],[duration,[end_size-100.0,end_size-100.0]]]}]);
    track["path"] =
        json!({"points":points,"layer":part.id,"start_size":start_size,"end_size":end_size});
    let mut after = before.clone();
    after["tracks"]
        .as_array_mut()
        .ok_or_else(|| invalid("No track list"))?
        .push(track);
    let history_dir = rig
        .plan_path
        .parent()
        .ok_or_else(|| invalid("No rig folder"))?
        .join(".motion-history");
    std::fs::create_dir_all(&history_dir)?;
    let undo = history_dir.join(format!("path-{stamp}.json"));
    std::fs::write(
        &undo,
        serde_json::to_vec_pretty(
            &json!({"spec":rig.plan_path,"before":before,"before_text":before_text,"after":after}),
        )
        .map_err(|e| invalid(e.to_string()))?,
    )?;
    std::fs::write(
        &rig.plan_path,
        serde_json::to_vec_pretty(&after).map_err(|e| invalid(e.to_string()))?,
    )?;
    crate::project::cmd_project_load_asset(s, &json!({"id":id,"reload":true}))?;
    Ok(json!({"ok":true,"character":id,"track":track_id,"undo":undo}))
}

pub fn cmd_motion_undo_path(s: &mut Session, params: &Value) -> Result<Value> {
    let id = load_character(s, params)?;
    let spec = s
        .assets
        .get(&id)
        .and_then(|a| a.cutout_rig.as_ref())
        .map(|r| r.plan_path.clone())
        .ok_or_else(|| invalid("No rig"))?;
    let path = std::path::PathBuf::from(
        params["undo"]
            .as_str()
            .ok_or_else(|| invalid("Missing undo token"))?,
    );
    let history = spec
        .parent()
        .ok_or_else(|| invalid("No rig folder"))?
        .join(".motion-history");
    if path.parent() != Some(history.as_path()) {
        return Err(invalid("Undo belongs to another object"));
    }
    let record: Value = serde_json::from_str(&std::fs::read_to_string(path)?)
        .map_err(|e| invalid(e.to_string()))?;
    let current: Value = serde_json::from_str(&std::fs::read_to_string(&spec)?)
        .map_err(|e| invalid(e.to_string()))?;
    if current != record["after"] || record["spec"] != json!(spec) {
        return Err(invalid(
            "Motion has changed since this path; undo would overwrite newer edits",
        ));
    }
    std::fs::write(
        &spec,
        record["before_text"]
            .as_str()
            .ok_or_else(|| invalid("Undo record has no original source text"))?,
    )?;
    crate::project::cmd_project_load_asset(s, &json!({"id":id,"reload":true}))?;
    Ok(json!({"ok":true,"character":id}))
}

/// Bake the existing engine samples into exporter keys. No second evaluator.
pub fn cmd_motion_export_track(s: &mut Session, p: &Value) -> Result<Value> {
    let sample = cmd_motion_sample(s, p)?;
    let id = load_character(s, p)?;
    let rig = preview_rig(
        s.assets
            .get(&id)
            .ok_or_else(|| invalid("Asset not loaded"))?,
    )?;
    let source_duration = sample["duration"]
        .as_f64()
        .ok_or_else(|| invalid("Missing sampled duration"))?;
    let duration = p["duration"].as_f64().unwrap_or(source_duration);
    if !duration.is_finite() || duration <= 0.0 || duration > 600.0 {
        return Err(invalid("duration must be in (0,600]"));
    }
    let frames = sample["frames"]
        .as_array()
        .ok_or_else(|| invalid("No sampled frames"))?;
    let mut motion = json!({});
    for (index, part) in rig.parts.iter().enumerate() {
        for prop in ["position", "rotation", "scale", "opacity"] {
            let keys: Vec<Value> = frames
                .iter()
                .map(|frame| {
                    let v = &frame["layers"][index][prop];
                    let value = match prop {
                        "position" => {
                            if v.is_array() {
                                v.clone()
                            } else {
                                json!([0, 0])
                            }
                        }
                        "rotation" => json!(v.as_f64().unwrap_or(0.0)),
                        "opacity" => json!((v.as_f64().unwrap_or(1.0) - 1.0) * 100.0),
                        _ => json!([
                            (v[0].as_f64().unwrap_or(1.0) - 1.0) * 100.0,
                            (v[1].as_f64().unwrap_or(1.0) - 1.0) * 100.0
                        ]),
                    };
                    json!([
                        frame["time"].as_f64().unwrap_or(0.0) * duration / source_duration,
                        value
                    ])
                })
                .collect();
            motion[&part.id][prop] = json!([{"kind":"keys","ease":"linear","keys":keys}]);
        }
    }
    let layers:Vec<Value>=rig.parts.iter().map(|part|json!({"id":part.id,"file":part.full_layer_path,"pivot":part.pivot,"parent":part.parent})).collect();
    let name = crate::workspace::sanitize_asset_id(p["track"].as_str().unwrap_or("motion"));
    let bounds = sample["bounds"].as_array();
    let b = |i: usize, default: f64| {
        bounds
            .and_then(|a| a.get(i))
            .and_then(Value::as_f64)
            .unwrap_or(default)
    };
    let margin = [
        (-b(0, 0.0)).max(0.0) + 8.0,
        (-b(1, 0.0)).max(0.0) + 8.0,
        (b(2, rig.canvas_size[0] as f64) - rig.canvas_size[0] as f64).max(0.0) + 8.0,
        (b(3, rig.canvas_size[1] as f64) - rig.canvas_size[1] as f64).max(0.0) + 8.0,
    ];
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let out = crate::workshop::project_dir(s)?.join(format!("tracks/{id}/{name}-{stamp}"));
    std::fs::create_dir_all(&out)?;
    let spec = out.join("motion.json");
    let data = json!({"canvas":rig.canvas_size,"fps":30,"margin":margin,"layers":layers,"tracks":[{"id":name,"duration":duration,"loop":false,"motion":motion}]});
    std::fs::write(&spec, data.to_string())?;
    let mut result = crate::exports::cmd_export_layered_motion(
        s,
        &json!({"spec":spec,"out_dir":out,"stills":3,"video":true}),
    )?;
    result["spec"] = json!(spec);
    result["duration"] = json!(duration);
    result["margin"] = json!(margin);
    result["asset"] = json!(id);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::CutoutRigPart;

    fn part(id: &str, size: [u32; 2], pivot: [f32; 2]) -> CutoutRigPart {
        CutoutRigPart { id: id.into(), label: id.into(), crop_size: size, pivot, ..CutoutRigPart::default() }
    }

    #[test]
    fn sway_turns_every_part_but_the_largest_and_keeps_flat_rigs_together() {
        let rig = LoadedCutoutRigPlan {
            character_bbox: [0, 0, 400, 200],
            parts: vec![part("a", [300, 150], [200.0, 100.0]), part("b", [40, 30], [20.0, 90.0]), part("c", [50, 20], [380.0, 60.0])],
            ..LoadedCutoutRigPlan::default()
        };
        let tracks = standard_tracks(&rig);
        let sway = tracks.iter().find(|t| t["id"] == "@sway").expect("sway is a standard action");
        assert_eq!((sway["loop"].as_bool(), sway["duration"].as_f64()), (Some(true), Some(4.0)));
        let motion = &sway["motion"];
        assert!(motion["a"]["rotation"].is_null(), "the largest part only bobs");
        for id in ["a", "b", "c"] {
            assert_eq!(motion[id]["position"], motion["a"]["position"], "{id} bobs with the rest of a flat rig");
        }
        let at = evaluate_layered_motion_spec(&rig, sway, 0.25, false, 1.0);
        assert!(at.angles_deg["b"].abs() > 1.0 && at.angles_deg["c"].abs() > 1.0, "{:?}", at.angles_deg);
        assert_ne!(motion["b"]["rotation"][0]["phase"], motion["c"]["rotation"][0]["phase"], "pivots at different places ripple");
    }
}
