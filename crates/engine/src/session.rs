//! Session state for Magic Studio.
//!
//! Holds the current project/assets, loaded manifests, and verified measurements,
//! and provides [`Session::execute`] for command dispatch.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{EngineError, Result};

/// The engine session. All project changes go through [`Session::execute`].
#[derive(Debug)]
pub struct Session {
    /// Current working directory for relative asset paths.
    pub cwd: PathBuf,
    /// Loaded assets keyed by their logical id.
    pub assets: HashMap<String, Asset>,
    /// Ordered connections in this project, separate from decoded/cached assets.
    pub project_assets: Vec<ProjectAsset>,
    /// The open `.magic` file; `None` until `project.new` or `project.open`.
    pub project_path: Option<PathBuf>,
    /// Where the final video project lives (HyperFrames/Remotion/FilmCraft). Single source of truth.
    pub video_project_path: Option<PathBuf>,
    /// Opaque native Theatre save-file state; absent on projects that have never saved Theatre edits.
    pub theatre_state: Option<Value>,
    /// Editable native sources and explicitly accepted immutable render receipts.
    pub animation_sources: BTreeMap<String, Value>,
    /// User-facing sessions publish Home history and recent receipts; passive library/tests do not.
    pub record_user_state: bool,
    /// Active chat selection belongs to this engine/window, never to project serialization.
    pub agent_selection: Option<Value>,
    /// Exclusive writer lease on the canonical project file for the live Studio session.
    pub project_owner: Option<std::sync::Arc<std::fs::File>>,
    /// The selected recording; uploads use the copy inside the contained HyperFrames project.
    pub song_audio: Option<PathBuf>,
    /// Whether the user approved the workshop draft PDF.
    pub draft_approved: bool,
    /// The contained HyperFrames video project.
    pub video_target: String,
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
    /// Project-file writes; `write_project_file` takes `&Session`, hence the cell.
    pub project_saves: std::sync::atomic::AtomicU64,
    /// Successful journaled commands since the project file was last written.
    pub edits_since_save: u64,
}

impl Default for Session {
    fn default() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            cwd,
            assets: HashMap::new(),
            project_assets: Vec::new(),
            project_path: None,
            video_project_path: None,
            theatre_state: None,
            animation_sources: BTreeMap::new(),
            record_user_state: false,
            agent_selection: None,
            project_owner: None,
            song_audio: None,
            draft_approved: false,
            video_target: "hyperframes".into(),
            active_asset: None,
            preview_mode: "walk".to_string(),
            preview_speed: 1.0,
            preview_range: (0, 72),
            last_result: None,
            blender_available: false,
            project_saves: std::sync::atomic::AtomicU64::new(0),
            edits_since_save: 0,
        }
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            cwd: self.cwd.clone(),
            assets: self.assets.clone(),
            project_assets: self.project_assets.clone(),
            project_path: self.project_path.clone(),
            video_project_path: self.video_project_path.clone(),
            theatre_state: self.theatre_state.clone(),
            animation_sources: self.animation_sources.clone(),
            record_user_state: self.record_user_state,
            agent_selection: self.agent_selection.clone(),
            project_owner: self.project_owner.clone(),
            song_audio: self.song_audio.clone(),
            draft_approved: self.draft_approved,
            video_target: self.video_target.clone(),
            active_asset: self.active_asset.clone(),
            preview_mode: self.preview_mode.clone(),
            preview_speed: self.preview_speed,
            preview_range: self.preview_range,
            last_result: self.last_result.clone(),
            blender_available: self.blender_available,
            project_saves: std::sync::atomic::AtomicU64::new(self.project_saves.load(std::sync::atomic::Ordering::Relaxed)),
            edits_since_save: self.edits_since_save,
        }
    }
}

