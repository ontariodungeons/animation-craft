//! Opaque Theatre save-file persistence in the selected project. No animation evaluation or rig edits.
use std::path::PathBuf;
use serde_json::{Value, json};
use crate::{EngineError, Result, Session};

fn invalid(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }
fn selected_project(s: &Session, params: &Value, required: bool) -> Result<PathBuf> {
    let selected=s.project_path.as_deref().ok_or_else(||invalid("No project is selected; open the intended .acproj before saving Theatre state"))?;
    let selected=std::fs::canonicalize(selected).map_err(|e|invalid(format!("Selected project is unavailable: {e}; reopen the intended .acproj before retrying")))?;
    let expected=params.get("expected_project");
    if required || expected.is_some() {
        let expected=expected.and_then(Value::as_str).filter(|v|!v.trim().is_empty())
            .ok_or_else(||invalid("motion.theatre.save requires expected_project=<selected .acproj>; no state was saved"))?;
        let expected=crate::project::project_file_path(s,&json!({"path":expected}))?;
        let expected=std::fs::canonicalize(expected).map_err(|e|invalid(format!("expected_project is unavailable: {e}; select the existing intended .acproj")))?;
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
    if s.project_path.is_none() { return Ok(json!({"ok":true,"available":false,"saved":false,"status":"unavailable","state":null,"reason":"No project selected; open the intended .acproj"})); }
    let project=selected_project(s,params,false)?;
    match &s.theatre_state {
        None=>Ok(json!({"ok":true,"available":true,"saved":false,"status":"no_saved_state","project":project,"state":null})),
        Some(state)=>{native_sheets(state)?;Ok(receipt(s,project,state,true))}
    }
}

pub fn save(s: &mut Session, params: &Value) -> Result<Value> {
    let project=selected_project(s,params,true)?;
    let state=params.get("state").ok_or_else(||invalid("Missing state; pass the native Theatre createContentOfSaveFile JSON object"))?;
    let sheets=native_sheets(state)?;
    for id in sheets.keys() {
        if !s.project_assets.iter().any(|asset|asset.id==*id) {
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
