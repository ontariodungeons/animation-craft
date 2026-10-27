//! Opaque Theatre save-file persistence in the selected project. No animation evaluation or rig edits.
use std::path::PathBuf;
use serde_json::{Value, json};
use crate::{EngineError, Result, Session};

fn invalid(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }
fn selected_project(s: &Session, params: &Value, required: bool) -> Result<PathBuf> {
    let selected=s.project_path.as_deref().ok_or_else(||invalid("No project is selected; open the intended .magic before saving Theatre state"))?;
    let selected=std::fs::canonicalize(selected).map_err(|e|invalid(format!("Selected project is unavailable: {e}; reopen the intended .magic before retrying")))?;
    let expected=params.get("expected_project");
    if required || expected.is_some() {
        let expected=expected.and_then(Value::as_str).filter(|v|!v.trim().is_empty())
            .ok_or_else(||invalid("motion.theatre.save requires expected_project=<selected .magic>; no state was saved"))?;
        let expected=crate::project::project_file_path(s,&json!({"path":expected}))?;
        let expected=std::fs::canonicalize(expected).map_err(|e|invalid(format!("expected_project is unavailable: {e}; select the existing intended .magic")))?;
        if selected!=expected { return Err(invalid("expected_project differs from the selected project; refresh the project identity and save the matching Theatre state")); }
    }
    Ok(selected)
}
fn native_sheets(state: &Value) -> Result<&serde_json::Map<String,Value>> {
    if !state.is_object() { return Err(invalid("state must be the native Theatre createContentOfSaveFile JSON object")); }
    if state["definitionVersion"]!="0.4.0" { return Err(invalid("Unsupported or missing Theatre definitionVersion; supply the native 0.4.0 save-file state without converting its tracks")); }
    if !state["revisionHistory"].as_array().is_some_and(|values|values.iter().all(Value::is_string)) {
        return Err(invalid("Native Theatre state requires revisionHistory as an array of revision strings"));
    }
    state["sheetsById"].as_object().filter(|sheets|!sheets.is_empty() && sheets.iter().all(|(id,value)|!id.trim().is_empty()&&value.is_object()))
        .ok_or_else(||invalid("Native Theatre state requires nonempty sheetsById with sheet objects keyed by connected asset IDs"))
}
fn receipt(s: &Session, project: PathBuf, state: &Value, reused: bool) -> Value {
    json!({"ok":true,"available":true,"saved":true,"status":"saved","project":project,"state":state,
        "sheet_ids":state["sheetsById"].as_object().map(|sheets|sheets.keys().collect::<Vec<_>>()),"reused":reused,
        "connected_asset_ids":s.project_assets.iter().map(|asset|&asset.id).collect::<Vec<_>>()})
}

pub fn read(s: &mut Session, params: &Value) -> Result<Value> {
    if s.project_path.is_none() { return Ok(json!({"ok":true,"available":false,"saved":false,"status":"unavailable","state":null,"reason":"No project selected; open the intended .magic"})); }
    let project=selected_project(s,params,false)?;
    match s.theatre_state.as_ref().filter(|state| !state.is_null()) {
        None=>Ok(json!({"ok":true,"available":true,"saved":false,"status":"no_saved_state","project":project,"state":null})),
        Some(state)=>{native_sheets(state)?;Ok(receipt(s,project,state,true))}
    }
}

pub fn save(s: &mut Session, params: &Value) -> Result<Value> {
    let project=selected_project(s,params,true)?;
    let state=params.get("state").ok_or_else(||invalid("Missing state; pass the native Theatre createContentOfSaveFile JSON object"))?;
    let sheets=native_sheets(state)?;
    for id in sheets.keys() {
        // Clip sheets are "<asset> / <clip>"; the legacy per-character sheet is the bare asset id.
        let asset_id=id.split(" / ").next().unwrap_or(id);
        if !s.project_assets.iter().any(|asset|asset.id==asset_id) {
            return Err(invalid(format!("Theatre sheet `{id}` is not a connected asset in the selected project; load the matching project/character before saving")));
        }
    }
    let disk:Value=serde_json::from_slice(&std::fs::read(&project)?).map_err(|e|invalid(format!("Cannot verify existing project JSON: {e}; repair it before saving Theatre state")))?;
    let reused=s.theatre_state.as_ref()==Some(state) && disk.get("theatre")==Some(state);
    if !reused {
        let previous=s.theatre_state.replace(state.clone());
        if let Err(error)=crate::project::write_project_file(s) { s.theatre_state=previous;return Err(error); }
    }
    Ok(receipt(s,project,state,reused))
}

