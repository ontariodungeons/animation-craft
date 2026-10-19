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
mod menus;
mod face_preview;
mod panels;

use animationcraft_engine::Session;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
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
    pub automation: automation::Registry,
    control_rx: Option<Receiver<ControlRequest>>,
    control_port: Option<u16>,
    repaint_ctx: Arc<Mutex<Option<egui::Context>>>,
    pending_screenshot: Option<(String, Sender<Result<Value, String>>, Instant)>,
    pending_vision: Option<(Sender<Result<Value, String>>, Instant)>,
    pending_clicks: Vec<egui::Pos2>,
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
            demo_preset: Some(DemoPreset::Hopper),
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
            status_line:
                "Hopper active — PhotoCraft transparent import + Attached Reference Sheets ready"
                    .to_string(),
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
    /// Create a new app and load the default Hopper manifest if present in the workspace.
    pub fn new() -> Self {
        let (job_tx, job_rx) = mpsc::channel();
        let mut app = Self {
            session: Session::default(),
            ui_state: UiState::default(),
            textures: HashMap::new(),
            face_preview: face_preview::FacePreview::default(),
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
        // Load Hopper's real manifests and measured telemetry if available on disk.
        let _ = app.dispatch("project.load_asset", json!({ "id": "hopper" }));
        app
    }

    /// Start a localhost JSON-lines TCP control server (`--control <port>`).
    pub fn start_control_server(&mut self, port: u16) {
        let (tx, rx) = mpsc::channel::<ControlRequest>();
        self.control_rx = Some(rx);
        self.control_port = Some(port);
        self.ui_state.status_line = format!(
            "Live CLI Bridge listening on 127.0.0.1:{port} (`animationcraft-cli --bridge {port} ...`)"
        );
        let repaint_ctx = Arc::clone(&self.repaint_ctx);

        std::thread::spawn(move || {
            let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else {
                return;
            };
            for stream in listener.incoming().flatten() {
                let tx_clone = tx.clone();
                let repaint_clone = Arc::clone(&repaint_ctx);
                std::thread::spawn(move || {
                    let Ok(peer) = stream.try_clone() else {
                        return;
                    };
                    let reader = BufReader::new(stream);
                    let mut writer = peer;
                    for line in reader.lines().map_while(Result::ok) {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let parsed: Value = match serde_json::from_str(trimmed) {
                            Ok(v) => v,
                            Err(e) => {
                                let err_line =
                                    json!({"ok": false, "error": e.to_string()}).to_string();
                                let _ = writeln!(writer, "{err_line}");
                                continue;
                            }
                        };
                        let id = parsed
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let params = parsed.get("params").cloned().unwrap_or_else(|| json!({}));
                        let (reply_tx, reply_rx) = mpsc::channel();
                        if tx_clone
                            .send(ControlRequest {
                                id: id.clone(),
                                params,
                                reply: reply_tx,
                            })
                            .is_err()
                        {
                            break;
                        }
                        if let Ok(guard) = repaint_clone.lock()
                            && let Some(ctx) = guard.as_ref()
                        {
                            ctx.request_repaint();
                        }
                        match reply_rx.recv() {
                            Ok(Ok(res)) => {
                                let out = json!({"ok": true, "id": id, "result": res}).to_string();
                                let _ = writeln!(writer, "{out}");
                            }
                            Ok(Err(err)) => {
                                let out = json!({"ok": false, "id": id, "error": err}).to_string();
                                let _ = writeln!(writer, "{out}");
                            }
                            Err(e) => {
                                let out = json!({"ok": false, "id": id, "error": e.to_string()})
                                    .to_string();
                                let _ = writeln!(writer, "{out}");
                            }
                        }
                    }
                });
            }
        });
    }

    /// Load or fetch a cached texture from disk, preserving PhotoCraft's transparent RGBA
    /// directly and only running solid-background stripping on unmatted opaque JPEGs.
    pub fn load_texture(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        let cache_key = path.to_string();
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
        match id {
            "ui.inspect" | "ui.elements" => {
                let filter = params.get("filter").and_then(Value::as_str);
                let elements = self.automation.query(filter);
                return Ok(json!({
                    "active_asset": self.session.active_asset,
                    "project_assets": self.session.project_assets,
                    "tab": format!("{:?}", self.ui_state.center_tab),
                    "anim_mode": format!("{:?}", self.ui_state.anim_mode),
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
                    "tab": match self.ui_state.center_tab {
                        CenterTab::OriginalView => "original",
                        CenterTab::AssetSheet => "asset_sheet",
                        CenterTab::TurnaroundSheet => "turnaround_sheet",
                        CenterTab::TwoD => "2d",
                        CenterTab::ThreeD => "3d",
                        CenterTab::FivePoseLineup => "lineup",
                    },
                    "active_asset": self.session.active_asset,
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
                if let Some(yaw) = params.get("yaw").and_then(Value::as_f64) {
                    self.ui_state.yaw_deg = yaw as f32;
                }
                if let Some(range) = params.get("pose_range_pct").and_then(Value::as_f64) {
                    self.ui_state.pose_range_pct = (range as f32).clamp(0.0, 100.0);
                }
                if let Some(height) = params.get("jump_height_pct").and_then(Value::as_f64) {
                    self.ui_state.jump_height_pct = (height as f32).clamp(0.0, 80.0);
                }
                if let Some(expression) = params.get("mouth_expression").and_then(Value::as_str) {
                    if !["smile", "laugh", "ooh", "mbp", "talk"].contains(&expression) { return Err("Unknown mouth expression".into()); }
                    self.ui_state.mouth_viseme = expression.into();
                }
                for (key, value, low, high) in [("mouth_opening", &mut self.ui_state.smile_open, 0.0, 100.0), ("eye_blink", &mut self.ui_state.eye_blink, 0.0, 100.0), ("eye_gaze", &mut self.ui_state.eye_gaze, -45.0, 45.0)] {
                    if let Some(number) = params.get(key).and_then(Value::as_f64) { *value = (number as f32).clamp(low, high); }
                }
                if let Some(playing) = params.get("playing").and_then(Value::as_bool) {
                    self.ui_state.playing = playing;
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
                self.ui_state.status_line = format!(
                    "CLI updated view: asset={:?}, tab={:?}, mode={:?}, yaw={:.0}°, range={:.0}%",
                    self.session.active_asset,
                    self.ui_state.center_tab,
                    self.ui_state.anim_mode,
                    self.ui_state.yaw_deg,
                    self.ui_state.pose_range_pct
                );
                return Ok(json!({
                    "updated": true,
                    "active_asset": self.session.active_asset,
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
        let res = self.session.execute(id, params).map_err(|e| e.to_string());
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
                    "project.load_asset"
                        | "asset.make_transparent"
                        | "asset.build_cutout_rig"
                        | "asset.load_manifest"
                        | "asset.cut_sheet"
                        | "intake.validate"
                ) {
                    self.textures.clear();
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
                        .unwrap_or("asset_sheet");
                    self.ui_state.center_tab = if slot == "turnaround_sheet" {
                        CenterTab::TurnaroundSheet
                    } else {
                        CenterTab::AssetSheet
                    };
                }
                if matches!(id, "project.load_asset" | "asset.load_manifest") {
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
            self.session = done.session;
            self.ui_state.busy_command = None;
            self.sync_ui_after_command(&done.id, &done.result);
        }
    }

    pub fn expire_stale_screenshots(&mut self, max_age: Duration) {
        let msg = "Viewport screenshot timed out because the window surface is suspended or unmapped by the compositor; focus the window or use headless vision.analyze";
        if self
            .pending_screenshot
            .as_ref()
            .is_some_and(|(_, _, t)| t.elapsed() >= max_age)
            && let Some((_, reply_tx, _)) = self.pending_screenshot.take()
        {
            let _ = reply_tx.send(Err(msg.to_string()));
        }
        if self
            .pending_vision
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() >= max_age)
            && let Some((reply_tx, _)) = self.pending_vision.take()
        {
            let _ = reply_tx.send(Err(msg.to_string()));
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

    pub fn render_headless_frame(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        for pos in self.pending_clicks.drain(..) {
            events.push(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 980.0));
        if events.is_empty() {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    ..Default::default()
                },
                |ui| {
                    self.render_workspace(ui);
                },
            );
            out.textures_delta.clear();
        } else {
            for ev in events {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen_rect),
                        events: vec![ev],
                        ..Default::default()
                    },
                    |ui| {
                        self.render_workspace(ui);
                    },
                );
                out.textures_delta.clear();
            }
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    ..Default::default()
                },
                |ui| {
                    self.render_workspace(ui);
                },
            );
            out.textures_delta.clear();
        }
    }

    /// Service pending control requests and background jobs when the native window is hidden or
    /// suspended by the compositor (e.g., on an inactive Wayland workspace).
    pub fn service_headless_control_tick(
        &mut self,
        headless_ctx: &egui::Context,
        screenshot_timeout: Duration,
    ) {
        self.poll_background_jobs_for_test();
        self.expire_stale_screenshots(screenshot_timeout);

        let mut pending = Vec::new();
        if let Some(rx) = &self.control_rx {
            while let Ok(req) = rx.try_recv() {
                pending.push(req);
            }
        }
        if pending.is_empty() {
            return;
        }

        if self.automation.elements.is_empty() {
            self.render_headless_frame(headless_ctx);
        }

        for req in pending {
            if req.id == "ui.screenshot" {
                let out_path = req
                    .params
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or("exports/animationcraft_window.png")
                    .to_string();
                self.pending_screenshot = Some((out_path, req.reply, Instant::now()));
                if let Ok(guard) = self.repaint_ctx.lock()
                    && let Some(ctx) = guard.as_ref()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                    ctx.request_repaint();
                }
            } else if req.id == "ui.vision" {
                self.pending_vision = Some((req.reply, Instant::now()));
                if let Ok(guard) = self.repaint_ctx.lock()
                    && let Some(ctx) = guard.as_ref()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                    ctx.request_repaint();
                }
            } else {
                let reply_val = self.dispatch(&req.id, req.params);
                if !self.pending_clicks.is_empty() || req.id == "ui.set_view" {
                    self.render_headless_frame(headless_ctx);
                }
                let _ = req.reply.send(reply_val);
                if let Ok(guard) = self.repaint_ctx.lock()
                    && let Some(ctx) = guard.as_ref()
                {
                    ctx.request_repaint();
                }
            }
        }
    }

    /// Wrap this app in a shared native runner that keeps `--control <port>` responsive even when
    /// the Wayland compositor suspends `eframe::App::ui` on an inactive workspace.
    pub fn into_native_app(self, ctx: egui::Context) -> Box<dyn eframe::App> {
        if let Ok(mut guard) = self.repaint_ctx.lock() {
            *guard = Some(ctx);
        }
        let has_control = self.control_port.is_some();
        let shared = Arc::new(Mutex::new(self));
        let last_ui_frame = Arc::new(Mutex::new(Instant::now()));

        if has_control {
            let app_clone = Arc::clone(&shared);
            let last_frame_clone = Arc::clone(&last_ui_frame);
            std::thread::spawn(move || {
                let headless_ctx = egui::Context::default();
                loop {
                    std::thread::sleep(Duration::from_millis(25));
                    let frame_age = last_frame_clone
                        .lock()
                        .map(|t| t.elapsed())
                        .unwrap_or(Duration::ZERO);
                    if frame_age >= Duration::from_millis(120)
                        && let Ok(mut app) = app_clone.lock()
                    {
                        app.service_headless_control_tick(
                            &headless_ctx,
                            Duration::from_millis(1200),
                        );
                    }
                }
            });
        }

        Box::new(SharedNativeApp {
            app: shared,
            last_ui_frame,
        })
    }
}

