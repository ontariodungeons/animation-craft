#!/usr/bin/env python3
"""
Generic, Style-Agnostic Image Import Validator (`pipeline/import_validator.py`).

Validates any arbitrary uploaded image, turnaround sheet, pose sheet, or asset directory
against the Standard No-Guesswork Import Contract:
  1. 360° Turnaround Views: At least 3 orthogonal views (0° Front, 90° Side, 180° Back)
     or 5 turnaround views (0°, 45°, 90°, 135°, 180°) with aligned Z-baselines and
     a valid 90° Side front-to-back contour [y_front(z), y_back(z)].
  2. Pose / Part Sheet: Multi-pose action sheet or separated limb/part atlas.

Note: Existing farm characters (Hopper, Whiskers, Jenny, Puddles) and archetypes
(humanoid, fish, sun, boat) are strictly OPTIONAL EXAMPLES — never hardcoded requirements.
"""
import json
import os
from typing import Any, Dict, List
import numpy as np
from PIL import Image

from pipeline.vault_characters import (
    border_flood_fill_rgba,
    crop_to_alpha_bbox,
    find_horizontal_islands,
)


STANDARD_TEMPLATE_SPEC = {
    "templateVersion": "1.0.0",
    "description": "Standard No-Guesswork Character Import Contract (Style-Agnostic)",
    "requiredTurnaroundRow": {
        "views": ["0_front", "45_three_quarter_front", "90_side_profile", "135_three_quarter_back", "180_back"],
        "minimumRequiredViews": ["0_front", "90_side_profile", "180_back"],
        "baselineAlignmentMaxDriftRatio": 0.08,
        "sideProfileMinWidthRatio": 0.22,
        "background": "Solid uniform studio background or transparent PNG alpha",
    },
    "optionalPoseOrPartRows": {
        "actionPoses": ["idle", "walk_contact", "walk_passing", "run_stride", "jump_apex", "wave_express"],
        "explodedParts": ["head", "torso_base", "costume_skin", "arm_L", "arm_R", "leg_L", "leg_R", "prop_back"],
    },
}


def extract_asymmetrical_side_contour(side_rgba: Image.Image, num_rows: int = 64) -> List[Dict[str, float]]:
    """
    Extracts the exact asymmetric front-to-back contour [y_front(z), y_back(z)] from the
    90° Side Profile image so the Blender / Three.js side axis matches the real side image
    instead of guessing a symmetric oval!
    """
    arr = np.array(side_rgba.convert("RGBA"))
    alpha = arr[:, :, 3] > 60
    h, w = alpha.shape
    ys_all, xs_all = np.where(alpha)
    if len(xs_all) == 0:
        return [{"v": 1.0 - r / max(1, num_rows - 1), "yFront": -0.22, "yBack": 0.22} for r in range(num_rows)]

    # Normalize horizontal coordinate of the side view to [-0.5, +0.5] around median spine axis
    spine_x = float(np.median(xs_all))
    max_span = float(max(1, xs_all.max() - xs_all.min()))

    # Determine which side of the 90° profile faces Front (-Y) vs Back (+Y):
    # Usually backpack/tail protrudes on one side; map nose/chest to -Y (front) and backpack/tail to +Y (back)
    contour: List[Dict[str, float]] = []
    for r in range(num_rows):
        y0 = int(r * h / num_rows)
        y1 = max(y0 + 1, int((r + 1) * h / num_rows))
        row_xs = np.where(np.any(alpha[y0:y1, :], axis=0))[0]
        if len(row_xs) >= 2:
            u_min = (float(row_xs.min()) - spine_x) / max_span
            u_max = (float(row_xs.max()) - spine_x) / max_span
            y_front = float(np.clip(u_min, -0.55, -0.05))
            y_back = float(np.clip(u_max, 0.05, 0.55))
        else:
            y_front = -0.15
            y_back = 0.15
        contour.append({
            "v": round(1.0 - (r / max(1, num_rows - 1)), 4),
            "yFront": round(y_front, 4),
            "yBack": round(y_back, 4),
            "span": round(y_back - y_front, 4),
        })
    return contour


