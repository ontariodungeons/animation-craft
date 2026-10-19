//! Session state for AnimationCraft.
//!
//! Holds the current project/assets, loaded manifests, and verified measurements,
//! and provides [`Session::execute`] for command dispatch.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{EngineError, Result};

/// The engine session. All project changes go through [`Session::execute`].
#[derive(Debug, Clone)]
pub struct Session {
    /// Current working directory for relative asset paths.
    pub cwd: PathBuf,
    /// Loaded assets keyed by their logical id.
    pub assets: HashMap<String, Asset>,
    /// Ordered connections in this project, separate from decoded/cached assets.
    pub project_assets: Vec<ProjectAsset>,
    /// Currently active asset id in the session.
    pub active_asset: Option<String>,
    /// Active 2D preview mode (`idle`, `walk`, `run`, `jump`, `wave`, `static`).
    pub preview_mode: String,
    /// Active 2D preview speed multiplier.
    pub preview_speed: f32,
    /// Active 2D preview frame range `(start_frame, end_frame)` at 24fps.
    pub preview_range: (u32, u32),
    /// Last command result, echoed back for inspection.
    pub last_result: Option<Value>,
    /// Whether Blender 5.3 is reachable at 127.0.0.1:9876.
    pub blender_available: bool,
}

impl Default for Session {
    fn default() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            cwd,
            assets: HashMap::new(),
            project_assets: vec![ProjectAsset {
                id: "hopper".into(),
                name: "Hopper".into(),
                preset: Some("hopper".into()),
                path: None,
                kind: "auto".into(),
            }],
            active_asset: None,
            preview_mode: "walk".to_string(),
            preview_speed: 1.0,
            preview_range: (0, 72),
            last_result: None,
            blender_available: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAsset {
    pub id: String,
    pub name: String,
    pub preset: Option<String>,
    pub path: Option<PathBuf>,
    /// `auto`, `image`, `poses`, `turnaround`, or `cutout_rig`.
    pub kind: String,
}

/// A reference image attached to a character once the user likes the original design.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttachedReference {
    pub label: String,
    /// Role/kind: `"asset_sheet"`, `"turnaround_sheet"`, or `"reference"`.
    pub kind: String,
    /// Transparent-background (Step 1) image path.
    pub path: PathBuf,
    /// Raw uploaded image path before background removal.
    pub raw_path: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    /// Automated telemetry reported by the tool on import.
    pub report: Option<Value>,
}

/// A loaded image/character asset with its decoded metadata, cut manifests, and measured telemetry.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    /// Primary Original Design image path (transparent Step 1 cutout or primary hero frame).
    pub path: PathBuf,
    /// Raw Original Design image path before Step 1 background removal (if available).
    #[serde(default)]
    pub raw_original_path: Option<PathBuf>,
    /// Automated tool inspection report for the Original Design image.
    #[serde(default)]
    pub original_inspection: Option<Value>,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub sheet_kind: Option<String>,
    pub validation: Option<Value>,
    /// Actual recommendation returned by the three-question intake command.
    #[serde(default)]
    pub intake_recommendation: Option<Value>,
    /// Attached full Pose / Asset Sheet image path (transparent background).
    #[serde(default)]
    pub attached_asset_sheet: Option<PathBuf>,
    /// Attached full Turnaround Sheet image path (transparent background).
    #[serde(default)]
    pub attached_turnaround_sheet: Option<PathBuf>,
    /// Reference images uploaded and attached to this character once the user likes the design.
    #[serde(default)]
    pub attached_references: Vec<AttachedReference>,
    /// Guided 2D Cutout Rig Plan (`character-rigging` + `pose-library-design` + PhotoCraft + EffectCraft 0.10.0).
    #[serde(default)]
    pub cutout_rig: Option<LoadedCutoutRigPlan>,
    pub pose_manifest: Option<LoadedManifest>,
    pub turnaround_manifest: Option<LoadedManifest>,
    pub turnaround_registration: Option<MeasuredTurnaroundRegistration>,
    pub motion_range: Option<MeasuredMotionRange>,
    pub parts: Vec<PartState>,
}

/// A single movable cutout layer part discovered and masked via `photocraft-cli`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CutoutRigPart {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: i32,
    pub parent: Option<String>,
    pub pivot: [f32; 2],
    pub pivot_uv: [f32; 2],
    pub joint_radius_px: u32,
    pub completed_overlap_pixels: u32,
    pub rotation: [f32; 2],
    pub crop_box: [u32; 4],
    pub crop_size: [u32; 2],
    pub cropped_path: PathBuf,
    pub full_layer_path: PathBuf,
}

