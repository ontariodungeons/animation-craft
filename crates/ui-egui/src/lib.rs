//! AnimationCraft egui frontend.
//!
//! Thin by design: all project changes go through `animationcraft_engine::Session::execute`;
//! this crate owns only presentation state and command dispatch. Swap it for another toolkit
//! without touching the engine.

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

pub mod automation;
mod bridge;
mod menus;
mod face_preview;
mod preview_export;
mod control_preview;
mod control_params;
mod panels;

use animationcraft_engine::Session;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct ControlRequest {
    pub id: String,
    pub params: Value,
    pub reply: Sender<Result<Value, String>>,
}

struct BackgroundJobResult {
    id: String,
    session: Session,
    result: Result<Value, String>,
}

/// The egui application wrapper that eframe runs.
pub struct AnimationCraftApp {
    session: Session,
    ui_state: UiState,
    textures: HashMap<String, egui::TextureHandle>,
    face_preview: face_preview::FacePreview,
    control_preview: control_preview::ControlPreview,
    pub automation: automation::Registry,
    pub(crate) control_rx: Option<Receiver<ControlRequest>>,
    pub(crate) control_port: Option<u16>,
    pub(crate) repaint_ctx: Arc<Mutex<Option<egui::Context>>>,
    pub(crate) pending_screenshot: Option<(String, Sender<Result<Value, String>>, Instant)>,
    pub(crate) pending_vision: Option<(Sender<Result<Value, String>>, Instant)>,
    pub(crate) pending_clicks: Vec<egui::Pos2>,
    job_tx: Sender<BackgroundJobResult>,
    job_rx: Receiver<BackgroundJobResult>,
}

impl Default for AnimationCraftApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Presentation-only UI state. All project/asset mutations go through the engine session.
#[derive(Debug, Clone)]
pub struct UiState {
    pub connect_asset_open: bool,
    pub connect_asset_path: String,
    pub connect_asset_name: String,
    pub connect_asset_kind: String,
    pub rename_asset: Option<(String, String)>,
    pub image_path: Option<String>,
    pub demo_preset: Option<DemoPreset>,
    /// Home screen "Recent" list (newest first), kept in the user's config folder.
    pub recent_projects: Vec<String>,
    /// Where Recent is kept; `None` (tests, headless use) keeps it in memory only.
    pub recent_file: Option<std::path::PathBuf>,
    /// Show the Workshop checklist even when it is complete (File ▸ Workshop).
    pub show_workshop: bool,
    /// Active stage tab in the top header bar (1 = Song, 2 = Transcribe, 3 = Storyboard, 4 = Animate Assets).
    pub active_stage: usize,
    /// Sub-layout inside Stage 4 (0 = Split Canvas + Inspector, 1 = Canvas Only, 2 = Parts & Sheets, 3 = Motion & Export).
    pub stage4_drawer: usize,
    /// When true, renders the classic full-window 3-column Standalone Animation Studio even when no `.acproj` is open (or when expanded from Stage 4).
    pub standalone_studio: bool,
    /// Last `workshop.status`, refreshed about once a second.
    pub workshop_status: Option<(Instant, Value)>,
    /// Last `workshop.files`, refreshed with the status.
    pub workshop_files: Vec<Value>,
    /// A file waiting for the user to confirm its removal.
    pub workshop_remove: Option<String>,
    /// The Workshop's Storyboard pane: shown, current page, and its last `workshop.storyboard`.
    pub storyboard_open: bool,
    pub storyboard_page: usize,
    pub storyboard_info: Option<Value>,
    pub storyboard_checked: Option<Instant>,
    pub new_subject_open: bool,
    pub new_subject_path: String,
    pub new_subject_name: String,
    pub new_subject_target_id: Option<String>,
    pub new_project_open: bool,
    pub new_project_name: String,
    pub new_project_audio: String,
    pub new_project_root: String,
    /// The project the recent list last recorded, to notice opens from any caller.
    pub recorded_project: Option<std::path::PathBuf>,
    pub archetype: String,
    pub q1_motion: String,
    pub q2_parts: String,
    pub strip_costume: bool,
    pub q3_target: String,
    pub bg_tolerance: u32,
    pub use_ocr: bool,
    pub recon_mode: String,
    pub inflation_depth_pct: u32,
    pub relief_detail_pct: u32,
    pub mesh_res: u32,
    pub show_topology_numbers: bool,
    pub stage_offset_x: f32,
    pub stage_offset_y: f32,
    pub center_tab: CenterTab,
    pub shading_mode: ShadingMode,
    pub show_ref_planes: bool,
    pub light_backdrop: bool,
    pub show_pivot_markers: bool,
    pub wasd_roam: bool,
    pub sprint_lock: bool,
    pub anim_mode: AnimMode,
    pub universal_verb: String,
    pub anim_speed: f32,
    pub anim_energy: f32,
    pub pose_range_pct: f32,
    pub jump_height_pct: f32,
    pub preview_zoom: f32,
    pub preview_actual_size: bool,
    pub yaw_deg: f32,
    pub explode_pct: f32,
    pub bump_strength: f32,
    pub proj_blend_pct: f32,
    pub playing: bool,
    pub playback_phase: f64,
    pub auto_turntable: bool,
    pub selected_pose_idx: usize,
    pub mouth_viseme: String,
    pub eye_blink: f32,
    pub eye_gaze: f32,
    pub knee_bend: f32,
    pub elbow_bend: f32,
    pub ear_splay: f32,
    pub ear_pitch: f32,
    pub head_yaw: f32,
    pub head_pitch: f32,
    pub smile_open: f32,
    pub arm_spread: f32,
    pub outliner_collections_mode: bool,
    pub show_qa_status: bool,
    pub busy_command: Option<String>,
    pub last_export: Option<String>,
    pub last_motion_validation: Option<Value>,
    pub selected_parts: std::collections::BTreeSet<String>,
    pub context_menu_part: Option<String>,
    pub selected_track_idx: usize,
    pub status_line: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            connect_asset_open: false,
            connect_asset_path: String::new(),
            connect_asset_name: String::new(),
            connect_asset_kind: "auto".into(),
            rename_asset: None,
            image_path: None,
            demo_preset: None,
            recent_projects: Vec::new(),
            recent_file: None,
            show_workshop: true,
            active_stage: 1,
            stage4_drawer: 0,
            standalone_studio: false,
            workshop_status: None,
            workshop_files: Vec::new(),
            workshop_remove: None,
            storyboard_open: true,
            storyboard_page: 0,
            storyboard_info: None,
            storyboard_checked: None,
            new_subject_open: false,
            new_subject_path: String::new(),
            new_subject_name: String::new(),
            new_subject_target_id: None,
            new_project_open: false,
            new_project_name: String::new(),
            new_project_audio: String::new(),
            new_project_root: String::new(),
            recorded_project: None,
            archetype: "humanoid_biped".to_string(),
            q1_motion: "walk_jump_talk".to_string(),
            q2_parts: String::new(),
            strip_costume: false,
            q3_target: "filmcraft_mp4".to_string(),
            bg_tolerance: 25,
            use_ocr: true,
            recon_mode: "volumetric".to_string(),
            inflation_depth_pct: 58,
            relief_detail_pct: 44,
            mesh_res: 32,
            show_topology_numbers: false,
            stage_offset_x: 0.0,
            stage_offset_y: 0.0,
            center_tab: CenterTab::OriginalView,
            shading_mode: ShadingMode::FeltPbr,
            show_ref_planes: false,
            light_backdrop: false,
            show_pivot_markers: false,
            wasd_roam: false,
            sprint_lock: false,
            anim_mode: AnimMode::Walk,
            universal_verb: "move_traverse".to_string(),
            anim_speed: 1.0,
            anim_energy: 100.0,
            pose_range_pct: 100.0,
            jump_height_pct: 30.0,
            preview_zoom: 1.0,
            preview_actual_size: false,
            yaw_deg: 45.0,
            explode_pct: 0.0,
            bump_strength: 35.0,
            proj_blend_pct: 65.0,
            playing: true,
            playback_phase: 0.0,
            auto_turntable: false,
            selected_pose_idx: 0,
            mouth_viseme: "smile".to_string(),
            eye_blink: 0.0,
            eye_gaze: 0.0,
            knee_bend: 0.0,
            elbow_bend: 0.0,
            ear_splay: 0.0,
            ear_pitch: 0.0,
            head_yaw: 0.0,
            head_pitch: 0.0,
            smile_open: 20.0,
            arm_spread: 0.0,
            outliner_collections_mode: false,
            show_qa_status: true,
            busy_command: None,
            last_export: None,
            last_motion_validation: None,
            selected_parts: std::collections::BTreeSet::new(),
            context_menu_part: None,
            selected_track_idx: 0,
            status_line: "Ready".to_string(),
        }
    }
}

