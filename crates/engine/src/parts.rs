//! Part visibility / solo / review state on the active asset.

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

/// Ensure every part of the active asset is visible and none are soloed.
pub fn cmd_part_show_all(s: &mut Session, _params: &Value) -> Result<Value> {
    let asset = s
        .current_asset_mut()
        .ok_or_else(|| EngineError::InvalidParams("no active asset in session".into()))?;
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

    let asset = s
        .current_asset_mut()
        .ok_or_else(|| EngineError::InvalidParams("no active asset in session".into()))?;

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

