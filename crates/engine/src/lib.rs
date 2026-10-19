//! AnimationCraft engine façade.
//!
//! Every user-visible action is a command with a stable id
//! (`sheet.slice`, `turnaround.register`, `mesh.build4dir`, `export.blender`…)
//! and JSON parameters, dispatched through [`Session::execute`].
//! The egui UI, the CLI, and any future control channel / MCP server all use
//! this one entry point; that is what makes the UI swappable and agent-drivable.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

pub mod commands;
pub mod control_bindings;
pub mod mesh_geometry;
pub mod session;

pub use commands::{CommandSpec, command_specs, find as find_command};
pub use session::{
    Asset, AttachedReference, LoadedManifest, ManifestFrame, MeasuredMotionRange,
    MeasuredTurnaroundRegistration, PartState, PoseMotionMetrics, ProjectAsset, Session,
};

use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("command `{0}` failed: {1}")]
    CommandFailed(String, String),
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("external tool `{0}` failed: {1}")]
    ExternalTool(String, String),
    #[error("not yet implemented")]
    NotImplemented,
}

pub type Result<T> = std::result::Result<T, EngineError>;

/// Rejects a parameter whose JSON type differs from the example value in the command's `params`
/// description (e.g. `"auto_cut": "true"` where the description says `false`). Every caller (CLI, MCP,
/// app, bridge) goes through `Session::execute`, so a mistyped value fails the same way everywhere
/// instead of a handler silently falling back to its default.
pub fn check_param_types(description: &str, params: &Value) -> Result<()> {
    let (Ok(Value::Object(expected)), Some(given)) =
        (serde_json::from_str::<Value>(description), params.as_object())
    else {
        return Ok(());
    };
    for (key, value) in given {
        let wanted = match expected.get(key) {
            Some(Value::Bool(_)) => "boolean",
            Some(Value::Number(_)) => "number",
            _ => continue,
        };
        let ok = if wanted == "boolean" { value.is_boolean() } else { value.is_number() };
        if !ok {
            return Err(EngineError::InvalidParams(format!("`{key}` must be a {wanted}, got {value}")));
        }
    }
    Ok(())
}

impl Session {
    /// Execute one command by id with JSON parameters, returning JSON.
    pub fn execute(&mut self, id: &str, params: Value) -> Result<Value> {
        let spec = commands::find(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        check_param_types(spec.params, &params)?;
        (spec.enabled)(self).map_err(|e| EngineError::CommandFailed(id.to_string(), e))?;
        let out = (spec.run)(self, &params)?;
        // Older import/load callers also appear in the shared project outliner.
        if matches!(id, "intake.validate" | "asset.load_manifest")
            && let Some(asset) = self.current_asset()
            && !self.project_assets.iter().any(|entry| entry.id == asset.id)
        {
            self.project_assets.push(ProjectAsset {
                id: asset.id.clone(),
                name: asset.id.replace('_', " "),
                preset: params
                    .get("preset")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                path: params
                    .get("manifest")
                    .or_else(|| params.get("image"))
                    .and_then(Value::as_str)
                    .map(|path| commands::resolve_workspace_path(self, path)),
                kind: "auto".into(),
            });
        }
        if spec.journal {
            self.record(out.clone());
        }
        Ok(out)
    }
}
