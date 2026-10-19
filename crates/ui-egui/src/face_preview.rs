//! Background PhotoCraft previews. Original artwork and engine assets are never mutated.
//! A preview is a pixel-change check inside bounded face regions, not a verified expression.
use crate::UiState;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    time::{Duration, Instant},
};

/// Total time the script may spend (it enforces this itself); the app kills it shortly after.
const SCRIPT_DEADLINE_SECS: u64 = 90;
const HARD_KILL_AFTER: Duration = Duration::from_secs(SCRIPT_DEADLINE_SECS + 10);
const CANCELLED: &str = "Facial preview cancelled";
const SCRIPT_ENV: &str = "ANIMATIONCRAFT_FACE_PREVIEW_SCRIPT";

type JobResult = Result<(Vec<PathBuf>, String), String>;

struct Job {
    key: String,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<JobResult>,
}

#[derive(Default)]
pub(crate) struct FacePreview {
    job: Option<Job>,
    /// Last key a job was started for; an unchanged key is never retried automatically.
    attempted: String,
    ready_key: String,
    frames: Vec<PathBuf>,
    status_key: String,
    pub status: String,
}

impl FacePreview {
    pub fn path(&mut self, ctx: &egui::Context, source: &Path, state: &UiState) -> PathBuf {
        let neutral = state.mouth_viseme == "smile"
            && (state.smile_open - 20.0).abs() < 0.1
            && state.eye_blink < 0.1
            && state.eye_gaze.abs() < 0.1;
        if neutral {
            self.reset();
            return source.to_path_buf();
        }
        let stamp = std::fs::metadata(source).and_then(|m| m.modified()).ok();
        let key = format!(
            "{}:{stamp:?}:{}:{:.0}:{:.0}:{:.0}",
            source.display(),
            state.mouth_viseme,
            state.smile_open,
            state.eye_blink,
            state.eye_gaze
        );
        // Status belongs to the asset/settings that produced it.
        if self.status_key != key {
            self.status.clear();
        }
        self.poll(&key);
        if self.job.is_none()
            && key != self.attempted
            && !ctx.input(|i| i.pointer.any_down())
        {
            self.start(ctx, source, state, key.clone());
        }
        if key == self.ready_key && !self.frames.is_empty() {
            let index = ((state.playback_phase * 3.0).max(0.0) as usize) % self.frames.len();
            self.frames[index].clone()
        } else {
            source.to_path_buf()
        }
    }

    fn reset(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::SeqCst);
        }
        self.attempted.clear();
        self.ready_key.clear();
        self.frames.clear();
        self.status.clear();
        self.status_key.clear();
    }

    /// Apply a finished job only if it is for the current asset and settings; cancel it otherwise.
    fn poll(&mut self, key: &str) {
        let Some(job) = self.job.take() else { return };
        match job.receiver.try_recv() {
            Ok(result) if job.key == key => match result {
                Ok((frames, warnings)) => {
                    self.ready_key = key.to_string();
                    self.frames = frames;
                    let mut status = String::from("PhotoCraft preview · review the detected face");
                    if !warnings.is_empty() {
                        status.push_str(&format!(" · warning: {warnings}"));
                    }
                    self.set_status(key, status);
                }
                Err(error) if error == CANCELLED => self.attempted.clear(),
                Err(error) => {
                    let hint = if error.contains("retry") { "" } else { " — move a control to retry" };
                    self.set_status(key, format!("{error}{hint}"));
                }
            },
            Ok(_) => {}
            Err(TryRecvError::Empty) => {
                if job.key != key {
                    job.cancel.store(true, Ordering::SeqCst);
                }
                self.job = Some(job);
            }
            Err(TryRecvError::Disconnected) => {
                if job.key == key {
                    self.set_status(key, "Facial preview worker stopped unexpectedly — move a control to retry".into());
                }
            }
        }
    }

    fn set_status(&mut self, key: &str, status: String) {
        self.status = status;
        self.status_key = key.to_string();
    }

    fn start(&mut self, ctx: &egui::Context, source: &Path, state: &UiState, key: String) {
        self.attempted = key.clone();
        self.set_status(&key, "Rendering facial preview with PhotoCraft…".into());
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.job = Some(Job { key, cancel: cancel.clone(), receiver });
        let args = JobArgs {
            source: source.to_path_buf(),
            expression: state.mouth_viseme.clone(),
            opening: state.smile_open,
            blink: state.eye_blink,
            gaze: state.eye_gaze,
        };
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(run_script(&args, &cancel));
            ctx.request_repaint();
        });
    }
}