/// A parsed `{character_id}_cutout_rig_plan.json` produced by `guided_cutout_rig_builder.py`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadedCutoutRigPlan {
    pub plan_path: PathBuf,
    pub character_id: String,
    pub rig_type: String,
    pub source_image: PathBuf,
    pub canvas_size: [u32; 2],
    pub character_bbox: [u32; 4],
    pub photocraft_psd: Option<PathBuf>,
    pub cutout_asset_sheet: Option<PathBuf>,
    pub parts: Vec<CutoutRigPart>,
    pub pose_library: Value,
    pub effectcraft: Value,
}

impl LoadedCutoutRigPlan {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let raw: Value = serde_json::from_str(&text).map_err(|e| {
            EngineError::InvalidParams(format!("invalid cutout rig plan JSON: {e}"))
        })?;
        if let (Some(canvas_arr), Some(layers_arr)) = (
            raw.get("canvas").and_then(Value::as_array),
            raw.get("layers").and_then(Value::as_array),
        ) {
            let cw = canvas_arr
                .first()
                .and_then(Value::as_u64)
                .unwrap_or(1920)
                .max(1) as u32;
            let ch = canvas_arr
                .get(1)
                .and_then(Value::as_u64)
                .unwrap_or(1080)
                .max(1) as u32;
            let character_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("layered_motion")
                .to_string();
            let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
            let mut parts = Vec::with_capacity(layers_arr.len());
            let mut root_file: Option<PathBuf> = None;
            for (idx, item) in layers_arr.iter().enumerate() {
                let id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("layer")
                    .to_string();
                let file_path = item
                    .get("file")
                    .and_then(Value::as_str)
                    .map(|f| {
                        let p = PathBuf::from(f);
                        if p.is_absolute() {
                            p
                        } else {
                            base_dir.join(p)
                        }
                    })
                    .unwrap_or_default();
                let parent = item
                    .get("parent")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                if parent.is_none() && root_file.is_none() && !file_path.as_os_str().is_empty() {
                    root_file = Some(file_path.clone());
                }
                let pivot = if let Some(piv_arr) = item.get("pivot").and_then(Value::as_array) {
                    [
                        piv_arr
                            .first()
                            .and_then(Value::as_f64)
                            .unwrap_or(cw as f64 * 0.5) as f32,
                        piv_arr
                            .get(1)
                            .and_then(Value::as_f64)
                            .unwrap_or(ch as f64 * 0.5) as f32,
                    ]
                } else {
                    [cw as f32 * 0.5, ch as f32 * 0.5]
                };
                parts.push(CutoutRigPart {
                    id: id.clone(),
                    label: id,
                    kind: "layered".to_string(),
                    layer: (idx as i32) + 1,
                    parent,
                    pivot,
                    pivot_uv: [pivot[0] / cw as f32, pivot[1] / ch as f32],
                    joint_radius_px: 14,
                    completed_overlap_pixels: 0,
                    rotation: [-45.0, 45.0],
                    crop_box: [0, 0, cw, ch],
                    crop_size: [cw, ch],
                    cropped_path: file_path.clone(),
                    full_layer_path: file_path,
                });
            }
            let source_image = root_file
                .or_else(|| parts.first().map(|p| p.full_layer_path.clone()))
                .unwrap_or_default();
            return Ok(Self {
                plan_path: path.to_path_buf(),
                character_id,
                rig_type: "layered_motion".to_string(),
                source_image,
                canvas_size: [cw, ch],
                character_bbox: [0, 0, cw, ch],
                photocraft_psd: None,
                cutout_asset_sheet: None,
                parts,
                pose_library: Value::Null,
                effectcraft: raw,
            });
        }
        let character_id = raw
            .get("character_id")
            .and_then(Value::as_str)
            .unwrap_or("hopper")
            .to_string();
        let rig_type = raw
            .get("rig_type")
            .and_then(Value::as_str)
            .unwrap_or("cutout_rig")
            .to_string();
        let source_image = raw
            .get("source_image")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_default();
        let canvas_arr = raw.get("canvas_size").and_then(Value::as_array);
        let canvas_size = [
            canvas_arr
                .and_then(|a| a.first())
                .and_then(Value::as_u64)
                .unwrap_or(1086) as u32,
            canvas_arr
                .and_then(|a| a.get(1))
                .and_then(Value::as_u64)
                .unwrap_or(1448) as u32,
        ];
        let bbox_arr = raw.get("character_bbox").and_then(Value::as_array);
        let character_bbox = [
            bbox_arr
                .and_then(|a| a.first())
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            bbox_arr
                .and_then(|a| a.get(1))
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            bbox_arr
                .and_then(|a| a.get(2))
                .and_then(Value::as_u64)
                .unwrap_or(canvas_size[0] as u64) as u32,
            bbox_arr
                .and_then(|a| a.get(3))
                .and_then(Value::as_u64)
                .unwrap_or(canvas_size[1] as u64) as u32,
        ];
        let photocraft_psd = raw
            .get("photocraft_psd")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let cutout_asset_sheet = raw
            .get("cutout_asset_sheet")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let parts: Vec<CutoutRigPart> = raw
            .get("parts")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let pose_library = raw.get("pose_library").cloned().unwrap_or(Value::Null);
        let effectcraft = raw.get("effectcraft").cloned().unwrap_or(Value::Null);
        Ok(Self {
            plan_path: path.to_path_buf(),
            character_id,
            rig_type,
            source_image,
            canvas_size,
            character_bbox,
            photocraft_psd,
            cutout_asset_sheet,
            parts,
            pose_library,
            effectcraft,
        })
    }
}