impl UiState {
    pub fn reset_pose_sliders(&mut self) {
        self.eye_blink = 0.0;
        self.eye_gaze = 0.0;
        self.knee_bend = 0.0;
        self.elbow_bend = 0.0;
        self.ear_splay = 0.0;
        self.ear_pitch = 0.0;
        self.head_yaw = 0.0;
        self.head_pitch = 0.0;
        self.smile_open = 20.0;
        self.arm_spread = 0.0;
        self.explode_pct = 0.0;
        self.stage_offset_x = 0.0;
        self.stage_offset_y = 0.0;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CenterTab {
    #[default]
    OriginalView,
    AssetSheet,
    TurnaroundSheet,
    TwoD,
    ThreeD,
    FivePoseLineup,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ShadingMode {
    #[default]
    FeltPbr,
    ClaySculpt,
    Wireframe,
    Normals,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AnimMode {
    #[default]
    Idle,
    Walk,
    Run,
    Jump,
    Wave,
    LookScout,
    Static,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DemoPreset {
    #[default]
    Hopper,
    Whiskers,
    Puddles,
    Jenny,
    Rusty,
    MissHayley,
    OldMac,
    Scout,
    MrPuddles,
    Sam,
    Maisy,
    Sun,
    CoralFish,
}

impl DemoPreset {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Hopper => "hopper",
            Self::Whiskers => "whiskers",
            Self::Puddles => "puddles",
            Self::Jenny | Self::Sun => "jenny",
            Self::Rusty | Self::CoralFish => "rusty",
            Self::MissHayley => "miss_hayley",
            Self::OldMac => "old_mac",
            Self::Scout => "scout",
            Self::MrPuddles => "mr_puddles",
            Self::Sam => "sam",
            Self::Maisy => "maisy",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hopper => "Hopper (Rabbit)",
            Self::Whiskers => "Whiskers (Tabby)",
            Self::Puddles => "Puddles (Duckling)",
            Self::Jenny => "Jenny (Chick)",
            Self::Rusty => "Rusty (Foal)",
            Self::MissHayley => "Miss Hayley (Teacher)",
            Self::OldMac => "Old Mac (Principal)",
            Self::Scout => "Scout (Puppy)",
            Self::MrPuddles => "Mr Puddles (Goose)",
            Self::Sam => "Sam (Piglet)",
            Self::Maisy => "Maisy (Calf)",
            Self::Sun => "Smiling Sun",
            Self::CoralFish => "Coral Fish",
        }
    }
}

impl AnimationCraftApp {
    /// Create a new app with no project open; the Home screen offers New and Open.
    pub fn new() -> Self {
        let (job_tx, job_rx) = mpsc::channel();
        let app = Self {
            session: Session::default(),
            ui_state: UiState::default(),
            textures: HashMap::new(),
            face_preview: face_preview::FacePreview::default(),
            control_preview: control_preview::ControlPreview::default(),
            automation: automation::Registry::default(),
            control_rx: None,
            control_port: None,
            repaint_ctx: Arc::new(Mutex::new(None)),
            pending_screenshot: None,
            pending_vision: None,
            pending_clicks: Vec::new(),
            job_tx,
            job_rx,
        };
        app
    }

    /// Keep the Home screen's Recent list in the user's config folder (the desktop app).
    pub fn use_recent_projects_file(&mut self) {
        if let Some(file) = panels::start::user_recent_file() {
            self.ui_state.recent_projects = panels::start::load_recent(&file);
            self.ui_state.recent_file = Some(file);
        }
    }

    /// Display source/prepared pixels as authored. Matting belongs to explicit pipeline commands.
    pub fn load_texture(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        let stamp = std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()));
        let cache_key = format!("{path}#{stamp:?}");
        if let Some(tex) = self.textures.get(&cache_key) {
            return Some(tex.clone());
        }
        // Show the file as it is; background removal belongs to the PhotoCraft import step.
        let img = image::open(path).ok()?.into_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        let handle = ctx.load_texture(&cache_key, color_image, egui::TextureOptions::LINEAR);
        self.textures.insert(cache_key, handle.clone());
        Some(handle)
    }

    /// Dispatch a command synchronously (used by TCP `--control` bridge and fast in-memory mutations).
    pub fn dispatch(&mut self, id: &str, params: Value) -> Result<Value, String> {
        preview_export::validate_params(id, &params)?;
        match id {
            "ui.inspect" | "ui.elements" | "ui.audit" => {
                let filter = params.get("filter").and_then(Value::as_str);
                let elements = self.automation.query(filter);
                let (win_w, win_h) = self
                    .automation
                    .find("panel.header")
                    .map(|h| (h.rect[0] + h.rect[2], 1400.0_f32))
                    .unwrap_or((1600.0, 980.0));
                let layout_audit = self.automation.layout_audit(win_w, win_h);
                if id == "ui.audit" {
                    return Ok(layout_audit);
                }
                return Ok(json!({
                    "active_asset": self.session.active_asset,
                    "controls": control_params::snapshot(&self.ui_state),
                    "project_assets": self.session.project_assets,
                    "tab": format!("{:?}", self.ui_state.center_tab),
                    "anim_mode": format!("{:?}", self.ui_state.anim_mode),
                    "layout_audit": layout_audit,
                    "element_count": elements.len(),
                    "elements": elements,
                }));
            }
            "ui.click" => {
                if let Some(elem_id) = params.get("id").and_then(Value::as_str) {
                    let elem = self
                        .automation
                        .find(elem_id)
                        .cloned()
                        .ok_or_else(|| format!("automation element `{elem_id}` not found"))?;
                    let cx = elem.rect[0] + elem.rect[2] * 0.5;
                    let cy = elem.rect[1] + elem.rect[3] * 0.5;
                    self.pending_clicks.push(egui::pos2(cx, cy));
                    return Ok(json!({
                        "clicked": elem.id,
                        "label": elem.label,
                        "center": [cx, cy],
                        "rect": elem.rect,
                    }));
                }
                let x = params
                    .get("x")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| "ui.click requires `id` or `x` and `y`".to_string())?
                    as f32;
                let y = params
                    .get("y")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| "ui.click requires `id` or `x` and `y`".to_string())?
                    as f32;
                self.pending_clicks.push(egui::pos2(x, y));
                return Ok(json!({
                    "clicked": "coordinates",
                    "center": [x, y],
                }));
            }
            "ui.state" => {
                let asset = self.session.current_asset();
                let pose_labels: Vec<String> = asset
                    .and_then(|a| a.pose_manifest.as_ref())
                    .map(|m| m.frames.iter().map(|f| f.label.clone()).collect())
                    .unwrap_or_default();
                let foot_lift_max_px = asset
                    .and_then(|a| a.motion_range.as_ref())
                    .map(|m| m.foot_lift_max_px);
                let max_vertical_drift_px = asset
                    .and_then(|a| a.turnaround_registration.as_ref())
                    .map(|r| r.max_vertical_drift_px);
                let attached_asset_sheet = asset
                    .and_then(|a| a.attached_asset_sheet.as_ref())
                    .map(|p| p.to_string_lossy().into_owned());
                let attached_turnaround_sheet = asset
                    .and_then(|a| a.attached_turnaround_sheet.as_ref())
                    .map(|p| p.to_string_lossy().into_owned());
                let cutout_part_count = asset
                    .and_then(|a| a.cutout_rig.as_ref())
                    .map(|r| r.parts.len())
                    .unwrap_or(0);
                return Ok(json!({
                    "project_assets": self.session.project_assets,
                    "active_stage": self.ui_state.active_stage,
                    "standalone_studio": self.ui_state.standalone_studio,
                    "stage4_drawer": self.ui_state.stage4_drawer,
                    "show_workshop": self.ui_state.show_workshop,
                    "tab": match self.ui_state.center_tab {
                        CenterTab::OriginalView => "original",
                        CenterTab::AssetSheet => "asset_sheet",
                        CenterTab::TurnaroundSheet => "turnaround_sheet",
                        CenterTab::TwoD => "2d",
                        CenterTab::ThreeD => "3d",
                        CenterTab::FivePoseLineup => "lineup",
                    },
                    "active_asset": self.session.active_asset,
                    "controls": control_params::snapshot(&self.ui_state),
                    "attached_asset_sheet": attached_asset_sheet,
                    "attached_turnaround_sheet": attached_turnaround_sheet,
                    "cutout_part_count": cutout_part_count,
                    "pose_labels": pose_labels,
                    "foot_lift_max_px": foot_lift_max_px,
                    "max_vertical_drift_px": max_vertical_drift_px,
                    "anim_mode": format!("{:?}", self.ui_state.anim_mode),
                    "pose_range_pct": self.ui_state.pose_range_pct,
                    "yaw_deg": self.ui_state.yaw_deg,
                    "playing": self.ui_state.playing,
                    "playback_phase": self.ui_state.playback_phase,
                    "jump_height_pct": self.ui_state.jump_height_pct,
                    "mouth_expression": self.ui_state.mouth_viseme,
                    "facial_preview_status": self.face_preview.status,
                    "busy_command": self.ui_state.busy_command,
                    "image_path": self.ui_state.image_path,
                    "last_export": self.ui_state.last_export,
                    "control_port": self.control_port,
                    "registered_ui_elements": self.automation.elements.len(),
                }));
            }
            "ui.set_view" => {
                self.ui_state = control_params::updated(&self.ui_state, &params)?;
                if let Some(tab) = params.get("tab").and_then(Value::as_str) {
                    self.ui_state.center_tab = if tab.eq_ignore_ascii_case("original")
                        || tab.eq_ignore_ascii_case("original_view")
                    {
                        CenterTab::OriginalView
                    } else if tab.eq_ignore_ascii_case("asset_sheet")
                        || tab.eq_ignore_ascii_case("pose_sheet")
                        || tab.eq_ignore_ascii_case("asset")
                    {
                        CenterTab::AssetSheet
                    } else if tab.eq_ignore_ascii_case("turnaround_sheet")
                        || tab.eq_ignore_ascii_case("turnaround")
                    {
                        CenterTab::TurnaroundSheet
                    } else if tab.eq_ignore_ascii_case("3d") {
                        CenterTab::ThreeD
                    } else if tab.eq_ignore_ascii_case("lineup") {
                        CenterTab::FivePoseLineup
                    } else {
                        CenterTab::TwoD
                    };
                }
                if let Some(mode) = params.get("mode").and_then(Value::as_str) {
                    self.apply_mode_str(mode);
                }
                if let Some( preset ) = params.get("preset").and_then(Value::as_str) {
                    self.dispatch("asset.load_manifest", json!({ "preset": preset }))?;
                }
                if let Some(track_val) = params.get("track") {
                    if let Some(idx) = track_val.as_u64() {
                        self.ui_state.selected_track_idx = idx as usize;
                        self.ui_state.playback_phase = 0.0;
                    } else if let Some(tid) = track_val.as_str() {
                        let found_idx = self
                            .session
                            .current_asset()
                            .and_then(|a| a.cutout_rig.as_ref())
                            .and_then(|r| r.effectcraft.get("tracks"))
                            .and_then(Value::as_array)
                            .and_then(|arr| {
                                arr.iter().position(|t| {
                                    t.get("id").and_then(Value::as_str) == Some(tid)
                                })
                            });
                        if let Some(idx) = found_idx {
                            self.ui_state.selected_track_idx = idx;
                            self.ui_state.playback_phase = 0.0;
                        }
                    }
                }
                if let Some(t_sec) = params.get("phase_sec").and_then(Value::as_f64) {
                    self.ui_state.playback_phase = t_sec.max(0.0) * 3.2;
                }
                if let Some(hide_id) = params.get("hide_part").and_then(Value::as_str)
                    && let Some(asset) = self.session.current_asset_mut()
                {
                    for st in &mut asset.parts {
                        if st.name == hide_id {
                            st.visible = false;
                        }
                    }
                    self.ui_state.selected_parts.remove(hide_id);
                }
                if let Some(show_id) = params.get("show_part").and_then(Value::as_str)
                    && let Some(asset) = self.session.current_asset_mut()
                {
                    for st in &mut asset.parts {
                        if st.name == show_id {
                            st.visible = true;
                        }
                    }
                }
                if params
                    .get("restore_parts")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && let Some(asset) = self.session.current_asset_mut()
                {
                    for st in &mut asset.parts {
                        st.visible = true;
                        st.solo = false;
                    }
                }
                if let Some(stage_num) = params.get("stage").and_then(Value::as_u64) {
                    let s_idx = (stage_num as usize).clamp(1, 4);
                    self.ui_state.active_stage = s_idx;
                    self.ui_state.show_workshop = s_idx < 4;
                    self.ui_state.standalone_studio = false;
                }
                if let Some(standalone) = params.get("standalone").and_then(Value::as_bool) {
                    self.ui_state.standalone_studio = standalone;
                    if !standalone && self.session.project_path.is_some() && self.ui_state.active_stage == 0 {
                        self.ui_state.active_stage = 4;
                    }
                }
                if let Some(drawer) = params.get("drawer") {
                    if let Some(d_idx) = drawer.as_u64() {
                        self.ui_state.stage4_drawer = (d_idx as usize).min(3);
                    } else if let Some(d_str) = drawer.as_str() {
                        self.ui_state.stage4_drawer = match d_str.to_ascii_lowercase().as_str() {
                            "canvas" | "canvas_only" => 1,
                            "parts" | "sheets" | "parts_sheets" => 2,
                            "motion" | "export" | "motion_export" => 3,
                            _ => 0,
                        };
                    }
                }
                self.ui_state.status_line = format!(
                    "CLI updated view: stage={}, standalone={}, asset={:?}, tab={:?}, mode={:?}",
                    self.ui_state.active_stage,
                    self.ui_state.standalone_studio,
                    self.session.active_asset,
                    self.ui_state.center_tab,
                    self.ui_state.anim_mode,
                );
                return Ok(json!({
                    "updated": true,
                    "active_stage": self.ui_state.active_stage,
                    "standalone_studio": self.ui_state.standalone_studio,
                    "stage4_drawer": self.ui_state.stage4_drawer,
                    "active_asset": self.session.active_asset,
                    "controls": control_params::snapshot(&self.ui_state),
                    "tab": format!("{:?}", self.ui_state.center_tab),
                    "anim_mode": format!("{:?}", self.ui_state.anim_mode),
                    "selected_pose_idx": self.ui_state.selected_pose_idx,
                    "selected_track_idx": self.ui_state.selected_track_idx,
                    "playback_phase": self.ui_state.playback_phase,
                    "yaw_deg": self.ui_state.yaw_deg,
                    "pose_range_pct": self.ui_state.pose_range_pct,
                }));
            }
            _ => {}
        }