/// One Theatre sheet per clip. Theatre normalizes slashed sheet paths to "a / b", so this is
/// already the id Theatre stores; the UI builds the same string.
pub fn clip_sheet_id(asset: &str, clip: &str) -> String {
    format!("{asset} / {}", clip.trim_start_matches('@'))
}
/// The character's clips: its custom and standard tracks plus the rest pose, which has no
/// timeline of its own (its sheet uses Theatre's default 10 s sequence).
fn clips(s: &mut Session, params: &Value) -> Result<(String, Vec<Value>)> {
    let tracks=crate::motion::cmd_motion_tracks(s,params)?;
    let asset=tracks["character"].as_str().ok_or_else(||invalid("No character loaded"))?.to_string();
    let mut clips:Vec<Value>=tracks["tracks"].as_array().into_iter().flatten()
        .map(|t|json!({"clip":t["id"],"label":t["label"],"duration":t["duration"],"standard":t["standard"],"channels":t["channels"]})).collect();
    clips.push(json!({"clip":"@rest","label":"Rest","duration":null,"standard":true,"channels":[]}));
    for clip in &mut clips {
        let sheet=clip_sheet_id(&asset,clip["clip"].as_str().unwrap_or_default());
        let sheet_val=s.theatre_state.as_ref().and_then(|state|state["sheetsById"].get(&sheet));
        let converted=sheet_val.is_some();
        let keyframed_parts:Vec<String>=sheet_val
            .and_then(|sh|sh["sequence"]["tracksByObject"].as_object())
            .map(|objs|objs.keys().cloned().collect())
            .unwrap_or_default();
        clip["sheet"]=json!(sheet);
        clip["status"]=json!(if converted {"converted"} else {"pending"});
        clip["keyframed_parts"]=json!(keyframed_parts);
    }
    Ok((asset,clips))
}
pub fn list(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset,clips)=clips(s,params)?;
    let legacy=s.theatre_state.as_ref().is_some_and(|state|state["sheetsById"].get(&asset).is_some());
    Ok(json!({"ok":true,"character":asset,"clips":clips,"legacy_sheet":if legacy {json!(asset)} else {Value::Null}}))
}
fn clip_event(s: &Session, stage: &str, clip: &Value, error: Option<&str>) {
    crate::activity::append(json!({"stage":stage,"project":s.project_path,"command":"motion.clips.convert","clip":clip["clip"],"sheet":clip["sheet"],"error":error}));
}
/// Give every clip its own sheet. Existing sheets and keyframes are never replaced; the legacy
/// per-character sheet stays, and its tracks are copied into the first custom clip's sheet
/// (the clip the Animation view opens on) when that sheet does not exist yet.
fn add_clip_sheets(state: &mut Value, asset: &str, clips: &[Value]) -> Vec<Value> {
    if !state["sheetsById"].is_object() { state["sheetsById"]=json!({}); }
    let legacy=state["sheetsById"].get(asset).cloned();
    let first_custom=clips.iter().find(|c|c["standard"]!=true).and_then(|c|c["sheet"].as_str()).map(str::to_owned);
    let mut results=Vec::new();
    for clip in clips {
        let sheet=clip["sheet"].as_str().unwrap_or_default().to_string();
        if state["sheetsById"].get(&sheet).is_some() { results.push(json!({"clip":clip["clip"],"sheet":sheet,"status":"converted","created":false}));continue; }
        let duration=clip["duration"].as_f64().unwrap_or(10.0);
        let mut new_sheet=json!({"sequence":{"type":"PositionalSequence","length":duration,"subUnitsPerUnit":30,"tracksByObject":{}},"staticOverrides":{"byObject":{}}});
        let mut migrated=false;
        if let Some(legacy)=legacy.as_ref().filter(|_|first_custom.as_deref()==Some(sheet.as_str())) {
            new_sheet["sequence"]["tracksByObject"]=legacy["sequence"]["tracksByObject"].clone();
            new_sheet["staticOverrides"]=legacy["staticOverrides"].clone();
            // Keep every copied keyframe inside the clip's sequence.
            let last=legacy["sequence"]["tracksByObject"].as_object().into_iter().flatten()
                .flat_map(|(_,o)|o["trackData"].as_object().into_iter().flatten().flat_map(|(_,t)|t["keyframes"].as_array().into_iter().flatten().filter_map(|k|k["position"].as_f64())))
                .fold(0.0_f64,f64::max);
            new_sheet["sequence"]["length"]=json!(duration.max(last));
            migrated=true;
        }
        state["sheetsById"][&sheet]=new_sheet;
        results.push(json!({"clip":clip["clip"],"sheet":sheet,"status":"converted","created":true,"migrated_legacy_keyframes":migrated}));
    }
    results
}
pub fn convert(s: &mut Session, params: &Value) -> Result<Value> {
    selected_project(s,params,false)?;
    let (asset,clips)=clips(s,params)?;
    let pending:Vec<&Value>=clips.iter().filter(|c|c["status"]!="converted").collect();
    if s.record_user_state { for clip in &pending { clip_event(s,"clip_sheet_started",clip,None); } }
    let mut state=s.theatre_state.clone().unwrap_or_else(||json!({"definitionVersion":"0.4.0","revisionHistory":[],"sheetsById":{}}));
    let legacy_kept=state["sheetsById"].get(&asset).is_some();
    // New sheets start empty: Theatre holds only edits made in it, added on top of the engine
    // motion. A sampled copy of the engine motion goes stale the moment the rig changes.
    let results=add_clip_sheets(&mut state,&asset,&clips);
    let created=results.iter().filter(|r|r["created"]==true).count();
    if created>0 {
        if state["revisionHistory"].as_array().is_some_and(|r|r.is_empty()) {
            state["revisionHistory"]=json!(["mc-clips-convert-0"]);
        }
        let previous=s.theatre_state.replace(state);
        if let Err(error)=crate::project::write_project_file(s) {
            s.theatre_state=previous;
            if s.record_user_state { for clip in &pending { clip_event(s,"clip_sheet_failed",clip,Some(&error.to_string())); } }
            return Err(error);
        }
    }
    if s.record_user_state { for clip in &pending { clip_event(s,"clip_sheet_converted",clip,None); } }
    Ok(json!({"ok":true,"character":asset,"created":created,"clips":results,"legacy_sheet_kept":legacy_kept}))
}