/// Visibility, solo, and verification state for a cut part from the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartState {
    pub name: String,
    pub visible: bool,
    pub solo: bool,
    #[serde(default)]
    pub verified: bool,
}

/// A parsed `manifest.json` produced by `pose_and_part_cutter.py` or `codex_accuracy_optimizer.py`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LoadedManifest {
    pub manifest_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub sheet_type: String,
    pub status: String,
    pub matting_engine: Option<String>,
    #[serde(default)]
    pub source_image: Option<PathBuf>,
    #[serde(default)]
    pub source_size: Option<[u32; 2]>,
    pub frames: Vec<ManifestFrame>,
    pub part_names: Vec<String>,
}

/// One cutout part entry inside a [`ManifestFrame`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestPart {
    pub name: String,
    pub file: String,
    pub resolved_path: PathBuf,
    pub size: [u32; 2],
    pub pivot: [f32; 2],
    pub bind_position: [f32; 2],
}

/// One frame entry from `manifest.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestFrame {
    pub index: usize,
    pub label: String,
    /// Relative file path inside `manifest_dir` (e.g. `frames/00_standing.png`).
    pub file: String,
    /// Resolved path on disk (`manifest_dir.join(&file)`).
    pub resolved_path: PathBuf,
    pub size: [u32; 2],
    #[serde(default)]
    pub source_box: Option<[u32; 4]>,
    pub anchor: Option<[f32; 2]>,
    #[serde(default)]
    pub parts: BTreeMap<String, ManifestPart>,
}

/// Measured silhouette motion envelope loaded from `motion_range.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuredMotionRange {
    /// Original measured JSON. Preserve nulls rather than reporting UI fallback zeroes as evidence.
    #[serde(default)]
    pub raw: Value,
    pub source_file: PathBuf,
    pub still_label: String,
    pub still_height_px: f32,
    pub width_min_delta_px: f32,
    pub width_max_delta_px: f32,
    pub width_max_pose: String,
    pub height_min_delta_px: f32,
    pub height_max_delta_px: f32,
    pub left_min_delta_px: f32,
    pub right_max_delta_px: f32,
    pub top_min_delta_px: f32,
    pub top_max_delta_px: f32,
    pub foot_lift_min_px: f32,
    pub foot_lift_max_px: f32,
    pub foot_lift_max_pose: String,
    pub poses: BTreeMap<String, PoseMotionMetrics>,
}

/// Per-pose measured silhouette metrics from `motion_range.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseMotionMetrics {
    pub label: String,
    pub frame_index: usize,
    pub width_px: f32,
    pub height_px: f32,
    pub foot_lift_px: f32,
    pub delta_width_px: f32,
    pub delta_height_px: f32,
}