        if self.ui_state.busy_command.is_some()
            && animationcraft_engine::find_command(id).is_some_and(|c| c.journal)
        {
            return Err("Wait for the background command before changing project assets".into());
        }
        self.sync_ui_before_command(id, &params);
        let res = if preview_export::uses_preview(&self.session, id, &params) {
            let textures = self.export_control_textures()?;
            preview_export::export(&mut self.session, &self.ui_state, &params, &textures, id == "export.cutout_mp4")
        } else {
            self.session.execute(id, params).map_err(|e| e.to_string())
        };
        self.sync_ui_after_command(id, &res);
        res
    }

    fn sync_ui_before_command(&mut self, id: &str, params: &Value) {
        if id == "anim.preview_2d" {
            if let Some(mode) = params.get("mode").and_then(Value::as_str) {
                self.apply_mode_str(mode);
            }
            if let Some(range) = params.get("pose_range_pct").and_then(Value::as_f64) {
                self.ui_state.pose_range_pct = (range as f32).clamp(0.0, 100.0);
            }
        }
        if id == "intake.validate"
            && let Some(img) = params.get("image").and_then(Value::as_str)
        {
            self.ui_state.image_path = Some(img.to_string());
        }
    }

    fn sync_ui_after_command(&mut self, id: &str, res: &Result<Value, String>) {
        match res {
            Ok(val) => {
                self.ui_state.image_path = self
                    .session
                    .current_asset()
                    .map(|asset| asset.path.to_string_lossy().into_owned());
                if matches!(
                    id,
                    "project.open"
                        | "project.new"
                        | "project.close"
                        | "project.add_asset"
                        | "project.remove_asset"
                ) || id.starts_with("workshop.")
                    || id.starts_with("video.")
                    || id.starts_with("storyboard.")
                {
                    self.ui_state.workshop_status = None;
                    self.ui_state.storyboard_info = None;
                    self.ui_state.storyboard_checked = None;
                    if matches!(id, "project.open" | "project.new" | "project.close") {
                        self.ui_state.storyboard_page = 0;
                        self.ui_state.new_subject_open = false;
                        self.ui_state.workshop_remove = None;
                        self.ui_state.active_stage = 1;
                        self.ui_state.show_workshop = true;
                        self.ui_state.standalone_studio = false;
                        self.textures.clear();
                    }
                }
                if matches!(
                    id,
                    "project.add_asset"
                        | "project.load_asset"
                        | "asset.make_transparent"
                        | "asset.build_cutout_rig"
                        | "asset.load_manifest"
                        | "asset.cut_sheet"
                        | "intake.validate"
                ) {
                    self.ui_state.workshop_status = None;
                }
                if id == "export.cutout_mp4" {
                    self.ui_state.last_export =
                        val.get("output").and_then(Value::as_str).map(str::to_owned);
                }
                if id == "export.layered_motion" {
                    if let Some(dir) = val.get("out_dir").and_then(Value::as_str) {
                        self.ui_state.last_export = Some(dir.to_owned());
                    }
                    self.ui_state.last_motion_validation = Some(val.clone());
                }
                if id == "validate.motion" {
                    self.ui_state.last_motion_validation = Some(val.clone());
                }
                if id == "intake.validate" {
                    let slot = val
                        .get("slot")
                        .and_then(Value::as_str)
                        .unwrap_or("original");
                    let has_turnaround = self.session.current_asset().is_some_and(|a| {
                        a.turnaround_manifest.is_some() || a.attached_turnaround_sheet.is_some()
                    });
                    let has_poses = self.session.current_asset().is_some_and(|a| {
                        a.pose_manifest.is_some() || a.attached_asset_sheet.is_some()
                    });
                    self.ui_state.center_tab = if slot == "turnaround_sheet" && has_turnaround {
                        CenterTab::TurnaroundSheet
                    } else if slot == "asset_sheet" && has_poses {
                        CenterTab::AssetSheet
                    } else {
                        CenterTab::OriginalView
                    };
                }
                if matches!(id, "project.load_asset" | "asset.load_manifest" | "intake.validate") {
                    self.ui_state.anim_mode = AnimMode::Idle;
                    self.ui_state.playback_phase = 0.0;
                    self.ui_state.stage_offset_x = 0.0;
                    self.ui_state.stage_offset_y = 0.0;
                    self.ui_state.selected_parts.clear();
                    self.ui_state.context_menu_part = None;
                    self.ui_state.selected_track_idx = 0;
                }
                self.ui_state.selected_pose_idx = 0;
                self.ui_state.status_line = format!(
                    "Executed `{id}` (active asset: {})",
                    self.session.active_asset.as_deref().unwrap_or("none")
                );
            }
            Err(err) => {
                self.ui_state.status_line = format!("Command `{id}`: {err}");
            }
        }
    }

    fn apply_mode_str(&mut self, mode: &str) {
        self.ui_state.anim_mode = match mode.to_ascii_lowercase().as_str() {
            "idle" | "standing" => AnimMode::Idle,
            "walk" | "walking" => AnimMode::Walk,
            "run" | "running" => AnimMode::Run,
            "jump" | "jumping" => AnimMode::Jump,
            "wave" | "waving" => AnimMode::Wave,
            "look" | "look_scout" | "explore" => AnimMode::LookScout,
            "static" => AnimMode::Static,
            _ => self.ui_state.anim_mode,
        };
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    pub fn ui_state(&self) -> &UiState {
        &self.ui_state
    }

    pub fn ui_state_mut(&mut self) -> &mut UiState {
        &mut self.ui_state
    }
    pub fn poll_background_jobs_for_test(&mut self) {
        while let Ok(done) = self.job_rx.try_recv() {
            // Merge worker session mutations without clobbering any live part-visibility or
            // preview settings that were adjusted while the background job ran.
            for (aid, worker_asset) in done.session.assets {
                if let Some(live_asset) = self.session.assets.get(&aid) {
                    let mut merged = worker_asset;
                    for p in &mut merged.parts {
                        if let Some(lp) = live_asset.parts.iter().find(|lp| lp.name == p.name) {
                            p.visible = lp.visible;
                            p.solo = lp.solo;
                            p.verified = lp.verified;
                        }
                    }
                    self.session.assets.insert(aid, merged);
                } else {
                    self.session.assets.insert(aid, worker_asset);
                }
            }
            self.session.project_assets = done.session.project_assets;
            self.session.active_asset = done.session.active_asset;
            self.session.draft_approved = done.session.draft_approved;
            self.session.last_result = done.session.last_result;
            self.ui_state.busy_command = None;
            self.sync_ui_after_command(&done.id, &done.result);
        }
    }

    pub fn render_workspace(&mut self, ui: &mut egui::Ui) {
        self.automation.begin_frame();

        let full = ui.max_rect();
        ui.painter()
            .rect_filled(full, 0.0, egui::Color32::from_rgb(16, 18, 24));

        let header_h = 96.0;
        let header_rect = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), header_h));
        ui.painter()
            .rect_filled(header_rect, 0.0, egui::Color32::from_rgb(24, 27, 36));
        {
            let inner = header_rect.shrink2(egui::vec2(8.0, 4.0));
            let mut top_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            top_ui.set_clip_rect(inner);
            crate::panels::header::header(self, &mut top_ui);
        }
        crate::panels::header::asset_dialogs(self, ui.ctx());
        crate::panels::workshop::new_project_dialog(self, ui.ctx());

        let body =
            egui::Rect::from_min_max(egui::pos2(full.min.x, header_rect.max.y + 1.0), full.max);
        let total_w = body.width().max(300.0);
        let left_w = (total_w * 0.25).clamp(135.0, 320.0);
        let right_w = (total_w * 0.26).clamp(140.0, 330.0);
        let left_x_max = (body.min.x + left_w).min(body.max.x - 100.0);
        let right_x_min = (body.max.x - right_w).max(left_x_max + 80.0);

        let left_rect = egui::Rect::from_min_max(body.min, egui::pos2(left_x_max, body.max.y));
        let center_rect = egui::Rect::from_min_max(
            egui::pos2(left_x_max + 1.0, body.min.y),
            egui::pos2(right_x_min - 1.0, body.max.y),
        );
        let right_rect = egui::Rect::from_min_max(egui::pos2(right_x_min, body.min.y), body.max);

        self.automation
            .add("panel.header", "panel", "Header Bar", header_rect);
        self.automation
            .add("panel.left", "panel", "Left Intake & Outliner", left_rect);
        self.automation.add(
            "panel.center",
            "panel",
            "Center Studio Viewport",
            center_rect,
        );
        self.automation
            .add("panel.right", "panel", "Right Motion & Export", right_rect);

        crate::panels::start::shortcuts(self, ui.ctx());
        crate::panels::start::record_open_project(self);
        if !crate::panels::start::project_open(self) {
            // Home uses the whole body: start card plus the workflow strip.
            let mut home = ui.new_child(egui::UiBuilder::new().max_rect(body));
            home.set_clip_rect(body);
            crate::panels::start::start_screen(self, &mut home);
            return;
        }
        if crate::panels::workshop::showing(self) {
            let mut workshop = ui.new_child(egui::UiBuilder::new().max_rect(body));
            workshop.set_clip_rect(body);
            crate::panels::workshop::workshop_screen(self, &mut workshop);
            return;
        }

        // Panel dividers
        let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 50, 64));
        ui.painter().line_segment(
            [
                egui::pos2(full.min.x, header_rect.max.y),
                egui::pos2(full.max.x, header_rect.max.y),
            ],
            stroke,
        );
        ui.painter().line_segment(
            [
                egui::pos2(left_x_max, body.min.y),
                egui::pos2(left_x_max, body.max.y),
            ],
            stroke,
        );
        ui.painter().line_segment(
            [
                egui::pos2(right_x_min, body.min.y),
                egui::pos2(right_x_min, body.max.y),
            ],
            stroke,
        );

        {
            let inner = left_rect.shrink(6.0);
            let mut left_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            left_ui.set_clip_rect(inner);
            crate::panels::left::left_panel(self, &mut left_ui);
        }

        {
            let inner = center_rect.shrink(6.0);
            let mut center_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            center_ui.set_clip_rect(inner);
            crate::panels::center::center_viewport(self, &mut center_ui);
        }

        {
            let inner = right_rect.shrink(6.0);
            let mut right_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            right_ui.set_clip_rect(inner);
            crate::panels::right::right_panel(self, &mut right_ui);
        }
    }
}

