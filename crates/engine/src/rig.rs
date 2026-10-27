//! 4-direction visual-hull mesh build and GLB/OBJ serialization.

use std::path::PathBuf;

use image::RgbaImage;
use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

use crate::photocraft::{cmd_asset_load_manifest, decontaminate_sprite_rgba};
use crate::workspace::workspace_root;

/// Run `mesh.build_4dir`: decontaminate turnaround/pose frames, build a visual-hull mesh, and write GLB/OBJ/ingest JSON.
pub fn cmd_mesh_build_4dir(s: &mut Session, params: &Value) -> Result<Value> {
    let preset_arg = params
        .get("preset")
        .or_else(|| params.get("character"))
        .or_else(|| params.get("asset"))
        .and_then(Value::as_str)
        .map(str::to_string);

    if preset_arg.is_none() && s.active_asset.is_none() {
        return Err(EngineError::NotImplemented);
    }

    if let Some(ref preset) = preset_arg
        && !s.assets.contains_key(preset)
    {
        let _ = cmd_asset_load_manifest(s, &json!({ "preset": preset }));
    }

    let asset_id = preset_arg
        .or_else(|| s.active_asset.clone())
        .ok_or(EngineError::NotImplemented)?;

    let asset = s
        .assets
        .get(&asset_id)
        .cloned()
        .ok_or_else(|| EngineError::InvalidParams(format!("asset `{asset_id}` not loaded")))?;

    let recon_mode = params
        .get("recon_mode")
        .and_then(Value::as_str)
        .unwrap_or("volumetric");
    let inflation_depth_pct = params
        .get("inflation_depth_pct")
        .and_then(Value::as_u64)
        .map(|v| v.clamp(10, 100) as u32)
        .unwrap_or(58);
    let relief_detail_pct = params
        .get("relief_detail_pct")
        .and_then(Value::as_u64)
        .map(|v| v.clamp(0, 100) as u32)
        .unwrap_or(44);
    let mesh_res = params
        .get("mesh_res")
        .and_then(Value::as_u64)
        .map(|v| v.clamp(12, 64) as u32)
        .unwrap_or(32);
    let proj_blend_pct = params
        .get("proj_blend_pct")
        .and_then(Value::as_u64)
        .map(|v| v.clamp(0, 100) as u32)
        .unwrap_or(65);
    let bump_intensity = params
        .get("bump_intensity")
        .and_then(Value::as_u64)
        .map(|v| v.clamp(0, 100) as u32)
        .unwrap_or(35);

    let root = workspace_root(s);
    let ingest_dir = root.join("exports/blender_ingest").join(&asset_id);
    let turnaround_out_dir = ingest_dir.join("turnaround");
    let poses_out_dir = ingest_dir.join("poses");
    std::fs::create_dir_all(&turnaround_out_dir)?;
    std::fs::create_dir_all(&poses_out_dir)?;

    let mut clean_turnaround_imgs: Vec<(String, PathBuf, RgbaImage)> = Vec::new();
    let mut target_ref_height: Option<u32> = asset
        .turnaround_registration
        .as_ref()
        .map(|r| r.target_height_px)
        .filter(|&h| h > 0);

    if let Some(tm) = &asset.turnaround_manifest {
        for frame in &tm.frames {
            if let Ok(dyn_img) = image::open(&frame.resolved_path) {
                let rgba = dyn_img.into_rgba8();
                if target_ref_height.is_none() {
                    target_ref_height = Some(rgba.height());
                }
                let (clean_img, _bbox) = decontaminate_sprite_rgba(&rgba, None);
                let out_file = turnaround_out_dir.join(format!(
                    "{:02}_{}.png",
                    frame.index,
                    super::workspace::sanitize_asset_id(&frame.label)
                ));
                clean_img
                    .save(&out_file)
                    .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;
                clean_turnaround_imgs.push((frame.label.clone(), out_file, clean_img));
            }
        }
    }

    let mut clean_pose_files: Vec<Value> = Vec::new();
    if let Some(pm) = &asset.pose_manifest {
        for frame in &pm.frames {
            if let Ok(dyn_img) = image::open(&frame.resolved_path) {
                let rgba = dyn_img.into_rgba8();
                let (clean_img, bbox) = decontaminate_sprite_rgba(&rgba, target_ref_height);
                let out_file = poses_out_dir.join(format!(
                    "{:02}_{}.png",
                    frame.index,
                    super::workspace::sanitize_asset_id(&frame.label)
                ));
                clean_img
                    .save(&out_file)
                    .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;
                if clean_turnaround_imgs.is_empty() {
                    clean_turnaround_imgs.push((
                        frame.label.clone(),
                        out_file.clone(),
                        clean_img.clone(),
                    ));
                }
                clean_pose_files.push(json!({
                    "index": frame.index,
                    "label": frame.label,
                    "path": out_file.to_string_lossy(),
                    "size": [clean_img.width(), clean_img.height()],
                    "bbox": bbox,
                }));
            }
        }
    }

    if clean_turnaround_imgs.is_empty()
        && asset.path.is_file()
        && let Ok(dyn_img) = image::open(&asset.path)
    {
        let (clean_img, _) = decontaminate_sprite_rgba(&dyn_img.into_rgba8(), None);
        let out_file = turnaround_out_dir.join("00_front.png");
        clean_img
            .save(&out_file)
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;
        clean_turnaround_imgs.push(("front".to_string(), out_file, clean_img));
    }

    let views: Vec<_> = clean_turnaround_imgs.iter()
        .filter(|(_, _, image)| image.pixels().any(|pixel| pixel[3] >= 96))
        .map(|(label, _, img)| (label.clone(), img.clone())).collect();
    if views.is_empty() {
        return Err(EngineError::InvalidParams("No readable foreground artwork is available to build a mesh".into()));
    }
    let crate::mesh_geometry::MeshGeometry { positions, normals, uvs, colors, indices } =
        crate::mesh_geometry::build(&views, crate::mesh_geometry::ShapeSettings {
            inflation_depth_pct, relief_detail_pct, mesh_res, proj_blend_pct, bump_intensity,
        });

    let default_glb = ingest_dir.join(format!("{asset_id}_4dir.glb"));
    let out_glb = params
        .get("output")
        .or_else(|| params.get("out_glb"))
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p))
        .unwrap_or(default_glb);
    let out_gltf = out_glb.with_extension("gltf");
    let out_obj = out_glb.with_extension("obj");
    let out_mtl = out_glb.with_extension("mtl");
    let out_ingest_json = ingest_dir.join(format!("{asset_id}_blender_ingest.json"));

    write_binary_glb(&out_glb, &positions, &normals, &uvs, &colors, &indices)?;
    write_wavefront_obj(&out_obj, &positions, &normals, &uvs, &indices)?;

    let turnaround_entries: Vec<Value> = clean_turnaround_imgs
        .iter()
        .enumerate()
        .map(|(idx, (lbl, path, img))| {
            json!({
                "index": idx,
                "label": lbl,
                "path": path.to_string_lossy(),
                "size": [img.width(), img.height()],
            })
        })
        .collect();

    let foot_lift_max_px = asset
        .motion_range
        .as_ref()
        .map(|m| m.foot_lift_max_px)
        .unwrap_or(20.0);
    let max_vertical_drift_px = asset
        .turnaround_registration
        .as_ref()
        .map(|r| r.max_vertical_drift_px)
        .unwrap_or(0.0);

    let ingest_payload = json!({
        "ok": true,
        "character": asset_id,
        "recon_mode": recon_mode,
        "reconstruction_params": {
            "inflation_depth_pct": inflation_depth_pct,
            "relief_detail_pct": relief_detail_pct,
            "mesh_res": mesh_res,
            "proj_blend_pct": proj_blend_pct,
            "bump_intensity": bump_intensity,
        },
        "mesh": {
            "glb": out_glb.to_string_lossy(),
            "gltf": out_gltf.to_string_lossy(),
            "obj": out_obj.to_string_lossy(),
            "mtl": out_mtl.to_string_lossy(),
            "vertex_count": positions.len(),
            "triangle_count": indices.len() / 3,
            "ground_plane_min_y": 0.0,
        },
        "joint_landmarks_ratio": {
            "ankle_z": 0.07,
            "knee_z": 0.24,
            "hip_z": 0.44,
            "waist_z": 0.54,
            "shoulder_z": 0.69,
            "elbow_z": 0.56,
            "neck_z": 0.74,
            "head_top_z": 1.0,
        },
        "measured_telemetry": {
            "foot_lift_max_px": foot_lift_max_px,
            "max_vertical_drift_px": max_vertical_drift_px,
            "target_reference_height_px": target_ref_height.unwrap_or(520),
        },
        "decontaminated_turnaround_views": turnaround_entries,
        "decontaminated_pose_frames": clean_pose_files,
        "verified_parts": asset.parts,
    });
    std::fs::write(
        &out_ingest_json,
        serde_json::to_string_pretty(&ingest_payload).unwrap_or_default(),
    )?;

    Ok(json!({
        "ok": true,
        "command": "mesh.build_4dir",
        "asset_id": asset_id,
        "recon_mode": recon_mode,
        "glb": out_glb.to_string_lossy(),
        "gltf": out_gltf.to_string_lossy(),
        "obj": out_obj.to_string_lossy(),
        "mtl": out_mtl.to_string_lossy(),
        "blender_ingest_json": out_ingest_json.to_string_lossy(),
        "vertex_count": positions.len(),
        "triangle_count": indices.len() / 3,
        "decontaminated_turnaround_count": turnaround_entries.len(),
        "decontaminated_pose_count": clean_pose_files.len(),
        "foot_lift_max_px": foot_lift_max_px,
        "max_vertical_drift_px": max_vertical_drift_px,
    }))
}