/// Measured turnaround registration loaded from `turnaround_registration.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuredTurnaroundRegistration {
    /// Original registration evidence as loaded, including unavailable fields.
    #[serde(default)]
    pub raw: Value,
    pub source_file: PathBuf,
    pub asset_root: PathBuf,
    pub reference_direction: String,
    pub target_height_px: u32,
    pub max_vertical_drift_px: f32,
    pub before_crown_drift_px: f32,
    pub before_foot_drift_px: f32,
    pub registration_passed: bool,
    pub frames: Vec<ManifestFrame>,
}

impl LoadedManifest {
    /// Load and parse a `manifest.json` from a file or its parent directory.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let manifest_path = if path.is_dir() {
            path.join("manifest.json")
        } else {
            path.to_path_buf()
        };
        let manifest_dir = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let text = std::fs::read_to_string(&manifest_path)?;
        let raw: Value = serde_json::from_str(&text)
            .map_err(|e| EngineError::InvalidParams(format!("invalid manifest JSON: {e}")))?;

        let sheet_type = raw
            .get("sheetType")
            .or_else(|| raw.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let status = raw
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("loaded")
            .to_string();
        let matting_engine = raw
            .get("mattingEngine")
            .and_then(Value::as_str)
            .map(str::to_string);
        let source_image = raw
            .get("sourceImage")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(|| {
                let first_src = raw
                    .get("frames")
                    .and_then(Value::as_array)?
                    .first()?
                    .get("sourceFile")
                    .and_then(Value::as_str)?;
                let raw_manifest_dir = Path::new(first_src).parent()?.parent()?;
                let raw_manifest_json = raw_manifest_dir.join("manifest.json");
                if let Ok(txt) = std::fs::read_to_string(&raw_manifest_json)
                    && let Ok(v) = serde_json::from_str::<Value>(&txt)
                    && let Some(si) = v.get("sourceImage").and_then(Value::as_str)
                {
                    let p = PathBuf::from(si);
                    if p.is_file() {
                        return Some(p);
                    }
                }
                let raw_atlas = raw_manifest_dir.join("atlas.png");
                raw_atlas.is_file().then_some(raw_atlas)
            })
            .or_else(|| {
                let atlas = manifest_dir.join("atlas.png");
                atlas.is_file().then_some(atlas)
            });
        let source_size = raw
            .get("sourceSize")
            .and_then(Value::as_array)
            .map(|s| {
                [
                    s.first().and_then(Value::as_u64).unwrap_or(0) as u32,
                    s.get(1).and_then(Value::as_u64).unwrap_or(0) as u32,
                ]
            })
            .filter(|sz| sz[0] > 0 && sz[1] > 0);

        let mut frames = Vec::new();
        let mut part_set = Vec::new();

        if let Some(arr) = raw.get("frames").and_then(Value::as_array) {
            for (idx, item) in arr.iter().enumerate() {
                let index = item
                    .get("index")
                    .and_then(Value::as_u64)
                    .unwrap_or(idx as u64) as usize;
                let label = item
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("frame")
                    .to_string();
                let file = item
                    .get("file")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let resolved_path = manifest_dir.join(&file);
                let size = item
                    .get("size")
                    .and_then(Value::as_array)
                    .map(|s| {
                        let w = s.first().and_then(Value::as_u64).unwrap_or(0) as u32;
                        let h = s.get(1).and_then(Value::as_u64).unwrap_or(0) as u32;
                        [w, h]
                    })
                    .unwrap_or([0, 0]);
                let source_box = item
                    .get("sourceBox")
                    .and_then(Value::as_array)
                    .map(|b| {
                        [
                            b.first().and_then(Value::as_u64).unwrap_or(0) as u32,
                            b.get(1).and_then(Value::as_u64).unwrap_or(0) as u32,
                            b.get(2).and_then(Value::as_u64).unwrap_or(0) as u32,
                            b.get(3).and_then(Value::as_u64).unwrap_or(0) as u32,
                        ]
                    })
                    .filter(|b| b[2] > 0 && b[3] > 0);
                let anchor = item.get("anchor").and_then(Value::as_array).map(|a| {
                    let x = a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    let y = a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    [x, y]
                });
                let mut frame_parts = BTreeMap::new();
                if let Some(parts_obj) = item.get("parts").and_then(Value::as_object) {
                    for (k, pval) in parts_obj {
                        let pfile = pval
                            .get("file")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        if pfile.is_empty() {
                            continue;
                        }
                        if !part_set.contains(k) {
                            part_set.push(k.clone());
                        }
                        let psize = pval
                            .get("size")
                            .and_then(Value::as_array)
                            .map(|s| {
                                [
                                    s.first().and_then(Value::as_u64).unwrap_or(0) as u32,
                                    s.get(1).and_then(Value::as_u64).unwrap_or(0) as u32,
                                ]
                            })
                            .unwrap_or([0, 0]);
                        let ppivot = pval
                            .get("pivot")
                            .and_then(Value::as_array)
                            .map(|a| {
                                [
                                    a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32,
                                    a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32,
                                ]
                            })
                            .unwrap_or([0.0, 0.0]);
                        let pbind = pval
                            .get("bindPosition")
                            .and_then(Value::as_array)
                            .map(|a| {
                                [
                                    a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32,
                                    a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32,
                                ]
                            })
                            .unwrap_or([0.0, 0.0]);
                        frame_parts.insert(
                            k.clone(),
                            ManifestPart {
                                name: k.clone(),
                                resolved_path: manifest_dir.join(&pfile),
                                file: pfile,
                                size: psize,
                                pivot: ppivot,
                                bind_position: pbind,
                            },
                        );
                    }
                }
                frames.push(ManifestFrame {
                    index,
                    label,
                    file,
                    resolved_path,
                    size,
                    source_box,
                    anchor,
                    parts: frame_parts,
                });
            }
        }

        if part_set.is_empty()
            && let Some(puppet_parts) = raw
                .get("puppet")
                .and_then(|p| p.get("parts"))
                .and_then(Value::as_object)
        {
            for k in puppet_parts.keys() {
                if !part_set.contains(k) {
                    part_set.push(k.clone());
                }
            }
        }

        Ok(Self {
            manifest_dir,
            manifest_path,
            sheet_type,
            status,
            matting_engine,
            source_image,
            source_size,
            frames,
            part_names: part_set,
        })
    }
}

