//! Bounded Blender jobs. Studio reads metadata; evaluation and rendering belong to the worker.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const UNIT: &str = "magicstudio-animation-worker.service";
const JSON_LIMIT: u64 = 16 * 1024 * 1024;
const PROPERTIES: &[&str] = &[
    "LoadState",
    "ActiveState",
    "SubState",
    "Description",
    "InvocationID",
    "Result",
    "ExecMainStatus",
    "ExecMainCode",
    "MemoryCurrent",
    "MemoryPeak",
    "MemoryMax",
    "MemorySwapCurrent",
    "MemorySwapMax",
    "CPUUsageNSec",
    "MainPID",
];
fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}
fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    // Limit the read itself, including a file that grows after metadata inspection.
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(JSON_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > JSON_LIMIT {
        return Err(invalid("Animation metadata exceeds 16 MiB"));
    }
    Ok(bytes)
}
fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&read_bytes(path)?).map_err(|e| invalid(e.to_string()))
}
fn write_json(path: &Path, value: &Value) -> Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(value).map_err(|e| invalid(e.to_string()))?,
    )?;
    fs::rename(temporary, path)?;
    Ok(())
}
fn worker_lock() -> Result<fs::File> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| invalid("A user systemd runtime is required"))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(PathBuf::from(runtime).join("magicstudio-animation-worker.lock"))?;
    file.try_lock().map_err(|_| {
        invalid("Another animation command is submitting or cancelling; retry shortly")
    })?;
    Ok(file)
}
fn service() -> Result<Value> {
    let output = Command::new("systemctl")
        .args(["--user", "show", UNIT])
        .arg(format!("--property={}", PROPERTIES.join(",")))
        .output()?;
    let mut state = json!({});
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some((key, value)) = line.split_once('=') {
            state[key] = json!(value);
        }
    }
    if state["LoadState"].is_null()
        || (!output.status.success() && state["LoadState"] != "not-found")
    {
        return Err(EngineError::ExternalTool(
            "systemctl".into(),
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }
    Ok(state)
}
fn occupied(state: &Value) -> bool {
    !matches!(state["ActiveState"].as_str(), Some("inactive" | "failed"))
}
fn number(state: &Value, key: &str) -> Value {
    state[key]
        .as_str()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v != u64::MAX)
        .map_or(Value::Null, |v| json!(v))
}
fn telemetry(state: &Value) -> Value {
    json!({"memory_current_bytes":number(state,"MemoryCurrent"),"memory_peak_bytes":number(state,"MemoryPeak"),
        "memory_max_bytes":number(state,"MemoryMax"),"swap_current_bytes":number(state,"MemorySwapCurrent"),
        "swap_max_bytes":number(state,"MemorySwapMax"),"cpu_usage_ns":number(state,"CPUUsageNSec")})
}
fn root(s: &Session) -> Result<PathBuf> {
    s.project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.join("animation/blender-jobs"))
        .ok_or_else(|| invalid("Open a .magic project first"))
}
fn job(s: &Session, p: &Value) -> Result<(String, PathBuf)> {
    let id = p["job"]
        .as_str()
        .ok_or_else(|| invalid("job is required"))?;
    if !id.starts_with("magicstudio-blender-")
        || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err(invalid("Invalid Blender job id"));
    }
    let dir = root(s)?.join(id);
    if !dir.join("request.json").is_file() {
        return Err(invalid("Job not found in this project"));
    }
    let request = read_json(&dir.join("request.json"))?;
    if request["project"].is_string() && request["project"] != json!(s.project_path) {
        return Err(invalid("Job belongs to another project"));
    }
    Ok((id.into(), dir))
}
fn bounded(p: &Value, key: &str, default: u64, min: u64, max: u64) -> Result<u64> {
    let value = match p.get(key) {
        None => default,
        Some(v) => v
            .as_u64()
            .ok_or_else(|| invalid(format!("{key} must be an integer")))?,
    };
    if !(min..=max).contains(&value) {
        return Err(invalid(format!("{key} must be {min}–{max}")));
    }
    Ok(value)
}
fn entry(s: &Session, p: &Value) -> Result<crate::ProjectAsset> {
    let id = p["character"]
        .as_str()
        .ok_or_else(|| invalid("character is required for legacy migration"))?;
    let mut asset = s
        .project_assets
        .iter()
        .find(|a| a.id == id)
        .cloned()
        .ok_or_else(|| invalid("Character is not registered"))?;
    if let Some(loaded) = s.assets.get(id) {
        asset.parts = loaded.parts.clone();
    }
    let path = asset
        .path
        .as_ref()
        .ok_or_else(|| invalid("Migration requires a saved rig file"))?;
    asset.path =
        Some(crate::workspace::resolve_workspace_path(s, &path.to_string_lossy()).canonicalize()?);
    Ok(asset)
}
pub fn cmd_blender_actions(s: &mut Session, p: &Value) -> Result<Value> {
    let asset = entry(s, p)?;
    let path = asset
        .path
        .as_ref()
        .ok_or_else(|| invalid("Missing rig path"))?;
    let raw = read_json(path)?;
    let spec = if raw["tracks"].is_array() {
        &raw
    } else {
        &raw["effectcraft"]
    };
    let source = spec["tracks"]
        .as_array()
        .ok_or_else(|| invalid("No saved layered animation tracks"))?;
    if source.len() > 1024 {
        return Err(invalid("Too many animation tracks"));
    }
    let tracks: Vec<Value> = source
        .iter()
        .map(|t| {
            json!({"id":t["id"],"duration":t["duration"],
        "loop":t["loop"].as_bool().unwrap_or(false),"standard":false})
        })
        .collect();
    Ok(
        json!({"character":asset.id,"tracks":tracks,"layers":[],"canvas":spec.get("canvas").unwrap_or(&raw["canvas_size"]),"spec":path}),
    )
}
/// Only the headless helper evaluates legacy motion; never attach a Studio project here.
pub fn cmd_blender_prepare(s: &mut Session, p: &Value) -> Result<Value> {
    if s.project_path.is_some() {
        return Err(invalid(
            "Preparation requires a separate headless worker session",
        ));
    }
    let path = PathBuf::from(
        p["request"]
            .as_str()
            .ok_or_else(|| invalid("request is required"))?,
    );
    let request = read_json(&path)?;
    let active = service()?;
    if active["Description"] != request["job"]
        || active["InvocationID"].as_str() != std::env::var("INVOCATION_ID").ok().as_deref()
        || !occupied(&active)
    {
        return Err(invalid(
            "Preparation must run inside the selected bounded worker service",
        ));
    }
    let mut asset: crate::ProjectAsset =
        serde_json::from_value(request["asset"].clone()).map_err(|e| invalid(e.to_string()))?;
    let spec = asset
        .path
        .clone()
        .ok_or_else(|| invalid("Missing rig path"))?;
    let bytes = read_bytes(&spec)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let mut raw: Value = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    let base = spec
        .parent()
        .ok_or_else(|| invalid("Missing rig directory"))?;
    let snapshot = path.with_file_name("rig-snapshot");
    fs::create_dir_all(&snapshot)?;
    fs::write(snapshot.join("original-rig.json"), bytes)?;
    let resolve = |value: &mut Value| {
        if let Some(text) = value.as_str().filter(|v| !v.is_empty()) {
            let original = PathBuf::from(text);
            *value = json!(if original.is_absolute() {
                original
            } else {
                base.join(original)
            });
        }
    };
    for key in [
        "source",
        "source_image",
        "photocraft_psd",
        "cutout_asset_sheet",
    ] {
        resolve(&mut raw[key]);
    }
    if let Some(layers) = raw["layers"].as_array_mut() {
        for layer in layers {
            resolve(&mut layer["file"]);
        }
    }
    if let Some(parts) = raw["parts"].as_array_mut() {
        for part in parts {
            for key in ["cropped_path", "full_layer_path"] {
                resolve(&mut part[key]);
            }
        }
    }
    if let Some(layers) = raw["layers"].as_object() {
        for id in layers.keys() {
            if id.is_empty()
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            {
                return Err(invalid("Unsafe keyed layer id"));
            }
            fs::copy(
                base.join(format!("{id}.png")),
                snapshot.join(format!("{id}.png")),
            )?;
        }
    }
    let snapshot_path = snapshot.join("rig.json");
    write_json(&snapshot_path, &raw)?;
    asset.path = Some(snapshot_path);
    let mut isolated = Session {
        project_assets: vec![asset],
        ..Session::default()
    };
    let mut meta = crate::motion::cmd_motion_tracks(&mut isolated, &request["motion"])?;
    let sample = crate::motion::cmd_motion_sample(&mut isolated, &request["motion"])?;
    meta["rig_sha256"] = json!(hash);
    meta["spec"] = json!(spec);
    write_json(
        &path.with_file_name("sample.json"),
        &json!({"meta":meta,"sample":sample}),
    )?;
    Ok(json!({"ok":true,"prepared":true}))
}
pub fn cmd_blender_start(s: &mut Session, p: &Value) -> Result<Value> {
    let _slot = worker_lock()?;
    let active = service()?;
    if occupied(&active) {
        return Err(invalid(format!(
            "Animation worker is busy with {}; finish or cancel it first",
            active["Description"]
        )));
    }
    let memory = bounded(p, "memory_mb", 4096, 512, 8192)?;
    let seconds = bounded(p, "timeout_seconds", 300, 10, 3600)?;
    let mut request = json!({"resolution_percent":bounded(p,"resolution_percent",50,10,100)?,
        "max_frames":bounded(p,"max_frames",90,1,18000)?,"unit":UNIT,"project":s.project_path,"character":p["character"],"track":p["track"]});
    if let Some(path) = p["blend"].as_str() {
        let path = crate::workspace::resolve_workspace_path(s, path).canonicalize()?;
        if path.extension().is_none_or(|e| e != "blend") {
            return Err(invalid("blend must be an existing .blend file"));
        }
        request["blend"] = json!(path);
    } else {
        let actions = cmd_blender_actions(s, p)?;
        if !p["track"].is_string()
            || !actions["tracks"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| t["id"] == p["track"])
        {
            return Err(invalid("Choose a saved animation track"));
        }
        request["asset"] =
            serde_json::to_value(entry(s, p)?).map_err(|e| invalid(e.to_string()))?;
        request["motion"] = json!({"character":p["character"],"track":p["track"],"fps":p["fps"].as_f64().unwrap_or(30.0)});
        // Use the rig's authored rate unless explicitly overridden.
        if p.get("fps").is_none() {
            request["motion"].as_object_mut().unwrap().remove("fps");
        }
        let executable = std::env::current_exe()?.with_file_name("mc");
        if !executable.is_file() {
            return Err(invalid(
                "Install mc beside Magic Studio to prepare legacy animations",
            ));
        }
        request["mc"] = json!(executable);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| invalid(e.to_string()))?
        .as_nanos();
    let id = format!("magicstudio-blender-{}-{stamp}", std::process::id());
    request["job"] = json!(id);
    let dir = root(s)?.join(&id);
    fs::create_dir_all(&dir)?;
    let request_path = dir.join("request.json");
    write_json(&request_path, &request)?;
    write_json(
        &dir.join("job.json"),
        &json!({"job":id,"job_id":id,"unit":UNIT,"project":s.project_path,
        "created_ns":stamp.to_string(),"character":p["character"],"track":p["track"],"source_blend":request["blend"]}),
    )?;
    write_json(
        &dir.join("progress.json"),
        &json!({"state":if request["blend"].is_string(){"loading"}else{"preparing"}}),
    )?;
    let worker = dir.join("worker.py");
    fs::write(
        &worker,
        include_str!("../../../pipeline/blender_animation_worker.py"),
    )?;
    let blender = std::env::var("BLENDER_BIN").unwrap_or_else(|_| "blender".into());
    // systemd-run waits for the start transaction while holding the start/cancel lock.
    let output = Command::new("systemd-run")
        .args(["--user", "--quiet", "--collect"])
        .arg(format!("--unit={UNIT}"))
        .arg(format!("--description={id}"))
        .arg(format!("--property=MemoryMax={memory}M"))
        .args([
            "--property=MemorySwapMax=0",
            "--property=CPUQuota=200%",
            "--property=TimeoutStopSec=5",
            "--property=KillMode=control-group",
            "--property=MemoryAccounting=yes",
            "--property=CPUAccounting=yes",
        ])
        .arg(format!("--property=RuntimeMaxSec={seconds}"))
        .arg(format!(
            "--property=StandardOutput=append:{}",
            dir.join("worker.log").display()
        ))
        .arg(format!(
            "--property=StandardError=append:{}",
            dir.join("worker.log").display()
        ))
        .arg(format!(
            "--property=ExecStopPost=/usr/bin/python3 {} --finalize {}",
            systemd_quote(&worker),
            systemd_quote(&request_path)
        ))
        .args([
            "--",
            &blender,
            "--background",
            "--factory-startup",
            "--disable-autoexec",
            "--threads",
            "2",
            "--python-exit-code",
            "1",
            "--python",
        ])
        .arg(&worker)
        .arg("--")
        .arg(&request_path)
        .output();
    let submission_error = match output {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(String::from_utf8_lossy(&output.stderr).into_owned()),
        Err(error) => Some(error.to_string()),
    };
    if let Some(error) = submission_error {
        write_json(
            &dir.join("result.json"),
            &json!({"job":id,"job_id":id,"project":s.project_path,"unit":UNIT,
            "invocation_id":Value::Null,"state":"failed","termination_reason":"submission","updated_at":now(),
            "telemetry":telemetry(&json!({})),"error":error}),
        )?;
        return Err(EngineError::ExternalTool("systemd-run".into(), error));
    }
    Ok(
        json!({"ok":true,"job":id,"state":"submitted","directory":dir,"memory_limit_mb":memory,"timeout_seconds":seconds}),
    )
}
fn systemd_quote(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    )
}
pub fn cmd_blender_list(s: &mut Session, _: &Value) -> Result<Value> {
    let root = root(s)?;
    let mut jobs = Vec::new();
    if root.is_dir() {
        for entry in fs::read_dir(root)? {
            let path = entry?.path().join("job.json");
            if path.is_file() {
                let value = read_json(&path)?;
                if !value["project"].is_string() || value["project"] == json!(s.project_path) {
                    jobs.push(value);
                }
            }
        }
    }
    jobs.sort_by(|a, b| b["created_ns"].as_str().cmp(&a["created_ns"].as_str()));
    Ok(json!({"jobs":jobs}))
}
pub fn cmd_blender_status(s: &mut Session, p: &Value) -> Result<Value> {
    let (id, dir) = job(s, p)?;
    let service = service()?;
    let invocation = read_json(&dir.join("invocation.json")).ok();
    let matches = service["Description"] == id
        && invocation
            .as_ref()
            .is_none_or(|i| i["invocation_id"] == service["InvocationID"]);
    if matches && occupied(&service) {
        let progress = read_json(&dir.join("progress.json")).ok();
        return Ok(
            json!({"job":id,"job_id":id,"project":s.project_path,"unit":UNIT,"invocation_id":service["InvocationID"],"updated_at":now(),
            "state":if dir.join("cancel-requested").exists() || service["ActiveState"]=="deactivating"{"cancelling"}else{"running"},
            "phase":progress.as_ref().map(|v|&v["state"]),"frames_completed":progress.as_ref().map(|v|&v["completed"]),"frames_total":progress.as_ref().map(|v|&v["total"]),
            "progress":progress,"service_state":service["ActiveState"],"main_pid":number(&service,"MainPID"),"telemetry":telemetry(&service),
            "service_result":service["Result"],"exec_main_status":number(&service,"ExecMainStatus"),"termination_reason":Value::Null,"log":dir.join("worker.log")}),
        );
    }
    if dir.join("result.json").is_file() {
        let mut value = read_json(&dir.join("result.json"))?;
        value["job"] = json!(id);
        return Ok(value);
    }
    Ok(
        json!({"job":id,"job_id":id,"project":s.project_path,"unit":UNIT,"invocation_id":invocation.map(|v|v["invocation_id"].clone()),
        "updated_at":now(),"state":"failed","termination_reason":"missing_receipt","telemetry":telemetry(&json!({})),
        "detail":"Worker is no longer active and left no terminal receipt; success and resource usage are unknown","log":dir.join("worker.log")}),
    )
}
pub fn cmd_blender_cancel(s: &mut Session, p: &Value) -> Result<Value> {
    let _slot = worker_lock()?;
    let (id, dir) = job(s, p)?;
    let active = service()?;
    if active["Description"] != id || !occupied(&active) {
        return cmd_blender_status(s, p);
    }
    write_json(&dir.join("cancel-requested"), &json!({"updated_at":now()}))?;
    let output = Command::new("systemctl")
        .args(["--user", "stop", "--no-block", UNIT])
        .output()?;
    if !output.status.success() {
        fs::remove_file(dir.join("cancel-requested"))?;
        return Err(EngineError::ExternalTool(
            "systemctl".into(),
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }
    cmd_blender_status(s, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_discovery_never_decodes_artwork_and_bounds_input() {
        let dir =
            std::env::temp_dir().join(format!("blender-actions-{}-{}", std::process::id(), now()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rig.json");
        write_json(
            &path,
            &json!({"canvas":[64,96],"layers":[{"id":"body","file":"missing.png"}],
            "tracks":[{"id":"exact-authored-id","duration":1.001,"loop":true,"standard":true}]}),
        )
        .unwrap();
        let mut session = Session {
            project_assets: vec![crate::ProjectAsset {
                id: "portrait".into(),
                name: "Portrait".into(),
                kind: "cutout_rig".into(),
                path: Some(path.clone()),
                preset: None,
                parts: vec![],
            }],
            ..Session::default()
        };
        let actions = cmd_blender_actions(&mut session, &json!({"character":"portrait"})).unwrap();
        assert_eq!(
            actions["tracks"],
            json!([{"id":"exact-authored-id","duration":1.001,"loop":true,"standard":false}])
        );
        assert_eq!(actions["layers"], json!([]));
        assert!(session.assets.is_empty());
        fs::File::create(&path)
            .unwrap()
            .set_len(JSON_LIMIT + 1)
            .unwrap();
        assert!(
            cmd_blender_actions(&mut session, &json!({"character":"portrait"}))
                .unwrap_err()
                .to_string()
                .contains("16 MiB")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn absent_metrics_stay_unknown_and_shutdown_stays_busy() {
        let readings = telemetry(
            &json!({"MemoryCurrent":"18446744073709551615","MemoryPeak":"[not set]","MemorySwapCurrent":"0"}),
        );
        assert!(readings["memory_current_bytes"].is_null());
        assert!(readings["memory_peak_bytes"].is_null());
        assert!(readings["cpu_usage_ns"].is_null());
        assert_eq!(readings["swap_current_bytes"], 0);
        for state in ["activating", "active", "deactivating", "reloading"] {
            assert!(occupied(&json!({"ActiveState":state})));
        }
        assert!(!occupied(&json!({"ActiveState":"inactive"})));
        assert!(occupied(&json!({})));
    }
}