impl eframe::App for AnimationCraftApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Ok(mut guard) = self.repaint_ctx.lock()
            && guard.is_none()
        {
            *guard = Some(ctx.clone());
        }
        for pos in self.pending_clicks.drain(..) {
            input.events.push(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_background_jobs_for_test();
        self.expire_stale_screenshots(Duration::from_millis(1200));

        let dropped_paths: Vec<String> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().display().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        });
        for dropped in dropped_paths {
            let slot = match self.ui_state.center_tab {
                CenterTab::TurnaroundSheet | CenterTab::ThreeD | CenterTab::FivePoseLineup => {
                    "turnaround_sheet"
                }
                _ => "asset_sheet",
            };
            let tolerance = self.ui_state.bg_tolerance;
            let use_ocr = self.ui_state.use_ocr;
            self.ui_state.image_path = Some(dropped.clone());
            self.exec_ui_command(UiCommand::Engine {
                id: "intake.validate",
                params: json!({
                    "image": dropped,
                    "slot": slot,
                    "attach_to_active": true,
                    "auto_cut": true,
                    "tolerance": tolerance,
                    "use_ocr": use_ocr,
                }),
            });
        }

        self.handle_screenshot_events(ui.ctx());
        self.drain_live_control_requests(ui.ctx());
        self.render_workspace(ui);

        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
}