impl Session {
    pub fn user_session()->Self{Self{record_user_state:true,..Self::default()}}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAsset {
    pub id: String,
    pub name: String,
    pub preset: Option<String>,
    pub path: Option<PathBuf>,
    /// `auto`, `image`, `poses`, `turnaround`, or `cutout_rig`.
    pub kind: String,
    /// Part visibility / solo / review state as last saved; applied when the asset loads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartState>,
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
    #[serde(default)]
    pub control_bindings: crate::control_bindings::ControlBindings,
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

fn load_control_bindings(raw: &Value, plan: &Path, parts: &[CutoutRigPart]) -> Result<crate::control_bindings::ControlBindings> {
    let mut bindings: crate::control_bindings::ControlBindings = serde_json::from_value(
        raw.get("control_bindings").cloned().unwrap_or_else(|| serde_json::json!({}))
    ).map_err(|e| EngineError::InvalidParams(format!("invalid control_bindings: {e}")))?;
    // Older plans explicitly declare a semantic kind. Preserve their face preview
    // only when there is one declared head; never infer it from the filename or position.
    if raw.get("control_bindings").is_none() {
        let mut heads = parts.iter().filter(|part| part.kind == "head");
        if let Some(head) = heads.next() {
            if heads.next().is_none() {
                bindings.face = Some(crate::control_bindings::FaceBinding { part: head.id.clone(), landmarks: None });
            }
        }
    }
    bindings.validate(&parts.iter().map(|p| p.id.clone()).collect::<Vec<_>>())
        .map_err(EngineError::InvalidParams)?;
    if let Some(face) = &mut bindings.face {
        if let Some(path) = &mut face.landmarks {
            if path.is_relative() { *path = plan.parent().unwrap_or(Path::new(".")).join(&*path); }
            if !path.is_file() { return Err(EngineError::InvalidParams(format!("face landmarks file does not exist: {}", path.display()))); }
        }
    }
    Ok(bindings)
}

impl LoadedCutoutRigPlan {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let raw: Value = serde_json::from_str(&text).map_err(|e| {
            EngineError::InvalidParams(format!("invalid cutout rig plan JSON: {e}"))
        })?;
        if let Some(layers)=raw.get("layers").and_then(Value::as_object){return Self::load_keyed_layers(path,&raw,layers);}
        if let (Some(canvas_arr), Some(layers_arr)) = (
            raw.get("canvas").and_then(Value::as_array),
            raw.get("layers").and_then(Value::as_array),
        ) {
            return Self::load_layered_motion_spec(path, &raw, canvas_arr, layers_arr);
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
        let control_bindings = load_control_bindings(&raw, path, &parts)?;
        Ok(Self {
            control_bindings,
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

    pub fn resolve_paths(&mut self, base: &Path) {
        let resolve = |p: &Path| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                base.join(p)
            }
        };
        if !self.source_image.as_os_str().is_empty() {
            self.source_image = resolve(&self.source_image);
        }
        self.cutout_asset_sheet = self.cutout_asset_sheet.as_deref().map(resolve);
        self.photocraft_psd = self.photocraft_psd.as_deref().map(resolve);
        for part in &mut self.parts {
            part.cropped_path = resolve(&part.cropped_path);
            part.full_layer_path = resolve(&part.full_layer_path);
        }
    }

    /// Older HyperFrames rigs store cropped PNGs as an explicitly ordered layer map.
    fn load_keyed_layers(path:&Path,raw:&Value,layers:&serde_json::Map<String,Value>)->Result<Self>{
        let invalid=|message:String|EngineError::InvalidParams(format!("Invalid keyed rig {}: {message}",path.display()));
        let canvas=raw["canvas"].as_array().filter(|v|v.len()==2).ok_or_else(||invalid("canvas must contain two positive integers".into()))?;
        let dims:Vec<u32>=canvas.iter().map(|v|v.as_u64().filter(|n|*n>0&&*n<=8192).map(|n|n as u32).ok_or_else(||invalid("canvas values must be 1..8192".into()))).collect::<Result<_>>()?;let(cw,ch)=(dims[0],dims[1]);
        let order=raw["layer_order"].as_array().filter(|v|!v.is_empty()).ok_or_else(||invalid("nonempty layer_order is required".into()))?;
        let ids:Vec<&str>=order.iter().map(|v|v.as_str().ok_or_else(||invalid("layer_order entries must be IDs".into()))).collect::<Result<_>>()?;
        let unique:std::collections::BTreeSet<_>=ids.iter().copied().collect();
        if layers.is_empty()||unique.len()!=ids.len()||unique.len()!=layers.len()||layers.keys().any(|k|!unique.contains(k.as_str())){return Err(invalid("layer_order must contain each nonempty layer ID exactly once".into()));}
        let base=path.parent().ok_or_else(||invalid("manifest directory missing".into()))?;let mut decoded=Vec::new();
        for id in &ids{
            if id.is_empty()||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b"_-".contains(&b)){return Err(invalid(format!("unsafe layer ID {id}")));}
            let layer=&layers[*id];let b=layer["box"].as_array().filter(|v|v.len()==4).ok_or_else(||invalid(format!("{id}: box must be [x,y,width,height]")))?;
            let b:Vec<u32>=b.iter().map(|v|v.as_u64().filter(|n|*n<=8192).map(|n|n as u32).ok_or_else(||invalid(format!("{id}: box requires nonnegative integers")))).collect::<Result<_>>()?;
            if b[2]==0||b[3]==0||b[0]+b[2]>cw||b[1]+b[3]>ch{return Err(invalid(format!("{id}: box exceeds canvas or has zero size")));}
            let p=layer["pivot"].as_array().filter(|v|v.len()==2).ok_or_else(||invalid(format!("{id}: pivot must be [x,y]")))?;
            let p:Vec<f32>=p.iter().map(|v|v.as_f64().filter(|n|n.is_finite()).map(|n|n as f32).ok_or_else(||invalid(format!("{id}: pivot must be finite")))).collect::<Result<_>>()?;
            if p[0]<0.0||p[1]<0.0||p[0]>cw as f32||p[1]>ch as f32{return Err(invalid(format!("{id}: pivot lies outside canvas")));}
            let file=base.join(format!("{id}.png"));let image=image::open(&file).map_err(|e|invalid(format!("{id}: cannot decode {}: {e}",file.display())))?;
            if image.width()!=b[2]||image.height()!=b[3]||!image.color().has_alpha(){return Err(invalid(format!("{id}: PNG dimensions/alpha do not match declared box {}x{}",b[2],b[3])));}
            decoded.push((id.to_string(),file,b,p,image.into_rgba8()));
        }
        // Derived canvas layers preserve every original PNG byte and declared placement.
        let cache=base.join(".layered_preview_cache/keyed");std::fs::create_dir_all(&cache)?;
        let mut rest=image::RgbaImage::new(cw,ch);let mut parts=Vec::new();let mut bbox=[cw,ch,0,0];let mut spec_layers=Vec::new();
        for (index,(id,file,b,p,pixels)) in decoded.into_iter().enumerate(){
            let mut full=image::RgbaImage::new(cw,ch);image::imageops::overlay(&mut full,&pixels,b[0] as i64,b[1] as i64);let full_path=cache.join(format!("{id}.png"));full.save(&full_path)?;
            image::imageops::overlay(&mut rest,&pixels,b[0] as i64,b[1] as i64);bbox=[bbox[0].min(b[0]),bbox[1].min(b[1]),bbox[2].max(b[0]+b[2]),bbox[3].max(b[1]+b[3])];
            spec_layers.push(serde_json::json!({"id":id,"file":full_path,"pivot":p}));
            parts.push(CutoutRigPart{id:id.clone(),label:id,kind:"layered".into(),layer:index as i32,parent:None,pivot:[p[0],p[1]],pivot_uv:[(p[0]-b[0] as f32)/b[2] as f32,(p[1]-b[1] as f32)/b[3] as f32],crop_box:[b[0],b[1],b[0]+b[2],b[1]+b[3]],crop_size:[b[2],b[3]],cropped_path:file,full_layer_path:full_path,..Default::default()});
        }
        let source=cache.join("assembled-rest.png");rest.save(&source)?;
        let controls=load_control_bindings(raw,path,&parts)?;
        Ok(Self{control_bindings:controls,plan_path:path.into(),character_id:raw["fish"].as_str().or(raw["character_id"].as_str()).unwrap_or("keyed_subject").into(),rig_type:"layered_motion".into(),source_image:source,canvas_size:[cw,ch],character_bbox:bbox,parts,effectcraft:serde_json::json!({"canvas":[cw,ch],"fps":30,"layers":spec_layers,"tracks":[]}),..Default::default()})
    }

    fn load_layered_motion_spec(
        path: &Path,
        raw: &Value,
        canvas_arr: &[Value],
        layers_arr: &[Value],
    ) -> Result<Self> {
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
        let cache_dir = base_dir.join(".layered_preview_cache").join(&character_id);
        let _ = std::fs::create_dir_all(&cache_dir);

        let mut parts = Vec::with_capacity(layers_arr.len());
        let mut root_file: Option<PathBuf> = None;
        let mut union_bbox: Option<[u32; 4]> = None;
        let mut composite_canvas = image::RgbaImage::new(cw, ch);
        let mut has_composite_pixels = false;

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

            let component_idx = item.get("component").and_then(Value::as_u64).map(|v| v as usize);
            let mut crop_box = [0, 0, cw, ch];
            let mut alpha_box = [0, 0, cw, ch];
            let mut cropped_path = file_path.clone();

            if file_path.is_file()
                && let Ok(dyn_img) = image::open(&file_path)
            {
                let mut rgba = dyn_img.into_rgba8();
                if let Some(comp_i) = component_idx {
                    isolate_connected_component_left_to_right(&mut rgba, comp_i);
                }
                if let Some(ab) = compute_alpha_bounds(&rgba, 16) {
                    alpha_box = ab;
                    let pad = 2u32;
                    let bx0 = ab[0].saturating_sub(pad);
                    let by0 = ab[1].saturating_sub(pad);
                    let bx1 = (ab[2] + pad).min(rgba.width());
                    let by1 = (ab[3] + pad).min(rgba.height());
                    crop_box = [bx0, by0, bx1.max(bx0 + 1), by1.max(by0 + 1)];
                    union_bbox = Some(match union_bbox {
                        Some(u) => [
                            u[0].min(crop_box[0]),
                            u[1].min(crop_box[1]),
                            u[2].max(crop_box[2]),
                            u[3].max(crop_box[3]),
                        ],
                        None => crop_box,
                    });

                    // Composite rest-pose visible layers (skip blink-closed overlay layers)
                    if !id.ends_with("closed") && rgba.width() == cw && rgba.height() == ch {
                        for y in crop_box[1]..crop_box[3] {
                            for x in crop_box[0]..crop_box[2] {
                                let src_p = *rgba.get_pixel(x, y);
                                let sa = src_p[3] as f32 / 255.0;
                                if sa > 0.004 {
                                    let dst_p = *composite_canvas.get_pixel(x, y);
                                    let da = dst_p[3] as f32 / 255.0;
                                    let out_a = sa + da * (1.0 - sa);
                                    let blend = |sc: u8, dc: u8| -> u8 {
                                        (((sc as f32 * sa) + (dc as f32 * da * (1.0 - sa)))
                                            / out_a.max(1e-5))
                                        .round()
                                        .clamp(0.0, 255.0)
                                            as u8
                                    };
                                    composite_canvas.put_pixel(
                                        x,
                                        y,
                                        image::Rgba([
                                            blend(src_p[0], dst_p[0]),
                                            blend(src_p[1], dst_p[1]),
                                            blend(src_p[2], dst_p[2]),
                                            (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
                                        ]),
                                    );
                                    has_composite_pixels = true;
                                }
                            }
                        }
                    }

                    let cw_crop = crop_box[2] - crop_box[0];
                    let ch_crop = crop_box[3] - crop_box[1];
                    let sub = image::imageops::crop_imm(&rgba, crop_box[0], crop_box[1], cw_crop, ch_crop).to_image();
                    let safe_id: String = id
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                        .collect();
                    let dest = cache_dir.join(format!("{idx:02}_{safe_id}_crop.png"));
                    if sub.save(&dest).is_ok() {
                        cropped_path = dest;
                    }
                }
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
            } else if let Some(piv_obj) = item.get("pivot").and_then(Value::as_object) {
                let anchor = piv_obj
                    .get("anchor")
                    .and_then(Value::as_str)
                    .unwrap_or("center");
                let x0 = alpha_box[0] as f32;
                let y0 = alpha_box[1] as f32;
                let x1 = alpha_box[2] as f32;
                let y1 = alpha_box[3] as f32;
                let cx = 0.5 * (x0 + x1);
                let cy = 0.5 * (y0 + y1);
                match anchor {
                    "top" => [cx, y0],
                    "bottom" => [cx, y1],
                    "left" => [x0, cy],
                    "right" => [x1, cy],
                    "topLeft" => [x0, y0],
                    "topRight" => [x1, y0],
                    "bottomLeft" => [x0, y1],
                    "bottomRight" => [x1, y1],
                    _ => [cx, cy],
                }
            } else {
                [
                    0.5 * (alpha_box[0] + alpha_box[2]) as f32,
                    0.5 * (alpha_box[1] + alpha_box[3]) as f32,
                ]
            };

            let crop_size = [
                (crop_box[2] - crop_box[0]).max(1),
                (crop_box[3] - crop_box[1]).max(1),
            ];
            let pivot_uv = [
                ((pivot[0] - crop_box[0] as f32) / crop_size[0] as f32).clamp(0.0, 1.0),
                ((pivot[1] - crop_box[1] as f32) / crop_size[1] as f32).clamp(0.0, 1.0),
            ];

            parts.push(CutoutRigPart {
                id: id.clone(),
                label: id,
                kind: "layered".to_string(),
                layer: (idx as i32) + 1,
                parent,
                pivot,
                pivot_uv,
                joint_radius_px: 10,
                completed_overlap_pixels: 0,
                rotation: [-60.0, 60.0],
                crop_box,
                crop_size,
                cropped_path,
                full_layer_path: file_path,
            });
        }

        let character_bbox = union_bbox.unwrap_or([0, 0, cw, ch]);
        let mut source_image = root_file
            .or_else(|| parts.first().map(|p| p.full_layer_path.clone()))
            .unwrap_or_default();

        let mut cutout_asset_sheet = None;
        if has_composite_pixels {
            let pad = 12u32;
            let cx0 = character_bbox[0].saturating_sub(pad);
            let cy0 = character_bbox[1].saturating_sub(pad);
            let cx1 = (character_bbox[2] + pad).min(cw);
            let cy1 = (character_bbox[3] + pad).min(ch);
            if cx1 > cx0 && cy1 > cy0 {
                let comp_crop =
                    image::imageops::crop_imm(&composite_canvas, cx0, cy0, cx1 - cx0, cy1 - cy0)
                        .to_image();
                let comp_dest = cache_dir.join(format!("{character_id}_composite.png"));
                if comp_crop.save(&comp_dest).is_ok() {
                    source_image = comp_dest;
                }
                let sheet_dest = cache_dir.join(format!("{character_id}_layer_contact_sheet.png"));
                if render_layer_contact_sheet(&comp_crop, [cx0, cy0], &parts, &sheet_dest) {
                    cutout_asset_sheet = Some(sheet_dest);
                }
            }
        }

        let control_bindings = load_control_bindings(raw, path, &parts)?;
        Ok(Self {
            control_bindings,
            plan_path: path.to_path_buf(),
            character_id,
            rig_type: "layered_motion".to_string(),
            source_image,
            canvas_size: [cw, ch],
            character_bbox,
            photocraft_psd: None,
            cutout_asset_sheet,
            parts,
            pose_library: Value::Null,
            effectcraft: raw.clone(),
        })
    }
}

fn render_layer_contact_sheet(
    comp_crop: &image::RgbaImage,
    comp_origin: [u32; 2],
    parts: &[CutoutRigPart],
    dest: &Path,
) -> bool {
    let sw = 1440u32;
    let sh = 900u32;
    let mut sheet = image::RgbaImage::from_pixel(sw, sh, image::Rgba([20, 24, 32, 255]));

    let draw_rect_border =
        |img: &mut image::RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, col: [u8; 4]| {
            let x1 = x1.min(img.width().saturating_sub(1));
            let y1 = y1.min(img.height().saturating_sub(1));
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            for x in x0..=x1 {
                img.put_pixel(x, y0, image::Rgba(col));
                img.put_pixel(x, y1, image::Rgba(col));
            }
            for y in y0..=y1 {
                img.put_pixel(x0, y, image::Rgba(col));
                img.put_pixel(x1, y, image::Rgba(col));
            }
        };

    let draw_pivot_dot = |img: &mut image::RgbaImage, px: i32, py: i32| {
        for dy in -5i32..=5 {
            for dx in -5i32..=5 {
                let x = px + dx;
                let y = py + dy;
                if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                    let d2 = dx * dx + dy * dy;
                    if d2 <= 16 {
                        img.put_pixel(x as u32, y as u32, image::Rgba([255, 85, 85, 255]));
                    } else if d2 <= 25 {
                        img.put_pixel(x as u32, y as u32, image::Rgba([255, 220, 90, 255]));
                    }
                }
            }
        }
    };

    // 1. Left composite overview panel (x: 24..504, y: 24..876)
    let lx0 = 24u32;
    let ly0 = 24u32;
    let lw = 480u32;
    let lh = 852u32;
    for y in ly0..(ly0 + lh) {
        for x in lx0..(lx0 + lw) {
            sheet.put_pixel(x, y, image::Rgba([28, 34, 46, 255]));
        }
    }
    draw_rect_border(&mut sheet, lx0, ly0, lx0 + lw - 1, ly0 + lh - 1, [68, 84, 112, 255]);

    let cw = comp_crop.width().max(1) as f32;
    let ch = comp_crop.height().max(1) as f32;
    let comp_scale = ((lw - 48) as f32 / cw).min((lh - 48) as f32 / ch).clamp(0.1, 2.5);
    let dw = (cw * comp_scale).round().max(1.0) as u32;
    let dh = (ch * comp_scale).round().max(1.0) as u32;
    let resized_comp =
        image::imageops::resize(comp_crop, dw, dh, image::imageops::FilterType::Triangle);
    let ox = lx0 + (lw.saturating_sub(dw)) / 2;
    let oy = ly0 + (lh.saturating_sub(dh)) / 2;
    image::imageops::overlay(&mut sheet, &resized_comp, ox as i64, oy as i64);

    for part in parts {
        let bx0 = ox as i32
            + (((part.crop_box[0] as i32 - comp_origin[0] as i32) as f32) * comp_scale).round()
                as i32;
        let by0 = oy as i32
            + (((part.crop_box[1] as i32 - comp_origin[1] as i32) as f32) * comp_scale).round()
                as i32;
        let bx1 = ox as i32
            + (((part.crop_box[2] as i32 - comp_origin[0] as i32) as f32) * comp_scale).round()
                as i32;
        let by1 = oy as i32
            + (((part.crop_box[3] as i32 - comp_origin[1] as i32) as f32) * comp_scale).round()
                as i32;
        draw_rect_border(
            &mut sheet,
            bx0.max(lx0 as i32) as u32,
            by0.max(ly0 as i32) as u32,
            bx1.max(0) as u32,
            by1.max(0) as u32,
            [80, 190, 245, 210],
        );
        let px = ox as i32 + ((part.pivot[0] - comp_origin[0] as f32) * comp_scale).round() as i32;
        let py = oy as i32 + ((part.pivot[1] - comp_origin[1] as f32) * comp_scale).round() as i32;
        draw_pivot_dot(&mut sheet, px, py);
    }

    // 2. Right grid of individual layer cards (x: 524..1416, y: 24..876)
    let gx0 = 524u32;
    let gy0 = 24u32;
    let gw = 892u32;
    let gh = 852u32;
    let cols = 3u32;
    let rows = ((parts.len() as u32).div_ceil(cols)).max(1);
    let pad = 12u32;
    let cell_w = (gw.saturating_sub(pad * (cols - 1))) / cols;
    let cell_h = (gh.saturating_sub(pad * (rows - 1))) / rows;

    for (idx, part) in parts.iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;
        let cx0 = gx0 + col * (cell_w + pad);
        let cy0 = gy0 + row * (cell_h + pad);
        for y in cy0..(cy0 + cell_h).min(sh) {
            for x in cx0..(cx0 + cell_w).min(sw) {
                sheet.put_pixel(x, y, image::Rgba([30, 36, 48, 255]));
            }
        }
        draw_rect_border(
            &mut sheet,
            cx0,
            cy0,
            cx0 + cell_w - 1,
            cy0 + cell_h - 1,
            [72, 92, 124, 255],
        );
        if let Ok(pimg) = image::open(&part.cropped_path) {
            let prgba = pimg.to_rgba8();
            let pw = prgba.width().max(1) as f32;
            let ph = prgba.height().max(1) as f32;
            let pscale = (((cell_w.saturating_sub(28)) as f32 / pw)
                .min((cell_h.saturating_sub(28)) as f32 / ph))
            .clamp(0.1, 2.2);
            let tdw = (pw * pscale).round().max(1.0) as u32;
            let tdh = (ph * pscale).round().max(1.0) as u32;
            let resized_part =
                image::imageops::resize(&prgba, tdw, tdh, image::imageops::FilterType::Triangle);
            let pox = cx0 + (cell_w.saturating_sub(tdw)) / 2;
            let poy = cy0 + (cell_h.saturating_sub(tdh)) / 2;
            image::imageops::overlay(&mut sheet, &resized_part, pox as i64, poy as i64);
            let piv_x = pox as i32 + (part.pivot_uv[0] * tdw as f32).round() as i32;
            let piv_y = poy as i32 + (part.pivot_uv[1] * tdh as f32).round() as i32;
            draw_pivot_dot(&mut sheet, piv_x, piv_y);
        }
    }

