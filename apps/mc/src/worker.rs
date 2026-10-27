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
        let mut ready = None;
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(value) => {
                    self.receiver = None;
                    if value["project"].as_str() == project {
                        ready = Some(value);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    if self.requested_project.as_deref() == project {
                        ready = Some(
                            json!({"project":project,"observation_error":"Worker observer disconnected","observed_at_ms":now_ms().to_string()}),
                        );
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(project) = project
            && self.receiver.is_none()
            && (self.requested_project.as_deref() != Some(project)
                || self
                    .requested_at
                    .is_none_or(|at| at.elapsed().as_secs() >= 2))
        {
            let project = project.to_string();
            let (sender, receiver) = std::sync::mpsc::channel();
            self.requested_project = Some(project.clone());
            self.requested_at = Some(std::time::Instant::now());
            self.receiver = Some(receiver);
            // Exactly one outstanding observer: a slow service query cannot accumulate threads.
            std::thread::spawn(move || {
                let _ = sender.send(observe(&project));
            });
        }
        ready
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn observe(project: &str) -> Value {
    let mut selection = Value::Null;
    let result = (|| -> Result<Value, String> {
        let path = std::path::Path::new(project)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        // These queries need only the canonical path, not artwork decoding or a writer lease.
        let mut session = Session {
            project_path: Some(path.clone()),
            ..Session::default()
        };
        let list = session
            .execute("animation.list", json!({}))
            .map_err(|e| e.to_string())?;
        let Some(metadata) = list["jobs"].as_array().and_then(|jobs| jobs.first()) else {
            return Ok(
                json!({"project":path,"state":"not_started","worker_owner":list["worker_owner"],"worker_owner_status":list["worker_owner_status"],"worker_owner_error":list["worker_owner_error"]}),
            );
        };
        let id = metadata["job"]
            .as_str()
            .ok_or("Job metadata has no job id")?;
        selection = metadata.clone();
        let mut status = session
            .execute("animation.status", json!({"job":id}))
            .map_err(|e| e.to_string())?;
        if status["job"].as_str() != Some(id) || status["project"] != json!(path) {
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
    let stamp = value[key]
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| value[key].as_u64())?;
    now_ms().checked_sub(stamp)
}

pub fn stale(value: &Value) -> bool {
    value["telemetry_stale"] == true
        || age_ms(value, "observed_at_ms").is_none_or(|age| age >= 6000)
}

pub fn headline(value: &Value) -> String {
    if value["observation_error"].is_string() {
        return "STATUS UNAVAILABLE · last result unknown".into();
    }
    let frames = match (
        value["frames_completed"].as_u64(),
        value["frames_total"].as_u64(),
    ) {
        (Some(done), Some(total)) if total > 0 && done <= total => {
            format!(" · {done}/{total} frames")
        }
        _ => String::new(),
    };
    match value["state"].as_str() {
        Some("running") if !stale(value) => {
            let spin = ["⠋", "⠙", "⠹", "⠸"][(now_ms() / 200 % 4) as usize];
            format!(
                "{spin} {}{frames}",
                value["phase"].as_str().unwrap_or("phase unknown")
            )
        }
        Some("running") => format!(
            "STALE · last {}{frames}",
            value["phase"].as_str().unwrap_or("phase unknown")
        ),
        Some("cancelling") => format!("CANCELLING · waiting for worker exit{frames}"),
        Some("completed") if value["partial"] == true => format!("PREVIEW RENDER COMPLETE{frames}"),
        Some("completed") => format!("RENDER COMPLETE{frames}"),
        Some("cancelled") => "CANCELLED · worker exited".into(),
        Some("failed") => format!(
            "BLOCKED · {}",
            value["termination_reason"]
                .as_str()
                .unwrap_or("failure reason unknown")
        ),
        Some("not_started") => "No render job observed for this project".into(),
        _ => "Worker state unknown".into(),
    }
}

/// One vocabulary for the worker state, shared by the top bar, the panels and the
/// CLI, so the screen and the terminal never use different words for one fact.
pub fn state_word(value: &Value) -> &'static str {
    if value["observation_error"].is_string() {
        return "UNAVAILABLE";
    }
    match value["state"].as_str() {
        Some("running") => {
            if stale(value) {
                "STALE"
            } else {
                "RUNNING"
            }
        }
        Some("cancelling") => "CANCELLING",
        Some("completed") => {
            if value["partial"] == true {
                "PREVIEW DONE"
            } else {
                "COMPLETE"
            }
        }
        Some("cancelled") => "CANCELLED",
        Some("failed") => "BLOCKED",
        Some("not_started") => "NO JOB",
        _ => "UNKNOWN",
    }
}

/// The state sentence for a panel: what is happening, without the counters that
/// already have their own row.
pub fn state_line(value: &Value) -> String {
    let word = state_word(value);
    match value["state"].as_str() {
        Some("running") => format!(
            "{word} · {}",
            value["phase"].as_str().unwrap_or("phase unknown")
        ),
        Some("failed") => format!(
            "{word} · {}",
            value["termination_reason"]
                .as_str()
                .unwrap_or("failure reason unknown")
        ),
        Some("completed") if value["partial"] == true => format!("{word} · shortened preview"),
        Some("cancelled") => "CANCELLED · worker exited".into(),
        Some("cancelling") => "CANCELLING · waiting for worker exit".into(),
        Some("not_started") => "No render job observed for this project".into(),
        _ => "Worker state unknown".into(),
    }
}

/// A short, honest form of the headline for the top bar's right lane: the same
/// evidence, fewer words. Never a different claim than `headline`.
pub fn short_headline(value: &Value) -> String {
    let frames = match (
        value["frames_completed"].as_u64(),
        value["frames_total"].as_u64(),
    ) {
        (Some(done), Some(total)) if total > 0 && done <= total => format!(" {done}/{total}"),
        _ => String::new(),
    };
    format!("{}{frames}", state_word(value))
}

pub fn mib(value: &Value) -> String {
    value
        .as_u64()
        .map(|n| format!("{:.0}MiB", n as f64 / 1048576.0))
        .unwrap_or_else(|| "?".into())
}

pub fn resource_line(value: &Value) -> String {
    let t = &value["telemetry"];
    let cap = t
        .get("memory_max_bytes")
        .filter(|v| !v.is_null())
        .unwrap_or(&value["limits"]["memory_max_bytes"]);
    format!(
        "Worker RAM {} / {} · peak {} · swap {} / {}",
        mib(&t["memory_current_bytes"]),
        mib(cap),
        mib(&t["memory_peak_bytes"]),
        mib(&t["swap_current_bytes"]),
        mib(&t["swap_max_bytes"])
    )
}
