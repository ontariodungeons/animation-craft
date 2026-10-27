//! Shared silhouette geometry used by the native preview and mesh export.
use crate::session::Asset;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeSettings {
    pub inflation_depth_pct: u32,
    pub relief_detail_pct: u32,
    pub mesh_res: u32,
    pub proj_blend_pct: u32,
    pub bump_intensity: u32,
}
#[derive(Clone, Debug, Default)]
pub struct MeshGeometry {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}
/// Same source priority and cleanup as mesh.build_4dir. Called only when source
/// metadata changes, never for each slider movement.
pub fn load_views(asset: &Asset) -> Vec<(String, image::RgbaImage)> {
    let mut views = Vec::new();
    if let Some(manifest) = &asset.turnaround_manifest {
        for frame in &manifest.frames {
            if let Ok(img) = image::open(&frame.resolved_path) {
                views.push((
                    frame.label.clone(),
                    crate::commands::decontaminate_sprite_rgba(&img.into_rgba8(), None).0,
                ));
            }
        }
    }
    if views.is_empty() {
        if let Some(manifest) = &asset.pose_manifest {
            for frame in &manifest.frames {
                if let Ok(img) = image::open(&frame.resolved_path) {
                    let target = asset
                        .turnaround_registration
                        .as_ref()
                        .map(|r| r.target_height_px)
                        .filter(|h| *h > 0);
                    views.push((
                        frame.label.clone(),
                        crate::commands::decontaminate_sprite_rgba(&img.into_rgba8(), target).0,
                    ));
                    break;
                }
            }
        }
    }
    if views.is_empty() {
        if let Ok(img) = image::open(&asset.path) {
            views.push((
                "front".into(),
                crate::commands::decontaminate_sprite_rgba(&img.into_rgba8(), None).0,
            ));
        }
    }
    views.retain(|(_, image)| image.pixels().any(|pixel| pixel[3] >= 96));
    views
}

pub fn build(views: &[(String, image::RgbaImage)], settings: ShapeSettings) -> MeshGeometry {
    let Some((_, fallback_img)) = views.first() else { return MeshGeometry::default(); };
    let fallback_img = fallback_img.clone();

    let find_view = |needle: &str| -> &image::RgbaImage {
        views
            .iter()
            .find(|(lbl, _)| lbl.to_ascii_lowercase().contains(needle))
            .map(|(_, img)| img)
            .unwrap_or(&fallback_img)
    };
    let front_img = find_view("front");
    let qfront_img = views
        .iter()
        .find(|(lbl, _)| lbl.to_ascii_lowercase().contains("quarter"))
        .map(|(_, img)| img)
        .unwrap_or(front_img);
    let side_img = find_view("side");
    let back_img = find_view("back");

    let ShapeSettings {
        inflation_depth_pct,
        relief_detail_pct,
        mesh_res,
        proj_blend_pct,
        bump_intensity,
    } = settings;
    let bounds_f = image_bounds(front_img);
    let bounds_q = image_bounds(qfront_img);
    let bounds_s = image_bounds(side_img);
    let bounds_b = image_bounds(back_img);
    let slices = mesh_res.clamp(12, 64) as usize;
    let rings = mesh_res.clamp(12, 64) as usize;
    let depth_scale = (inflation_depth_pct as f32 / 58.0).clamp(0.25, 1.85);
    let relief_amp =
        (relief_detail_pct as f32 / 100.0) * 0.035 + (bump_intensity as f32 / 100.0) * 0.02;
    let blend_w = (proj_blend_pct as f32 / 100.0).clamp(0.0, 1.0);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for iz in 0..=slices {
        // z_y runs strictly from 0.0 (feet planted on ground plane) to 1.0 (crown)
        let u_height = iz as f32 / slices as f32;
        let v_img = 1.0 - u_height;
        let (cx_f, rx_f, col_f) = sample_row_span(front_img, v_img, bounds_f);
        let (_cx_q, rx_q, col_q) = sample_row_span(qfront_img, v_img, bounds_q);
        let (cz_s, rz_s_raw, col_s) = sample_row_span(side_img, v_img, bounds_s);
        let (_cx_b, _rx_b, col_b) = sample_row_span(back_img, v_img, bounds_b);

        let rx = (rx_f * (1.0 - 0.25 * blend_w) + rx_q * (0.25 * blend_w)).max(0.025);
        let rz = (rz_s_raw * depth_scale).max(0.025);

        // Taper very bottom and top rings slightly for clean manifold caps while keeping z >= 0.0
        let cap_taper = if iz == 0 || iz == slices { 0.55 } else { 1.0 };

        for it in 0..=rings {
            let v_theta = it as f32 / rings as f32;
            let theta = v_theta * std::f32::consts::TAU;
            let cos_t = theta.cos();
            let sin_t = theta.sin();

            let relief =
                1.0 + relief_amp * ((3.0 * theta + u_height * std::f32::consts::TAU).cos() * 0.5);
            let x = cx_f + rx * cap_taper * relief * cos_t;
            let z = cz_s + rz * cap_taper * relief * sin_t;
            let y = u_height.max(0.0); // Ground plane strictly at y = 0.0 (glTF Y-up)

            let nx = cos_t / rx.max(0.01);
            let nz = sin_t / rz.max(0.01);
            let n_len = (nx * nx + nz * nz).sqrt().max(1e-5);

            let w_front = cos_t.max(0.0);
            let w_back = (-cos_t).max(0.0);
            let w_side = sin_t.abs();
            let w_sum = (w_front + w_back + w_side).max(1e-4);
            let r_col = (col_f[0] * w_front
                + col_b[0] * w_back
                + (col_s[0] * 0.6 + col_q[0] * 0.4) * w_side)
                / w_sum;
            let g_col = (col_f[1] * w_front
                + col_b[1] * w_back
                + (col_s[1] * 0.6 + col_q[1] * 0.4) * w_side)
                / w_sum;
            let b_col = (col_f[2] * w_front
                + col_b[2] * w_back
                + (col_s[2] * 0.6 + col_q[2] * 0.4) * w_side)
                / w_sum;

            positions.push([x, y, z]);
            normals.push([nx / n_len, 0.0, nz / n_len]);
            uvs.push([v_theta, v_img]);
            colors.push([r_col, g_col, b_col, 1.0]);
        }
    }

    let stride = (rings + 1) as u32;
    for iz in 0..slices as u32 {
        for it in 0..rings as u32 {
            let i00 = iz * stride + it;
            let i10 = (iz + 1) * stride + it;
            let i01 = iz * stride + it + 1;
            let i11 = (iz + 1) * stride + it + 1;
            indices.extend_from_slice(&[i00, i01, i11, i00, i11, i10]);
        }
    }

    // Bottom sole center cap at y = 0.0 and top crown cap at y = 1.0
    let bottom_center_idx = positions.len() as u32;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 1.0]);
    colors.push([0.35, 0.30, 0.28, 1.0]);
    for it in 0..rings as u32 {
        indices.extend_from_slice(&[bottom_center_idx, it + 1, it]);
    }

    let top_center_idx = positions.len() as u32;
    positions.push([0.0, 1.0, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.0]);
    colors.push([0.75, 0.62, 0.50, 1.0]);
    let top_ring_start = (slices as u32) * stride;
    for it in 0..rings as u32 {
        indices.extend_from_slice(&[top_center_idx, top_ring_start + it, top_ring_start + it + 1]);
    }

    MeshGeometry {
        positions,
        normals,
        uvs,
        colors,
        indices,
    }
}

