//! Read-only worker observations through the shared registry. No renderer or service control.
use magicstudio_engine::Session;
use serde_json::{Value, json};

#[derive(Default)]
pub struct Poller {
    receiver: Option<std::sync::mpsc::Receiver<Value>>,
    requested_at: Option<std::time::Instant>,
    requested_project: Option<String>,
}

impl Poller {
    pub fn poll(&mut self, project: Option<&str>) -> Option<Value> {
        let mut ready=None;
        if let Some(receiver)=&self.receiver {
            match receiver.try_recv() {
                Ok(value)=>{self.receiver=None;if value["project"].as_str()==project{ready=Some(value);}},
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                    self.receiver=None;
                    if self.requested_project.as_deref()==project {
                        ready=Some(json!({"project":project,"observation_error":"Worker observer disconnected","observed_at_ms":now_ms().to_string()}));
                    }
                },
                Err(std::sync::mpsc::TryRecvError::Empty)=>{},
            }
        }
        if let Some(project)=project
            && self.receiver.is_none()
            && (self.requested_project.as_deref()!=Some(project) || self.requested_at.is_none_or(|at|at.elapsed().as_secs()>=2)) {
            let project=project.to_string();
            let (sender,receiver)=std::sync::mpsc::channel();
            self.requested_project=Some(project.clone());
            self.requested_at=Some(std::time::Instant::now());
            self.receiver=Some(receiver);
            // Exactly one outstanding observer: a slow service query cannot accumulate threads.
            std::thread::spawn(move||{let _=sender.send(observe(&project));});
        }
        ready
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_millis() as u64
}

pub fn observe(project: &str) -> Value {
    let mut selection=Value::Null;
    let result = (|| -> Result<Value, String> {
        let path = std::path::Path::new(project).canonicalize().map_err(|e|e.to_string())?;
        // These queries need only the canonical path, not artwork decoding or a writer lease.
        let mut session = Session { project_path: Some(path.clone()), ..Session::default() };
        let list = session.execute("blender.list", json!({})).map_err(|e|e.to_string())?;
        let Some(metadata) = list["jobs"].as_array().and_then(|jobs|jobs.first()) else {
            return Ok(json!({"project":path,"state":"not_started","worker_owner":list["worker_owner"],"worker_owner_status":list["worker_owner_status"],"worker_owner_error":list["worker_owner_error"]}));
        };
        let id = metadata["job"].as_str().ok_or("Job metadata has no job id")?;
        selection=metadata.clone();
        let mut status = session.execute("blender.status", json!({"job":id})).map_err(|e|e.to_string())?;
        if status["job"].as_str()!=Some(id) || status["project"]!=json!(path) {
            return Err("Worker observation does not match the selected job/project".into());
        }
        status["selection"] = metadata.clone();
        Ok(status)
    })();
    let mut value = result.unwrap_or_else(|error|json!({"project":project,"job":selection["job"],"selection":selection,"observation_error":error}));
    value["observed_at_ms"] = json!(now_ms().to_string());
    value
}

pub fn age_ms(value: &Value, key: &str) -> Option<u64> {
    let stamp = value[key].as_str().and_then(|s|s.parse().ok()).or_else(||value[key].as_u64())?;
    now_ms().checked_sub(stamp)
}

pub fn stale(value: &Value) -> bool {
    value["telemetry_stale"]==true || age_ms(value,"observed_at_ms").is_none_or(|age|age>=6000)
}

