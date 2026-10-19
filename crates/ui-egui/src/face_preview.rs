//! Background PhotoCraft previews. Original artwork and engine assets are never mutated.
use crate::UiState;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
};

#[derive(Default)]
pub(crate) struct FacePreview {
    requested: String,
    ready_key: String,
    frames: Vec<PathBuf>,
    receiver: Option<Receiver<Result<(String, Vec<PathBuf>), String>>>,
    pub status: String,
}

impl FacePreview {
    pub fn path(&mut self, ctx: &egui::Context, source: &Path, state: &UiState) -> PathBuf {
        if let Some(receiver) = &self.receiver {
            if let Ok(result) = receiver.try_recv() {
                self.receiver = None;
                match result {
                    Ok((key, frames)) => {
                        self.ready_key = key;
                        self.frames = frames;
                        self.status = "PhotoCraft preview · review the detected face".into();
                    }
                    Err(error) => {
                        self.status = error;
                    }
                }
            }
        }
        let neutral = state.mouth_viseme == "smile"
            && (state.smile_open - 20.0).abs() < 0.1
            && state.eye_blink < 0.1
            && state.eye_gaze.abs() < 0.1;
        if neutral {
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
        if key != self.requested && self.receiver.is_none() && !ctx.input(|i| i.pointer.any_down())
        {
            self.requested = key.clone();
            self.status = "Rendering facial preview with PhotoCraft…".into();
            let (sender, receiver) = mpsc::channel();
            self.receiver = Some(receiver);
            let source = source.to_path_buf();
            let expression = state.mouth_viseme.clone();
            let opening = state.smile_open;
            let blink = state.eye_blink;
            let gaze = state.eye_gaze;
            let ctx = ctx.clone();
            let job_key = key.clone();
            std::thread::spawn(move || {
                let script = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../pipeline/facial_expression_preview.py");
                let result = std::process::Command::new("python3")
                    .arg(script)
                    .arg("--source")
                    .arg(&source)
                    .arg("--out-dir")
                    .arg(std::env::temp_dir().join("animationcraft-face-preview"))
                    .arg("--expression")
                    .arg(expression)
                    .arg("--opening")
                    .arg(opening.round().to_string())
                    .arg("--blink")
                    .arg(blink.round().to_string())
                    .arg("--gaze")
                    .arg(gaze.round().to_string())
                    .output()
                    .map_err(|e| e.to_string())
                    .and_then(|output| {
                        if !output.status.success() {
                            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
                        }
                        let result: serde_json::Value =
                            serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
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
                        Ok((job_key, frames))
                    });
                let _ = sender.send(result);
                ctx.request_repaint();
            });
        }
        if key == self.ready_key && !self.frames.is_empty() {
            let index = ((state.playback_phase * 3.0).max(0.0) as usize) % self.frames.len();
            self.frames[index].clone()
        } else {
            source.to_path_buf()
        }
    }
}