impl MeasuredMotionRange {
    /// Load and parse a `motion_range.json` produced by `codex_accuracy_optimizer.py`.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let file_path = if path.is_dir() {
            path.join("motion_range.json")
        } else {
            path.to_path_buf()
        };
        let text = std::fs::read_to_string(&file_path)?;
        let raw: Value = serde_json::from_str(&text)
            .map_err(|e| EngineError::InvalidParams(format!("invalid motion_range JSON: {e}")))?;

        let still = raw.get("still").unwrap_or(&Value::Null);
        let still_label = still
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("standing")
            .to_string();
        let still_height_px = still.get("heightPx").and_then(Value::as_f64).unwrap_or(0.0) as f32;

        let limits = raw.get("limits").unwrap_or(&Value::Null);
        let f_num = |section: &str, key: &str| -> f32 {
            limits
                .get(section)
                .and_then(|s| s.get(key))
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32
        };
        let f_str = |section: &str, key: &str| -> String {
            limits
                .get(section)
                .and_then(|s| s.get(key))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string()
        };

        let mut poses = BTreeMap::new();
        if let Some(poses_obj) = raw.get("poses").and_then(Value::as_object) {
            for (label, p_val) in poses_obj {
                let frame_index =
                    p_val.get("frameIndex").and_then(Value::as_u64).unwrap_or(0) as usize;
                let metrics = p_val.get("metrics").unwrap_or(&Value::Null);
                let delta = p_val.get("deltaFromStill").unwrap_or(&Value::Null);
                let width_px = metrics
                    .get("widthPx")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let height_px = metrics
                    .get("heightPx")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let foot_lift_px = metrics
                    .get("footLiftPx")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let delta_width_px =
                    delta.get("widthPx").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let delta_height_px =
                    delta.get("heightPx").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                poses.insert(
                    label.clone(),
                    PoseMotionMetrics {
                        label: label.clone(),
                        frame_index,
                        width_px,
                        height_px,
                        foot_lift_px,
                        delta_width_px,
                        delta_height_px,
                    },
                );
            }
        }

        Ok(Self {
            source_file: file_path,
            still_label,
            raw: raw.clone(),
            still_height_px,
            width_min_delta_px: f_num("widthPx", "minDeltaFromStill"),
            width_max_delta_px: f_num("widthPx", "maxDeltaFromStill"),
            width_max_pose: f_str("widthPx", "maxPose"),
            height_min_delta_px: f_num("heightPx", "minDeltaFromStill"),
            height_max_delta_px: f_num("heightPx", "maxDeltaFromStill"),
            left_min_delta_px: f_num("leftPx", "minDeltaFromStill"),
            right_max_delta_px: f_num("rightPx", "maxDeltaFromStill"),
            top_min_delta_px: f_num("topPx", "minDeltaFromStill"),
            top_max_delta_px: f_num("topPx", "maxDeltaFromStill"),
            foot_lift_min_px: f_num("footLiftPx", "min"),
            foot_lift_max_px: f_num("footLiftPx", "max"),
            foot_lift_max_pose: f_str("footLiftPx", "maxPose"),
            poses,
        })
    }
}

