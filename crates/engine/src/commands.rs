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
//!   - `project` — .acproj lifecycle + connect/load/remove/update assets
//!   - `workshop` — workshop/storyboard/video/assets/audio + search
//!
//! This file keeps only the `CommandSpec` registry, `command_specs()`, `find()`,
//! `check_param_types()`, and the `build()` array wiring every id to its handler.

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
use crate::image_cmds::{cmd_image_break_apart, cmd_image_color_to_alpha, cmd_image_content_aware_fill, cmd_image_image_size, cmd_image_puppet_warp};
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
use crate::doctor::cmd_app_doctor;
use crate::workshop::{
    cmd_audio_analyze, cmd_audio_transcribe, cmd_assets_assess, cmd_assets_fixlist, cmd_assets_fixlist_add,
    cmd_assets_request_sheet, cmd_project_search, cmd_query_list, cmd_storyboard_approve,
    cmd_storyboard_edit, cmd_storyboard_import_openmontage, cmd_storyboard_init,
    cmd_storyboard_read, cmd_storyboard_search, cmd_storyboard_toc,
    cmd_video_init, cmd_video_place_references, cmd_video_server_start, cmd_video_server_stop,
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

const fn is_long_running_id(id: &str) -> bool {
    matches!(
        id.as_bytes(),
        b"project.intake"
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
            | b"audio.transcribe"
            | b"audio.analyze"
            | b"workshop.build_storyboard"
            | b"workshop.split_panels"
            | b"video.place_references"
            | b"image.generate"
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
        query!("project.attach", "Attach Project Metadata Without Switching Studio", "{\"path\":\"existing .acproj path\"}", crate::project::cmd_project_attach),
        query!("project.identity", "Canonical Project Identity", "{\"path\":\"existing .acproj path\"}", crate::project::cmd_project_identity),
        query!("project.catalog", "Home Project Navigation History", "{}", crate::project_catalog::catalog),
        cmd!("project.root", "Inspect / Set Production Project Root", ["File"], None, "{\"root\":\"optional absolute production root; omitted to inspect\"}", always, crate::project_settings::command),
        query!("backstage.events", "Recent Activity Receipts", "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}", crate::activity::events),
        query!("backstage.status", "Backstage — Live Production Status", "{\"trace\":false}", crate::command_center::status),
        query!("booth.events", "Recent Activity Receipts (alias of backstage.events)", "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}", crate::activity::events),
        query!("booth.status", "Backstage (alias of backstage.status)", "{\"trace\":false}", crate::command_center::status),
        query!("center.events", "Recent Activity Receipts (alias of backstage.events)", "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}", crate::activity::events),
        query!("ui.feedback", "Live Studio Feedback", "{\"project\":\"optional canonical project path\"}", crate::command_center::requires_ui),
        query!("ui.components", "Rendered Studio Components", "{\"region\":\"#view-project\",\"filter\":\"optional ID filter\",\"limit\":30}", crate::command_center::requires_ui),
        query!("center.status", "Backstage (alias of backstage.status)", "{\"trace\":false}", crate::command_center::status),
        query!("studio.cues", "Cues at a Song Time / Uses of a Clip (read-only)", "{\"project\":\"existing .acproj path\",\"at\":43.2,\"clip\":\"clip id; give at or clip\"}", crate::studio_cues::cues),
        cmd!("project.intake", "New / Resume Project Intake", ["File"], None, "{\"path\":\"optional explicit .acproj override\",\"root\":\"optional per-request root override; defaults to project.root\",\"name\":\"song name\",\"audio\":\"optional existing audio; analyzed only without valid existing timing\",\"transcript\":\"optional existing JSON\",\"captions\":\"optional original SRT\",\"audiomap\":\"optional existing audiomap.json\",\"materials\":[\"optional notes/reference file\"],\"resume\":false,\"replace_transcript\":false,\"hyperframes_cli\":\"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}", always, crate::command_center::intake),
        cmd!("motion.path", "Draw a timed motion path", ["Animation"], None, "{\"character\":\"asset id\",\"track\":\"source track\",\"layer\":\"optional root layer\",\"points\":[[0,0],[100,100]],\"duration\":3,\"start_size\":100,\"end_size\":100}", always, crate::motion::cmd_motion_path),
        cmd!("motion.undo_path", "Undo drawn motion path", ["Edit"], None, "{\"character\":\"asset id\",\"undo\":\"history token\"}", always, crate::motion::cmd_motion_undo_path),
        query!("agent.session", "Current Chat Session", "{}", cmd_agent_session),
        cmd!("sprocket.send", "Send a message to Sprocket (the app's in-app agent)", ["Agent"], None, "{\"message\":\"text\",\"new_chat\":true,\"session\":\"optional expected existing session; requires new_chat=false\",\"wait\":300,\"free_only\":true}", always, crate::agent::cmd_sprocket_send),
        query!("sprocket.status", "Sprocket Session Status", "{}", cmd_agent_session),
        query!("sprocket.log", "Sprocket Transcript / Reply Retrieval", "{\"refresh\":false}", cmd_agent_log),
        query!("agent.config", "Current Window Chat Selection", "{\"selection\":{\"model\":\"optional active model; shared defaults unchanged\"}}", cmd_agent_config),
        query!("agent.models", "Available Chat Models", "{\"agent\":\"optional catalog adapter: opencode\"}", cmd_agent_models),
        cmd!("agent.new_chat", "New Agent Chat", ["Agent"], None, "{\"free_only\":false,\"image_task\":false}", always, cmd_agent_new_chat),
        cmd!("motion.export_track", "Export a preview action as transparent video", ["Animation"], None, "{\"character\":\"asset id\",\"track\":\"track id\",\"amount\":35,\"duration\":2.58}", always, crate::motion::cmd_motion_export_track),
        query!("motion.tracks", "Inspect animation tracks and layers", "{\"character\":\"asset id\"}", crate::motion::cmd_motion_tracks),
        query!("motion.sample", "Sample engine motion for drawing", "{\"character\":\"asset id\",\"track\":\"track id\",\"fps\":30,\"rest\":false,\"amount\":100}", crate::motion::cmd_motion_sample),
        query!("motion.theatre.read", "Read native Theatre state", "{\"expected_project\":\"optional selected .acproj identity\"}", crate::theatre::read),
        query!("motion.clips", "List clips and their Theatre sheets", "{\"character\":\"asset id\"}", crate::theatre::list),
        cmd!("motion.clips.convert", "Give every clip its own Theatre sheet", [], None, "{\"character\":\"asset id\"}", always, crate::theatre::convert),
        cmd!("motion.theatre.save", "Save native Theatre state", [], None, "{\"expected_project\":\"required selected .acproj identity\",\"state\":{\"definitionVersion\":\"0.4.0\",\"revisionHistory\":[],\"sheetsById\":{}}}", always, crate::theatre::save),
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
            "{\"path\":\"folder/name.acproj\",\"audio\":\"optional song audio\",\"overwrite\":false}",
            always,
            cmd_project_new
        ),
        cmd!(
            "project.open",
            "Open Project",
            ["File"],
            Some("Cmd+O"),
            "{\"path\":\"folder/name.acproj\"}",
            always,
            cmd_project_open
        ),
        cmd!(
            "project.save",
            "Save Project",
            ["File"],
            Some("Cmd+S"),
            "{\"path\":\"optional_save_as.acproj\"}",
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
            "{\"path\":\"image_or_manifest\",\"preset\":\"alternative_to_path\",\"name\":\"optional\",\"id\":\"optional\",\"kind\":\"auto|image|poses|turnaround|cutout_rig\",\"character\":\"name (outside image -> assets/characters/<name>/parts/)\",\"background\":\"name (outside image -> assets/backgrounds/<name>/)\"}",
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
        cmd!(
            "image.break_apart",
            "Break Image Into Parts",
            ["Edit", "Break Apart"],
            None,
            "{\"asset\":\"optional_id\",\"regions\":{\"part_id\":[[x,y]]},\"out_dir\":\"optional_dir\"}",
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
        query!("workshop.files", "Workshop Files", "{\"filter\": \"optional substring\", \"group\": \"optional audio|liner_notes|transcript|storyboard|assets|other\"}", cmd_workshop_files),
        query!("workshop.storyboard", "Storyboard Pages", "{}", cmd_workshop_storyboard),
        query!("project.search", "Search Project, Files, Storyboard & Commands", "{\"query\": \"search text\"}", cmd_project_search),
        query!("assets.assess", "Assess Asset (clean or recreate?)", "{\"path\": \"layer.png\", \"joints\": [\"optional sides a part joins on: left, right, top, bottom\"]}", cmd_assets_assess),
        query!("assets.fixlist", "Asset Fix List", "{}", cmd_assets_fixlist),
        cmd!("assets.fixlist_add", "Add to Asset Fix List", ["Workshop"], None,
            "{\"asset\": \"id\", \"path\": \"png\", \"decision\": \"clean|recreate\", \"problem\": \"what is wrong\", \"tool\": \"exact command (clean)\", \"reference\": \"optional\", \"status\": \"open|done\"}", always, cmd_assets_fixlist_add),
        cmd!("assets.request_sheet", "Request Asset Sheet (one pass)", ["Workshop"], None, "{}", always, cmd_assets_request_sheet),
        cmd!("image.image_size", "Image Size (normalise a sheet)", ["Edit"], None, "{\"source\":\"path.png\",\"out\":\"path.png\",\"width\":2048,\"height\":2048,\"anchor\":\"center\",\"extensionColor\":\"#ffffff\"}", always, cmd_image_image_size),
        query!("agent.log", "Agent Transcript / Reply Retrieval", "{\"refresh\":false}", cmd_agent_log),
        cmd!("agent.send", "Send to the app agent", ["Agent"], None, "{\"message\":\"text\",\"attachments\":[\"png\"],\"new_chat\":true,\"session\":\"optional expected existing session; requires new_chat=false\",\"wait\":300,\"free_only\":false,\"image_task\":false}", always, cmd_agent_send),
        cmd!("image.generate", "Generate Image (Codex)", ["Project Assets"], None,
            "{\"prompt\":\"image description\",\"model\":\"gpt-6-luna (default; selected Codex model, not pixel resolution)\",\"quality\":\"auto (only verified option)\",\"references\":[\"project-relative or absolute project PNG; max 5\"],\"transparent_background\":true,\"name\":\"optional asset name\",\"timeout_s\":300,\"project\":\"optional existing .acproj for CLI targeting\"}",
            always, crate::image_generation::generate),
        cmd!("assets.agent_sheets", "Draw Asset Sheets (agent)", ["Rig & Matting"], None, "{\"character\":\"optional id\",\"kinds\":[\"turnaround\",\"skeleton\",\"poses\"],\"agent\":\"codex|omp|claude\",\"run\":true,\"check\":false,\"timeout_s\":1800,\"note\":\"optional\"}", always, cmd_assets_agent_sheets),
        cmd!("storyboard.init", "Create Storyboard Template", ["Storyboard"], None, "{}", always, cmd_storyboard_init),
        query!("storyboard.design", "Inspect storyboard placements", "{}", crate::storyboard_design::inspect),
        cmd!("storyboard.place_asset", "Place an asset on a storyboard page", ["Storyboard"], None, "{\"asset\":\"id\",\"page\":1,\"x\":100,\"y\":100,\"width\":160,\"placement\":\"optional existing placement id\"}", always, crate::storyboard_design::place),
        cmd!("storyboard.remove_placement", "Remove a draft image placement", ["Storyboard"], None, "{\"placement\":\"placement id\"}", always, crate::storyboard_design::remove),
        cmd!("storyboard.save_version", "Save storyboard design as a new version", ["Storyboard"], None, "{\"draft\":\"optional reviewed draft manifest.json\"}", always, crate::storyboard_design::save),
        query!("storyboard.toc", "Storyboard Chapters", "{}", cmd_storyboard_toc),
        query!("storyboard.doctor", "Diagnose source section spillover (read-only, explicit only)", "{}", crate::storyboard::cmd_storyboard_doctor),
        query!("storyboard.read", "Read Scene", "{\"scene\": 1}", cmd_storyboard_read),
        query!("storyboard.search", "Search Storyboard", "{\"query\": \"text\"}", cmd_storyboard_search),
        cmd!("storyboard.edit", "Edit Scene", ["Storyboard"], None,
            "{\"scene\": 1, \"field\": \"native: action|continuity|image_prompt|animation_prompt|animated_text|drawing|lyrics (see storyboard.read text_fields); legacy: lyric|prompt|panel|note|status|time|reference|skills|reference_prompt|layer_stack|build_steps|acceptance_checks|plan_vs_actual\", \"line\": 1, \"value\": \"text, image path, start-end or status\"}", always, cmd_storyboard_edit),
        cmd!("storyboard.import_openmontage", "Import Scenes from an OpenMontage Project", ["Storyboard"], None,
            "{\"path\": \"existing project folder\", \"overwrite\": false}", always, cmd_storyboard_import_openmontage),
        cmd!("storyboard.approve", "Approve Scene", ["Storyboard"], None, "{\"scene\": 1, \"approved\": true}", always, cmd_storyboard_approve),
        cmd!("workshop.set_audio", "Set Song Audio", ["Workshop"], None,
            "{\"path\": \"song audio file (referenced, not copied)\"}", always, cmd_workshop_set_audio),
        cmd!("video.place_track", "Place a rendered track in HyperFrames", ["Video"], None, "{\"id\":\"clip id\",\"path\":\"video file\",\"start\":0,\"duration\":2,\"rect\":[0,0,300,500],\"scene\":14}", always, crate::video_target::cmd_video_place_track),
        cmd!("workshop.set_target", "Set Video Target", ["Workshop"], None,
            "{\"target\": \"remotion|filmcraft|hyperframes\"}", always, cmd_workshop_set_target),
        cmd!("workshop.add", "Add Liner Notes / Panels / Assets", ["Workshop"], None,
            "{\"kind\": \"liner_notes|panels|assets\", \"paths\": [\"file\"]}", always, cmd_workshop_add),
        cmd!("workshop.split_panels", "Split Storyboard Grid (PhotoCraft)", ["Workshop"], None,
            "{\"image\": \"storyboard grid\", \"columns\": 3, \"rows\": 2, \"inset\": 8, \"prompts\": \"optional prompt.md with a numbered panel list\"}", always, cmd_workshop_split_panels),
        cmd!("workshop.import_transcript", "Import Transcript", ["Workshop"], None,
            "{\"path\": \"transcript.json\", \"captions\": \"optional original SRT; without it use provisional timed-word fallback\", \"audiomap\": \"optional existing audiomap.json\", \"replace\": false, \"filmcraft_project\": \"optional .fcproj\"}", always, cmd_workshop_import_transcript),
        cmd!("workshop.build_storyboard", "Build Storyboard PDF", ["Workshop"], None,
            "{\"template\":\"pdf|backlot\",\"source\":\"optional OpenMontage project folder\",\"draft\":false,\"resume\":\"optional existing draft manifest.json\",\"transcript\":\"optional exact native source; requires draft=true\",\"allow_timing_overrun\":false,\"panels\":\"optional scene target for four chorus phrases\"}", always, cmd_workshop_build_storyboard),
        cmd!("workshop.approve_draft", "Approve Storyboard", ["Workshop"], None,
            "{\"approved\": true}", always, cmd_workshop_approve_draft),
        cmd!("workshop.remove", "Remove Workshop File", ["Workshop"], None,
            "{\"path\": \"file in the project folder (moved to trash), or the song audio (unlinked only)\"}", always, cmd_workshop_remove),
        cmd!("video.init", "Initialize Target Video Project", ["Workshop"], None,
            "{\"target\": \"optional remotion|filmcraft|hyperframes\", \"hyperframes_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH, no npx install\", \"replace_scaffold\": \"optional false; explicitly rebuild initial scaffold from linked song/timing with backups; existing artwork/captions preserved\", \"captions\": true, \"assemble\": false, \"audiomap\": \"existing music-to-video audiomap for derived plan/completed frames; no analysis rerun\"}", always, cmd_video_init),
        cmd!("video.server_start", "Start Video Editor Web Server", ["Workshop"], None,
            "{\"target\": \"optional remotion|hyperframes\", \"port\": 3057, \"hyperframes_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}", always, cmd_video_server_start),
        cmd!("video.server_stop", "Stop Video Editor Web Server", ["Workshop"], None,
            "{}", always, cmd_video_server_stop),
        cmd!("video.place_references", "Place Storyboard Panels in FilmCraft", ["Workshop"], None,
            "{\"project\": \"optional .fcproj, default the song's\", \"images\": [\"optional, default the storyboard panels\"], \"track\": \"V1\"}", always, cmd_video_place_references),
        cmd!(
            "audio.analyze",
            "Analyze Song (HyperFrames)",
            ["File"],
            None,
            "{\"audio\": \"optional, default: the project's song\", \"overwrite\": false, \"analyze_beats\": true, \"hyperframes_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH; no npx install\"}",
            always,
            cmd_audio_analyze
        ),
        cmd!(
            "audio.transcribe",
            "Transcribe Song (FilmCraft)",
            ["File"],
            None,
            "{\"audio\": \"path\", \"out_dir\": \"optional, default: the open project's folder\", \"name\": \"optional\", \"model\": \"whisper-small\", \"language\": \"en\", \"diarize\": false, \"overwrite\": false}",
            always,
            cmd_audio_transcribe
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
            "{}",
            has_asset,
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
            "{\"part\": \"name\", \"visible\": true, \"solo\": false}",
            has_asset,
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
            "app.doctor",
            "Doctor: Lingering Processes, Cleanup & Ponytail Smells",
            "{\"clean\": false, \"keep_port\": 0}",
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
            .execute("project.new", json!({"path": "x.acproj", "overwrite": "yes"}))
            .unwrap_err();
        assert!(matches!(err, EngineError::InvalidParams(_)));
    }
}
