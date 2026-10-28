//! The command registry: every user-visible action is a command with a stable id
//! (`domain.verb`) and JSON params, dispatched through [`crate::Session::execute`].
//!
//! Command handlers live in focused modules by job (declared in `lib.rs`):
//!   - `workspace` — path/CLI resolution
//!   - `parts` — part visibility/solo/review
//!   - `turnaround` — turnaround accuracy + 2D preview
//!   - `photocraft` — intake, matting, sheet cutting, manifest loading, agent prompt
//!   - `rig` — 4-dir mesh build + GLB/OBJ writers
//!   - `exports` — EffectCraft/Three.js/React/Blender/FilmCraft/cutout MP4/layered motion/validate
//!   - `image_cmds` — break apart / puppet warp / color to alpha / content-aware fill
//!   - `vision` — in-memory pixel analysis
//!   - `project` — .magic lifecycle + connect/load/remove/update assets
//!   - `workshop` — workshop/storyboard/video/assets/audio + search
//!
//! This file keeps only the `CommandSpec` registry, `command_specs()`, `find()`,
//! `check_param_types()`, and the `build()` array wiring every id to its handler.

use crate::storyboard::cmd_storyboard_template;
use std::sync::OnceLock;

use serde_json::Value;

use crate::{EngineError, Result, Session};

// Re-export the public engine surface that other crates address via `crate::commands::*`.
pub use crate::workspace::{
    sanitize_asset_id, asset_id_for_import, workspace_root, resolve_workspace_path,
    resolve_craft_cli, build_photocraft_run_args, build_effectcraft_script_args,
    build_filmcraft_load_args, build_blender_check_args,
};
pub use crate::photocraft::{run_photocraft_transparent_step1, decontaminate_sprite_rgba};
pub use crate::vision::{analyze_rgba_image, analyze_viewport_region};
pub use crate::project::write_project_file;

// Import command handlers into this module so the `build()` array can reference them.
use crate::image_cmds::{cmd_image_break_apart, cmd_image_color_to_alpha, cmd_image_content_aware_fill, cmd_image_image_size, cmd_image_register_frame, cmd_image_puppet_warp};
use crate::parts::{cmd_part_set_state, cmd_part_show_all};
use crate::photocraft::{
    cmd_asset_build_cutout_rig, cmd_asset_cut_sheet, cmd_asset_extract_background,
    cmd_asset_load_manifest, cmd_asset_make_transparent, cmd_export_agent_prompt, cmd_intake_compare_reference,
    cmd_intake_three_question, cmd_intake_validate,
};
use crate::project::{
    cmd_project_add_asset, cmd_project_close, cmd_project_list_assets, cmd_project_load_asset,
    cmd_project_new, cmd_project_open, cmd_project_remove_asset, cmd_project_save, cmd_project_update_asset,
};
use crate::rig::cmd_mesh_build_4dir;
use crate::sequence::cmd_sequence_render;
use crate::sheets::cmd_assets_agent_sheets;
use crate::agent::{cmd_agent_config, cmd_agent_models, cmd_agent_session, cmd_agent_log, cmd_agent_new_chat, cmd_agent_send};
use crate::turnaround::{cmd_anim_preview_2d, cmd_turnaround_optimize_accuracy};
use crate::exports::{
    cmd_effectcraft_run_script, cmd_export_blender,
    cmd_export_cutout_mp4, cmd_export_effectcraft, cmd_export_filmcraft,
    cmd_export_layered_motion,
    cmd_export_character_iteration, cmd_scene_farm_school_lineup, cmd_validate_motion,
};
use crate::vision::cmd_vision_analyze;
use crate::doctor::{cmd_app_analytics, cmd_app_doctor};
use crate::workshop::{
    cmd_audio_analyze, cmd_assets_assess, cmd_assets_fixlist, cmd_assets_fixlist_add,
    cmd_assets_request_sheet, cmd_project_search, cmd_query_list, cmd_storyboard_approve,
    cmd_storyboard_edit, cmd_storyboard_import_openmontage, cmd_storyboard_init,
    cmd_storyboard_toc,
    cmd_video_init, cmd_video_server_start, cmd_video_server_stop,
    cmd_workshop_add, cmd_workshop_approve_draft, cmd_workshop_build_storyboard,
    cmd_workshop_files, cmd_workshop_import_transcript,
    cmd_workshop_remove, cmd_workshop_set_audio, cmd_workshop_set_target, cmd_workshop_split_panels,
    cmd_workshop_status, cmd_workshop_storyboard,
};

// ---------- CommandSpec + registry ----------

/// Metadata + implementation for one command.
pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// Menu placement, e.g. `["File", "New"]`. Empty = not in menus.
    pub menu: &'static [&'static str],
    /// Default shortcut (egui-style names, `Cmd` = ⌘ on macOS / Ctrl elsewhere).
    pub shortcut: Option<&'static str>,
    /// Parameter description (JSON-ish, for agents and the MCP schema).
    pub params: &'static str,
    pub enabled: fn(&Session) -> std::result::Result<(), String>,
    pub run: fn(&mut Session, &Value) -> Result<Value>,
    /// Record in the journal (false for pure queries).
    pub journal: bool,
    /// Runs external CLI tools or heavy image processing; UI dispatches on a background thread.
    pub long_running: bool,
}

impl CommandSpec {
    pub fn describe(&self) -> Value {
        serde_json::json!({"id":self.id,"label":self.label,"menu":self.menu,
            "shortcut":self.shortcut,"params":self.params,"journal":self.journal,
            "long_running":self.long_running})
    }
}

pub fn is_read_only_command(id: &str, params: &Value) -> bool {
    match id {
        "project.catalog" => params.get("forget").is_none_or(Value::is_null),
        "project.attach"
        | "project.identity"
        | "project.list_assets"
        | "project.search"
        | "workshop.status"
        | "workshop.files"
        | "workshop.storyboard"
        | "storyboard.read"
        | "storyboard.design"
        | "storyboard.backgrounds"
        | "storyboard.toc"
        | "storyboard.template"
        | "storyboard.doctor"
        | "character.assets"
        | "assets.assess"
        | "assets.fixlist"
        | "motion.tracks"
        | "motion.clips"
        | "motion.sample"
        | "motion.theatre.read"
        | "validate.motion"
        | "animation.sources"
        | "animation.status"
        | "animation.list"
        | "blender.actions"
        | "blender.status"
        | "blender.list"
        | "synfig.discover"
        | "opentoonz.discover"
        | "mlt.discover"
        | "song.lyrics"
        | "words.align"
        | "words.read"
        | "words.studio_track"
        | "words.effects"
        | "words.track"
        | "skills.list"
        | "skills.info"
        | "crew.list"
        | "agent.session"
        | "agent.models"
        | "command.list"
        | "studio.cues"
        | "vision.analyze"
        | "ui.state"
        | "ui.feedback"
        | "ui.elements"
        | "ui.components"
        | "ui.controls"
        | "ui.unconfigured" => true,
        "project.root" => params.get("root").is_none_or(Value::is_null),
        "skills.limits" => {
            params.get("max_words").is_none_or(Value::is_null)
                && params.get("max_steps").is_none_or(Value::is_null)
        }
        "skills.diagram" => params.get("render").and_then(Value::as_bool) != Some(true),
        "agent.log" => params.get("refresh").and_then(Value::as_bool) != Some(true),
        "app.analytics" => params.get("scan").and_then(Value::as_bool) != Some(true),
        "app.doctor" => params.get("clean").and_then(Value::as_bool) != Some(true),
        _ => false,
    }
}