impl MeasuredTurnaroundRegistration {
    /// Load and parse a `turnaround_registration.json` produced by `codex_accuracy_optimizer.py`.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let file_path = if path.is_dir() {
            path.join("turnaround_registration.json")
        } else {
            path.to_path_buf()
        };
        let text = std::fs::read_to_string(&file_path)?;
        let raw: Value = serde_json::from_str(&text).map_err(|e| {
            EngineError::InvalidParams(format!("invalid turnaround_registration JSON: {e}"))
        })?;

        let parent_dir = file_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let asset_root = raw
            .get("assetRoot")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .unwrap_or_else(|| parent_dir.join("registered_turnaround"));

        let reference_direction = raw
            .get("referenceDirection")
            .and_then(Value::as_str)
            .unwrap_or("front")
            .to_string();
        let target_height_px = raw
            .get("target")
            .and_then(|t| t.get("heightPx"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let max_vertical_drift_px = raw
            .get("maxVerticalDriftPx")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        let before_crown_drift_px = raw
            .get("before")
            .and_then(|b| b.get("crownDriftPx"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        let before_foot_drift_px = raw
            .get("before")
            .and_then(|b| b.get("footDriftPx"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        let registration_passed = raw
            .get("rasterRegistrationPassed")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut frames = Vec::new();
        if let Some(arr) = raw.get("frames").and_then(Value::as_array) {
            for (idx, item) in arr.iter().enumerate() {
                let index = item
                    .get("index")
                    .and_then(Value::as_u64)
                    .unwrap_or(idx as u64) as usize;
                let label = item
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("view")
                    .to_string();
                let file = item
                    .get("file")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let resolved_path = asset_root.join(&file);
                let size = item
                    .get("size")
                    .and_then(Value::as_array)
                    .map(|s| {
                        let w = s.first().and_then(Value::as_u64).unwrap_or(0) as u32;
                        let h = s.get(1).and_then(Value::as_u64).unwrap_or(0) as u32;
                        [w, h]
                    })
                    .unwrap_or([0, 0]);
                let anchor = item.get("anchor").and_then(Value::as_array).map(|a| {
                    let x = a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    let y = a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    [x, y]
                });
                frames.push(ManifestFrame {
                    index,
                    label,
                    file,
                    resolved_path,
                    size,
                    source_box: None,
                    anchor,
                    parts: BTreeMap::new(),
                });
            }
        }

        Ok(Self {
            source_file: file_path,
            asset_root,
            raw: raw.clone(),
            reference_direction,
            target_height_px,
            max_vertical_drift_px,
            before_crown_drift_px,
            before_foot_drift_px,
            registration_passed,
            frames,
        })
    }
}

impl Asset {
    /// Synchronize `self.parts` and attached sheet paths from the loaded pose or turnaround manifest
    /// while preserving any existing visibility/solo toggles for matching part names.
    pub fn sync_parts_from_manifests(&mut self) {
        let mut names = Vec::new();
        if let Some(rig) = &self.cutout_rig {
            for part in &rig.parts {
                if !names.contains(&part.id) {
                    names.push(part.id.clone());
                }
            }
        }
        if let Some(pm) = &self.pose_manifest {
            if self.attached_asset_sheet.is_none() {
                self.attached_asset_sheet = pm.source_image.clone();
            }
            if let Some(sheet_path) = &self.attached_asset_sheet
                && !self
                    .attached_references
                    .iter()
                    .any(|r| r.kind == "asset_sheet")
            {
                let [w, h] = pm.source_size.unwrap_or([1376, 768]);
                self.attached_references.push(AttachedReference {
                    label: format!("Pose Sheet ({} poses)", pm.frames.len()),
                    kind: "asset_sheet".to_string(),
                    path: sheet_path.clone(),
                    raw_path: pm.source_image.clone(),
                    width: w,
                    height: h,
                    report: None,
                });
            }
            for n in &pm.part_names {
                if !names.contains(n) {
                    names.push(n.clone());
                }
            }
        }
        if let Some(tm) = &self.turnaround_manifest {
            if self.attached_turnaround_sheet.is_none() {
                self.attached_turnaround_sheet = tm.source_image.clone();
            }
            if let Some(sheet_path) = &self.attached_turnaround_sheet
                && !self
                    .attached_references
                    .iter()
                    .any(|r| r.kind == "turnaround_sheet")
            {
                let [w, h] = tm.source_size.unwrap_or([1376, 768]);
                self.attached_references.push(AttachedReference {
                    label: format!("Turnaround Sheet ({} views)", tm.frames.len()),
                    kind: "turnaround_sheet".to_string(),
                    path: sheet_path.clone(),
                    raw_path: tm.source_image.clone(),
                    width: w,
                    height: h,
                    report: None,
                });
            }
            for n in &tm.part_names {
                if !names.contains(n) {
                    names.push(n.clone());
                }
            }
        }
        let existing: HashMap<String, (bool, bool, bool)> = self
            .parts
            .iter()
            .map(|p| (p.name.clone(), (p.visible, p.solo, p.verified)))
            .collect();
        self.parts = names
            .into_iter()
            .map(|name| {
                let (visible, solo, verified) =
                    existing.get(&name).copied().unwrap_or((true, false, false));
                PartState {
                    name,
                    visible,
                    solo,
                    verified,
                }
            })
            .collect();
    }
}

impl Session {
    /// Record the result of the last executed command.
    pub fn record(&mut self, value: Value) {
        self.last_result = Some(value);
    }

    /// Return a reference to the currently active [`Asset`], if any.
    pub fn current_asset(&self) -> Option<&Asset> {
        let id = self.active_asset.as_deref()?;
        self.assets.get(id)
    }

    /// Return a mutable reference to the currently active [`Asset`], if any.
    pub fn current_asset_mut(&mut self) -> Option<&mut Asset> {
        let id = self.active_asset.clone()?;
        self.assets.get_mut(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn loads_arbitrary_layered_motion_spec_and_syncs_parts_alongside_pose_manifest() {
        let tmp = std::env::temp_dir().join(format!(
            "ac_layered_spec_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let spec_path = tmp.join("drone_spec.json");
        std::fs::write(
            &spec_path,
            json!({
                "canvas": [640, 480],
                "fps": 24,
                "layers": [
                    { "id": "rotor-l", "file": "layers/rotor-l.png", "pivot": [180, 120], "parent": "chassis" },
                    { "id": "rotor-r", "file": "layers/rotor-r.png", "pivot": [460, 120], "parent": "chassis" },
                    { "id": "chassis", "file": "layers/chassis.png", "pivot": [320, 240] }
                ],
                "tracks": [
                    { "id": "hover", "duration": 2.0, "motion": {} }
                ]
            })
            .to_string(),
        )
        .unwrap();

        let loaded = LoadedCutoutRigPlan::load_from_path(&spec_path).unwrap();
        assert_eq!(loaded.rig_type, "layered_motion");
        assert_eq!(loaded.canvas_size, [640, 480]);
        assert_eq!(loaded.parts.len(), 3);
        assert_eq!(loaded.parts[0].id, "rotor-l");
        assert_eq!(loaded.parts[0].parent.as_deref(), Some("chassis"));
        assert_eq!(loaded.parts[2].id, "chassis");
        assert_eq!(loaded.source_image, tmp.join("layers/chassis.png"));

        let mut asset = Asset {
            id: "drone".into(),
            pose_manifest: Some(LoadedManifest {
                manifest_path: tmp.join("pose_manifest.json"),
                manifest_dir: tmp.clone(),
                sheet_type: "action".into(),
                status: "loaded".into(),
                matting_engine: None,
                source_image: None,
                source_size: Some([640, 480]),
                frames: Vec::new(),
                part_names: vec!["pose_frame".into()],
            }),
            cutout_rig: Some(loaded),
            ..Asset::default()
        };
        asset.sync_parts_from_manifests();
        let part_names: Vec<&str> = asset.parts.iter().map(|p| p.name.as_str()).collect();
        assert!(part_names.contains(&"rotor-l"));
        assert!(part_names.contains(&"rotor-r"));
        assert!(part_names.contains(&"chassis"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