/// A UI-bound command that can be dispatched from the egui thread.
#[derive(Debug, Clone)]
pub enum UiCommand {
    Engine {
        id: &'static str,
        params: Value,
    },
}

impl UiCommand {
    /// Convert to engine command id + params.
    pub fn to_engine_call(&self) -> (&'static str, Value) {
        match self {
            UiCommand::Engine { id, params } => (id, params.clone()),
        }
    }

    fn is_long_running(&self) -> bool {
        let UiCommand::Engine { id, .. } = self;
        animationcraft_engine::find_command(id).is_some_and(|c| c.long_running)
    }
}

impl AnimationCraftApp {
    /// Execute a UI command. Long-running subprocess commands run on a background worker
    /// thread so the egui window and TCP control server never freeze.
    pub fn exec_ui_command(&mut self, cmd: UiCommand) {
        let (id, mut params) = cmd.to_engine_call();
        if let Err(error) = preview_export::validate_params(id, &params) {
            self.ui_state.status_line = error;
            return;
        }
        if id == "asset.cut_sheet" && params.get("source").is_none() {
            let slot = if self.ui_state.center_tab == CenterTab::TurnaroundSheet {
                "turnaround_sheet"
            } else {
                "asset_sheet"
            };
            let source = self
                .session
                .current_asset()
                .and_then(|a| panels::sheet_path(a, slot));
            let Some(source) = source else {
                self.ui_state.status_line = "Attach a sheet before slicing.".into();
                return;
            };
            params["source"] = serde_json::json!(source);
            let revision = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            params["out_dir"] = serde_json::json!(format!("exports/sheet_reviews/{revision}"));
        }
        if cmd.is_long_running() {
            if self.ui_state.busy_command.is_some() {
                self.ui_state.status_line =
                    "A background pipeline command is already running…".to_string();
                return;
            }
            let snapshot_export = preview_export::uses_preview(&self.session, id, &params);
            let export_textures = if snapshot_export {
                match self.export_control_textures() {
                    Ok(textures) => textures,
                    Err(error) => {
                        self.ui_state.status_line = error;
                        return;
                    }
                }
            } else {
                std::collections::BTreeMap::new()
            };
            let export_state = self.ui_state.clone();
            self.sync_ui_before_command(id, &params);
            self.ui_state.busy_command = Some(id.to_string());
            self.ui_state.status_line = format!("Running `{id}` in background…");
            let mut session_clone = self.session.clone();
            let tx = self.job_tx.clone();
            let id_owned = id.to_string();
            std::thread::spawn(move || {
                let result = if snapshot_export {
                    preview_export::export(
                        &mut session_clone,
                        &export_state,
                        &params,
                        &export_textures,
                        id_owned == "export.cutout_mp4",
                    )
                } else {
                    session_clone
                        .execute(&id_owned, params)
                        .map_err(|e| e.to_string())
                };
                let _ = tx.send(BackgroundJobResult {
                    id: id_owned,
                    session: session_clone,
                    result,
                });
            });
        } else {
            let _ = self.dispatch(id, params);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_control_tick_services_commands_and_times_out_screenshots_when_window_suspended() {
        let mut app = AnimationCraftApp::new();
        // The workspace tabs show once an asset is connected (Home shows before that). Without a
        // project file there is no song workshop to complete first.
        let project = std::env::temp_dir().join(format!("ac_headless_{}.png", std::process::id()));
        image::RgbaImage::new(8, 8).save(&project).unwrap();
        app.dispatch("project.add_asset", json!({"path": project})).unwrap();
        let ctx = egui::Context::default();
        let (tx, rx) = mpsc::channel();
        app.control_rx = Some(rx);

        // 1. ui.state works without any active eframe::App::ui calls
        let (reply_tx, reply_rx) = mpsc::channel();
        tx.send(ControlRequest {
            id: "ui.state".into(),
            params: json!({}),
            reply: reply_tx,
        })
        .unwrap();
        app.service_headless_control_tick(&ctx, Duration::from_secs(1));
        let state_res = reply_rx
            .recv_timeout(Duration::from_millis(500))
            .unwrap()
            .unwrap();
        assert!(state_res["tab"].is_string());

        // 2. ui.inspect works via headless layout frame
        let (reply_tx2, reply_rx2) = mpsc::channel();
        tx.send(ControlRequest {
            id: "ui.inspect".into(),
            params: json!({ "filter": "tab." }),
            reply: reply_tx2,
        })
        .unwrap();
        app.service_headless_control_tick(&ctx, Duration::from_secs(1));
        let inspect_res = reply_rx2
            .recv_timeout(Duration::from_millis(500))
            .unwrap()
            .unwrap();
        assert!(inspect_res["element_count"].is_u64());

        // 3. ui.click works via headless layout frame
        let (reply_tx3, reply_rx3) = mpsc::channel();
        tx.send(ControlRequest {
            id: "ui.click".into(),
            params: json!({ "id": "tab.asset_sheet" }),
            reply: reply_tx3,
        })
        .unwrap();
        app.service_headless_control_tick(&ctx, Duration::from_secs(1));
        let click_res = reply_rx3
            .recv_timeout(Duration::from_millis(500))
            .unwrap()
            .unwrap();
        assert_eq!(click_res["clicked"], "tab.asset_sheet");
        assert_eq!(app.ui_state.center_tab, CenterTab::AssetSheet);

        // 4. ui.screenshot fails cleanly when window is hidden/suspended instead of hanging forever
        let (reply_tx4, reply_rx4) = mpsc::channel();
        app.pending_screenshot = Some((
            "test.png".into(),
            reply_tx4,
            Instant::now() - Duration::from_secs(3),
        ));
        app.expire_stale_screenshots(Duration::from_secs(1));
        let screenshot_res = reply_rx4.recv_timeout(Duration::from_millis(500)).unwrap();
        assert!(screenshot_res.is_err());
        assert!(screenshot_res.unwrap_err().contains("timed out"));
        // 5. app.quit without a native window says so instead of returning success and
        //    leaving an unreachable process behind.
        let (reply_tx5, reply_rx5) = mpsc::channel();
        tx.send(ControlRequest {
            id: "app.quit".into(),
            params: json!({}),
            reply: reply_tx5,
        })
        .unwrap();
        app.service_headless_control_tick(&ctx, Duration::from_secs(1));
        let quit_res = reply_rx5.recv_timeout(Duration::from_millis(500)).unwrap();
        assert!(quit_res.unwrap_err().contains("needs the native window"));
        let _ = std::fs::remove_file(&project);
    }
}

#[cfg(test)]
mod texture_loading_tests {
    use super::*;
    #[test]
    fn opaque_layer_pixels_survive_loading_and_replaced_files_invalidate_cache() {
        let root = std::env::temp_dir().join(format!("animationcraft-texture-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("layer.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([80,120,160,255])).save(&path).unwrap();
        let ctx = egui::Context::default();
        let mut app = AnimationCraftApp::new();
        let mut handle = None;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            handle = app.load_texture(ui.ctx(), &path.to_string_lossy());
        });
        let first = handle.unwrap();
        let delta = out.textures_delta.set.get(&first.id()).unwrap().first().unwrap();
        let egui::ImageData::Color(pixels) = &delta.image;
        assert!(pixels.pixels.iter().all(|pixel| *pixel == egui::Color32::from_rgb(80,120,160)));
        out.textures_delta.clear();
        image::RgbaImage::from_pixel(9, 8, image::Rgba([180,20,60,255])).save(&path).unwrap();
        let replacement = app.load_texture(&ctx, &path.to_string_lossy()).unwrap();
        assert_ne!(first.id(), replacement.id());
        assert_eq!(replacement.size(), [9,8]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
