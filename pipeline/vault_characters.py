"""
Registry, Artifact-Free Turnaround Slicer, Edge Color Decontaminator, and
Anatomical Part Extractor for the user's real character images in:
  /home/jesse/main-vault/Creative/Design/Characters/
  /home/jesse/main-vault/Creative/Design/Characters/3d poses/
"""
from collections import deque
import os
from typing import Dict, Any, List, Tuple
import numpy as np
from PIL import Image, ImageFilter

VAULT_ROOT = "/home/jesse/main-vault/Creative/Design/Characters"
POSES_DIR = os.path.join(VAULT_ROOT, "3d poses")

VAULT_CHARACTER_CATALOG: Dict[str, Dict[str, Any]] = {
    "hopper": {
        "id": "hopper",
        "name": "Hopper (Cream Rabbit Student)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Hopper-no-background.png_202607180130.jpeg"),
        "turnaround": os.path.join(POSES_DIR, "Character_turnaround_sheet_anima…_202607180130_5.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Create_pose_sheet_for_puppet_202607180130_15.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide4.PNG_202607180130.jpeg"),
    },
    "whiskers": {
        "id": "whiskers",
        "name": "Whiskers (Grey Tabby Kitten)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Whiskers-no-back.png_202607180130.jpeg"),
        "turnaround": os.path.join(POSES_DIR, "Character_turnaround_sheet_anima…_202607180130_17.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Create_pose_sheet_for_puppet_202607180130_20.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide5.PNG_202607180130.jpeg"),
    },
    "jenny": {
        "id": "jenny",
        "name": "Jenny / Penny (Yellow Chick)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Penny.jpeg"),
        "turnaround": os.path.join(POSES_DIR, "Character_turnaround_sheet_anima…_202607180130_9.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Create_pose_sheet_for_puppet_202607180130_3.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide7.PNG_202607180130.jpeg"),
    },
    "puddles": {
        "id": "puddles",
        "name": "Puddles (Yellow Duckling)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Puddles-tranparent.png_202607180130.jpeg"),
        "turnaround": os.path.join(POSES_DIR, "Character_turnaround_sheet_anima…_202607180130_12.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Create_pose_sheet_for_puppet_202607180130_13.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide9.PNG_202607180130.jpeg"),
    },
    "scout": {
        "id": "scout",
        "name": "Scout (Explorer Puppy)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Scout-no-back.png_202607180130.jpeg"),
        "turnaround": os.path.join(VAULT_ROOT, "Scout-no-back.png_202607180130.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Create_pose_sheet_for_puppet_202607180130_5.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide6.PNG_202607180130.jpeg"),
    },
    "rusty": {
        "id": "rusty",
        "name": "Rusty (Brown Foal)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Rusty-transparent.png_202607180130.jpeg"),
        "turnaround": os.path.join(POSES_DIR, "Character_turnaround_sheet_anima…_202607180130_19.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Create_pose_sheet_for_puppet_202607180130_24.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide11.PNG_202607180130.jpeg"),
    },
    "sam": {
        "id": "sam",
        "name": "Sam (Pink Piglet)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Sam.png_202607180130.jpeg"),
        "turnaround": os.path.join(POSES_DIR, "Character_turnaround_sheet_anima…_202607180130_21.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Create_pose_sheet_for_puppet_202607180130_33.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide10.PNG_202607180130.jpeg"),
    },
    "maisy": {
        "id": "maisy",
        "name": "Maisy (Black & White Calf)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Slide8.PNG_202607180130.jpeg"),
        "turnaround": os.path.join(POSES_DIR, "Character_turnaround_sheet_anima…_202607180130_29.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Character_turnaround_sheet_anima…_202607180130_29.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide8.PNG_202607180130.jpeg"),
    },
    "mr_puddles": {
        "id": "mr_puddles",
        "name": "Mr Puddles (Adult Duck)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Mr Puddles.png"),
        "turnaround": os.path.join(VAULT_ROOT, "Mr Puddles.png"),
        "poseSheet": os.path.join(VAULT_ROOT, "Mr Puddles.png"),
        "slide": os.path.join(VAULT_ROOT, "Mr Puddles.png"),
    },
    "old_mac": {
        "id": "old_mac",
        "name": "Old Mac (Farmer Teacher)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "old_mac-tranparent.png_202607180130.jpeg"),
        "turnaround": os.path.join(VAULT_ROOT, "old_mac-tranparent.png_202607180130.jpeg"),
        "poseSheet": os.path.join(POSES_DIR, "Create_pose_sheet_for_puppet_202607180130_25.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Slide3.PNG_202607180130.jpeg"),
    },
    "miss_hayley": {
        "id": "miss_hayley",
        "name": "Miss Hayley (Teacher)",
        "archetype": "humanoid_biped",
        "cutout": os.path.join(VAULT_ROOT, "Miss Hayley.png"),
        "turnaround": os.path.join(VAULT_ROOT, "Miss Hayley.png"),
        "poseSheet": os.path.join(VAULT_ROOT, "Hayley-Draft.jpeg"),
        "slide": os.path.join(VAULT_ROOT, "Miss Hayley.png"),
    },
}


