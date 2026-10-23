//! Render saved Synfig scenes through the shared bounded animation worker.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::PathBuf;

fn executable(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let candidate = std::env::var_os("SYNFIG_BIN").filter(|_| name == "synfig");
    let candidates: Vec<PathBuf> = if let Some(path) = candidate {
        vec![PathBuf::from(path)]
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|p| p.join(name))
            .collect()
    };
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
    let path = executable("synfig");
    Ok(
        json!({"backend":"synfig", "available":path.is_some(), "executable":path,
        "version":Value::Null,"version_note":"Queried inside the bounded render job; discovery does not launch the renderer",
        "install_dependency":"synfig CLI (Arch package synfig); python-pillow; ffmpeg with libvpx-vp9",
        "formats":["sif","sifz"], "controls":["first_frame","last_frame","fps","width","height"],
        "selections":[], "limitations":["Saved root canvas only; unpack .sfg into a self-contained .sif bundle first", "External canvas links, symlinks and assets outside bundle are rejected", "System fonts and renderer modules are runtime dependencies, not bundled assets"]}),
    )
}

pub fn start(s: &mut Session, p: &Value) -> Result<Value> {
    let invalid = |m: &str| EngineError::InvalidParams(m.into());
    let tool = executable("synfig").ok_or_else(|| {
        invalid(
            "Synfig CLI unavailable: install package synfig or set SYNFIG_BIN to its executable",
        )
    })?;
    let ffmpeg = executable("ffmpeg")
        .ok_or_else(|| invalid("ffmpeg executable required for alpha clip encoding"))?;
    let source = crate::workspace::resolve_workspace_path(
        s,
        p["source"]
            .as_str()
            .ok_or_else(|| invalid("source .sif/.sifz is required"))?,
    )
    .canonicalize()?;
    if !source.is_file()
        || !matches!(
            source.extension().and_then(|e| e.to_str()),
            Some("sif" | "sifz")
        )
    {
        return Err(invalid(
            "source must be a saved .sif or .sifz; unpack .sfg first",
        ));
    }
    let bundle = p["bundle"]
        .as_str()
        .map(|v| crate::workspace::resolve_workspace_path(s, v))
        .unwrap_or_else(|| source.parent().unwrap_or(&source).to_path_buf())
        .canonicalize()?;
    if !bundle.is_dir() || !source.starts_with(&bundle) {
        return Err(invalid(
            "source must be inside its dependency bundle directory",
        ));
    }
    for (key, min, max) in [
        ("first_frame", 0, 18000),
        ("last_frame", 0, 18000),
        ("width", 2, 4096),
        ("height", 2, 4096),
    ] {
        if let Some(value) = p.get(key)
            && !value.as_u64().is_some_and(|v| v >= min && v <= max)
        {
            return Err(invalid(
                "frame range must be integers 0..18000; dimensions 2..4096",
            ));
        }
    }
    if let Some(value) = p.get("fps")
        && !value
            .as_f64()
            .is_some_and(|v| v.is_finite() && v > 0.0 && v <= 120.0)
    {
        return Err(invalid("fps must be in (0,120]"));
    }
    if p.get("canvas").is_some() {
        return Err(invalid("Exported canvas selection is not yet supported"));
    }
    let mut request = p.clone();
    request["source"] = json!(source);
    request["bundle"] = json!(bundle);
    request["executable"] = json!(tool);
    request["ffmpeg"] = json!(ffmpeg);
    crate::blender::submit(s, p, request, "synfig")
}