const fn is_long_running_id(id: &str) -> bool {
    matches!(
        id.as_bytes(),
        b"project.intake"
            | b"blender.start"
            | b"mlt.start"
            | b"opentoonz.start"
            | b"opentoonz.render"
            | b"synfig.start"
            | b"motion.pose.export"
            | b"motion.export_track"
            | b"motion.place_section"
            | b"rig.from_sheet"
            | b"words.design"
            | b"sequence.render"
            | b"animation.source.render"
            | b"video.init"
            | b"intake.validate"
            | b"intake.three_question"
            | b"asset.make_transparent"
            | b"asset.extract_background"
            | b"asset.build_cutout_rig"
            | b"asset.cut_sheet"
            | b"turnaround.optimize_accuracy"
            | b"mesh.build_4dir"
            | b"export.effectcraft"
            | b"export.filmcraft"
            | b"export.blender"
            | b"export.character_iteration"
            | b"export.cutout_mp4"
            | b"export.layered_motion"
            | b"validate.motion"
            | b"audio.analyze"
            | b"workshop.build_storyboard"
            | b"workshop.split_panels"
            | b"video.snapshot"
            | b"video.render"
            | b"image.generate"
            | b"image.register_frame"
    )
}

macro_rules! cmd {
    ($id:literal, $label:literal, [$($m:literal),*], $sc:expr, $params:literal, $en:expr, $run:expr) => {
        CommandSpec { id: $id, label: $label, menu: &[$($m),*], shortcut: $sc, params: $params, enabled: $en, run: $run, journal: true, long_running: is_long_running_id($id) }
    };
}

macro_rules! query {
    ($id:literal, $label:literal, $params:literal, $run:expr) => {
        CommandSpec {
            id: $id,
            label: $label,
            menu: &[],
            shortcut: None,
            params: $params,
            enabled: always,
            run: $run,
            journal: false,
            long_running: false,
        }
    };
}

pub fn command_specs() -> &'static [CommandSpec] {
    static SPECS: OnceLock<Vec<CommandSpec>> = OnceLock::new();
    SPECS.get_or_init(build)
}

pub fn find(id: &str) -> Option<&'static CommandSpec> {
    command_specs().iter().find(|c| c.id == id)
}

// ---------- enablement ----------

pub(crate) fn always(_: &Session) -> std::result::Result<(), String> {
    Ok(())
}

fn has_asset(s: &Session) -> std::result::Result<(), String> {
    if s.assets.is_empty() {
        Err("no asset loaded".into())
    } else {
        Ok(())
    }
}

/// Rejects a parameter whose JSON type differs from the example value in the command's `params`
/// description. Every caller (CLI, MCP, app, bridge) goes through `Session::execute`, so a
/// mistyped value fails the same way everywhere.
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

// ---------- command registry (every id maps to one handler above) ----------

