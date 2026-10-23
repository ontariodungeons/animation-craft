//! Project-matched access to existing validated Studio endpoints. Never starts or switches Studio.
use serde_json::{Value, json};
use std::path::Path;
pub fn call(target: &str, id: &str, mut params: Value) -> Result<Value, String> {
    #[cfg(unix)]
    {
        let requested = params["_expected_project"]
            .as_str()
            .or(params["project"].as_str())
            .map(str::to_owned);
        let expected = requested
            .as_deref()
            .map(|p| magicstudio_engine::studio_sessions::canonical(Path::new(p)))
            .transpose()?;
        let selected = if let Some(path) = target.strip_prefix("unix:") {
            let feedback = magicstudio_engine::studio_sessions::request(
                Path::new(path),
                "ui.feedback",
                json!({}),
            )?;
            Some((path.to_string(), feedback))
        } else if let Some(project) = &expected {
            magicstudio_engine::studio_sessions::for_project(project)?.map(|v| {
                (
                    v["endpoint"]
                        .as_str()
                        .unwrap_or_default()
                        .trim_start_matches("unix:")
                        .to_string(),
                    v,
                )
            })
        } else {
            let discovery = magicstudio_engine::studio_sessions::discover();
            let sessions = discovery["sessions"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            match sessions.as_slice(){[]=>None,[one]=>Some((one["endpoint"].as_str().unwrap_or_default().trim_start_matches("unix:").to_string(),one.clone())),_=>return Err("Multiple Studios available; pass project=<canonical project path> or an explicit pinned endpoint. Nothing switched.".into())}
        };
        if let Some((path, feedback)) = selected {
            if let Some(expected) = expected {
                let active = feedback["active_project"]
                    .as_str()
                    .or(feedback["canonical_project"].as_str())
                    .ok_or("Studio has no project; attachment refused")?;
                if magicstudio_engine::studio_sessions::canonical(Path::new(active))? != expected
                {
                    return Err(
                        "Pinned Studio changed project; command refused, no automatic reassignment"
                            .into(),
                    );
                }
                params["_expected_project"] = json!(expected);
                let instance = Path::new(&path)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .ok_or("Missing Studio instance")?;
                if let Some(pin) = params["_expected_instance"].as_str() {
                    if pin != instance {
                        return Err(
                            "Pinned Studio instance changed; explicit reattach required".into()
                        );
                    }
                }
                params["_expected_instance"] = json!(instance);
                if id != "ui.feedback"
                    && id != "ui.components"
                    && feedback["project_guard_version"] != 1
                {
                    return Err("Matched Studio requires the current guarded-attachment build before commands can run. No fallback writer or project switch was used.".into());
                }
            }
            let mut result = if id=="ui.feedback"||id=="ui.components" {
                magicstudio_engine::studio_sessions::request(Path::new(&path),id,params)?
            } else {
                magicstudio_engine::studio_sessions::request_slow(Path::new(&path),id,params)?
            };
            if result.is_object() {
                result["endpoint"] = json!(format!("unix:{path}"));
                result["instance"] = json!(Path::new(&path).file_name().and_then(|v| v.to_str()));
            }
            return Ok(result);
        }
        if id == "ui.feedback" {
            return Ok(
                json!({"ok":true,"connection":"not_observed_open","project":requested,"ui_ready":false,"unsaved_state":"unknown","recovery":"No matching responsive Studio. No app launched or switched."}),
            );
        }
    }
    Err("No matching connected Studio; no command executed or app launched".into())
}