/// Remove a character's Theatre keyframes so its clips play the engine motion again, for one clip
/// (`track`) or all of them. What is removed goes to the project journal, so it can be restored.
pub fn clear(s: &mut Session, params: &Value) -> Result<Value> {
    let project=selected_project(s,params,true)?;
    let asset=params.get("character").and_then(Value::as_str).filter(|v|!v.trim().is_empty())
        .ok_or_else(||invalid("motion.theatre.clear requires character=<asset id>; nothing was cleared"))?.to_string();
    let only=params.get("track").and_then(Value::as_str).map(|t|clip_sheet_id(&asset,t));
    let Some(mut state)=s.theatre_state.clone().filter(|state|!state.is_null()) else {
        return Ok(json!({"ok":true,"character":asset,"cleared":[],"removed_tracks":0}));
    };
    let prefix=format!("{asset} / ");
    let mut cleared=Vec::new();
    let mut removed_tracks=0usize;
    for (id,sheet) in state["sheetsById"].as_object_mut().into_iter().flatten() {
        if !(id==&asset || id.starts_with(&prefix)) || only.as_ref().is_some_and(|o|o!=id) { continue; }
        let tracks=std::mem::replace(&mut sheet["sequence"]["tracksByObject"],json!({}));
        let overrides=std::mem::replace(&mut sheet["staticOverrides"]["byObject"],json!({}));
        let parts:Vec<String>=tracks.as_object().into_iter().flatten().chain(overrides.as_object().into_iter().flatten()).map(|(k,_)|k.clone()).collect();
        if parts.is_empty() { continue; }
        removed_tracks+=parts.len();
        cleared.push(json!({"sheet":id,"parts":parts,"removed":{"tracksByObject":tracks,"staticOverrides":overrides}}));
    }
    if cleared.is_empty() { return Ok(json!({"ok":true,"character":asset,"project":project,"cleared":[],"removed_tracks":0})); }
    let previous=s.theatre_state.replace(state);
    if let Err(error)=crate::project::write_project_file(s) { s.theatre_state=previous;return Err(error); }
    crate::activity::append(json!({"stage":"theatre_cleared","project":s.project_path,"command":"motion.theatre.clear","character":asset,"cleared":cleared}));
    let summary:Vec<Value>=cleared.iter().map(|c|json!({"sheet":c["sheet"],"parts":c["parts"]})).collect();
    Ok(json!({"ok":true,"character":asset,"project":project,"cleared":summary,"removed_tracks":removed_tracks}))
}