pub fn build() -> Vec<CommandSpec> {
    vec![
        cmd!("blender.start", "Build / Render Blender Animation", ["Animation"], None,
            "{\"character\":\"rig id for initial migration\",\"track\":\"track id\",\"blend\":\"optional saved .blend instead of migration\",\"resolution_percent\":50,\"max_frames\":90,\"memory_mb\":4096,\"timeout_seconds\":300}", always, crate::blender::cmd_blender_start),
        query!("synfig.discover", "Discover Synfig Native Renderer", "{}", crate::synfig::discover),
        cmd!("synfig.start", "Render Saved Synfig Scene", ["Animation"], None,
            "{\"source\":\"saved .sif/.sifz\",\"bundle\":\"self-contained source directory\",\"first_frame\":0,\"last_frame\":23,\"fps\":24,\"width\":512,\"height\":512,\"memory_mb\":4096,\"timeout_seconds\":300}", always, crate::synfig::start),
        query!("animation.sources", "Registered native animation sources", "{\"expected_project\":\"optional project identity\"}", crate::animation_links::sources),
        cmd!("animation.source.set", "Register editable native source", ["Animation"], None, "{\"expected_project\":\"selected .magic\",\"id\":\"stable source id\",\"backend\":\"blender|synfig|opentoonz|mlt\",\"source\":\"editable native file; MLT assembly JSON\",\"bundle\":\"optional dependency root\",\"character\":\"optional paired rig ID\",\"track\":\"optional paired action ID\",\"expected_revision\":0}", always, crate::animation_links::set),
        cmd!("animation.source.render", "Render registered native source", ["Animation"], None, "{\"expected_project\":\"selected .magic\",\"id\":\"source id\",\"expected_revision\":1,\"options\":{}}", always, crate::animation_links::render),
        cmd!("animation.source.accept", "Accept completed native result", ["Animation"], None, "{\"expected_project\":\"selected .magic\",\"id\":\"source id\",\"expected_revision\":1,\"job\":\"completed full job id\"}", always, crate::animation_links::accept),
        query!("animation.status", "Native Animation Job Progress", "{\"job\":\"job id\"}", crate::blender::cmd_blender_status),
        query!("animation.list", "Native Animation Jobs and Global Owner", "{}", crate::blender::cmd_blender_list),
        query!("animation.cancel", "Cancel Native Animation Job", "{\"job\":\"job id\"}", crate::blender::cmd_blender_cancel),
        query!("blender.actions", "Saved Blender Migration Actions", "{\"character\":\"rig id\"}", crate::blender::cmd_blender_actions),
        query!("blender.prepare", "Headless Bounded Migration Preparation", "{\"request\":\"worker request.json\"}", crate::blender::cmd_blender_prepare),
        query!("blender.status", "Blender Job Progress", "{\"job\":\"job id\"}", crate::blender::cmd_blender_status),
        query!("blender.list", "Saved Blender Jobs", "{}", crate::blender::cmd_blender_list),
        query!("blender.cancel", "Cancel Blender Job", "{\"job\":\"job id\"}", crate::blender::cmd_blender_cancel),
        query!("mlt.discover", "Discover MLT assembly runtime", "{}", crate::mlt::discover),
        cmd!("mlt.start", "Assemble clips and audio with MLT", ["Video"], None,
            "{\"timeline\":\"generic timeline JSON path\",\"memory_mb\":4096,\"timeout_seconds\":300}", always, crate::mlt::start),
        cmd!("opentoonz.start", "Render OpenToonz through shared worker", ["Animation"], None,
            "{\"scene\":\"saved .tnz\",\"bundle\":\"optional self-contained project root\",\"first\":1,\"last\":24,\"step\":1,\"shrink\":1,\"nthreads\":2,\"memory_mb\":4096,\"timeout_seconds\":300}", always, crate::opentoonz::cmd_opentoonz_start),
        query!("opentoonz.discover", "OpenToonz Availability and Render Plan", "{\"scene\":\"optional .tnz path; plans without rendering\",\"first\":\"optional one-based\",\"last\":\"optional one-based\"}", crate::opentoonz::cmd_opentoonz_discover),
        cmd!("opentoonz.render", "Render OpenToonz Scene Frames", ["Animation"], None, "{\"request\":\"worker mode: job request.json (scene, range); only inside its bounded worker\",\"scene\":\".tnz path\",\"out\":\"new absolute frames folder\",\"first\":\"optional one-based\",\"last\":\"optional one-based\",\"step\":1,\"shrink\":1,\"nthreads\":2}", always, crate::opentoonz::cmd_opentoonz_render),
        query!("project.attach", "Attach Project Metadata Without Switching Studio", "{\"path\":\"existing .magic path\"}", crate::project::cmd_project_attach),
        query!("project.identity", "Canonical Project Identity", "{\"path\":\"existing .magic path\"}", crate::project::cmd_project_identity),
        query!("project.catalog", "Home Project Navigation History", "{}", crate::project_catalog::catalog),
        cmd!("project.root", "Inspect / Set Production Project Root", ["File"], None, "{\"root\":\"optional absolute production root; omitted to inspect\"}", always, crate::project_settings::command),
        query!("skills.list", "Registered knowledge skills (info blocks)", "{}", crate::skills_registry::cmd_skills_list),
        query!("crew.list", "Magic Studio MOPPETS and shared department AGENTS.md files", "{\"workspace\":\"optional workshop root\"}", crate::crew::cmd_list),
        cmd!("crew.save", "Save one department cast and shared instructions (revision guarded)", [], None, "{\"workspace\":\"workshop root from crew.list\",\"department\":\"creative|animation|production|backstage|review\",\"expected_revision\":\"from crew.list\",\"document\":\"department and instructions from crew.list\"}", always, crate::crew::cmd_save),
        query!("skills.info", "One registered skill's info block", "{\"id\":\"skill id\"}", crate::skills_registry::cmd_skills_info),
        cmd!("skills.limits", "Read / explicitly configure Learning block caps", [], None, "{\"max_words\":\"optional explicit cap\",\"max_steps\":\"optional explicit cap\"}", always, crate::skills_registry::cmd_skills_limits),
        cmd!("skills.diagram", "Archify view derived from Learning task records", [], None, "{\"kind\":\"production|correction\",\"render\":false}", always, crate::skills_registry::cmd_skills_diagram),
        cmd!("skills.edit", "Save Learning Markdown (revision guarded)", [], None, "{\"id\":\"block id\",\"markdown\":\"complete Markdown\",\"expected_revision\":\"from skills.info\"}", always, crate::skills_registry::cmd_skills_edit),
        cmd!("skills.import", "Import a cited draft reference (never execute)", [], None, "{\"id\":\"new block id\",\"source\":\"https URL or existing skill/package path\",\"category\":\"optional\"}", always, crate::skills_registry::cmd_skills_import),
        cmd!("skills.remove", "Remove Learning block reversibly", [], None, "{\"id\":\"block id\",\"expected_revision\":\"from skills.info\"}", always, crate::skills_registry::cmd_skills_remove),
        cmd!("skills.undo", "Restore removed Learning block", [], None, "{\"id\":\"block id\",\"expected_revision\":\"from remove receipt\"}", always, crate::skills_registry::cmd_skills_undo),
        cmd!("skills.add", "Register / update one knowledge skill", [], None, "{\"id\":\"skill id\",\"name\":\"display name\",\"description\":\"what it does\",\"status\":\"researching|implemented|not_implemented\",\"source\":\"url or reference\",\"implemented_at\":[\"command ids\"],\"implemented_in\":[\"file paths\"],\"integration_notes\":\"where it plugs in\"}", always, crate::skills_registry::cmd_skills_add),
        query!("backstage.events", "Recent Activity Receipts", "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}", crate::activity::events),
        query!("backstage.status", "Backstage — Live Production Status", "{\"trace\":false}", crate::command_center::status),
        query!("booth.events", "Recent Activity Receipts (alias of backstage.events)", "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}", crate::activity::events),
        query!("booth.status", "Backstage (alias of backstage.status)", "{\"trace\":false}", crate::command_center::status),
        query!("center.events", "Recent Activity Receipts (alias of backstage.events)", "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}", crate::activity::events),
        query!("ui.state", "Live window state", "{}", crate::command_center::requires_ui),
        query!("ui.elements", "Visible controls", "{\"region\":\"optional CSS selector\"}", crate::command_center::requires_ui),
        query!("ui.unconfigured", "Unconfigured controls", "{}", crate::command_center::requires_ui),
        cmd!("ui.click", "Click a control", [], None, "{\"id\":\"element ID\",\"instance\":0,\"files\":[]}", always, crate::command_center::requires_ui),
        cmd!("ui.set", "Set a control", [], None, "{\"id\":\"element ID\",\"value\":\"value\",\"instance\":0}", always, crate::command_center::requires_ui),
        cmd!("ui.key", "Press a key", [], None, "{\"key\":\"key name\",\"id\":\"optional element ID\"}", always, crate::command_center::requires_ui),
        cmd!("ui.drag", "Drag a control", [], None, "{\"id\":\"optional element ID\",\"from\":[],\"to\":[]}", always, crate::command_center::requires_ui),
        cmd!("ui.look", "Capture window and state", [], None, "{\"output\":\"optional PNG path\",\"region\":\"optional CSS selector\",\"click\":\"optional element ID\"}", always, crate::command_center::requires_ui),
        cmd!("ui.screenshot", "Capture window", [], None, "{\"output\":\"optional PNG path\"}", always, crate::command_center::requires_ui),
        cmd!("ui.focus", "Focus window", [], None, "{}", always, crate::command_center::requires_ui),
        cmd!("ui.eval", "Evaluate window JavaScript", [], None, "{\"js\":\"JavaScript body\"}", always, crate::command_center::requires_ui),
        cmd!("app.quit", "Guarded window close", [], None, "{}", always, crate::command_center::requires_ui),
        query!("ui.feedback", "Live Studio Feedback", "{\"project\":\"optional canonical project path\"}", crate::command_center::requires_ui),
        query!("ui.components", "Rendered Studio Components", "{\"region\":\"#view-project\",\"filter\":\"optional ID filter\",\"limit\":30}", crate::command_center::requires_ui),
        query!("ui.controls", "Studio control telemetry", "{\"region\":\"optional region\",\"offset\":0,\"limit\":64}", crate::command_center::requires_ui),
        cmd!("ui.attention", "Request attention at a Studio panel", ["Window"], None, "{\"project\":\"exact open .magic path\",\"id\":\"registered panel id\",\"reason\":\"what needs attention\",\"clear\":false}", always, crate::command_center::requires_ui),
        query!("center.status", "Backstage (alias of backstage.status)", "{\"trace\":false}", crate::command_center::status),
        query!("studio.cues", "Cues at a Song Time / Uses of a Clip (read-only)", "{\"project\":\"existing .magic path\",\"at\":43.2,\"clip\":\"clip id; give at or clip\"}", crate::studio_cues::cues),
        cmd!("project.migrate", "Migrate a Legacy .acproj to .magic", ["File"], None, "{\"path\":\"legacy .acproj file\"}", always, crate::project::cmd_project_migrate),
        cmd!("project.intake", "New / Resume Project Intake", ["File"], None, "{\"path\":\"optional explicit .magic override\",\"root\":\"optional per-request root override; defaults to project.root\",\"name\":\"song name\",\"audio\":\"optional existing audio; analyzed only without valid existing timing\",\"transcript\":\"optional existing JSON\",\"captions\":\"optional original SRT\",\"audiomap\":\"optional existing audiomap.json\",\"materials\":[\"optional notes/reference file\"],\"skills\":[{\"id\":\"skill id\",\"name\":\"display name\",\"description\":\"what it does\",\"status\":\"researching|implemented|not_implemented\",\"source\":\"url or reference\",\"implemented_at\":[\"command ids\"],\"implemented_in\":[\"file paths\"],\"integration_notes\":\"where it plugs in\"}],\"resume\":false,\"replace_transcript\":false,\"hyperframes_cli\":\"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}", always, crate::command_center::intake),
        cmd!("motion.layers.set", "Set planar layer stacking", ["Animation"], None,
            "{\"expected_project\":\"selected .magic\",\"character\":\"asset id\",\"expected_rig_sha256\":\"current hash\",\"part\":\"alternative to order: part ID\",\"depth\":0,\"order\":[\"back-part\",\"front-part\"]}", always, crate::motion_authoring::layers_set),
        cmd!("motion.part.set", "Set saved action part channels", ["Animation"], None, "{\"expected_project\":\"selected .magic\",\"character\":\"asset id\",\"track\":\"authored action id\",\"part\":\"part id\",\"expected_rig_sha256\":\"motion.tracks hash\",\"position\":[0,0],\"scale\":[100,100],\"rotation\":0,\"pivot\":[0,0]}", always, crate::motion_authoring::part_set),
        cmd!("motion.part.reset", "Reset explicit saved part fields", ["Animation"], None, "{\"expected_project\":\"selected .magic\",\"character\":\"asset id\",\"track\":\"action id\",\"part\":\"part id\",\"expected_rig_sha256\":\"hash\",\"fields\":[\"position\",\"scale\",\"rotation\"]}", always, crate::motion_authoring::part_reset),
        cmd!("motion.action.set", "Edit authored action metadata", ["Animation"], None, "{\"expected_project\":\"selected .magic\",\"character\":\"asset id\",\"track\":\"stable action id\",\"expected_rig_sha256\":\"hash\",\"label\":\"Action name\",\"duration\":3,\"loop\":false}", always, crate::motion_authoring::action_set),
        cmd!("motion.edit.undo", "Undo a guarded rig edit", ["Animation"], None, "{\"expected_project\":\"selected .magic\",\"character\":\"asset id\",\"expected_rig_sha256\":\"current hash\",\"undo\":\"receipt token\"}", always, crate::motion_authoring::undo),
        cmd!("motion.pose.export", "Export and register sampled pending pose", ["Animation"], None, "{\"expected_project\":\"selected .magic\",\"character\":\"asset id\",\"track\":\"action id\",\"time\":0,\"rest\":false,\"amount\":100,\"expected_rig_sha256\":\"hash\",\"theatre_state\":null,\"reference_asset\":\"optional asset ID\"}", always, crate::motion_authoring::pose_export),
        cmd!("motion.path", "Draw a timed motion path", ["Animation"], None, "{\"character\":\"asset id\",\"track\":\"source track\",\"layer\":\"optional root layer\",\"points\":[[0,0],[100,100]],\"duration\":3,\"start_size\":100,\"end_size\":100}", always, crate::motion::cmd_motion_path),
        cmd!("motion.undo_path", "Undo drawn motion path", ["Edit"], None, "{\"character\":\"asset id\",\"undo\":\"history token\"}", always, crate::motion::cmd_motion_undo_path),
        query!("agent.session", "Current Chat Session", "{}", cmd_agent_session),
        cmd!("sprocket.send", "Send a message to Sprocket (the app's in-app agent)", ["Agent"], None, "{\"message\":\"text\",\"new_chat\":true,\"session\":\"optional expected existing session; requires new_chat=false\",\"wait\":300,\"free_only\":true}", always, crate::agent::cmd_sprocket_send),
        query!("sprocket.status", "Sprocket Session Status", "{}", cmd_agent_session),
        cmd!("agent.persist", "Persist the Active Chat Selection to Shared Settings", ["Agent"], None, "{}", always, crate::agent::cmd_agent_persist),
        query!("sprocket.models", "Sprocket Model Catalog (local first, then free)", "{}", crate::agent::cmd_sprocket_models),
        cmd!("sprocket.use", "Switch Sprocket's Model", ["Agent"], None, "{\"model\":\"provider/id, e.g. ollama/llama3.2:latest\"}", always, crate::agent::cmd_sprocket_use),
        query!("sprocket.log", "Sprocket Transcript / Reply Retrieval", "{\"refresh\":false}", cmd_agent_log),
        query!("agent.config", "Current Window Chat Selection", "{\"selection\":{\"model\":\"optional active model; shared defaults unchanged\"}}", cmd_agent_config),
        query!("agent.models", "Available Chat Models", "{\"agent\":\"optional catalog adapter: opencode\"}", cmd_agent_models),
        cmd!("agent.new_chat", "New Agent Chat", ["Agent"], None, "{\"free_only\":false,\"image_task\":false}", always, cmd_agent_new_chat),
        cmd!("motion.export_track", "Export a preview action as transparent video", ["Animation"], None, "{\"character\":\"asset id\",\"track\":\"track id\",\"amount\":35,\"rest\":false,\"duration\":2.58}", always, crate::motion::cmd_motion_export_track),
        cmd!("motion.place_section", "Animate a song section: draw the path, export, gate and place it in the video", ["Animation"], None, "{\"character\":\"asset id\",\"track\":\"source track (default the first)\",\"layer\":\"optional root layer\",\"points\":[[0,0],[100,100]],\"start_size\":100,\"end_size\":100,\"amount\":100,\"id\":\"video clip id\",\"start\":0,\"duration\":3,\"rect\":[0,0,1920,1080]}", always, crate::motion::cmd_motion_place_section),
        cmd!("motion.action_track", "Resolve an action and configure its playable rig", ["Animation"], None, "{\"character\":\"asset id\",\"action\":\"walk\",\"track\":\"optional track override\",\"rest\":false}", always, crate::motion::cmd_motion_action_track),
        query!("motion.tracks", "Inspect animation tracks and layers", "{\"character\":\"asset id\"}", crate::motion::cmd_motion_tracks),
        query!("character.assets", "Retrieve imported character.magic config, sheets, angles, pose strips, and drafts", "{\"character\":\"asset id\"}", crate::character::cmd_character_assets),
        cmd!("character.import", "Import an image or sheet into a character folder as original draft, reference, or sheet", ["Animation"], None, "{\"character\":\"asset id\",\"path\":\"/path/to/sheet.png\",\"role\":\"original_draft|reference|turnaround|poses|skeleton\"}", always, crate::character::cmd_character_import),
        cmd!("character.configure", "Configure character angle, direction, or pose in character.magic", ["Animation"], None, "{\"character\":\"asset id\",\"angle\":\"three_quarter_right\",\"direction\":\"right\",\"pose\":\"right\"}", always, crate::character::cmd_character_configure),
        query!("motion.sample", "Sample engine motion for drawing", "{\"character\":\"asset id\",\"track\":\"track id\",\"fps\":30,\"rest\":false,\"amount\":100,\"time\":0}", crate::motion::cmd_motion_sample),
        query!("motion.theatre.read", "Read native Theatre state", "{\"expected_project\":\"optional selected .magic identity\"}", crate::theatre::read),
        query!("motion.clips", "List clips and their Theatre sheets", "{\"character\":\"asset id\"}", crate::theatre::list),
        cmd!("motion.clips.convert", "Give every clip its own Theatre sheet", [], None, "{\"character\":\"asset id\"}", always, crate::theatre::convert),
        cmd!("motion.theatre.clear", "Clear a character's Theatre keyframes", [], None, "{\"expected_project\":\"required selected .magic identity\",\"character\":\"asset id\",\"track\":\"optional clip id; all clips when omitted\"}", always, crate::theatre::clear),
        cmd!("motion.theatre.save", "Save native Theatre state", [], None, "{\"expected_project\":\"required selected .magic identity\",\"state\":{\"definitionVersion\":\"0.4.0\",\"revisionHistory\":[],\"sheetsById\":{}}}", always, crate::theatre::save),
        query!(
            "project.list_assets",
            "List Project Assets",
            "{}",
            cmd_project_list_assets
        ),
        cmd!(
            "project.new",
            "New Project",
            ["File"],
            Some("Cmd+N"),
            "{\"path\":\"folder/name.magic\",\"audio\":\"optional song audio\",\"overwrite\":false}",
            always,
            cmd_project_new
        ),
        cmd!(
            "project.open",
            "Open Project",
            ["File"],
            Some("Cmd+O"),
            "{\"path\":\"folder/name.magic\"}",
            always,
            cmd_project_open
        ),
        cmd!(
            "project.save",
            "Save Project",
            ["File"],
            Some("Cmd+S"),
            "{\"path\":\"optional_save_as.magic\"}",
            always,
            cmd_project_save
        ),
        cmd!(
            "project.close",
            "Close Project",
            ["File"],
            None,
            "{}",
            always,
            cmd_project_close
        ),
        cmd!(
            "project.add_asset",
            "Connect Project Asset",
            ["Project Assets"],
            None,
            "{\"path\":\"image_or_manifest\",\"preset\":\"alternative_to_path\",\"name\":\"optional\",\"id\":\"optional\",\"kind\":\"auto|image|poses|turnaround|cutout_rig\",\"character\":\"name (outside image -> assets/characters/<name>/parts/)\",\"background\":\"name (outside image -> assets/backgrounds/<name>/)\",\"image\":\"name (outside image -> assets/images/<name>/)\"}",
            always,
            cmd_project_add_asset
        ),
        cmd!(
            "project.remove_asset",
            "Remove Project Asset",
            ["Project Assets"],
            None,
            "{\"id\":\"asset_id\"}",
            always,
            cmd_project_remove_asset
        ),
        cmd!(
            "project.update_asset",
            "Rename / Reorder Asset",
            ["Project Assets"],
            None,
            "{\"id\":\"asset_id\",\"name\":\"optional\",\"index\":0}",
            always,
            cmd_project_update_asset
        ),
        cmd!(
            "project.load_asset",
            "Load Connected Asset",
            ["Project Assets"],
            None,
            "{\"id\":\"asset_id\",\"reload\":false}",
            always,
            cmd_project_load_asset
        ),
        cmd!(
            "asset.make_transparent",
            "Make Transparent",
            ["Rig & Matting"],
            None,
            "{\"tolerance\":25}",
            has_asset,
            cmd_asset_make_transparent
        ),
        cmd!(
            "asset.extract_background",
            "Extract Background",
            ["Rig & Matting"],
            None,
            "{\"asset\":\"optional_id\",\"rect\":\"optional [x,y,w,h] around the subject\",\"point\":\"optional [x,y] on the subject\",\"expand\":6}",
            has_asset,
            cmd_asset_extract_background
        ),
        cmd!(
            "asset.build_cutout_rig",
            "Build Cutout Rig",
            ["Rig & Matting"],
            None,
            "{\"mode\":\"optional\",\"speed\":1.0,\"duration\":3.0,\"parts\":\"optional_csv\"}",
            has_asset,
            cmd_asset_build_cutout_rig
        ),
        cmd!("rig.from_sheet", "Rig From Sheet", ["Rig & Matting"], None, "{\"sheet\":\"rig sheet asset id or PNG (image.generate rig_parts=...)\",\"name\":\"rig id\",\"out_dir\":\"optional; default assets/characters/<name>/rig\",\"pivots\":{\"part id\":\"[u,v] in the part, 0-1; corrects a joint without a new sheet\"},\"places\":{\"part id\":\"[cx,cy] centre in canvas pixels; corrects a misplaced part\"},\"order\":[\"optional: part ids back to front\"]}", always, crate::image_cmds::cmd_rig_from_sheet),
        cmd!(
            "image.break_apart",
            "Break Image Into Parts",
            ["Edit", "Break Apart"],
            None,
            "{\"asset\":\"optional_id\",\"regions\":{\"part_id\":[[x,y],[x,y],[x,y]]},\"out_dir\":\"optional_dir\",\"rig_order\":[\"optional: part ids back to front; also writes rig.json for project.add_asset kind=cutout_rig\"]}",
            always,
            cmd_image_break_apart
        ),
        cmd!(
            "image.puppet_warp",
            "Puppet Warp Mesh",
            ["Edit", "Break Apart"],
            None,
            "{\"asset\":\"optional_id\",\"source\":\"path\",\"out\":\"path\",\"pins\":[{\"src\":[x,y],\"dst\":[x,y]}],\"mode\":\"rigid|normal|distort\",\"density\":\"fewer|normal|more\",\"expansion\":2}",
            always,
            cmd_image_puppet_warp
        ),
        cmd!(
            "image.color_to_alpha",
            "Color to Alpha",
            ["Edit", "Break Apart"],
            None,
            "{\"asset\":\"optional_id\",\"source\":\"path\",\"out\":\"path\",\"color\":\"#FFFFFF\",\"transparencyThreshold\":0,\"opacityThreshold\":1}",
            always,
            cmd_image_color_to_alpha
        ),
        cmd!(
            "image.content_aware_fill",
            "Content-Aware Fill",
            ["Edit", "Break Apart"],
            None,
            "{\"asset\":\"optional_id\",\"source\":\"path\",\"out\":\"path\",\"select\":{\"id\":\"select.rect\",\"params\":{}},\"contents\":\"contentAware\",\"color\":\"#hex\"}",
            always,
            cmd_image_content_aware_fill
        ),
        cmd!(
            "sequence.render",
            "Render Image Sequence",
            ["Animation", "Sequence"],
            None,
            "{\"name\":\"sequence\",\"canvas\":[1080,1080],\"fps\":30,\"duration\":4.0,\"background\":\"#101820|null\",\"layers\":[{\"id\":\"title\",\"file\":\"path.png\",\"center\":[540,540],\"scale\":100,\"entry\":{\"kind\":\"fade|rise|drop|left|right|zoom_in|zoom_out|turn\",\"duration\":0.6,\"delay\":0.0,\"ease\":\"linear|smooth|in|out\"},\"exit\":{\"kind\":\"fade\",\"duration\":0.5,\"delay\":3.4}}],\"stills\":0,\"video\":true}",
            always,
            cmd_sequence_render
        ),
        cmd!(
            "validate.motion",
            "Validate Motion Clip",
            ["Export"],
            None,
            "{\"video\":\"clip.webm\",\"frames\":0,\"fps\":30,\"loop\":false,\"reference\":\"optional_image\",\"spec\":\"optional_spec\"}",
            always,
            cmd_validate_motion
        ),
        cmd!(
            "export.layered_motion",
            "Export Layered Motion Video",
            ["Export"],
            None,
            "{\"spec\":\"layered_motion_spec.json\",\"out_dir\":\"optional_dir\",\"tracks\":\"optional_csv\",\"stills\":0,\"video\":true,\"gpu\":true,\"crf\":24}",
            always,
            cmd_export_layered_motion
        ),
        cmd!(
            "export.cutout_mp4",
            "Export Cutout MP4",
            ["Export"],
            None,
            "{\"output\":\"optional.mp4\",\"mode\":\"optional\",\"speed\":1.0,\"duration\":3.0}",
            has_asset,
            cmd_export_cutout_mp4
        ),
        query!("workshop.status", "Workshop Stages", "{}", cmd_workshop_status),
        query!("song.lyrics", "Song Lyrics (Artist Sheet or Transcript)", "{}", crate::lyrics::cmd_song_lyrics),
        query!("words.align", "Artist Words vs Timing Reference", "{}", crate::words::cmd_words_align),
        query!("words.read", "Words Animations", "{\"animation_id\":\"optional\"}", crate::words::cmd_words_read),
        query!("words.studio_track", "Lyric Clips in the Video", "{}", crate::words::cmd_words_studio_track),
        cmd!("words.lyrics_view", "Stage, Lyrics-Only or Scene-Only View of the Video", ["Design"], None, "{\"show\":\"still (one saved scene image plus audio and lyrics; requires animation_id), stage, lyrics or scene\"}", always, crate::words::cmd_words_lyrics_view),
        query!("words.effects", "Effects from the HyperFrames Registry", "{\"kind\":\"optional: text, drawing or background\"}", crate::words::cmd_words_effects),
        cmd!("words.effect_install", "Install a Registry Effect", ["Design"], None, "{\"name\":\"per-word-rise\"}", always, crate::words::cmd_words_effect_install),
        cmd!("words.captions", "Captions by the Artist's Lines", ["Design"], None, "{\"by\":\"section (default) or line\",\"out\":\"optional .srt path; default lyrics.captions.srt in the project\",\"to\":\"video: the video's caption track shows the printed lines (earlier source kept as captions.vN.html)\"}", always, crate::words::cmd_words_captions),
        cmd!("words.track", "Lyric Line in the Video", ["Design"], None, "{}", always, crate::words::cmd_words_track),
        cmd!("words.design", "Design Every Sung Line", ["Design"], None, "{\"recipe\":\"optional; default lyrics-design.json in the project (palette, band, hero words and their moves)\",\"apply\":\"true (default) uses each section in the video\",\"section\":\"optional animation id\",\"layout\":\"optional kinetic layout\",\"entrance\":\"optional kinetic entrance\",\"overlay\":\"optional overlay layer\",\"seed\":\"varies layouts and entrances per section\",\"kinetic\":\"false keeps static line designs\"}", always, crate::words::cmd_words_design),
        cmd!("words.apply", "Use Designed Lyrics in the Video", ["Design"], None, "{\"animation_id\":\"words-chorus-1\",\"use\":true}", always, crate::words::cmd_words_apply),
        cmd!("words.save", "Save Words Animation", ["Design"], None, "{\"animation_id\":\"words-chorus-1\",\"expected_source_revision\":0,\"style\":{\"size\":120},\"words\":[{\"word_id\":\"ch1-l1/w5\",\"action\":\"cry\",\"emphasis\":true}]}", always, crate::words::cmd_words_save),
        cmd!("words.link", "Link Drawing to a Lyric Word", ["Design"], None, "{\"animation_id\":\"source section\",\"index\":0,\"word_id\":\"target word ID\",\"target_animation_id\":\"optional target section\",\"expected_source_revision\":0,\"expected_target_revision\":0}", always, crate::words::cmd_words_link),
        cmd!("words.compose", "Compose Words Track (HyperFrames)", ["Design"], None, "{\"animation_id\":\"words-chorus-1\",\"instance_id\":\"optional: isolated composition for a repeated placement\"}", always, crate::words::cmd_words_compose),
        query!("workshop.files", "Workshop Files", "{\"filter\": \"optional substring\", \"group\": \"optional audio|liner_notes|transcript|storyboard|assets|other\"}", cmd_workshop_files),
        query!("workshop.storyboard", "Storyboard Pages", "{}", cmd_workshop_storyboard),
        query!("project.search", "Search Project, Files, Storyboard & Commands", "{\"query\": \"search text\"}", cmd_project_search),
        query!("assets.assess", "Assess Asset (clean or recreate?)", "{\"path\": \"layer.png\", \"joints\": [\"optional sides a part joins on: left, right, top, bottom\"]}", cmd_assets_assess),
        query!("assets.fixlist", "Asset Fix List", "{}", cmd_assets_fixlist),
        cmd!("assets.fixlist_add", "Add to Asset Fix List", ["Workshop"], None,
            "{\"asset\": \"id\", \"path\": \"png\", \"decision\": \"clean|recreate\", \"problem\": \"what is wrong\", \"tool\": \"exact command (clean)\", \"reference\": \"optional\", \"status\": \"open|done\"}", always, cmd_assets_fixlist_add),
        cmd!("assets.request_sheet", "Request Asset Sheet (one pass)", ["Workshop"], None, "{}", always, cmd_assets_request_sheet),
        cmd!("image.register_frame", "Register a pose on a shared canvas", ["Edit"], None, "{\"source\":\"pose.png\",\"out\":\"new-version.png\",\"canvas\":[1400,900],\"source_pivot\":[490,389],\"target_pivot\":[700,450],\"scale\":1.15}", always, cmd_image_register_frame),
        cmd!("image.image_size", "Image Size (normalise a sheet)", ["Edit"], None, "{\"source\":\"path.png\",\"out\":\"path.png\",\"width\":2048,\"height\":2048,\"anchor\":\"center\",\"extensionColor\":\"#ffffff\"}", always, cmd_image_image_size),
        query!("agent.log", "Agent Transcript / Reply Retrieval", "{\"refresh\":false}", cmd_agent_log),
        cmd!("agent.send", "Send to the app agent", ["Agent"], None, "{\"message\":\"text\",\"attachments\":[\"png\"],\"new_chat\":true,\"session\":\"optional expected existing session; requires new_chat=false\",\"wait\":300,\"free_only\":false,\"image_task\":false}", always, cmd_agent_send),
        cmd!("image.generate", "Generate Image (Codex)", ["Project Assets"], None,
            "{\"prompt\":\"image description\",\"model\":\"gpt-6-luna (default; selected Codex model, not pixel resolution)\",\"quality\":\"auto (only verified option)\",\"reference_asset\":\"optional registered reference PNG asset ID\",\"references\":[\"project-relative or absolute project PNG; max 5 including reference_asset\"],\"poses\":[\"optional 2 to 8 ordered pose descriptions; requires a reference\"],\"rig_parts\":[{\"id\":\"part id\",\"description\":\"what the part is\",\"parent\":\"optional part id\",\"pivot\":\"optional [u,v] in the part, 0-1\"}],\"character\":\"optional registered source character ID\",\"scene\":1,\"animation\":\"optional intended action name\",\"transparent_background\":true,\"name\":\"optional asset name\",\"timeout_s\":300,\"project\":\"optional existing .magic for CLI targeting\"}",
            always, crate::image_generation::generate),
        cmd!("assets.agent_sheets", "Draw Asset Sheets (agent)", ["Rig & Matting"], None, "{\"character\":\"optional id\",\"kinds\":[\"turnaround\",\"skeleton\",\"poses\"],\"agent\":\"codex|omp|claude\",\"run\":true,\"check\":false,\"timeout_s\":1800,\"note\":\"optional\"}", always, cmd_assets_agent_sheets),
        cmd!("storyboard.create", "Create Storyboard / Split Scene", ["Storyboard"], None, "{\"scene\":\"optional existing scene number or ID to split\",\"word\":\"recorded word ID; new scene starts here\",\"expected_revision\":\"revision returned by storyboard.read; required for scene changes\"}", always, crate::storyboard_crud::create),
        cmd!("storyboard.update", "Update Storyboard", ["Storyboard"], None, "{\"scene\":\"scene number or ID\",\"page\":\"alternative document page\",\"field\":\"native text field\",\"value\":\"text\",\"edge\":\"optional start|end: move a scene cut, without retiming lyrics\",\"word\":\"existing recorded word ID\",\"expected_revision\":\"required for grouping or restore\",\"restore\":\"optional saved manifest.json\",\"backgrounds\":[\"optional registered image IDs to assign to the Backgrounds & locations section\"]}", always, crate::storyboard_crud::update),
        cmd!("storyboard.delete", "Remove Scene Page / Combine Coverage", ["Storyboard"], None, "{\"scene\":\"scene number or ID\",\"into\":\"previous|next; preserves lyric timing and coverage\",\"expected_revision\":\"revision returned by storyboard.read\"}", always, crate::storyboard_crud::delete),
        cmd!("storyboard.template", "Inspect Storyboard Template", ["Storyboard"], None, "{\"path\":\"magic-studio or its canonical path; omit to inspect the only supported template\"}", always, cmd_storyboard_template),
        cmd!("storyboard.init", "Create Magic Studio Storyboard", ["Storyboard"], None, "{\"empty\":false}", always, cmd_storyboard_init),
        query!("storyboard.backgrounds", "Storyboard Background Section Images", "{}", crate::storyboard_crud::backgrounds),
        query!("storyboard.design", "Inspect storyboard placements", "{}", crate::storyboard_design::inspect),
        cmd!("storyboard.place_asset", "Place an asset on a storyboard page", ["Storyboard"], None, "{\"asset\":\"id\",\"page\":1,\"x\":100,\"y\":100,\"width\":160,\"placement\":\"optional existing placement id\"}", always, crate::storyboard_design::place),
        cmd!("storyboard.remove_placement", "Remove a draft image placement", ["Storyboard"], None, "{\"placement\":\"placement id\"}", always, crate::storyboard_design::remove),
        cmd!("storyboard.save_version", "Save storyboard design as a new version", ["Storyboard"], None, "{\"draft\":\"optional reviewed draft manifest.json\"}", always, crate::storyboard_design::save),
        query!("storyboard.toc", "Storyboard Chapters", "{}", cmd_storyboard_toc),
        query!("storyboard.doctor", "Diagnose source section spillover (read-only, explicit only)", "{}", crate::storyboard::cmd_storyboard_doctor),
        query!("storyboard.read", "Read Storyboard / Scene / Page", "{\"scene\":\"optional scene number or stable ID\",\"page\":\"optional page number; omitted returns the full contents\"}", crate::storyboard_crud::read),
        cmd!("storyboard.edit", "Edit Storyboard Text", ["Storyboard"], None,
            "{\"scene\": 1, \"field\": \"story|background.main|background.alternate|cast|action|continuity|image_prompt|animation_prompt|animated_text|drawing|lyrics (page=1 edits overview; see storyboard.design text_fields)\", \"line\": 1, \"value\": \"text\", \"save\": \"native fields: true (default) saves a version now; false keeps the edit in the draft for one storyboard.save_version\"}", always, cmd_storyboard_edit),
        cmd!("storyboard.import_openmontage", "Import Scenes from an OpenMontage Project", ["Storyboard"], None,
            "{\"path\": \"existing project folder\", \"overwrite\": false}", always, cmd_storyboard_import_openmontage),
        cmd!("storyboard.approve", "Approve Scene", ["Storyboard"], None, "{\"scene\": 1, \"approved\": true}", always, cmd_storyboard_approve),
        cmd!("workshop.set_audio", "Set Song Audio", ["Workshop"], None,
            "{\"path\": \"song audio file; creates the HyperFrames project and starts Studio\", \"hyperframes_cli\": \"optional absolute executable\"}", always, cmd_workshop_set_audio),
        cmd!("video.place_track", "Place a rendered track in HyperFrames", ["Video"], None, "{\"id\":\"clip id\",\"path\":\"video file; omit with blender_job\",\"animation_job\":\"optional completed native animation job id\",\"blender_job\":\"optional completed full Blender job id; uses its verified clip, maps rect (the original canvas) to the rendered frame and keeps its frozen provenance\",\"start\":0,\"duration\":2,\"rect\":[0,0,300,500],\"scene\":14,\"generation_receipt\":\"optional native image generation.json; links source poses and character\",\"character\":\"optional registered source motion asset; requires track\",\"track\":\"optional source motion track; requires character\",\"playback_rate\":1}", always, crate::video_target::cmd_video_place_track),
        cmd!("workshop.set_target", "Set Video Target", ["Workshop"], None,
            "{\"target\": \"hyperframes\"}", always, cmd_workshop_set_target),
        cmd!("workshop.add", "Add Liner Notes / Panels / Assets", ["Workshop"], None,
            "{\"kind\": \"liner_notes|panels|assets\", \"paths\": [\"file\"]}", always, cmd_workshop_add),
        cmd!("workshop.split_panels", "Split Storyboard Grid (PhotoCraft)", ["Workshop"], None,
            "{\"image\": \"storyboard grid\", \"columns\": 3, \"rows\": 2, \"inset\": 8, \"prompts\": \"optional prompt.md with a numbered panel list\"}", always, cmd_workshop_split_panels),
        cmd!("workshop.import_transcript", "Import Transcript", ["Workshop"], None,
            "{\"path\": \"transcript.json\", \"captions\": \"optional original SRT; without it use provisional timed-word fallback\", \"audiomap\": \"optional existing audiomap.json\", \"replace\": false, \"filmcraft_project\": \"optional .fcproj\"}", always, cmd_workshop_import_transcript),
        cmd!("workshop.build_storyboard", "Build Storyboard from Template", ["Workshop"], None,
            "{\"template\":\"magic-studio (the only supported storyboard template)\",\"source\":\"optional OpenMontage project folder\",\"draft\":false,\"resume\":\"optional existing draft manifest.json\",\"transcript\":\"optional exact native source; requires draft=true\",\"allow_timing_overrun\":false,\"panels\":\"optional scene target for four chorus phrases\"}", always, cmd_workshop_build_storyboard),
        cmd!("workshop.approve_draft", "Approve Storyboard", ["Workshop"], None,
            "{\"approved\": true}", always, cmd_workshop_approve_draft),
        cmd!("workshop.remove", "Remove Workshop File", ["Workshop"], None,
            "{\"path\": \"file in the project folder (moved to trash), or the song audio (unlinked only)\"}", always, cmd_workshop_remove),
        cmd!("video.init", "Initialize Target Video Project", ["Workshop"], None,
            "{\"target\": \"optional hyperframes\", \"hyperframes_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH, no npx install\", \"captions\": true, \"assemble\": false, \"audiomap\": \"legacy parameter; retired assembly is rejected without changing the project\"}", always, cmd_video_init),
        cmd!("video.server_start", "Start Video Editor Web Server", ["Workshop"], None,
            "{\"target\": \"optional hyperframes\", \"port\": \"optional; HyperFrames selects an available port by default\", \"hyperframes_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}", always, cmd_video_server_start),
        cmd!("video.server_stop", "Stop Video Editor Web Server", ["Workshop"], None,
            "{}", always, cmd_video_server_stop),
        cmd!("video.snapshot", "Take one exact frame from HyperFrames", ["Video"], None,
            "{\"target\": \"optional hyperframes\", \"at\": \"seconds or array of seconds\", \"out\": \"output PNG path for one time; directory for multiple times\", \"hyperframes_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}", always, crate::video_target::cmd_video_snapshot),
        cmd!("video.render", "Render HyperFrames composition", ["Video"], None,
            "{\"target\": \"optional hyperframes\", \"out\": \"output path\", \"fps\": \"frames per second (default 24)\", \"workers\": \"bounded worker count (default 4, max 16)\", \"start/end\": \"unsupported by official HyperFrames CLI; any supplied value is rejected\", \"hyperframes_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}", always, crate::video_target::cmd_video_render),
        cmd!(
            "audio.analyze",
            "Analyze Song (HyperFrames)",
            ["File"],
            None,
            "{\"audio\": \"optional, default: the project's song\", \"overwrite\": false, \"analyze_beats\": true, \"model\": \"large-v3\", \"hyperframes_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH; no npx install\"}",
            always,
            cmd_audio_analyze
        ),
        cmd!(
            "intake.validate",
            "Validate Image Intake",
            ["File", "Import"],
            None,
            "{\"image\": \"path\", \"asset\": \"optional_id\", \"slot\": \"auto|original|asset_sheet|turnaround_sheet\", \"attach_to_active\": false, \"auto_cut\": false, \"tolerance\": 25, \"use_ocr\": false}",
            always,
            cmd_intake_validate
        ),
        cmd!(
            "intake.three_question",
            "Three-Question Intake",
            ["File", "Intake"],
            None,
            "{\"image\":\"optional_path\",\"q1\":\"motion\",\"q2\":\"parts_csv\",\"q3\":\"target\",\"archetype\":\"optional\",\"verb\":\"optional\"}",
            has_asset,
            cmd_intake_three_question
        ),
        cmd!(
            "intake.compare_reference",
            "Compare Reference vs Import (Jev + OCR + Rotation)",
            ["File", "Intake"],
            None,
            "{\"slot\":\"all|original|asset_sheet|turnaround\",\"yaw\":0.0}",
            has_asset,
            cmd_intake_compare_reference
        ),
        cmd!(
            "part.show_all",
            "Show All Parts",
            ["Edit"],
            None,
            "{\"character\":\"optional captured asset ID\",\"expected_project\":\"required with character\"}",
            always,
            cmd_part_show_all
        ),
        cmd!(
            "part.replace_image",
            "Replace One Rig Part Image",
            ["Edit"],
            None,
            "{\"asset\":\"asset id\",\"part\":\"stable part id\",\"candidate\":\"project-local PNG\",\"expected_rig_sha256\":\"captured rig hash\",\"expected_part_sha256\":\"captured source hash\",\"expected_candidate_sha256\":\"captured candidate hash\",\"expected_revision\":0}",
            always,
            crate::part_replacement::cmd_part_replace_image
        ),
        cmd!(
            "part.undo_image",
            "Undo Rig Part Image Replacement",
            ["Edit"],
            None,
            "{\"asset\":\"asset id\",\"undo\":\"replacement receipt token\"}",
            always,
            crate::part_replacement::cmd_part_undo_image
        ),
        CommandSpec {
            id: "part.open_photocraft",
            label: "Open Part in PhotoCraft",
            menu: &["Animation", "Parts"],
            shortcut: None,
            params: "{\"asset\":\"optional asset id\",\"part\":\"stable part id\",\"which\":\"current|original\",\"spawn\":false}",
            enabled: always,
            run: crate::part_replacement::cmd_part_open_photocraft,
            journal: false,
            long_running: false,
        },
        cmd!(
            "export.agent_prompt",
            "Export Agent Rig Prompt",
            ["File", "Export"],
            None,
            "{\"output\":\"optional_markdown_path\"}",
            has_asset,
            cmd_export_agent_prompt
        ),
        cmd!(
            "asset.cut_sheet",
            "Cut Sheet (PhotoCraft)",
            ["Edit"],
            None,
            "{\"asset\": \"optional_id\", \"out_dir\": \"optional_dir\", \"kind\": \"auto\", \"layout\": \"optional_layout.json\"}",
            has_asset,
            cmd_asset_cut_sheet
        ),
        cmd!(
            "asset.load_manifest",
            "Load Asset Manifest",
            ["File", "Open Manifest"],
            None,
            "{\"preset\": \"hopper|whiskers|puddles|jenny|rusty\", \"manifest\": \"optional_path\"}",
            always,
            cmd_asset_load_manifest
        ),
        cmd!(
            "part.set_state",
            "Set Part Visibility / Solo",
            ["Edit"],
            None,
            "{\"character\":\"optional captured asset ID\",\"expected_project\":\"required with character\",\"part\": \"name\", \"visible\": true, \"solo\": false}",
            always,
            cmd_part_set_state
        ),
        cmd!(
            "turnaround.optimize_accuracy",
            "Optimize Turnaround Accuracy",
            ["Turnaround"],
            None,
            "{\"asset\": \"optional_id\", \"turnaround_dir\": \"path\", \"poses_dir\": \"path\", \"out_dir\": \"path\"}",
            always,
            cmd_turnaround_optimize_accuracy
        ),
        cmd!(
            "anim.preview_2d",
            "Preview 2D Animation",
            ["Animation"],
            None,
            "{\"mode\": \"idle|walk|run|jump|wave|static\"}",
            always,
            cmd_anim_preview_2d
        ),
        cmd!(
            "mesh.build_4dir",
            "Build 4-Direction Mesh",
            ["Mesh"],
            None,
            "{\"format\": \"glb\"}",
            always,
            cmd_mesh_build_4dir
        ),
        cmd!(
            "export.effectcraft",
            "Export EffectCraft Animation (.jsx)",
            ["File", "Export"],
            None,
            "{\"output\": \"path.jsx\", \"mode\": \"optional\", \"speed\": 1.0, \"duration\": 3.0}",
            always,
            cmd_export_effectcraft
        ),
        cmd!(
            "effectcraft.run_script",
            "Run EffectCraft Script (.jsx)",
            ["File", "Export"],
            None,
            "{\"script\": \"path.jsx\"}",
            always,
            cmd_effectcraft_run_script
        ),
        cmd!(
            "export.filmcraft",
            "Export FilmCraft",
            ["File", "Export"],
            None,
            "{\"media\": \"path.mp4\", \"output\": \"path.fcproj\", \"frame_out\": \"path.png\"}",
            always,
            cmd_export_filmcraft
        ),
        cmd!(
            "export.blender",
            "Export Blender 5.3",
            ["File", "Export"],
            None,
            "{\"archetype\": \"humanoid_biped\", \"verb\": \"move_traverse\", \"out_mp4\": \"path.mp4\", \"out_png\": \"path.png\"}",
            always,
            cmd_export_blender
        ),
        cmd!(
            "export.character_iteration",
            "Export 2D Layers + 6-Action Track",
            ["File", "Export"],
            None,
            "{\"character\": \"hopper|whiskers|puddles|jenny|rusty|miss_hayley|old_mac|all\", \"out_root\": \"exports/design_share\"}",
            always,
            cmd_export_character_iteration
        ),
        cmd!(
            "scene.farm_school_lineup",
            "Export Farm School 6-Character Lineup Scene (Blender 5.3)",
            ["File", "Export"],
            None,
            "{\"out_dir\": \"exports/farm_school_milestone\", \"character\": \"optional_single_char\"}",
            always,
            cmd_scene_farm_school_lineup
        ),
        query!(
            "vision.analyze",
            "Direct Pixel Vision Analysis",
            "{\"character\": \"optional_slug\", \"image\": \"optional_path\"}",
            cmd_vision_analyze
        ),
        query!(
            "app.analytics",
            "Analytics: Saved Code Measurements & Status",
            "{\"scan\": false}",
            cmd_app_analytics
        ),
        query!(
            "app.doctor",
            "Doctor: CLI Availability and Video Server Metadata",
            "{\"clean\": false}",
            cmd_app_doctor
        ),
        query!("command.list", "List Commands", "{\"filter\":\"optional command ID or search text\"}", cmd_query_list),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn command_list_returns_all_registered_commands() {
        let mut s = Session::default();
        let result = crate::workshop::cmd_query_list(&mut s, &json!({})).unwrap();
        let commands = result["commands"].as_array().unwrap();
        assert_eq!(commands.len(), command_specs().len());
    }

    #[test]
    fn action_export_dispatches_without_holding_shared_session_during_render() {
        assert!(find("motion.export_track").unwrap().long_running);
    }

    #[test]
    fn find_returns_none_for_unknown_command() {
        assert!(find("unknown.command").is_none());
    }

    #[test]
    fn session_execute_unknown_command_returns_error() {
        let mut s = Session::default();
        let err = s.execute("unknown.command", json!({})).unwrap_err();
        assert!(matches!(err, EngineError::UnknownCommand(_)));
    }

    #[test]
    fn check_param_types_rejects_wrong_json_type() {
        let mut s = Session::default();
        let err = s
            .execute("project.new", json!({"path": "x.magic", "overwrite": "yes"}))
            .unwrap_err();
        assert!(matches!(err, EngineError::InvalidParams(_)));
    }
}
