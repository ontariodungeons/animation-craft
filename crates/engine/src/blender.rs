//! Bounded Blender jobs. The Studio engine never waits for rendering or owns Blender memory.
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};
use serde_json::{Value, json};
use crate::{EngineError, Result, Session};

fn invalid(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }
fn root(s: &Session) -> Result<PathBuf> {
    s.project_path.as_ref().and_then(|p|p.parent()).map(|p|p.join("animation/blender-jobs"))
        .ok_or_else(||invalid("Open a .magic project first"))
}
fn job(s: &Session, p: &Value) -> Result<(String, PathBuf)> {
    let id=p["job"].as_str().ok_or_else(||invalid("job is required"))?;
    if !id.starts_with("magicstudio-blender-") || !id.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-') {
        return Err(invalid("Invalid Blender job id"));
    }
    let dir=root(s)?.join(id);
    if !dir.join("request.json").is_file(){return Err(invalid("Job not found in this project"));}
    Ok((id.into(),dir))
}
fn bounded(p: &Value, key: &str, default: u64, min: u64, max: u64) -> Result<u64> {
    let value=match p.get(key){None=>default,Some(v)=>v.as_u64().ok_or_else(||invalid(format!("{key} must be an integer")))?};
    if !(min..=max).contains(&value){return Err(invalid(format!("{key} must be {min}–{max}")));}
    Ok(value)
}
pub fn cmd_blender_start(s: &mut Session, p: &Value) -> Result<Value> {
    let jobs=cmd_blender_list(s,&json!({}))?;
    for entry in jobs["jobs"].as_array().into_iter().flatten(){
        if cmd_blender_status(s,entry)?["state"]=="running" {
            return Err(invalid(format!("Blender job {} is still running; finish or cancel it first",entry["job"])));
        }
    }
    let mut request=json!({"resolution_percent":bounded(p,"resolution_percent",50,10,100)?,
        "max_frames":bounded(p,"max_frames",90,1,18000)?});
    let memory=bounded(p,"memory_mb",4096,512,16384)?;
    let seconds=bounded(p,"timeout_seconds",300,10,3600)?;
    if let Some(path)=p["blend"].as_str(){
        let path=crate::workspace::resolve_workspace_path(s,path).canonicalize()?;
        if path.extension().is_none_or(|e|e!="blend"){return Err(invalid("blend must be an existing .blend file"));}
        request["blend"]=json!(path);
    } else {
        request["meta"]=crate::motion::cmd_motion_tracks(s,p)?;
        request["sample"]=crate::motion::cmd_motion_sample(s,p)?;
    }
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|invalid(e.to_string()))?.as_nanos();
    let id=format!("magicstudio-blender-{}-{stamp}",std::process::id());
    let dir=root(s)?.join(&id);
    fs::create_dir_all(&dir)?;
    let request_path=dir.join("request.json");
    fs::write(&request_path,serde_json::to_vec(&request).map_err(|e|invalid(e.to_string()))?)?;
    fs::write(dir.join("job.json"),serde_json::to_vec(&json!({"job":id,"created_ns":stamp.to_string(),"character":p["character"],"track":p["track"],"source_blend":request["blend"]})).map_err(|e|invalid(e.to_string()))?)?;
    // Embed the worker so an installed mc does not depend on a developer checkout.
    let worker=dir.join("worker.py");
    fs::write(&worker,include_str!("../../../pipeline/blender_animation_worker.py"))?;
    let blender=std::env::var("BLENDER_BIN").unwrap_or_else(|_|"blender".into());
    let output=Command::new("systemd-run").args(["--user","--quiet","--collect"])
        .arg(format!("--unit={id}"))
        .arg(format!("--property=MemoryMax={memory}M"))
        .args(["--property=MemorySwapMax=0","--property=CPUQuota=200%"])
        .arg(format!("--property=RuntimeMaxSec={seconds}"))
        .arg(format!("--property=StandardOutput=append:{}",dir.join("worker.log").display()))
        .arg(format!("--property=StandardError=append:{}",dir.join("worker.log").display()))
        .args(["--",&blender,"--background","--factory-startup","--disable-autoexec","--threads","2","--python-exit-code","1","--python"])
        .arg(&worker).arg("--").arg(&request_path).output()?;
    if !output.status.success(){return Err(EngineError::ExternalTool("systemd-run".into(),String::from_utf8_lossy(&output.stderr).into()));}
    Ok(json!({"ok":true,"job":id,"state":"submitted","directory":dir,"memory_limit_mb":memory,"timeout_seconds":seconds}))
}
pub fn cmd_blender_list(s: &mut Session, _: &Value) -> Result<Value> {
    let root=root(s)?;
    let mut jobs=Vec::new();
    if root.is_dir(){for entry in fs::read_dir(root)? {
        let path=entry?.path().join("job.json");
        if path.is_file(){
            let value:Value=serde_json::from_slice(&fs::read(path)?).map_err(|e|invalid(e.to_string()))?;
            jobs.push(value);
        }
    }}
    jobs.sort_by(|a,b|b["created_ns"].as_str().cmp(&a["created_ns"].as_str()));
    Ok(json!({"jobs":jobs}))
}
pub fn cmd_blender_status(s: &mut Session, p: &Value) -> Result<Value> {
    let (id,dir)=job(s,p)?;
    let result=dir.join("result.json");
    if result.is_file(){
        let mut value:Value=serde_json::from_slice(&fs::read(result)?).map_err(|e|invalid(e.to_string()))?;
        value["job"]=json!(id);
        return Ok(value);
    }
    if dir.join("cancelled").exists(){return Ok(json!({"job":id,"state":"cancelled"}));}
    let output=Command::new("systemctl").args(["--user","show",&id,"--property=ActiveState","--value"]).output()?;
    let active=String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let progress=fs::read(dir.join("progress.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok());
    Ok(json!({"job":id,"state":if active=="active"||active=="activating"{"running"}else{"failed"},
        "progress":progress,"log":dir.join("worker.log"),"service_state":active,
        "detail":if active=="active"||active=="activating"{Value::Null}else{json!("Worker exited without a result; inspect its log and service journal (including resource limits)")}}))
}
pub fn cmd_blender_cancel(s: &mut Session, p: &Value) -> Result<Value> {
    let (id,dir)=job(s,p)?;
    if dir.join("result.json").exists(){return cmd_blender_status(s,p);}
    let output=Command::new("systemctl").args(["--user","stop",&id]).output()?;
    if !output.status.success(){return Err(EngineError::ExternalTool("systemctl".into(),String::from_utf8_lossy(&output.stderr).into()));}
    fs::write(dir.join("cancelled"),b"cancelled by user\n")?;
    cmd_blender_status(s,p)
}
