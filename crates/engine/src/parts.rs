//! Part visibility / solo / review state on the active asset.

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

// Explicit targets do not borrow another caller's active selection.
fn target<'a>(s: &'a mut Session, params: &Value) -> Result<&'a mut crate::Asset> {
    if let Some(character) = params.get("character") {
        crate::motion_authoring::project_guard(s, params)?;
        let id = character
            .as_str()
            .ok_or_else(|| EngineError::InvalidParams("character must be an asset ID".into()))?;
        let selected = s.active_asset.clone();
        let loaded = crate::project::cmd_project_load_asset(s, &json!({"id":id}));
        s.active_asset = selected;
        loaded?;
        return s
            .assets
            .get_mut(id)
            .ok_or_else(|| EngineError::InvalidParams("Target asset unavailable".into()));
    }
    if params.get("expected_project").is_some() {
        crate::motion_authoring::project_guard(s, params)?;
    }
    s.current_asset_mut()
        .ok_or_else(|| EngineError::InvalidParams("no active asset in session".into()))
}

/// Ensure every part of the active asset is visible and none are soloed.
pub fn cmd_part_show_all(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = target(s, params)?;
    for part in &mut asset.parts {
        part.visible = true;
        part.solo = false;
    }
    Ok(json!({
        "ok": true,
        "command": "part.show_all",
        "asset_id": asset.id,
        "parts": asset.parts,
    }))
}

/// Update visibility/solo/review state of one part (or `"*"`/`"all"` for every part) on the active asset.
pub fn cmd_part_set_state(s: &mut Session, params: &Value) -> Result<Value> {
    let part_name = params
        .get("part")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing 'part' parameter".into()))?;
    let visible = params.get("visible").and_then(Value::as_bool);
    let solo = params.get("solo").and_then(Value::as_bool);
    let verified = params.get("verified").and_then(Value::as_bool);

    let asset = target(s, params)?;

    if part_name == "*" || part_name.eq_ignore_ascii_case("all") {
        for p in &mut asset.parts {
            if let Some(v) = visible {
                p.visible = v;
            }
            if let Some(so) = solo {
                p.solo = so;
            }
            if let Some(ver) = verified {
                p.verified = ver;
            }
        }
        return Ok(json!({
            "ok": true,
            "command": "part.set_state",
            "part": "*",
            "parts": asset.parts,
        }));
    }

    let entry = asset
        .parts
        .iter_mut()
        .find(|p| p.name == part_name)
        .ok_or_else(|| {
            EngineError::InvalidParams(format!("part `{part_name}` not found in active asset"))
        })?;

    if let Some(v) = visible {
        entry.visible = v;
    }
    if let Some(so) = solo {
        entry.solo = so;
    }
    if let Some(ver) = verified {
        entry.verified = ver;
    }

    let updated = entry.clone();
    Ok(json!({
        "ok": true,
        "command": "part.set_state",
        "part": updated.name,
        "visible": updated.visible,
        "solo": updated.solo,
        "verified": updated.verified,
    }))
}