struct SharedNativeApp {
    app: Arc<Mutex<AnimationCraftApp>>,
    last_ui_frame: Arc<Mutex<Instant>>,
}

impl eframe::App for SharedNativeApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Ok(mut app) = self.app.lock() {
            app.raw_input_hook(ctx, input);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if let Ok(mut t) = self.last_ui_frame.lock() {
            *t = Instant::now();
        }
        if let Ok(mut app) = self.app.lock() {
            app.ui(ui, frame);
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
        // Feed bridge clicks before egui processes input, rather than appending to processed input.
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
        // Poll completed background subprocess commands so the UI thread never blocks
        self.poll_background_jobs_for_test();
        self.expire_stale_screenshots(Duration::from_millis(1200));

        // Handle drag-and-drop image import directly into the active character's sheet tab
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
                    "turnaround_sheet".to_string()
                }
                _ => "asset_sheet".to_string(),
            };
            let tolerance = self.ui_state.bg_tolerance;
            let use_ocr = self.ui_state.use_ocr;
            self.ui_state.image_path = Some(dropped.clone());
            self.exec_ui_command(UiCommand::IntakeValidate {
                image_path: dropped,
                slot,
                attach_to_active: true,
                auto_cut: true,
                tolerance,
                use_ocr,
            });
        }

        // Check if a requested viewport screenshot event arrived on this frame
        let screenshot_img = ui.ctx().input(|i| {
            for ev in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    return Some(image.clone());
                }
            }
            None
        });
        if let Some(color_img) = screenshot_img {
            let w = color_img.width() as u32;
            let h = color_img.height() as u32;
            let mut raw = Vec::with_capacity((w * h * 4) as usize);
            for px in &color_img.pixels {
                raw.extend_from_slice(&px.to_srgba_unmultiplied());
            }
            // Window backdrop, 2D stage fill, and 2D stage grid lines.
            let bg_colors: &[[u8; 3]] = if self.ui_state.light_backdrop {
                &[[235, 238, 244], [225, 228, 235], [191, 197, 208]]
            } else {
                &[[16, 18, 24], [20, 24, 32], [31, 37, 49]]
            };
            // Measure the stage canvas, not the whole window; fall back to the window if no canvas is shown.
            let ppp = ui.ctx().pixels_per_point();
            let region = ["stage.viewport_2d", "pose.canvas"].iter().find_map(|id| {
                let el = self.automation.elements.iter().find(|e| e.id == *id)?;
                let x0 = ((el.rect[0] * ppp).round().max(0.0) as u32).min(w);
                let y0 = ((el.rect[1] * ppp).round().max(0.0) as u32).min(h);
                let x1 = (((el.rect[0] + el.rect[2]) * ppp).round() as u32).min(w);
                let y1 = (((el.rect[1] + el.rect[3]) * ppp).round() as u32).min(h);
                (x1 > x0 && y1 > y0).then(|| (*id, [x0, y0, x1 - x0, y1 - y0]))
            });
            let mut vision_metrics = match region {
                Some((id, [x0, y0, rw, rh])) => {
                    let mut crop = Vec::with_capacity((rw * rh * 4) as usize);
                    for y in y0..y0 + rh {
                        let start = ((y * w + x0) * 4) as usize;
                        crop.extend_from_slice(&raw[start..start + (rw * 4) as usize]);
                    }
                    let mut m = animationcraft_engine::commands::analyze_viewport_region(
                        &crop, rw, rh, bg_colors,
                    );
                    m["region"] = json!({ "id": id, "rect_px": [x0, y0, rw, rh] });
                    m
                }
                None => {
                    let mut m =
                        animationcraft_engine::commands::analyze_viewport_region(&raw, w, h, bg_colors);
                    m["region"] = json!({ "id": "window", "rect_px": [0, 0, w, h] });
                    m
                }
            };
            vision_metrics["note"] =
                json!("pixel coordinates are relative to region.rect_px within the window screenshot");
            if let Some((reply_tx, _)) = self.pending_vision.take() {
                let _ = reply_tx.send(Ok(json!({
                    "active_asset": self.session.active_asset,
                    "tab": format!("{:?}", self.ui_state.center_tab),
                    "anim_mode": format!("{:?}", self.ui_state.anim_mode),
                    "viewport": vision_metrics,
                    "element_count": self.automation.elements.len(),
                })));
            }
            if let Some((out_path, reply_tx, _)) = self.pending_screenshot.take() {
                let save_res = match image::RgbaImage::from_raw(w, h, raw) {
                    Some(buf) => buf
                        .save(&out_path)
                        .map(|()| {
                            json!({
                                "saved": out_path,
                                "width": w,
                                "height": h,
                                "vision": vision_metrics,
                            })
                        })
                        .map_err(|e| e.to_string()),
                    None => Err("Failed to construct RgbaImage from screenshot buffer".to_string()),
                };
                let _ = reply_tx.send(save_res);
            }
        }

        // Process any incoming live CLI control commands (`--control <port>`)
        let mut pending = Vec::new();
        if let Some(rx) = &self.control_rx {
            while let Ok(req) = rx.try_recv() {
                pending.push(req);
            }
        }
        for req in pending {
            if req.id == "ui.screenshot" {
                let out_path = req
                    .params
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or("exports/animationcraft_window.png")
                    .to_string();
                self.pending_screenshot = Some((out_path, req.reply, Instant::now()));
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
                ui.ctx().request_repaint();
            } else if req.id == "ui.vision" {
                self.pending_vision = Some((req.reply, Instant::now()));
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
                ui.ctx().request_repaint();
            } else {
                let reply_val = self.dispatch(&req.id, req.params);
                let _ = req.reply.send(reply_val);
                ui.ctx().request_repaint();
            }
        }

        self.render_workspace(ui);

        // Keep repainting smoothly for live 2D sprite animation and TCP control responsiveness
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
    IntakeValidate {
        image_path: String,
        slot: String,
        attach_to_active: bool,
        auto_cut: bool,
        tolerance: u32,
        use_ocr: bool,
    },
    IntakeThreeQuestion {
        archetype: String,
        q1: String,
        q2: String,
        q3: String,
        strip_costume: bool,
    },
    AssetCutSheet {
        asset_id: String,
        tolerance: u32,
        use_ocr: bool,
    },
    CutReviewedSheet {
        asset_id: String,
        source: String,
        kind: String,
        layout: Option<String>,
        tolerance: u32,
        use_ocr: bool,
    },
    LoadPreset {
        preset: String,
    },
    SetPartState {
        part: String,
        visible: Option<bool>,
        solo: Option<bool>,
        verified: Option<bool>,
    },
    PartShowAll,
    TurnaroundRegister,
    AnimPreview2d {
        mode: String,
    },
    MeshBuild4Dir {
        recon_mode: String,
        inflation_depth_pct: u32,
        relief_detail_pct: u32,
        mesh_res: u32,
        proj_blend_pct: u32,
        bump_intensity: u32,
    },
    ExportAgentPrompt {
        output: String,
    },
    ExportEffectcraft {
        output: String,
    },
    ExportFilmcraft {
        output: String,
    },
    ExportBlender {
        output: String,
        script: String,
    },
    ExportCharacterIteration {
        character: String,
    },
    ExportLayeredMotion {
        spec: String,
        out_dir: Option<String>,
    },
    ValidateMotion {
        video: String,
        reference: Option<String>,
        spec: Option<String>,
        loop_clip: bool,
    },
}