/// Write a binary glTF 2.0 (`.glb`) plus an embedded-JSON `.gltf` sidecar.
fn write_binary_glb(
    path: &Path,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    colors: &[[f32; 4]],
    indices: &[u32],
) -> Result<()> {
    let mut bin: Vec<u8> = Vec::new();
    let mut min_pos = [f32::INFINITY; 3];
    let mut max_pos = [f32::NEG_INFINITY; 3];

    let pos_offset = bin.len();
    for p in positions {
        for i in 0..3 {
            if p[i] < min_pos[i] {
                min_pos[i] = p[i];
            }
            if p[i] > max_pos[i] {
                max_pos[i] = p[i];
            }
            bin.extend_from_slice(&p[i].to_le_bytes());
        }
    }
    let pos_len = bin.len() - pos_offset;

    let norm_offset = bin.len();
    for n in normals {
        for v in n {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    let norm_len = bin.len() - norm_offset;

    let uv_offset = bin.len();
    for uv in uvs {
        for v in uv {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    let uv_len = bin.len() - uv_offset;

    let col_offset = bin.len();
    for c in colors {
        for v in c {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    let col_len = bin.len() - col_offset;

    let idx_offset = bin.len();
    for idx in indices {
        bin.extend_from_slice(&idx.to_le_bytes());
    }
    let idx_len = bin.len() - idx_offset;

    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }

    let gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "Magic Studio Rust 4-Dir Visual Hull Builder"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": "CharacterVisualHull", "mesh": 0 }],
        "meshes": [{
            "name": "CharacterVisualHullMesh",
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1,
                    "TEXCOORD_0": 2,
                    "COLOR_0": 3
                },
                "indices": 4,
                "mode": 4
            }]
        }],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": positions.len(),
                "type": "VEC3",
                "min": min_pos,
                "max": max_pos
            },
            {
                "bufferView": 1,
                "componentType": 5126,
                "count": normals.len(),
                "type": "VEC3"
            },
            {
                "bufferView": 2,
                "componentType": 5126,
                "count": uvs.len(),
                "type": "VEC2"
            },
            {
                "bufferView": 3,
                "componentType": 5126,
                "count": colors.len(),
                "type": "VEC4"
            },
            {
                "bufferView": 4,
                "componentType": 5125,
                "count": indices.len(),
                "type": "SCALAR"
            }
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": pos_offset, "byteLength": pos_len, "target": 34962 },
            { "buffer": 0, "byteOffset": norm_offset, "byteLength": norm_len, "target": 34962 },
            { "buffer": 0, "byteOffset": uv_offset, "byteLength": uv_len, "target": 34962 },
            { "buffer": 0, "byteOffset": col_offset, "byteLength": col_len, "target": 34962 },
            { "buffer": 0, "byteOffset": idx_offset, "byteLength": idx_len, "target": 34963 }
        ],
        "buffers": [{ "byteLength": bin.len() }]
    });

    let mut json_bytes = serde_json::to_vec(&gltf_json)
        .map_err(|e| EngineError::InvalidParams(format!("glTF JSON serialization failed: {e}")))?;
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut glb: Vec<u8> = Vec::with_capacity(total_len);
    glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // "glTF"
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());

    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    glb.extend_from_slice(&json_bytes);

    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    glb.extend_from_slice(&bin);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, glb)?;

    let gltf_path = path.with_extension("gltf");
    let mut ascii_gltf = gltf_json;
    if let Some(buffers) = ascii_gltf.get_mut("buffers").and_then(Value::as_array_mut)
        && let Some(first_buf) = buffers.first_mut().and_then(Value::as_object_mut)
    {
        first_buf.insert(
            "uri".to_string(),
            json!(format!(
                "data:application/octet-stream;base64,{}",
                encode_base64(&bin)
            )),
        );
    }
    let ascii_str = serde_json::to_string_pretty(&ascii_gltf)
        .map_err(|e| EngineError::InvalidParams(format!("glTF ASCII serialization failed: {e}")))?;
    std::fs::write(gltf_path, ascii_str)?;
    Ok(())
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Write a Wavefront `.obj` + `.mtl` sidecar.
fn write_wavefront_obj(
    path: &Path,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Result<()> {
    let mtl_path = path.with_extension("mtl");
    let mtl_filename = mtl_path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| "character.mtl".to_string());
    let mut out = format!(
        "# Magic Studio 4-Dir Visual Hull OBJ\nmtllib {mtl_filename}\no CharacterVisualHull\nusemtl MAT_Character_Felt\n"
    );
    for p in positions {
        out.push_str(&format!("v {:.5} {:.5} {:.5}\n", p[0], p[1], p[2]));
    }
    for uv in uvs {
        out.push_str(&format!("vt {:.5} {:.5}\n", uv[0], uv[1]));
    }
    for n in normals {
        out.push_str(&format!("vn {:.5} {:.5} {:.5}\n", n[0], n[1], n[2]));
    }
    for tri in indices.chunks_exact(3) {
        let i0 = tri[0] + 1;
        let i1 = tri[1] + 1;
        let i2 = tri[2] + 1;
        out.push_str(&format!("f {i0}/{i0}/{i0} {i1}/{i1}/{i1} {i2}/{i2}/{i2}\n"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, out)?;
    let mtl_content = "# Magic Studio Wavefront Material Library (.MTL)\nnewmtl MAT_Character_Felt\nKd 0.9451 0.8745 0.7804\nKs 0.0800 0.0800 0.0800\nNs 30.00\nd 1.0\n";
    std::fs::write(mtl_path, mtl_content)?;
    Ok(())
}

use std::path::Path;