struct JobArgs {
    source: PathBuf,
    expression: String,
    opening: f32,
    blink: f32,
    gaze: f32,
}

/// Script location: explicit override, else `pipeline/` beside the executable or the working
/// directory (or any parent of either). Nothing is baked in at build time.
fn find_script() -> Result<PathBuf, String> {
    if let Some(explicit) = std::env::var_os(SCRIPT_ENV) {
        let path = PathBuf::from(explicit);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!("{SCRIPT_ENV} does not point to a file"))
        };
    }
    let starts = [
        std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)),
        std::env::current_dir().ok(),
    ];
    for start in starts.into_iter().flatten() {
        for dir in start.ancestors() {
            let candidate = dir.join("pipeline/facial_expression_preview.py");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(format!("facial_expression_preview.py not found; set {SCRIPT_ENV} to its path"))
}

fn output_root() -> PathBuf {
    let user: String = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".into())
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    std::env::temp_dir().join(format!("animationcraft-face-preview-{user}"))
}

fn last_line(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    line.chars().take(300).collect()
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    })
}

fn stop(child: &mut std::process::Child) {
    #[cfg(unix)]
    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", child.id())]).status();
    let _ = child.kill();
    let _ = child.wait();
}

fn run_script(args: &JobArgs, cancel: &AtomicBool) -> JobResult {
    let script = find_script()?;
    let mut command = Command::new("python3");
    command
        .arg(script)
        .arg("--source")
        .arg(&args.source)
        .arg("--out-dir")
        .arg(output_root())
        .arg("--expression")
        .arg(&args.expression)
        .arg("--opening")
        .arg(args.opening.round().to_string())
        .arg("--blink")
        .arg(args.blink.round().to_string())
        .arg("--gaze")
        .arg(args.gaze.round().to_string())
        .arg("--deadline")
        .arg(SCRIPT_DEADLINE_SECS.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Own process group, so stopping the job also stops the PhotoCraft processes it started.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command.spawn().map_err(|e| format!("Could not start python3: {e}"))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => return Err(format!("Lost the facial preview process: {e}")),
        }
        if cancel.load(Ordering::SeqCst) {
            stop(&mut child);
            return Err(CANCELLED.into());
        }
        if started.elapsed() > HARD_KILL_AFTER {
            stop(&mut child);
            return Err(format!("Facial preview was stopped after {}s. Adjust a control to retry.", HARD_KILL_AFTER.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    let out = stdout.join().unwrap_or_default();
    let err = stderr.join().unwrap_or_default();
    if !status.success() {
        let message = last_line(&err);
        return Err(if message.is_empty() { "Facial preview failed without a message".into() } else { message });
    }
    let result: serde_json::Value =
        serde_json::from_slice(&out).map_err(|e| format!("Facial preview returned unreadable output: {e}"))?;
    let frames: Vec<PathBuf> = result
        .get("frames")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .map(PathBuf::from)
        .collect();
    if frames.is_empty() || frames.iter().any(|p| !p.is_file()) {
        return Err("Facial preview produced no usable images".into());
    }
    let warnings: Vec<&str> = result
        .get("warnings")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .collect();
    Ok((frames, warnings.join("; ")))
}