impl UiCommand {
    /// Convert to engine command id + params.
    pub fn to_engine_call(&self) -> (&'static str, Value) {
        match self {
            UiCommand::Engine { id, params } => (id, params.clone()),
            UiCommand::IntakeValidate {
                image_path,
                slot,
                attach_to_active,
                auto_cut,
                tolerance,
                use_ocr,
            } => (
                "intake.validate",
                serde_json::json!({
                    "image": image_path,
                    "slot": slot,
                    "attach_to_active": attach_to_active,
                    "auto_cut": auto_cut,
                    "tolerance": tolerance,
                    "use_ocr": use_ocr,
                }),
            ),
            UiCommand::IntakeThreeQuestion {
                archetype,
                q1,
                q2,
                q3,
                strip_costume,
            } => (
                "intake.three_question",
                serde_json::json!({
                    "archetype": archetype,
                    "q1": q1,
                    "q2": q2,
                    "q3": q3,
                    "strip_costume": strip_costume,
                }),
            ),
            UiCommand::AssetCutSheet {
                asset_id,
                tolerance,
                use_ocr,
            } => (
                "asset.cut_sheet",
                serde_json::json!({
                    "asset": asset_id,
                    "tolerance": tolerance,
                    "use_ocr": use_ocr,
                }),
            ),
            UiCommand::CutReviewedSheet { asset_id, source, kind, layout, tolerance, use_ocr } => {
                let revision = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
                let mut params = serde_json::json!({"asset":asset_id,"source":source,"kind":kind,"tolerance":tolerance,"use_ocr":use_ocr,
                    "out_dir":format!("exports/sheet_reviews/{asset_id}/{revision}")});
                if let Some(layout) = layout { params["layout"] = serde_json::json!(layout); }
                ("asset.cut_sheet", params)
            },
            UiCommand::LoadPreset { preset } => (
                "asset.load_manifest",
                serde_json::json!({ "preset": preset }),
            ),
            UiCommand::SetPartState {
                part,
                visible,
                solo,
                verified,
            } => (
                "part.set_state",
                serde_json::json!({
                    "part": part,
                    "visible": visible,
                    "solo": solo,
                    "verified": verified,
                }),
            ),
            UiCommand::PartShowAll => ("part.show_all", serde_json::json!({})),
            UiCommand::TurnaroundRegister => {
                ("turnaround.optimize_accuracy", serde_json::json!({}))
            }
            UiCommand::AnimPreview2d { mode } => {
                ("anim.preview_2d", serde_json::json!({ "mode": mode }))
            }
            UiCommand::MeshBuild4Dir {
                recon_mode,
                inflation_depth_pct,
                relief_detail_pct,
                mesh_res,
                proj_blend_pct,
                bump_intensity,
            } => (
                "mesh.build_4dir",
                serde_json::json!({
                    "recon_mode": recon_mode,
                    "inflation_depth_pct": inflation_depth_pct,
                    "relief_detail_pct": relief_detail_pct,
                    "mesh_res": mesh_res,
                    "proj_blend_pct": proj_blend_pct,
                    "bump_intensity": bump_intensity,
                }),
            ),
            UiCommand::ExportAgentPrompt { output } => (
                "export.agent_prompt",
                serde_json::json!({ "output": output }),
            ),
            UiCommand::ExportEffectcraft { output } => (
                "export.effectcraft",
                serde_json::json!({ "output": output }),
            ),
            UiCommand::ExportFilmcraft { output } => {
                ("export.filmcraft", serde_json::json!({ "output": output }))
            }
            UiCommand::ExportBlender { output, script } => (
                "export.blender",
                serde_json::json!({ "out_mp4": output, "script": script }),
            ),
            UiCommand::ExportCharacterIteration { character } => (
                "export.character_iteration",
                serde_json::json!({ "character": character }),
            ),
            UiCommand::ExportLayeredMotion { spec, out_dir } => {
                let mut params = serde_json::json!({ "spec": spec });
                if let Some(dir) = out_dir {
                    params["out_dir"] = serde_json::json!(dir);
                }
                ("export.layered_motion", params)
            }
            UiCommand::ValidateMotion {
                video,
                reference,
                spec,
                loop_clip,
            } => {
                let mut params = serde_json::json!({
                    "video": video,
                    "loop": loop_clip,
                });
                if let Some(r) = reference {
                    params["reference"] = serde_json::json!(r);
                }
                if let Some(s) = spec {
                    params["spec"] = serde_json::json!(s);
                }
                ("validate.motion", params)
            }
        }
    }

