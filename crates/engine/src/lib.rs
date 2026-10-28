//! Magic Studio engine façade.
//!
//! Every user-visible action is a command with a stable id
//! (`sheet.slice`, `turnaround.register`, `mesh.build4dir`, `export.blender`…)
//! and JSON parameters, dispatched through [`Session::execute`].
//! The Tauri UI, CLI, and MCP server all use
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

pub mod agent;
pub mod blender;
pub mod animation_links;
pub mod opentoonz;
pub mod synfig;
pub(crate) mod audio_trace;
pub mod activity;
pub mod character;
pub mod complexity;
pub mod commands;
pub mod command_center;
pub mod control_bindings;
pub mod doctor;
pub mod exports;
pub mod image_cmds;
pub mod mesh_geometry;
pub mod motion;
pub mod mlt;
pub mod motion_authoring;
pub mod theatre;
pub mod parts;
pub mod part_replacement;
pub mod photocraft;
pub mod panels;
pub mod project;
pub mod project_settings;
pub mod skills_registry;
pub mod crew;
pub mod rig;
pub mod sequence;
pub mod sheets;
pub mod image_generation;
pub mod session;
pub mod lyrics;
pub mod words;
pub mod storyboard;
pub mod storyboard_design;
pub mod storyboard_crud;
pub mod studio_cues;
pub mod turnaround;
pub mod video_target;
pub mod vision;
pub mod workshop;
pub mod workspace;

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
    pub fn execute(&mut self,id:&str,params:Value)->Result<Value>{
        let _context=activity::enter(id,&params);
        // Cast writes are journaled workshop files, independent of the open song's writer lease.
        let project_edit=commands::find(id).is_some_and(|c|c.journal) && id!="crew.save";
        let _writer=if project_edit && !matches!(id,"project.open"|"project.close"|"project.new"|"project.intake") {
            self.project_path.as_deref().map(|path|project::claim(self,path)).transpose()?
        }else{None};
        let prior_owner=self.project_owner.clone();
        if let Some(owner)=&_writer{self.project_owner=Some(owner.clone());}
        let record=self.record_user_state&&commands::find(id).is_none_or(|c|c.journal);
        if record{activity::command(self.project_path.as_deref(),id,&params,"dispatch_running",None);}
        let started=std::time::Instant::now();
        let (saves_before,project_before)=(self.project_saves.load(std::sync::atomic::Ordering::Relaxed),self.project_path.clone());
        let result=self.execute_inner(id,params.clone());
        // An edit is a successful journaled command that left the project file unwritten.
        if result.as_ref().is_ok_and(|v|v["ok"]!=false) && project_edit {
            if self.project_saves.load(std::sync::atomic::Ordering::Relaxed)!=saves_before || self.project_path!=project_before {self.edits_since_save=0;}
            else {self.edits_since_save+=1;}
        }
        if record{if let Ok(value)=&result{activity::result(self.project_path.as_deref(),id,value,started.elapsed().as_millis());}}
        if record{match &result{Ok(v)=>activity::command(self.project_path.as_deref(),id,&params,if v["ok"]==false{"engine_error"}else if v["intake_ready"]==false{"engine_needs_work"}else{"engine_success"},None),Err(e)=>activity::command(self.project_path.as_deref(),id,&params,"engine_error",Some(&e.to_string()))};}
        if _writer.is_some(){self.project_owner=prior_owner;}
        result
    }
    fn execute_inner(&mut self, id: &str, params: Value) -> Result<Value> {
        let spec = commands::find(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        check_param_types(spec.params, &params)?;
        (spec.enabled)(self).map_err(|e| EngineError::CommandFailed(id.to_string(), e))?;
        let assets_before = serde_json::to_value(&self.project_assets).ok();
        let out = (spec.run)(self, &params)?;
        // Older import/load callers also appear in the shared project outliner.
        if matches!(id, "intake.validate" | "asset.load_manifest")
            && let Some(asset) = self.current_asset()
        {
            let asset_id = asset.id.clone();
            let asset_path = asset.path.clone();
            let display_name = params
                .get("name")
                .or_else(|| params.get("asset"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| asset_id.replace('_', " "));
            let path = if id == "intake.validate" && !asset_path.as_os_str().is_empty() {
                Some(asset_path)
            } else {
                params
                    .get("manifest")
                    .or_else(|| params.get("image"))
                    .and_then(Value::as_str)
                    .map(|path| commands::resolve_workspace_path(self, path))
            };
            if let Some(existing) = self
                .project_assets
                .iter_mut()
                .find(|entry| entry.id == asset_id)
            {
                if id == "intake.validate" && path.is_some() {
                    existing.path = path;
                }
            } else {
                self.project_assets.push(ProjectAsset {
                    id: asset_id,
                    name: display_name,
                    preset: params
                        .get("preset")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    path,
                    kind: "auto".into(),
                    parts: Vec::new(),
                });
            }
        }
        // Keep the open project file in step with its asset connections.
        if spec.journal && !matches!(id, "project.open" | "project.new" | "project.close")
            && self.project_path.is_some()
            && serde_json::to_value(&self.project_assets).ok() != assets_before
        {
            commands::write_project_file(self)?;
        }
        if spec.journal {
            self.record(out.clone());
        }
        Ok(out)
    }
}

pub mod local_ipc;

pub mod project_catalog;
pub mod studio_sessions;