def resolve_real_vault_image(image_or_name: str, prefer: str = "turnaround") -> str:
    if not image_or_name or not str(image_or_name).strip():
        raise FileNotFoundError("No character ID or source image path provided")

    if os.path.exists(image_or_name):
        return os.path.abspath(image_or_name)

    low = str(image_or_name).lower().strip()
    if low in VAULT_CHARACTER_CATALOG:
        entry = VAULT_CHARACTER_CATALOG[low]
        candidate = entry.get(prefer) or entry.get("cutout") or entry.get("turnaround")
        if candidate and os.path.exists(candidate):
            return candidate

    for key, entry in VAULT_CHARACTER_CATALOG.items():
        if key in low or low in key:
            candidate = entry.get(prefer) or entry.get("cutout") or entry.get("turnaround")
            if candidate and os.path.exists(candidate):
                return candidate

    for root, _, files in os.walk(VAULT_ROOT):
        for f in files:
            if low.replace("_", " ") in f.lower() or low in f.lower():
                return os.path.join(root, f)

    raise FileNotFoundError(f"Could not resolve source image or character: {image_or_name!r}")


def _keep_largest_connected_components(mask: np.ndarray, min_area_ratio: float = 0.002, max_components: int = 5) -> np.ndarray:
    """Removes floating background crumbs/specks while keeping the main character island(s)."""
    h, w = mask.shape
    visited = np.zeros((h, w), dtype=bool)
    min_pixels = max(24, int(h * w * min_area_ratio))
    components: List[List[Tuple[int, int]]] = []

    ys, xs = np.where(mask)
    for sy, sx in zip(ys, xs):
        if visited[sy, sx]:
            continue
        comp = []
        q = deque([(sy, sx)])
        visited[sy, sx] = True
        while q:
            cy, cx = q.popleft()
            comp.append((cy, cx))
            for ny, nx in ((cy - 1, cx), (cy + 1, cx), (cy, cx - 1), (cy, cx + 1)):
                if 0 <= ny < h and 0 <= nx < w and not visited[ny, nx] and mask[ny, nx]:
                    visited[ny, nx] = True
                    q.append((ny, nx))
        if len(comp) >= min_pixels:
            components.append(comp)

    components.sort(key=len, reverse=True)
    clean_mask = np.zeros((h, w), dtype=bool)
    for comp in components[:max_components]:
        for cy, cx in comp:
            clean_mask[cy, cx] = True
    return clean_mask