fn image_bounds(img: &image::RgbaImage) -> [u32; 4] {
    let w = img.width().max(1);
    let h = img.height().max(1);
    let mut min_y = h;
    let mut max_y = 0u32;
    let mut min_x = w;
    let mut max_x = 0u32;
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] >= 96 {
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }
    [min_x, min_y, max_x, max_y]
}
fn sample_row_span(img: &image::RgbaImage, v_norm: f32, bounds: [u32; 4]) -> (f32, f32, [f32; 3]) {
    let [min_x, min_y, max_x, max_y] = bounds;
    let w = img.width();
    let h = img.height();
    if max_y <= min_y || max_x <= min_x {
        return (0.0, 0.18, [0.78, 0.62, 0.48]);
    }
    let body_h = (max_y - min_y).max(1) as f32;
    let y = (min_y as f32 + v_norm.clamp(0.0, 1.0) * body_h)
        .round()
        .clamp(min_y as f32, max_y as f32) as u32;

    let mut row_x0 = w;
    let mut row_x1 = 0u32;
    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    let mut count = 0u64;

    for dy in -1i32..=1i32 {
        let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
        for x in 0..w {
            let p = img.get_pixel(x, sy);
            if p[3] >= 96 {
                if x < row_x0 {
                    row_x0 = x;
                }
                if x > row_x1 {
                    row_x1 = x;
                }
                r_sum += u64::from(p[0]);
                g_sum += u64::from(p[1]);
                b_sum += u64::from(p[2]);
                count += 1;
            }
        }
    }
    let global_cx = 0.5 * (min_x + max_x) as f32;
    if row_x1 <= row_x0 || count == 0 {
        return (0.0, 0.06, [0.72, 0.58, 0.46]);
    }
    let row_cx = 0.5 * (row_x0 + row_x1) as f32;
    let center_offset = (row_cx - global_cx) / body_h;
    let half_span = (0.5 * (row_x1 - row_x0) as f32 / body_h).clamp(0.035, 0.48);
    let col = [
        (r_sum as f32 / count as f32) / 255.0,
        (g_sum as f32 / count as f32) / 255.0,
        (b_sum as f32 / count as f32) / 255.0,
    ];
    (center_offset, half_span, col)
}