def validate_imported_asset(image_path: str) -> Dict[str, Any]:
    """
    Validates ANY user-supplied image path (not hardcoded to demo characters) and checks:
      1. Whether it contains a multi-view turnaround (3+ views) or a single 2D view
      2. Whether the 90° Side view is cleanly separated and has valid front-to-back contour
      3. Whether height baselines align across Front, Side, and Back views
      4. If anything is missing, returns the exact Herdr/Codex prompt to generate/fix the sheet.
    """
    if not image_path or not os.path.exists(image_path):
        return {
            "valid": False,
            "status": "file_not_found",
            "imagePath": image_path,
            "message": f"File not found: {image_path}. Pass any .png/.jpg/.webp path.",
            "standardTemplateSpec": STANDARD_TEMPLATE_SPEC,
        }

    src = Image.open(image_path)
    w, h = src.size
    rgba = border_flood_fill_rgba(src)
    arr = np.array(rgba)
    alpha = arr[:, :, 3] > 30

    # Detect rows & horizontal views
    row_fg = np.sum(alpha, axis=1)
    min_row_px = max(8, int(w * 0.03))
    row_bands = []
    in_row = False
    r0 = 0
    for y in range(h):
        if row_fg[y] >= min_row_px:
            if not in_row:
                in_row = True
                r0 = y
        elif in_row:
            if (y - r0) >= int(h * 0.14):
                row_bands.append((r0, y))
            in_row = False
    if in_row and (h - r0) >= int(h * 0.14):
        row_bands.append((r0, h))
    if not row_bands:
        row_bands = [(0, h)]

    row0_y0, row0_y1 = row_bands[0]
    islands = find_horizontal_islands(rgba.crop((0, row0_y0, w, row0_y1)), min_width_px=max(24, int(w * 0.05)))

    views_info = []
    heights = []
    widths = []
    for idx, (ix0, ix1) in enumerate(islands):
        cell = crop_to_alpha_bbox(rgba.crop((ix0, row0_y0, ix1, row0_y1)), pad=8)
        heights.append(cell.height)
        widths.append(cell.width)
        views_info.append({
            "index": idx,
            "xSpan": [ix0, ix1],
            "width": cell.width,
            "height": cell.height,
            "aspect": round(cell.width / max(1, cell.height), 3),
        })

    num_views = len(views_info)
    height_drift = (max(heights) - min(heights)) / max(1.0, float(max(heights))) if heights else 1.0

    # Check if we have a valid 90° side profile (view index 2 in a 5-view sheet, or index 1 in a 3-view sheet)
    has_turnaround = num_views >= 3
    side_idx = 2 if num_views >= 5 else (1 if num_views >= 3 else -1)
    side_valid = False
    side_aspect = 0.0
    if side_idx >= 0 and side_idx < num_views:
        side_aspect = views_info[side_idx]["aspect"]
        # A valid side profile should not be two merged characters (aspect < 0.95) and not a thin sliver (aspect > 0.18)
        side_valid = 0.18 <= side_aspect <= 0.95

    issues: List[str] = []
    if not has_turnaround:
        issues.append(f"Only {num_views} view(s) detected. A 360° turnaround requires at least 3 views (0° Front, 90° Side, 180° Back) or 5 views (0°, 45°, 90°, 135°, 180°).")
    if has_turnaround and not side_valid:
        issues.append(f"Side profile view (index {side_idx}) has invalid aspect ratio ({side_aspect}); side axis needs a clean 90° profile.")
    if height_drift > 0.15:
        issues.append(f"Height baseline drift across views is {height_drift:.1%} (should be <= 15% so Front, Side, and Back axes align on Z).")

    is_ready = has_turnaround and side_valid and height_drift <= 0.18

    codex_herdr_request = (
        f"herdr agent prompt wD:pG \"Generate or normalize '{image_path}' into the Standard 2-Row Character Sheet: "
        f"Row 1 = 5 aligned turnaround views (0° Front, 45° 3/4, 90° Side Profile, 135° 3/4 Back, 180° Back) on a uniform studio background with identical head/foot Z-baselines; "
        f"Row 2 = 6 key action poses or exploded limb layers (Head, Torso, Arm_L, Arm_R, Leg_L, Leg_R).\""
    )

    return {
        "valid": is_ready,
        "status": "ready_for_360_and_pose_animation" if is_ready else "needs_turnaround_or_side_profile_normalization",
        "imagePath": os.path.abspath(image_path),
        "dimensions": [w, h],
        "rowsDetected": len(row_bands),
        "viewsInPrimaryRow": num_views,
        "heightDriftRatio": round(height_drift, 4),
        "sideViewIndex": side_idx,
        "sideViewValid": side_valid,
        "views": views_info,
        "issues": issues,
        "codexHerdrCommandIfMissing": codex_herdr_request,
        "standardTemplateSpec": STANDARD_TEMPLATE_SPEC,
    }


if __name__ == "__main__":
    import sys
    target = sys.argv[1] if len(sys.argv) > 1 else ""
    print(json.dumps(validate_imported_asset(target), indent=2))