def decontaminate_and_dilate_rgba(rgba_arr: np.ndarray, erode_px: int = 1, dilate_uv_px: int = 10) -> Tuple[Image.Image, Image.Image]:
    """
    1. Erodes contaminated 1px white/grey studio halo on silhouette boundary.
    2. Replaces boundary ring RGB with nearest clean interior character RGB (color decontamination).
    3. Produces both a clean alpha cutout AND a UV-dilated texture map so 3D mesh edges never sample white background!
    """
    h, w, _ = rgba_arr.shape
    fg = rgba_arr[:, :, 3] > 128

    # Remove tiny isolated specks
    fg = _keep_largest_connected_components(fg, min_area_ratio=0.0015, max_components=6)

    # Compute distance inward from background (how deep inside the character each pixel is)
    inward_dist = np.zeros((h, w), dtype=np.int16)
    q = deque()
    for y in range(h):
        for x in range(w):
            if not fg[y, x]:
                inward_dist[y, x] = 0
                q.append((y, x))
            else:
                inward_dist[y, x] = 9999

    while q:
        cy, cx = q.popleft()
        nd = inward_dist[cy, cx] + 1
        for ny, nx in ((cy - 1, cx), (cy + 1, cx), (cy, cx - 1), (cy, cx + 1)):
            if 0 <= ny < h and 0 <= nx < w and inward_dist[ny, nx] > nd:
                inward_dist[ny, nx] = nd
                q.append((ny, nx))

    # Core interior pixels (at least erode_px + 4 inside the silhouette) have zero background color bleed
    core_depth = max(3, erode_px + 4)
    core_mask = inward_dist >= core_depth
    if np.sum(core_mask) < 50:
        core_mask = inward_dist >= 2
    if np.sum(core_mask) < 50:
        core_mask = fg

    # Propagate clean core RGB outward to both boundary ring (1..2px) and exterior UV bleed zone (up to dilate_uv_px)
    clean_rgb = rgba_arr[:, :, :3].copy()
    out_dist = np.full((h, w), 9999, dtype=np.int16)
    oq = deque()
    cy_list, cx_list = np.where(core_mask)
    for cy, cx in zip(cy_list, cx_list):
        out_dist[cy, cx] = 0
        oq.append((cy, cx))

    max_prop = max(4, dilate_uv_px + 3)
    while oq:
        cy, cx = oq.popleft()
        nd = out_dist[cy, cx] + 1
        if nd > max_prop:
            continue
        src_color = clean_rgb[cy, cx]
        for ny, nx in ((cy - 1, cx), (cy + 1, cx), (cy, cx - 1), (cy, cx + 1)):
            if 0 <= ny < h and 0 <= nx < w and out_dist[ny, nx] > nd:
                out_dist[ny, nx] = nd
                clean_rgb[ny, nx] = src_color
                oq.append((ny, nx))

    # Build clean anti-aliased alpha channel (erode 1px of background fringe, smooth 1px ramp)
    eroded_fg = inward_dist > erode_px
    alpha = np.zeros((h, w), dtype=np.uint8)
    alpha[inward_dist == (erode_px + 1)] = 185
    alpha[inward_dist >= (erode_px + 2)] = 255

    # Clear 2px outer frame to guarantee zero border lines
    alpha[:2, :] = 0
    alpha[-2:, :] = 0
    alpha[:, :2] = 0
    alpha[:, -2:] = 0

    clean_rgba = np.dstack([clean_rgb, alpha]).astype(np.uint8)
    # For UV projection in 3D, keep dilated RGB with full alpha in the dilation band so UV seams never show white
    uv_alpha = np.where(out_dist <= dilate_uv_px, 255, alpha).astype(np.uint8)
    dilated_rgba = np.dstack([clean_rgb, uv_alpha]).astype(np.uint8)

    return Image.fromarray(clean_rgba, mode="RGBA"), Image.fromarray(dilated_rgba, mode="RGBA")