pub fn headline(value: &Value) -> String {
    if value["observation_error"].is_string() { return "STATUS UNAVAILABLE · last result unknown".into(); }
    let frames = match (value["frames_completed"].as_u64(),value["frames_total"].as_u64()) {
        (Some(done),Some(total)) if total>0 && done<=total=>format!(" · {done}/{total} frames"),
        _=>String::new(),
    };
    match value["state"].as_str() {
        Some("running") if !stale(value)=>{
            let spin=["⠋","⠙","⠹","⠸"][(now_ms()/200%4) as usize];
            format!("{spin} {}{frames}",value["phase"].as_str().unwrap_or("phase unknown"))
        },
        Some("running")=>format!("STALE · last {}{frames}",value["phase"].as_str().unwrap_or("phase unknown")),
        Some("cancelling")=>format!("CANCELLING · waiting for worker exit{frames}"),
        Some("completed") if value["partial"]==true=>format!("PREVIEW RENDER COMPLETE{frames}"),
        Some("completed")=>format!("RENDER COMPLETE{frames}"),
        Some("cancelled")=>"CANCELLED · worker exited".into(),
        Some("failed")=>format!("BLOCKED · {}",value["termination_reason"].as_str().unwrap_or("failure reason unknown")),
        Some("not_started")=>"No render job observed for this project".into(),
        _=>"Worker state unknown".into(),
    }
}

pub fn mib(value: &Value) -> String {
    value.as_u64().map(|n|format!("{:.0}MiB",n as f64/1048576.0)).unwrap_or_else(||"?".into())
}

pub fn resource_line(value: &Value) -> String {
    let t=&value["telemetry"];
    let cap=t.get("memory_max_bytes").filter(|v|!v.is_null()).unwrap_or(&value["limits"]["memory_max_bytes"]);
    format!("Worker RAM {} / {} · peak {} · swap {} / {}",mib(&t["memory_current_bytes"]),mib(cap),mib(&t["memory_peak_bytes"]),mib(&t["swap_current_bytes"]),mib(&t["swap_max_bytes"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metrics_keep_null_and_measured_zero_distinct() {
        assert_eq!(mib(&Value::Null),"?");
        assert_eq!(mib(&json!(0)),"0MiB");
        assert!(resource_line(&json!({"telemetry":{"memory_current_bytes":0,"memory_peak_bytes":null}})).contains("RAM 0MiB / ? · peak ?"));
    }
    #[test]
    fn receipt_fallback_keeps_completion_but_never_animates_as_live() {
        let receipt=json!({"state":"completed","telemetry_stale":true,"telemetry_source":"terminal_receipt","observed_at_ms":now_ms().to_string()});
        assert!(stale(&receipt));
        assert_eq!(headline(&receipt),"RENDER COMPLETE");
        let cancelling=json!({"state":"cancelling"});
        assert!(headline(&cancelling).contains("waiting for worker exit"));
    }
    #[test]
    fn stale_or_missing_progress_never_becomes_zero_or_live_animation() {
        let old=json!({"state":"running","phase":"rendering","frames_completed":42,"frames_total":180,"observed_at_ms":"0"});
        assert_eq!(headline(&old),"STALE · last rendering · 42/180 frames");
        let fresh=json!({"state":"running","phase":"preparing","frames_completed":null,"frames_total":null,"observed_at_ms":now_ms().to_string()});
        assert!(!headline(&fresh).contains("0/"));
        assert_eq!(headline(&json!({"state":"failed","termination_reason":"timeout"})),"BLOCKED · timeout");
    }
    #[test]
    fn polling_keeps_one_outstanding_query_and_discards_other_projects() {
        let (sender,receiver)=std::sync::mpsc::channel();
        let mut poller=Poller{receiver:Some(receiver),requested_at:Some(std::time::Instant::now()),requested_project:Some("/selected.magic".into())};
        assert!(poller.poll(Some("/selected.magic")).is_none());
        assert!(poller.receiver.is_some());
        sender.send(json!({"project":"/previous.magic","state":"running"})).unwrap();
        assert!(poller.poll(Some("/selected.magic")).is_none());
        assert!(poller.receiver.is_none());
    }
    #[test]
    fn saved_project_observation_does_not_write_or_open_artwork() {
        let dir=std::path::PathBuf::from("/tmp/opencode").join(format!("backstage-query-{}-{}",std::process::id(),now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let project=dir.join("Fixture.magic");
        std::fs::write(&project,b"{}").unwrap();
        let value=observe(project.to_str().unwrap());
        assert_eq!(value["state"],"not_started", "{value}");
        assert_eq!(std::fs::read(&project).unwrap(),b"{}");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(),1,"polling must not create project state, animation files or journals");
        std::fs::remove_file(project).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