    sheet.save(dest).is_ok()
}

pub(crate) fn compute_alpha_bounds(rgba: &image::RgbaImage, threshold: u8) -> Option<[u32; 4]> {
    let w = rgba.width();
    let h = rgba.height();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;
    for y in 0..h {
        for x in 0..w {
            if rgba.get_pixel(x, y)[3] > threshold {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + 1);
                max_y = max_y.max(y + 1);
                found = true;
            }
        }
    }
    found.then_some([min_x, min_y, max_x, max_y])
}

fn isolate_connected_component_left_to_right(rgba: &mut image::RgbaImage, index: usize) {
    let w = rgba.width() as usize;
    let h = rgba.height() as usize;
    if w == 0 || h == 0 {
        return;
    }
    let mut labels = vec![0u32; w * h];
    let mut comp_areas: Vec<(u32, usize, usize)> = Vec::new(); // (label, area, min_x)
    let mut current_label = 0u32;
    let mut stack = Vec::new();

    for sy in 0..h {
        for sx in 0..w {
            let idx = sy * w + sx;
            if labels[idx] != 0 || rgba.get_pixel(sx as u32, sy as u32)[3] <= 16 {
                continue;
            }
            current_label += 1;
            labels[idx] = current_label;
            stack.push((sx, sy));
            let mut area = 0usize;
            let mut min_x = sx;
            while let Some((cx, cy)) = stack.pop() {
                area += 1;
                min_x = min_x.min(cx);
                let y_start = cy.saturating_sub(1);
                let y_end = (cy + 1).min(h - 1);
                let x_start = cx.saturating_sub(1);
                let x_end = (cx + 1).min(w - 1);
                for ny in y_start..=y_end {
                    for nx in x_start..=x_end {
                        let nidx = ny * w + nx;
                        if labels[nidx] == 0 && rgba.get_pixel(nx as u32, ny as u32)[3] > 16 {
                            labels[nidx] = current_label;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
            comp_areas.push((current_label, area, min_x));
        }
    }

    let max_area = comp_areas.iter().map(|c| c.1).max().unwrap_or(0);
    if max_area == 0 {
        return;
    }
    let min_area = if max_area < 50 {
        max_area
    } else {
        50.max((max_area as f64 * 0.02) as usize)
    };
    let mut valid: Vec<(u32, usize)> = comp_areas
        .into_iter()
        .filter(|c| c.1 >= min_area)
        .map(|(lbl, _, min_x)| (lbl, min_x))
        .collect();
    valid.sort_by_key(|c| c.1);
    let Some(&(keep_label, _)) = valid.get(index) else {
        return;
    };
    for y in 0..h {
        for x in 0..w {
            if labels[y * w + x] != keep_label {
                rgba.put_pixel(x as u32, y as u32, image::Rgba([0, 0, 0, 0]));
            }
        }
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
    #[serde(default)]
    pub qa_issues: Vec<String>,
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
                    qa_issues: item.pointer("/qa/issues").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default(),
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
            let mut parsed_puppet_parts = BTreeMap::new();
            for (k, pval) in puppet_parts {
                if !part_set.contains(k) {
                    part_set.push(k.clone());
                }
                let pfile = pval
                    .get("file")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if pfile.is_empty() {
                    continue;
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
                parsed_puppet_parts.insert(
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
            if !parsed_puppet_parts.is_empty() {
                for frame in &mut frames {
                    if frame.parts.is_empty() {
                        frame.parts = parsed_puppet_parts.clone();
                    }
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
                    qa_issues: item.pointer("/qa/issues").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default(),
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