def border_flood_fill_rgba(
    img: Image.Image,
    tolerance: int = 22,
    step_tol: int = 9,
    erode_px: int = 1,
) -> Image.Image:
    """
    Adaptive background flood-fill with local background surface estimation,
    Sobel/texture barrier protection for cream fur, bottom caption strip,
    and 3px edge color decontamination.
    """
    rgba = img.convert("RGBA")
    arr = np.array(rgba, dtype=np.int16)
    h, w, _ = arr.shape

    # If image already has transparent background
    if arr[0, 0, 3] < 20 and arr[0, w - 1, 3] < 20:
        clean_img, _ = decontaminate_and_dilate_rgba(np.array(rgba, dtype=np.uint8), erode_px=erode_px)
        return clean_img

    # Estimate per-column background color from top border strip (rows 2..12) to handle studio lighting vignette
    top_strip = arr[2:min(14, h // 10), :, :3]
    col_bg = np.median(top_strip, axis=0)  # shape (w, 3)

    # Distance from each pixel to its column's studio background color
    diff = np.max(np.abs(arr[:, :, :3] - col_bg[np.newaxis, :, :]), axis=2)

    # Compute local 3x3 color saturation/variance barrier so flood-fill doesn't eat into rim-lit cream wool cheeks
    rgb_max = np.max(arr[:, :, :3], axis=2)
    rgb_min = np.min(arr[:, :, :3], axis=2)
    chroma = rgb_max - rgb_min
    bg_chroma = float(np.median(chroma[2:12, :]))

    bg_candidates = (diff <= tolerance) & ((chroma - bg_chroma) <= 14)

    visited = np.zeros((h, w), dtype=bool)
    q = deque()

    # Strip bottom 7.5% caption band ("EXPORT NAME • OMFS_... 05") if aspect >= 1.45
    caption_y = int(h * 0.915) if (w / max(1, h)) >= 1.45 else h - 2

    for x in range(w):
        if bg_candidates[0, x]:
            visited[0, x] = True
            q.append((0, x))
        for by in range(caption_y, h):
            visited[by, x] = True
            if bg_candidates[by, x]:
                q.append((by, x))
    for y in range(h):
        if bg_candidates[y, 0] and not visited[y, 0]:
            visited[y, 0] = True
            q.append((y, 0))
        if bg_candidates[y, w - 1] and not visited[y, w - 1]:
            visited[y, w - 1] = True
            q.append((y, w - 1))

    while q:
        cy, cx = q.popleft()
        cur_rgb = arr[cy, cx, :3]
        for ny, nx in ((cy - 1, cx), (cy + 1, cx), (cy, cx - 1), (cy, cx + 1)):
            if 0 <= ny < h and 0 <= nx < w and not visited[ny, nx] and bg_candidates[ny, nx]:
                step_diff = int(np.max(np.abs(arr[ny, nx, :3] - cur_rgb)))
                if step_diff <= step_tol:
                    visited[ny, nx] = True
                    q.append((ny, nx))

    out = np.array(rgba, dtype=np.uint8)
    out[visited, 3] = 0
    # Also clear enclosed background pockets trapped between touching toes/feet in the lower leg band (68%..92% height)
    leg_y0, leg_y1 = int(h * 0.68), caption_y
    trapped_bg = (diff[leg_y0:leg_y1, :] <= 12) & ((chroma[leg_y0:leg_y1, :] - bg_chroma) <= 5)
    out[leg_y0:leg_y1, :][trapped_bg, 3] = 0
    clean_img, _ = decontaminate_and_dilate_rgba(out, erode_px=erode_px)
    return clean_img


def crop_to_alpha_bbox(rgba_img: Image.Image, pad: int = 12) -> Image.Image:
    arr = np.array(rgba_img, dtype=np.uint8)
    ys, xs = np.where(arr[:, :, 3] > 25)
    if len(xs) == 0 or len(ys) == 0:
        return rgba_img
    x0 = max(0, int(xs.min()) - pad)
    y0 = max(0, int(ys.min()) - pad)
    x1 = min(rgba_img.width, int(xs.max()) + 1 + pad)
    y1 = min(rgba_img.height, int(ys.max()) + 1 + pad)
    cropped = rgba_img.crop((x0, y0, x1, y1))
    c_arr = np.array(cropped, dtype=np.uint8)
    c_arr[:2, :, 3] = 0
    c_arr[-2:, :, 3] = 0
    c_arr[:, :2, 3] = 0
    c_arr[:, -2:, 3] = 0
    return Image.fromarray(c_arr, mode="RGBA")


def find_horizontal_islands(rgba_band: Image.Image | np.ndarray, min_width_px: int = 40) -> List[Tuple[int, int]]:
    if isinstance(rgba_band, np.ndarray):
        alpha = rgba_band.astype(np.uint8) * 255 if rgba_band.dtype == bool else (rgba_band[:, :, 3] if rgba_band.ndim == 3 else rgba_band)
    else:
        alpha = np.array(rgba_band)[:, :, 3]
    h, w = alpha.shape
    # Measure horizontal column occupancy across torso/head (12%..72% height) so floor shadows between feet never merge two poses!
    torso_band = alpha[int(h * 0.12):int(h * 0.72), :]
    col_fg = np.sum(torso_band > 30, axis=0)
    islands: List[Tuple[int, int]] = []
    in_isl = False
    x0 = 0
    for x in range(w):
        if col_fg[x] >= 6:
            if not in_isl:
                in_isl = True
                x0 = x
        elif in_isl:
            if x - x0 >= min_width_px:
                islands.append((max(0, x0 - 8), min(w, x + 8)))
            in_isl = False
    if in_isl and (w - x0 >= min_width_px):
        islands.append((max(0, x0 - 8), w))

    # If any island is wider than 33% of sheet width (two characters touching at wing tips), split at local minimum near midpoint
    final_islands: List[Tuple[int, int]] = []
    for ix0, ix1 in islands:
        if (ix1 - ix0) > int(w * 0.33):
            mid_start = ix0 + int((ix1 - ix0) * 0.35)
            mid_end = ix0 + int((ix1 - ix0) * 0.65)
            split_x = mid_start + int(np.argmin(col_fg[mid_start:mid_end]))
            final_islands.append((ix0, split_x))
            final_islands.append((split_x, ix1))
        else:
            final_islands.append((ix0, ix1))
    return final_islands


def extract_turnaround_depth_profile(side_rgba: Image.Image, num_rows: int = 48) -> List[Dict[str, float]]:
    """
    Measures the exact Front-to-Back depth profile (min_x..max_x at each height row)
    from the 90° Side Profile turnaround slice so 3D reconstruction matches the character's
    real snout/beak, belly, backpack, and tail depth!
    """
    arr = np.array(side_rgba.convert("RGBA"))
    alpha = arr[:, :, 3] > 60
    h, w = alpha.shape
    profile = []
    for r in range(num_rows):
        y0 = int(r * h / num_rows)
        y1 = max(y0 + 1, int((r + 1) * h / num_rows))
        row_xs = np.where(np.any(alpha[y0:y1, :], axis=0))[0]
        if len(row_xs) > 2:
            min_u = float(row_xs.min()) / max(1, w - 1)
            max_u = float(row_xs.max()) / max(1, w - 1)
            span = max(0.12, max_u - min_u)
            center = 0.5 * (min_u + max_u)
        else:
            span = 0.28
            center = 0.5
        profile.append({"v": 1.0 - (r / max(1, num_rows - 1)), "depthSpan": round(span, 4), "depthCenter": round(center, 4)})
    return profile


def slice_real_character_image(
    image_path: str,
    out_dir: str,
    tolerance: int = 22,
    step_tol: int = 9,
    erode_px: int = 1,
) -> Dict[str, Any]:
    os.makedirs(out_dir, exist_ok=True)
    real_path = resolve_real_vault_image(image_path)
    src = Image.open(real_path)
    w, h = src.size
    aspect = w / max(1, h)

    rgba_sheet = border_flood_fill_rgba(src, tolerance=tolerance, step_tol=step_tol, erode_px=erode_px)
    clean_sheet_path = os.path.join(out_dir, "clean_turnaround_sheet.png")
    rgba_sheet.save(clean_sheet_path)

    views_paths: List[str] = []
    dilated_views_paths: List[str] = []
    sheet_type = "single_cutout"

    if aspect >= 1.45:
        lower_name = os.path.basename(real_path).lower()
        if "pose_sheet" in lower_name or "create_pose_sheet" in lower_name:
            sheet_type = "pose_sheet"
            top_y = int(h * 0.08)
            bot_y = int(h * 0.49)
        else:
            sheet_type = "turnaround_5view"
            top_y = int(h * 0.08)
            bot_y = int(h * 0.915)

        band = rgba_sheet.crop((0, top_y, w, bot_y))
        islands = find_horizontal_islands(band, min_width_px=int(w * 0.06))
        if len(islands) >= 3:
            raw_cells = [band.crop((ix0, 0, ix1, band.height)) for (ix0, ix1) in islands]
        else:
            cols = 5
            col_w = w // cols
            raw_cells = [band.crop((i * col_w, 0, (i + 1) * col_w, band.height)) for i in range(cols)]

        # Normalize all 5 turnaround views to the SAME canvas height & baseline so Front/Side/Back align 1:1 in 3D!
        cropped_cells = [crop_to_alpha_bbox(c, pad=10) for c in raw_cells]
        target_h = max(c.height for c in cropped_cells)
        target_w = max(c.width for c in cropped_cells)

        for i, c in enumerate(cropped_cells):
            aligned = Image.new("RGBA", (target_w, target_h), (0, 0, 0, 0))
            ox = (target_w - c.width) // 2
            # Align feet to the bottom baseline (target_h - c.height)
            oy = target_h - c.height
            aligned.paste(c, (ox, oy))
            clean_aligned, dilated_aligned = decontaminate_and_dilate_rgba(np.array(aligned), erode_px=erode_px, dilate_uv_px=12)

            p = os.path.join(out_dir, f"view_{i}.png")
            dp = os.path.join(out_dir, f"view_{i}_dilated.png")
            clean_aligned.save(p)
            dilated_aligned.save(dp)
            views_paths.append(p)
            dilated_views_paths.append(dp)
    else:
        rgba_full = crop_to_alpha_bbox(rgba_sheet, pad=12)
        clean_aligned, dilated_aligned = decontaminate_and_dilate_rgba(np.array(rgba_full), erode_px=0, dilate_uv_px=12)
        p = os.path.join(out_dir, "view_0.png")
        dp = os.path.join(out_dir, "view_0_dilated.png")
        clean_aligned.save(p)
        dilated_aligned.save(dp)
        views_paths.append(p)
        dilated_views_paths.append(dp)

    hero_rgba = Image.open(views_paths[0]).convert("RGBA")
    hero_path = os.path.join(out_dir, "hero_full.png")
    hero_rgba.save(hero_path)

    front_view_path = views_paths[0]
    qtr_front_path = views_paths[1] if len(views_paths) > 1 else views_paths[0]
    side_view_path = views_paths[2] if len(views_paths) > 2 else views_paths[0]
    qtr_back_path = views_paths[3] if len(views_paths) > 3 else side_view_path
    back_view_path = views_paths[4] if len(views_paths) > 4 else views_paths[-1]

    side_rgba = Image.open(side_view_path).convert("RGBA")
    depth_profile = extract_turnaround_depth_profile(side_rgba, num_rows=48)

    hw, hh = hero_rgba.size

    def save_region(name: str, box: Tuple[float, float, float, float]) -> str:
        x0 = max(0, int(hw * box[0]))
        y0 = max(0, int(hh * box[1]))
        x1 = min(hw, int(hw * box[2]))
        y1 = min(hh, int(hh * box[3]))
        part = hero_rgba.crop((x0, y0, x1, y1))
        out_p = os.path.join(out_dir, f"part_{name}.png")
        part.save(out_p)
        return out_p

    parts = {
        "head_ears": save_region("head_ears", (0.04, 0.0, 0.96, 0.52)),
        "torso": save_region("torso", (0.20, 0.44, 0.80, 0.76)),
        "arm_l": save_region("arm_l", (0.0, 0.44, 0.28, 0.74)),
        "arm_r": save_region("arm_r", (0.72, 0.44, 1.0, 0.74)),
        "leg_upper_l": save_region("leg_upper_l", (0.14, 0.68, 0.50, 0.86)),
        "leg_shin_l": save_region("leg_shin_l", (0.10, 0.82, 0.50, 1.0)),
        "leg_upper_r": save_region("leg_upper_r", (0.50, 0.68, 0.86, 0.86)),
        "leg_shin_r": save_region("leg_shin_r", (0.50, 0.82, 0.90, 1.0)),
    }

    return {
        "sourceImage": real_path,
        "cleanSheetPng": clean_sheet_path,
        "sheetType": sheet_type,
        "heroFullPng": hero_path,
        "views": views_paths,
        "dilatedViews": dilated_views_paths,
        "turnaroundViews": {
            "front": front_view_path,
            "qtrFront": qtr_front_path,
            "side": side_view_path,
            "qtrBack": qtr_back_path,
            "back": back_view_path,
            "frontDilated": dilated_views_paths[0],
            "sideDilated": dilated_views_paths[2] if len(dilated_views_paths) > 2 else dilated_views_paths[0],
            "backDilated": dilated_views_paths[4] if len(dilated_views_paths) > 4 else dilated_views_paths[-1],
        },
        "depthProfile": depth_profile,
        "parts": parts,
        "heroSize": {"width": hw, "height": hh},
    }