    fn is_long_running(&self) -> bool {
        if let UiCommand::Engine { id, .. } = self {
            return matches!(
                *id,
                "asset.make_transparent"
                    | "asset.build_cutout_rig"
                    | "export.cutout_mp4"
                    | "export.layered_motion"
                    | "validate.motion"
            );
        }
        matches!(
            self,
            UiCommand::IntakeValidate { .. }
                | UiCommand::IntakeThreeQuestion { .. }
                | UiCommand::AssetCutSheet { .. }
                | UiCommand::CutReviewedSheet { .. }
                | UiCommand::TurnaroundRegister
                | UiCommand::MeshBuild4Dir { .. }
                | UiCommand::ExportEffectcraft { .. }
                | UiCommand::ExportFilmcraft { .. }
                | UiCommand::ExportBlender { .. }
                | UiCommand::ExportCharacterIteration { .. }
                | UiCommand::ExportLayeredMotion { .. }
                | UiCommand::ValidateMotion { .. }
        )
    }
}

impl AnimationCraftApp {
    /// Execute a UI command. Long-running subprocess commands run on a background worker
    /// thread so the egui window and TCP control server never freeze.
    pub fn exec_ui_command(&mut self, cmd: UiCommand) {
        let (id, mut params) = cmd.to_engine_call();
        if matches!(cmd, UiCommand::AssetCutSheet { .. }) {
            let slot = if self.ui_state.center_tab == CenterTab::TurnaroundSheet { "turnaround_sheet" } else { "asset_sheet" };
            let source = self.session.current_asset().and_then(|a| panels::sheet_path(a, slot));
            let Some(source) = source else { self.ui_state.status_line = "Attach a sheet before slicing.".into(); return; };
            params["source"] = serde_json::json!(source);
            let revision = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
            params["out_dir"] = serde_json::json!(format!("exports/sheet_reviews/{revision}"));
        }
        if cmd.is_long_running() {
            if self.ui_state.busy_command.is_some() {
                self.ui_state.status_line =
                    "A background pipeline command is already running…".to_string();
                return;
            }
            self.sync_ui_before_command(id, &params);
            self.ui_state.busy_command = Some(id.to_string());
            self.ui_state.status_line = format!("Running `{id}` in background…");
            let mut session_clone = self.session.clone();
            let tx = self.job_tx.clone();
            let id_owned = id.to_string();
            std::thread::spawn(move || {
                let result = session_clone
                    .execute(&id_owned, params)
                    .map_err(|e| e.to_string());
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
    fn every_ui_command_maps_to_a_registered_engine_command() {
        let sample_commands = vec![
            UiCommand::Engine {
                id: "project.list_assets",
                params: json!({}),
            },
            UiCommand::Engine {
                id: "project.add_asset",
                params: json!({"preset": "hopper"}),
            },
            UiCommand::Engine {
                id: "project.remove_asset",
                params: json!({"id": "hopper"}),
            },
            UiCommand::Engine {
                id: "project.update_asset",
                params: json!({"id": "hopper", "name": "Hopper"}),
            },
            UiCommand::Engine {
                id: "project.load_asset",
                params: json!({"id": "hopper"}),
            },
            UiCommand::Engine {
                id: "asset.make_transparent",
                params: json!({}),
            },
            UiCommand::Engine {
                id: "asset.build_cutout_rig",
                params: json!({}),
            },
            UiCommand::Engine {
                id: "export.cutout_mp4",
                params: json!({}),
            },
            UiCommand::IntakeValidate {
                image_path: "test.png".into(),
                slot: "asset_sheet".into(),
                attach_to_active: true,
                auto_cut: true,
                tolerance: 25,
                use_ocr: false,
            },
            UiCommand::IntakeThreeQuestion {
                archetype: "humanoid_biped".into(),
                q1: "walk_jump_talk".into(),
                q2: "head,torso".into(),
                q3: "filmcraft_mp4".into(),
                strip_costume: false,
            },
            UiCommand::AssetCutSheet {
                asset_id: "hopper".into(),
                tolerance: 25,
                use_ocr: true,
            },
            UiCommand::LoadPreset {
                preset: "hopper".into(),
            },
            UiCommand::SetPartState {
                part: "head".into(),
                visible: Some(true),
                solo: Some(false),
                verified: Some(true),
            },
            UiCommand::PartShowAll,
            UiCommand::TurnaroundRegister,
            UiCommand::AnimPreview2d {
                mode: "walk".into(),
            },
            UiCommand::MeshBuild4Dir {
                recon_mode: "volumetric".into(),
                inflation_depth_pct: 58,
                relief_detail_pct: 44,
                mesh_res: 32,
                proj_blend_pct: 65,
                bump_intensity: 35,
            },
            UiCommand::ExportAgentPrompt {
                output: "exports/prompt.md".into(),
            },
            UiCommand::ExportEffectcraft {
                output: "exports/effectcraft_demo/build_scene.jsx".into(),
            },
            UiCommand::ExportFilmcraft {
                output: "exports/hopper_filmcraft.fcproj".into(),
            },
            UiCommand::ExportBlender {
                output: "exports/hopper_4dir_blender53.mp4".into(),
                script: "pipeline/blender53_bridge.py".into(),
            },
            UiCommand::ExportCharacterIteration {
                character: "hopper".into(),
            },
            UiCommand::ExportLayeredMotion {
                spec: "spec.json".into(),
                out_dir: None,
            },
            UiCommand::ValidateMotion {
                video: "clip.webm".into(),
                reference: None,
                spec: None,
                loop_clip: false,
            },
        ];

        for cmd in sample_commands {
            let (id, _params) = cmd.to_engine_call();
            assert!(
                animationcraft_engine::commands::find(id).is_some(),
                "UiCommand mapped to unregistered engine command `{id}`"
            );
        }
    }

    #[test]
    fn headless_control_tick_services_commands_and_times_out_screenshots_when_window_suspended() {
        let mut app = AnimationCraftApp::new();
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
        assert_eq!(state_res["tab"], "original");

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
        assert!(inspect_res["element_count"].as_u64().unwrap_or(0) >= 3);

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
    }
}
