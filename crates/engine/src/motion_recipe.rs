//! One CLI/MCP batch; every render and placement goes through the selected live Studio.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run(s: &mut Session, p: &Value) -> Result<Value> {
    if s.project_path.is_some() {
        return Err(EngineError::InvalidParams("Run mc exec motion.recipe path=<song.magic> studio=<port> without --bridge; the batch connects each command to Studio itself".into()));
    }
    let raw = p["path"].as_str().ok_or_else(|| EngineError::InvalidParams("path is required".into()))?;
    let project = crate::workspace::resolve_workspace_path(s, raw).canonicalize()?;
    if !project.is_file() || project.extension().is_none_or(|ext| ext != "magic") {
        return Err(EngineError::InvalidParams("project must be an existing .magic file".into()));
    }
    let bridge = p["studio"].as_u64().filter(|port| *port > 0 && *port <= 65535)
        .ok_or_else(|| EngineError::InvalidParams("studio must be the selected Studio port".into()))?;
    let directory = project.parent().ok_or_else(|| EngineError::InvalidParams("project has no parent".into()))?.join("animation");
    let root = crate::workspace::workspace_root(s);
    let mc = std::env::current_exe()?.with_file_name("mc");
    if !mc.is_file() {
        return Err(EngineError::InvalidParams("Build mc beside this executable before running a recipe".into()));
    }
    let request = p["_request_id"].as_str().map(str::to_owned).unwrap_or_else(|| format!("motion-recipe-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()));
    let context = json!({"_request_id": request, "_origin": "motion.recipe"});
    let _context = crate::activity::enter("motion.recipe", &context);
    crate::activity::command(Some(&project), "motion.recipe", &context, "dispatch_running", None);
    let started = std::time::Instant::now();
    let result: Result<Value> = (|| {
        crate::workspace::run_tool_in(&root, "python3", &[
            root.join("scripts/compile_motion_specs.py").to_string_lossy().into_owned(),
            directory.to_string_lossy().into_owned(),
        ])?;
        let mut args = vec![root.join("scripts/render_motion_recipe.py").to_string_lossy().into_owned(),
            directory.join("recipe/04-render-clips.jsonl").to_string_lossy().into_owned(),
            "--mc".into(), mc.to_string_lossy().into_owned(), "--bridge".into(), bridge.to_string(),
            "--project".into(), project.to_string_lossy().into_owned(), "--request-id".into(), request];
        if let Some(scene) = p["scene"].as_str() { args.extend(["--scene".into(), scene.into()]); }
        if p["rerender"] == true { args.push("--force".into()); }
        if p["place"].as_bool().unwrap_or(true) {
            args.extend(["--place".into(), directory.join("recipe/05-place-in-video.jsonl").to_string_lossy().into_owned()]);
        }
        let output = crate::workspace::run_tool_in(&root, "python3", &args)?;
        let steps: Vec<Value> = output.lines().map(serde_json::from_str).collect::<std::result::Result<_, _>>()
            .map_err(|e| EngineError::ExternalTool("motion recipe".into(), e.to_string()))?;
        Ok(json!({"ok": true, "project": project, "steps": steps,
                  "receipts": directory.join("recipe/receipts/current.json")}))
    })();
    match &result {
        Ok(value) => {
            crate::activity::result(Some(&project), "motion.recipe", value, started.elapsed().as_millis());
            crate::activity::command(Some(&project), "motion.recipe", &context, "engine_success", None);
        }
        Err(error) => { crate::activity::command(Some(&project), "motion.recipe", &context, "engine_error", Some(&error.to_string())); }
    }
    result
}
