//! Clip/audio assembly submitted to the shared animation worker; no editor/scheduler.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::PathBuf;
fn executable(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let explicit = if name == "melt" {
        std::env::var_os("MLT_MELT")
    } else {
        None
    };
    let candidates: Vec<_> = explicit.map(|v| vec![PathBuf::from(v)]).unwrap_or_else(|| {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|p| p.join(name))
            .collect()
    });
    candidates
        .into_iter()
        .find(|p| {
            p.is_file()
                && p.metadata()
                    .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        })
        .and_then(|p| p.canonicalize().ok())
}
pub fn discover(_: &mut Session, _: &Value) -> Result<Value> {
    let melt = executable("melt");
    let ffmpeg = executable("ffmpeg");
    let ffprobe = executable("ffprobe");
    let configured = melt.is_some() && ffmpeg.is_some() && ffprobe.is_some();
    Ok(
        json!({"backend":"mlt","available":melt.is_some(),"executable":melt,"ffmpeg":ffmpeg,"ffprobe":ffprobe,
        "configured":configured,"render_capable":Value::Null,"render_reason":if configured{"Native avformat/codec capability is checked in the bounded job"}else{"Install MLT melt and FFmpeg/ffprobe; MLT_MELT may select an explicit executable"},
        "input":"generic timeline JSON","output":"editable .mlt + opaque H264/AAC MP4",
        "supported":["ordered frame-exact cuts at matching FPS","embedded clip audio or explicit replacement audio bed","frozen local media receipts"],
        "unsupported":["arbitrary MLT XML import","multi-layer film composition","transitions/effects/captions","alpha output","implicit frame-rate conversion"]}),
    )
}
pub fn start(s: &mut Session, p: &Value) -> Result<Value> {
    let invalid = |v: &str| EngineError::InvalidParams(v.into());
    let melt = executable("melt")
        .ok_or_else(|| invalid("MLT melt unavailable; install mlt or configure MLT_MELT"))?;
    let ffmpeg = executable("ffmpeg").ok_or_else(|| invalid("FFmpeg required"))?;
    let ffprobe = executable("ffprobe").ok_or_else(|| invalid("ffprobe required"))?;
    let path = crate::workspace::resolve_workspace_path(
        s,
        p["timeline"]
            .as_str()
            .ok_or_else(|| invalid("timeline JSON path required"))?,
    )
    .canonicalize()?;
    if !path.is_file() || std::fs::metadata(&path)?.len() > 1024 * 1024 {
        return Err(invalid("timeline must be a JSON file no larger than 1 MiB"));
    }
    let request = json!({"timeline":path,"melt":melt,"ffmpeg":ffmpeg,"ffprobe":ffprobe});
    crate::blender::submit(s, p, request, "mlt")
}
