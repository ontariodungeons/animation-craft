//! New/Open use the actual adjacent animationcraft-cli executable and this window's private bridge.
//! The correlated bridge reply is rendered by the initiating UI handler, without recursive Open.
use serde_json::{Value, json};
use std::{path::{Path, PathBuf}, sync::Mutex};
use tauri::Manager;

#[derive(Default)]
pub struct Pending(Mutex<Option<(String, String)>>);

struct Request<'a>(&'a Pending);
impl Drop for Request<'_> {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.0.0.lock() { *pending = None; }
    }
}

pub fn is_ui_request(app: &tauri::AppHandle, id: &str, params: &Value) -> bool {
    app.state::<Pending>().0.lock().ok().is_some_and(|pending| {
        pending.as_ref().is_some_and(|(command, token)| {
            command == id && params["_ui_dispatch_token"].as_str() == Some(token.as_str())
        })
    })
}

fn program() -> Result<PathBuf, String> {
    let shell = std::env::current_exe().map_err(|e| e.to_string())?;
    let adjacent = shell.parent().ok_or("Studio executable directory unavailable")?.join("animationcraft-cli");
    if !adjacent.is_file() {
        return Err(format!("New/Open require the CLI beside Studio: {}. Build/install animationcraft-cli with the shell; no direct-engine fallback was used.", adjacent.display()));
    }
    Ok(adjacent)
}

fn invoke(program: &Path, endpoint: &str, id: &str, params: &Value) -> Result<Value, String> {
    let mut command = std::process::Command::new(program);
    command.args(["--bridge", endpoint, "exec", id]);
    for (key, value) in params.as_object().ok_or("CLI parameters must be an object")? {
        command.arg(format!("{key}={value}"));
    }
    let output = command.output().map_err(|e| format!("Cannot launch {}: {e}", program.display()))?;
    if !output.status.success() {
        return Err(format!("{} {id} failed ({}): {}",program.display(),output.status,String::from_utf8_lossy(&output.stderr).trim()));
    }
    let mut result: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("CLI {id} returned invalid JSON: {e}"))?;
    if !result.is_object() || result["ok"] == false {
        return Err(format!("CLI {id} did not return a successful project result: {result}"));
    }
    result["_cli"] = json!({"program":program,"command":id,"endpoint":endpoint,"exit_success":true});
    Ok(result)
}

pub fn execute(app: &tauri::AppHandle, id: &str, mut params: Value) -> Result<Value, String> {
    #[cfg(unix)]
    let endpoint = format!("unix:{}",app.try_state::<crate::bridge::LocalSocket>().ok_or("Studio private bridge unavailable; no command executed")?.0.display());
    #[cfg(not(unix))]
    let endpoint = return Err("New/Open CLI transport requires the private Studio bridge on this platform".into());
    let token = crate::feedback::new_request();
    let pending = app.state::<Pending>();
    {
        let mut slot = pending.0.lock().map_err(|e| e.to_string())?;
        if slot.is_some() { return Err("Another New/Open command is still running".into()); }
        *slot = Some((id.to_owned(),token.clone()));
    }
    let _request = Request(&pending);
    params["_ui_dispatch_token"] = json!(token);
    let mut result = invoke(&program()?, &endpoint, id, &params)?;
    if result["reused_session"] == true { return Ok(result); }
    let state = app.state::<crate::Engine>().with(None,crate::state_of)?;
    for (key,value) in state.as_object().ok_or("Studio state unavailable after CLI command")? {
        result[key] = value.clone();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    fn fixture(body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path=std::env::temp_dir().join(format!("animationcraft-cli-fixture-{}-{}",std::process::id(),crate::feedback::new_request()));
        std::fs::write(&path,format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o700)).unwrap();path
    }
    #[test]
    #[cfg(unix)]
    fn cli_receipt_preserves_problems_and_uses_explicit_bridge() {
        let cli=fixture("[ \"$1\" = '--bridge' ] && [ \"$2\" = 'unix:/chosen.sock' ] && [ \"$3\" = 'exec' ] && [ \"$4\" = 'project.intake' ] || exit 8\nprintf '%s' '{\"ok\":true,\"project\":\"/selected/A&B.acproj\",\"intake_ready\":false,\"problems\":[\"Needs transcript\"],\"_ui_engine_revision\":9}'");
        let result=invoke(&cli,"unix:/chosen.sock","project.intake",&json!({"name":"A&B"})).unwrap();
        assert_eq!(result["problems"][0],"Needs transcript");assert_eq!(result["_ui_engine_revision"],9);assert_eq!(result["_cli"]["exit_success"],true);
        std::fs::remove_file(cli).unwrap();
    }
    #[test]
    #[cfg(unix)]
    fn process_failure_and_invalid_or_error_payloads_are_not_success() {
        for body in ["echo 'unavailable bridge' >&2; exit 3","printf 'not-json'","printf '%s' '{\"ok\":false}'"] {
            let cli=fixture(body);assert!(invoke(&cli,"unix:/chosen.sock","project.open",&json!({})).is_err());std::fs::remove_file(cli).unwrap();
        }
        assert!(invoke(Path::new("/missing/animationcraft-cli"),"unix:/chosen.sock","project.open",&json!({})).is_err());
    }
}
