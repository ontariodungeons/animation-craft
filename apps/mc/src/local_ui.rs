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
            match sessions.as_slice(){
                [] => None,
                [one] => Some((one["endpoint"].as_str().unwrap_or_default().trim_start_matches("unix:").to_string(), one.clone())),
                _ => {
                    // A pinned endpoint turns "which of these?" into an answer the caller can repeat.
                    let pinned = crate::studios::pinned_endpoint().and_then(|endpoint| sessions.iter().find(|s| s["endpoint"].as_str() == Some(endpoint.as_str())).cloned());
                    match pinned {
                        Some(one) => Some((one["endpoint"].as_str().unwrap_or_default().trim_start_matches("unix:").to_string(), one)),
                        None => match pick_by_cwd(&sessions,&std::env::current_dir().unwrap_or_default()){
                            Some(one) => Some((one["endpoint"].as_str().unwrap_or_default().trim_start_matches("unix:").to_string(), one)),
                            None => return Err("Multiple Studios available; pass project=<path> or --bridge <port>. `mc studios` lists them, `mc studios <port>` pins one, and `mc backstage --print project=<path>` reads the same project without a window. Nothing switched.".into()),
                        }
                    }
                }
            }
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

/// With several Studios and no project named, the one whose project folder contains `cwd`
/// (the deepest, if nested). Ambiguity is not guessed: a tie or no match picks nothing.
fn pick_by_cwd(sessions: &[Value], cwd: &Path) -> Option<Value> {
    let cwd = std::fs::canonicalize(cwd).ok()?;
    let mut best: Option<(usize, &Value)> = None;
    let mut tie = false;
    for s in sessions {
        let Some(dir) = s["canonical_project"].as_str().map(Path::new).and_then(Path::parent) else { continue };
        if !cwd.starts_with(dir) { continue; }
        let depth = dir.components().count();
        match best {
            Some((d, _)) if d > depth => {}
            Some((d, _)) if d == depth => tie = true,
            _ => { best = Some((depth, s)); tie = false; }
        }
    }
    if tie { None } else { best.map(|(_, s)| s.clone()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn studio(project: &str) -> Value { json!({"canonical_project": project, "endpoint": format!("unix:/s/{project}")}) }

    #[test]
    fn the_studio_whose_project_folder_contains_the_agent_is_chosen_and_ties_are_not_guessed() {
        let root = std::env::temp_dir().join(format!("pick-cwd-{}", std::process::id()));
        let (a, b, inner) = (root.join("a"), root.join("b"), root.join("a/inner"));
        for d in [&a, &b, &inner] { std::fs::create_dir_all(d).unwrap(); }
        let (pa, pb, pi) = (a.join("A.magic"), b.join("B.magic"), inner.join("I.magic"));
        let sessions = vec![studio(pa.to_str().unwrap()), studio(pb.to_str().unwrap())];
        let pick = |s: &[Value], cwd: &Path| pick_by_cwd(s, cwd).map(|v| v["canonical_project"].as_str().unwrap().to_owned());
        assert_eq!(pick(&sessions, &a), Some(pa.to_string_lossy().into()));
        assert_eq!(pick(&sessions, &b), Some(pb.to_string_lossy().into()));
        assert_eq!(pick(&sessions, &root), None, "a folder above every project matches none");
        let nested = vec![studio(pa.to_str().unwrap()), studio(pi.to_str().unwrap())];
        assert_eq!(pick(&nested, &inner), Some(pi.to_string_lossy().into()), "the deepest folder wins");
        let twins = vec![studio(pa.to_str().unwrap()), studio(a.join("Other.magic").to_str().unwrap())];
        assert_eq!(pick(&twins, &a), None, "two projects in one folder is ambiguous");
        let _ = std::fs::remove_dir_all(&root);
    }
}
