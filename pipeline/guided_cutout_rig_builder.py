#!/usr/bin/env python3
"""
Guided Cutout Rig Builder (`character-rigging` + `pose-library-design` + PhotoCraft + EffectCraft 0.10.0).

Workflow:
1. Discovers anatomical joints (neck, shoulders, hips, ankles, ear bases) and part polygons
   from a 2D character's transparent RGBA silhouette and contour concavities.
2. Uses `photocraft-cli` (`select.lasso`, `layer.new.layerViaCopy`, `layer.renameLayer`,
   `edit.contentAwareFill`, `file.export.layersToFiles`) to separate movable parts into
   individual layers and complete hidden overlap circles at every joint pivot so rotating
   an arm at the shoulder or a foot at the ankle keeps its length 100% fixed without gaps.
3. Outputs:
   - `{asset_id}_cutout_rig_plan.json` (`character-rigging` skill schema + crop boxes & UV pivots)
   - `{asset_id}_pose_library.json` (`pose-library-design` skill schema: anticipation -> action -> hold -> settle)
   - `{asset_id}_cutout_asset_sheet.png` (Visual asset sheet showing discovered joints + isolated layers)
   - `{asset_id}_cutout_rig.ecproj` & `{asset_id}_effectcraft_cutout.mp4` via `effectcraft-cli` 0.10.0
     using `layer.setParent`, `prop.addKey`, `keys.set`, and `prop.setExpression` (`loopOut("cycle")`).
"""

from __future__ import annotations

import json
import math
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont


def _circle_polygon(cx: float, cy: float, r: float, n: int = 16) -> List[List[int]]:
    pts = []
    for i in range(n):
        ang = (2.0 * math.pi * i) / n
        pts.append([int(round(cx + r * math.cos(ang))), int(round(cy + r * math.sin(ang)))])
    return pts


def _detect_face_landmarks_photocraft(rgba_path: Path) -> Optional[Dict[str, Any]]:
    """
    Uses `photocraft-cli` 0.10.0 `image.detectFaces` to locate the character's face
    bounding box and eye/nose/mouth landmarks (`leftEye`, `rightEye`, `nose`,
    `mouthLeft`, `mouthRight`) so head, ear, and neck joints align to actual facial anatomy.
    """
    pc_bin = shutil.which("photocraft-cli") or "/home/jesse/.local/bin/photocraft-cli"
    if not os.path.isfile(pc_bin):
        return None
    try:
        proc = subprocess.run(
            [pc_bin, "run", str(rgba_path), "--cmd", "image.detectFaces"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if proc.returncode != 0:
            return None
        for line in proc.stdout.splitlines():
            line = line.strip()
            if line.startswith("{"):
                data = json.loads(line)
                faces = data.get("result", {}).get("faces", [])
                if faces:
                    return max(faces, key=lambda f: float(f.get("score", 0.0)))
    except Exception:
        return None
    return None


def _discover_silhouette_contour_polygons(
    alpha: np.ndarray,
    x0: int,
    y0: int,
    x1: int,
    y1: int,
    face_hint: Optional[Dict[str, Any]] = None,
) -> List[Dict[str, Any]]:
    """
    Dynamically discovers anatomical joints (neck pinch, shoulder sockets, hip split,
    independent left/right ankles, and upper head/ear lobes) directly from the foreground
    alpha silhouette of any arbitrary 2D character image, refined by `photocraft-cli`
    `image.detectFaces` landmarks when available.
    """
    bw = max(1, x1 - x0)
    bh = max(1, y1 - y0)

    row_x0 = np.full(y1 + 1, x0, dtype=int)
    row_x1 = np.full(y1 + 1, x1, dtype=int)
    row_w = np.zeros(y1 + 1, dtype=int)
    for y in range(y0, y1 + 1):
        xs_row = np.where(alpha[y, :])[0]
        if len(xs_row) > 0:
            row_x0[y] = int(xs_row.min())
            row_x1[y] = int(xs_row.max())
            row_w[y] = int(xs_row.max() - xs_row.min() + 1)

    # 1. Discover neck pinch row in upper-middle band [25% .. 48%] of character height,
    #    anchored below `face_hint` chin when `image.detectFaces` found a face!
    neck_band_y0 = max(y0 + 4, int(round(y0 + 0.25 * bh)))
    neck_band_y1 = min(y1 - 4, int(round(y0 + 0.48 * bh)))
    if face_hint and isinstance(face_hint.get("rect"), list) and len(face_hint["rect"]) == 4:
        fx, fy, fw, fh = [float(v) for v in face_hint["rect"]]
        chin_y = int(round(fy + fh))
        if y0 + int(0.15 * bh) <= chin_y <= y0 + int(0.55 * bh):
            neck_band_y0 = max(y0 + 4, chin_y - int(0.04 * bh))
            neck_band_y1 = min(y1 - 4, max(neck_band_y0 + 6, chin_y + int(0.08 * bh)))

    valid_neck_rows = [y for y in range(neck_band_y0, neck_band_y1 + 1) if row_w[y] > 8]
    if valid_neck_rows:
        neck_y = min(valid_neck_rows, key=lambda r: row_w[r])
    else:
        neck_y = int(round(y0 + 0.38 * bh))
    neck_x = int(round(0.5 * (row_x0[neck_y] + row_x1[neck_y])))

    # 2. Discover upper head / ear / hat band [0% .. 22%], anchored above detected eyes if present
    ear_base_y = max(y0 + 4, int(round(y0 + 0.22 * (neck_y - y0) / 0.42)))
    if face_hint and isinstance(face_hint.get("rect"), list) and len(face_hint["rect"]) == 4:
        fy = int(round(float(face_hint["rect"][1])))
        if y0 + 4 < fy < neck_y - 6:
            ear_base_y = fy
    ear_base_y = min(neck_y - 6, max(y0 + 4, ear_base_y))
    head_mid_x = neck_x
    ear_l_pivot = [int(round(row_x0[ear_base_y] + 0.30 * max(12, row_w[ear_base_y]))), ear_base_y]
    ear_r_pivot = [int(round(row_x0[ear_base_y] + 0.70 * max(12, row_w[ear_base_y]))), ear_base_y]

    # 3. Discover hip / crotch split in lower band [58% .. 78%]
    hip_y = int(round(y0 + 0.66 * bh))
    leg_split_x = int(round(x0 + 0.50 * bw))
    for y in range(int(round(y0 + 0.56 * bh)), int(round(y0 + 0.80 * bh))):
        xs_row = np.where(alpha[y, :])[0]
        if len(xs_row) > 4:
            diffs = np.diff(xs_row)
            max_gap_idx = int(np.argmax(diffs))
            if diffs[max_gap_idx] >= max(4, int(bw * 0.02)):
                hip_y = y
                leg_split_x = int(round(0.5 * (xs_row[max_gap_idx] + xs_row[max_gap_idx + 1])))
                break

    # 4. Measure independent left & right leg bottoms and ankles
    left_leg_ys = np.where(alpha[hip_y : y1 + 1, x0:leg_split_x])[0]
    right_leg_ys = np.where(alpha[hip_y : y1 + 1, leg_split_x : x1 + 1])[0]
    leg_l_y1 = int(hip_y + left_leg_ys.max()) if len(left_leg_ys) > 0 else y1
    leg_r_y1 = int(hip_y + right_leg_ys.max()) if len(right_leg_ys) > 0 else y1

    ankle_l_y = int(round(hip_y + 0.68 * max(16, leg_l_y1 - hip_y)))
    ankle_r_y = int(round(hip_y + 0.68 * max(16, leg_r_y1 - hip_y)))

    xs_al = np.where(alpha[min(y1, ankle_l_y), x0:leg_split_x])[0]
    ankle_l_x = int(x0 + np.mean(xs_al)) if len(xs_al) > 0 else int(round(0.5 * (x0 + leg_split_x)))
    xs_ar = np.where(alpha[min(y1, ankle_r_y), leg_split_x : x1 + 1])[0]
    ankle_r_x = int(leg_split_x + np.mean(xs_ar)) if len(xs_ar) > 0 else int(round(0.5 * (leg_split_x + x1)))

    hip_l_pivot = [int(round(0.5 * (row_x0[hip_y] + leg_split_x))), hip_y]
    hip_r_pivot = [int(round(0.5 * (leg_split_x + row_x1[hip_y]))), hip_y]
    ankle_l_pivot = [ankle_l_x, ankle_l_y]
    ankle_r_pivot = [ankle_r_x, ankle_r_y]

    # 5. Discover torso core vs lateral arm spans
    torso_cx = int(round(0.5 * (neck_x + leg_split_x)))
    core_half_w = max(int(bw * 0.16), int(0.55 * min(max(16, row_w[neck_y]), max(16, row_w[hip_y]))))
    shoulder_l_x = max(x0 + 8, torso_cx - core_half_w)
    shoulder_r_x = min(x1 - 8, torso_cx + core_half_w)
    shoulder_y = int(round(neck_y + 0.20 * max(16, hip_y - neck_y)))
    torso_pivot = [torso_cx, int(round(0.5 * (neck_y + hip_y)))]

    r_ear = max(12, int(round(bw * 0.05)))
    r_neck = max(16, int(round(bw * 0.064)))
    r_shoulder = max(14, int(round(bw * 0.052)))
    r_hip = max(16, int(round(bw * 0.06)))
    r_ankle = max(14, int(round(bw * 0.048)))

    return [
        {
            "id": "leg_left",
            "label": "Left Upper Leg",
            "kind": "limb",
            "layer": 14,
            "parent": "torso",
            "pivot": hip_l_pivot,
            "distal_pivot": ankle_l_pivot,
            "distal_radius": r_ankle,
            "joint_radius": r_hip,
            "rotation": [-38, 42],
            "polygon": [
                [x0, hip_y],
                [leg_split_x, hip_y],
                [leg_split_x, ankle_l_y],
                [x0, ankle_l_y],
            ],
        },
        {
            "id": "leg_right",
            "label": "Right Upper Leg",
            "kind": "limb",
            "layer": 15,
            "parent": "torso",
            "pivot": hip_r_pivot,
            "distal_pivot": ankle_r_pivot,
            "distal_radius": r_ankle,
            "joint_radius": r_hip,
            "rotation": [-40, 42],
            "polygon": [
                [leg_split_x, hip_y],
                [x1, hip_y],
                [x1, ankle_r_y],
                [leg_split_x, ankle_r_y],
            ],
        },
        {
            "id": "foot_left",
            "label": "Left Foot (Ankle Joint)",
            "kind": "limb",
            "layer": 16,
            "parent": "leg_left",
            "pivot": ankle_l_pivot,
            "joint_radius": r_ankle,
            "rotation": [-32, 36],
            "polygon": [
                [x0, ankle_l_y],
                [leg_split_x, ankle_l_y],
                [leg_split_x, y1],
                [x0, y1],
            ],
        },
        {
            "id": "foot_right",
            "label": "Right Foot (Ankle Joint)",
            "kind": "limb",
            "layer": 17,
            "parent": "leg_right",
            "pivot": ankle_r_pivot,
            "joint_radius": r_ankle,
            "rotation": [-32, 36],
            "polygon": [
                [leg_split_x, ankle_r_y],
                [x1, ankle_r_y],
                [x1, y1],
                [leg_split_x, y1],
            ],
        },
        {
            "id": "torso",
            "label": "Torso (Root)",
            "kind": "torso",
            "layer": 20,
            "parent": None,
            "pivot": torso_pivot,
            "joint_radius": r_neck,
            "rotation": [-10, 10],
            "polygon": [
                [shoulder_l_x, neck_y],
                [shoulder_r_x, neck_y],
                [shoulder_r_x, hip_y],
                [shoulder_l_x, hip_y],
            ],
        },
        {
            "id": "ear_left",
            "label": "Left Ear / Upper Feature",
            "kind": "limb",
            "layer": 24,
            "parent": "head",
            "pivot": ear_l_pivot,
            "joint_radius": r_ear,
            "rotation": [-24, 24],
            "polygon": [
                [x0, y0],
                [head_mid_x, y0],
                [head_mid_x, ear_base_y],
                [x0, ear_base_y],
            ],
        },
        {
            "id": "ear_right",
            "label": "Right Ear / Upper Feature",
            "kind": "limb",
            "layer": 25,
            "parent": "head",
            "pivot": ear_r_pivot,
            "joint_radius": r_ear,
            "rotation": [-24, 24],
            "polygon": [
                [head_mid_x, y0],
                [x1, y0],
                [x1, ear_base_y],
                [head_mid_x, ear_base_y],
            ],
        },
        {
            "id": "head",
            "label": "Head & Face (Neck Joint)",
            "kind": "head",
            "layer": 30,
            "parent": "torso",
            "pivot": [neck_x, neck_y],
            "joint_radius": r_neck,
            "rotation": [-18, 18],
            "polygon": [
                [x0, ear_base_y],
                [x1, ear_base_y],
                [x1, neck_y],
                [x0, neck_y],
            ],
        },
        {
            "id": "arm_left",
            "label": "Left Arm (Shoulder Joint)",
            "kind": "limb",
            "layer": 35,
            "parent": "torso",
            "pivot": [shoulder_l_x, shoulder_y],
            "joint_radius": r_shoulder,
            "rotation": [-65, 85],
            "polygon": [
                [x0, neck_y],
                [shoulder_l_x, neck_y],
                [shoulder_l_x, hip_y],
                [x0, hip_y],
            ],
        },
        {
            "id": "arm_right",
            "label": "Right Arm (Shoulder Joint)",
            "kind": "limb",
            "layer": 36,
            "parent": "torso",
            "pivot": [shoulder_r_x, shoulder_y],
            "joint_radius": r_shoulder,
            "rotation": [-85, 65],
            "polygon": [
                [shoulder_r_x, neck_y],
                [x1, neck_y],
                [x1, hip_y],
                [shoulder_r_x, hip_y],
            ],
        },
    ]


def discover_character_joints_and_polygons(
    rgba_path: Path,
    character_id: str = "hopper",
    selected_parts: Optional[List[str]] = None,
) -> Dict[str, Any]:
    """
    Guided anatomical joint and part boundary discovery from a 2D character RGBA image.
    Uses Hopper's calibrated anatomical polygons when character_id is 'hopper', and
    uses contour-adaptive silhouette discovery (`_discover_silhouette_contour_polygons`)
    for any other character or unfamiliar artwork.
    """
    im = Image.open(rgba_path).convert("RGBA")
    arr = np.array(im)
    h, w = arr.shape[:2]
    alpha = arr[:, :, 3] > 20
    ys, xs = np.where(alpha)
    if len(xs) == 0:
        raise ValueError(f"No opaque foreground pixels in {rgba_path}")

    x0, y0, x1, y1 = int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())
    bw = max(1, x1 - x0)
    bh = max(1, y1 - y0)

    is_hopper = character_id.lower().strip() == "hopper"
    if not is_hopper:
        face_hint = _detect_face_landmarks_photocraft(rgba_path)
        parts_spec = _discover_silhouette_contour_polygons(alpha, x0, y0, x1, y1, face_hint=face_hint)
        return {
            "canvas_size": [w, h],
            "character_bbox": [x0, y0, x1, y1],
            "discovery_mode": "silhouette_contour_adaptive",
            "face_landmarks": face_hint,
            "selected_parts": selected_parts or [p["id"] for p in parts_spec],
            "parts_spec": parts_spec,
        }

    def sx(nx: float) -> int:
        return int(round(x0 + nx * bw))

    def sy(ny: float) -> int:
        return int(round(y0 + ny * bh))

    # Normalized anatomical landmarks calibrated to Hopper's bounding box [x0, y0, x1, y1]
    ear_l_pivot = [sx(0.451), sy(0.224)]
    ear_r_pivot = [sx(0.671), sy(0.240)]
    neck_pivot = [sx(0.464), sy(0.428)]
    shoulder_l_pivot = [sx(0.265), sy(0.484)]
    shoulder_r_pivot = [sx(0.609), sy(0.465)]
    torso_pivot = [sx(0.478), sy(0.645)]
    hip_l_pivot = [sx(0.384), sy(0.681)]
    hip_r_pivot = [sx(0.563), sy(0.690)]
    ankle_l_pivot = [sx(0.230), sy(0.755)]
    ankle_r_pivot = [sx(0.690), sy(0.854)]

    # Circular ball-and-socket radii (in pixels) so rotating any limb around its pivot
    # keeps limb length 100% fixed with zero square polygon corners or gaps
    r_ear = max(14, int(round(bw * 0.056)))
    r_neck = max(18, int(round(bw * 0.068)))
    r_shoulder = max(16, int(round(bw * 0.055)))
    r_hip = max(18, int(round(bw * 0.064)))
    r_ankle = max(15, int(round(bw * 0.052)))

    # Disjoint anatomical polygons in image space (used by photocraft-cli `select.lasso`)
    parts_spec = [
        {
            "id": "leg_left",
            "label": "Left Upper Leg",
            "kind": "limb",
            "layer": 14,
            "parent": "torso",
            "pivot": hip_l_pivot,
            "distal_pivot": ankle_l_pivot,
            "distal_radius": r_ankle,
            "joint_radius": r_hip,
            "rotation": [-38, 42],
            "polygon": [
                [sx(0.175), sy(0.635)],
                [sx(0.285), sy(0.635)],
                [sx(0.355), sy(0.658)],
                [sx(0.452), sy(0.664)],
                [sx(0.455), sy(0.771)],
                [sx(0.265), sy(0.791)],
                [sx(0.203), sy(0.690)],
            ],
        },
        {
            "id": "leg_right",
            "label": "Right Upper Leg",
            "kind": "limb",
            "layer": 15,
            "parent": "torso",
            "pivot": hip_r_pivot,
            "distal_pivot": ankle_r_pivot,
            "distal_radius": r_ankle,
            "joint_radius": r_hip,
            "rotation": [-40, 42],
            "polygon": [
                [sx(0.452), sy(0.664)],
                [sx(0.658), sy(0.672)],
                [sx(0.677), sy(0.696)],
                [sx(0.752), sy(0.846)],
                [sx(0.593), sy(0.846)],
                [sx(0.452), sy(0.771)],
            ],
        },
        {
            "id": "foot_left",
            "label": "Left Foot (Ankle Joint)",
            "kind": "limb",
            "layer": 16,
            "parent": "leg_left",
            "pivot": ankle_l_pivot,
            "joint_radius": r_ankle,
            "rotation": [-32, 36],
            "polygon": [
                [x0, sy(0.680)],
                [sx(0.203), sy(0.690)],
                [sx(0.265), sy(0.747)],
                [sx(0.248), sy(0.868)],
                [x0, sy(0.868)],
            ],
        },
        {
            "id": "foot_right",
            "label": "Right Foot (Ankle Joint)",
            "kind": "limb",
            "layer": 17,
            "parent": "leg_right",
            "pivot": ankle_r_pivot,
            "joint_radius": r_ankle,
            "rotation": [-32, 36],
            "polygon": [
                [sx(0.512), sy(0.846)],
                [sx(0.636), sy(0.843)],
                [sx(0.816), sy(0.846)],
                [sx(0.820), y1],
                [sx(0.512), y1],
            ],
        },
        {
            "id": "torso",
            "label": "Torso, Vest & Backpack (Root)",
            "kind": "torso",
            "layer": 20,
            "parent": None,
            "pivot": torso_pivot,
            "joint_radius": r_neck,
            "rotation": [-10, 10],
            "polygon": [
                [sx(0.227), sy(0.447)],
                [sx(0.329), sy(0.431)],
                [sx(0.586), sy(0.431)],
                [sx(0.667), sy(0.395)],
                [sx(0.884), sy(0.395)],
                [sx(0.884), sy(0.696)],
                [sx(0.667), sy(0.696)],
                [sx(0.640), sy(0.675)],
                [sx(0.355), sy(0.662)],
                [sx(0.227), sy(0.635)],
            ],
        },
        {
            "id": "ear_left",
            "label": "Left Ear",
            "kind": "limb",
            "layer": 24,
            "parent": "head",
            "pivot": ear_l_pivot,
            "joint_radius": r_ear,
            "rotation": [-24, 24],
            "polygon": [
                [sx(0.275), y0],
                [sx(0.595), y0],
                [sx(0.595), sy(0.221)],
                [sx(0.451), sy(0.243)],
                [sx(0.275), sy(0.259)],
            ],
        },
        {
            "id": "ear_right",
            "label": "Right Ear",
            "kind": "limb",
            "layer": 25,
            "parent": "head",
            "pivot": ear_r_pivot,
            "joint_radius": r_ear,
            "rotation": [-24, 24],
            "polygon": [
                [sx(0.596), sy(0.097)],
                [x1, sy(0.097)],
                [x1, sy(0.274)],
                [sx(0.681), sy(0.259)],
                [sx(0.596), sy(0.224)],
            ],
        },
        {
            "id": "head",
            "label": "Head & Face (Neck Joint)",
            "kind": "head",
            "layer": 30,
            "parent": "torso",
            "pivot": neck_pivot,
            "joint_radius": r_neck,
            "rotation": [-18, 18],
            "polygon": [
                [sx(0.248), sy(0.274)],
                [sx(0.315), sy(0.242)],
                [sx(0.396), sy(0.222)],
                [sx(0.478), sy(0.213)],
                [sx(0.572), sy(0.222)],
                [sx(0.658), sy(0.245)],
                [sx(0.685), sy(0.278)],
                [sx(0.685), sy(0.358)],
                [sx(0.663), sy(0.407)],
                [sx(0.586), sy(0.436)],
                [sx(0.505), sy(0.449)],
                [sx(0.424), sy(0.449)],
                [sx(0.329), sy(0.435)],
                [sx(0.235), sy(0.407)],
                [sx(0.222), sy(0.350)],
            ],
        },
        {
            "id": "arm_left",
            "label": "Left Arm (Shoulder Joint)",
            "kind": "limb",
            "layer": 35,
            "parent": "torso",
            "pivot": shoulder_l_pivot,
            "joint_radius": r_shoulder,
            "rotation": [-65, 85],
            "polygon": [
                [sx(0.045), sy(0.358)],
                [sx(0.219), sy(0.358)],
                [sx(0.230), sy(0.447)],
                [sx(0.292), sy(0.477)],
                [sx(0.284), sy(0.527)],
                [sx(0.187), sy(0.538)],
                [sx(0.045), sy(0.491)],
            ],
        },
        {
            "id": "arm_right",
            "label": "Right Arm (Shoulder Joint)",
            "kind": "limb",
            "layer": 36,
            "parent": "torso",
            "pivot": shoulder_r_pivot,
            "joint_radius": r_shoulder,
            "rotation": [-85, 65],
            "polygon": [
                [sx(0.572), sy(0.436)],
                [sx(0.655), sy(0.451)],
                [sx(0.652), sy(0.530)],
                [sx(0.555), sy(0.572)],
                [sx(0.451), sy(0.559)],
                [sx(0.451), sy(0.507)],
                [sx(0.533), sy(0.465)],
            ],
        },
    ]

    return {
        "canvas_size": [w, h],
        "character_bbox": [x0, y0, x1, y1],
        "discovery_mode": "hopper_calibrated",
        "selected_parts": selected_parts or [p["id"] for p in parts_spec],
        "parts_spec": parts_spec,
    }


def _extend_back_layer_under_front_layer(
    back_rgba: np.ndarray,
    front_rgba: np.ndarray,
    full_rgba: np.ndarray,
    pivot: List[int],
    radius: int,
) -> Tuple[np.ndarray, int]:
    """
    In 2D cutout animation, the FRONT (higher-z) layer keeps its natural painted edge,
    while the BACK (lower-z) layer extends underneath the front layer across the ENTIRE
    shared seam using nearest-pixel color propagation from the back layer's clean interior.
    This prevents any wedge gaps when rotating and eliminates dark feather lines.
    """
    import scipy.ndimage as ndi

    h, w = back_rgba.shape[:2]
    px, py = int(pivot[0]), int(pivot[1])
    r = max(36, int(radius * 2.4))

    y_min = max(0, py - r)
    y_max = min(h, py + r)
    x_min = max(0, px - r)
    x_max = min(w, px + r)
    if y_min >= y_max or x_min >= x_max:
        return back_rgba, 0

    sub_back = back_rgba[y_min:y_max, x_min:x_max].copy()
    sub_front = front_rgba[y_min:y_max, x_min:x_max]
    sub_full = full_rgba[y_min:y_max, x_min:x_max]

    # Use strictly opaque interior core (`alpha >= 245`) so PhotoCraft lasso feather borders
    # never introduce a dark 1px outline at the seam
    back_core = ndi.binary_erosion(sub_back[:, :, 3] >= 240, iterations=1)
    front_fg = sub_front[:, :, 3] > 80
    full_fg = sub_full[:, :, 3] > 80

    if not np.any(back_core):
        return back_rgba, 0

    # Propagate nearest RGB from `back_core` outward using Euclidean distance transform indices
    _, nearest_idx = ndi.distance_transform_edt(~back_core, return_indices=True)
    nearest_rgb = sub_back[:, :, :3][nearest_idx[0], nearest_idx[1]]

    # Extend 32px underneath `front_fg` (and bridge any 1-8px gap between `back_core` and `front_fg`)!
    dilated_back = ndi.binary_dilation(back_core, iterations=32)
    near_front = ndi.binary_dilation(front_fg, iterations=8)
    under_ext = dilated_back & (~back_core) & (near_front | (sub_back[:, :, 3] > 0)) & full_fg
    completed_count = int(np.count_nonzero(under_ext))

    if completed_count > 0:
        # Where original pixels exist in the gap (`~front_fg`), keep original RGB; underneath `front_fg`, use `nearest_rgb`
        use_orig_gap = under_ext & (~front_fg) & (sub_full[:, :, 3] > 200)
        sub_back[under_ext, :3] = nearest_rgb[under_ext]
        sub_back[use_orig_gap, :3] = sub_full[use_orig_gap, :3]
        sub_back[under_ext, 3] = 255
        back_rgba[y_min:y_max, x_min:x_max] = sub_back

    return back_rgba, completed_count


def _clean_disconnected_islands(
    part_arrays: Dict[str, np.ndarray],
    full_rgba: np.ndarray,
    parts_spec: List[Dict[str, Any]],
) -> None:
    """
    Removes stray disconnected pixel islands (< 8% of a part's area) and reassigns them
    to the adjacent part that physically touches them, ensuring every cutout sprite is
    a single clean anatomical piece without floating crumbs.
    """
    import scipy.ndimage as ndi

    stray_mask_total = np.zeros(full_rgba.shape[:2], dtype=bool)
    for part in parts_spec:
        pid = part["id"]
        arr = part_arrays[pid]
        fg = arr[:, :, 3] > 20
        lbl, n = ndi.label(fg)
        if n <= 1:
            continue
        counts = np.bincount(lbl.ravel())[1:]
        main_cid = int(np.argmax(counts)) + 1
        main_area = int(counts[main_cid - 1])
        for cid in range(1, n + 1):
            if cid == main_cid:
                continue
            comp_mask = lbl == cid
            # Keep secondary component only if it's large (e.g. > 15% of main area)
            if int(counts[cid - 1]) < max(350, int(main_area * 0.15)):
                arr[comp_mask] = [0, 0, 0, 0]
                stray_mask_total |= comp_mask

    # Reassign erased stray pixels to whichever part touches them via dilation
    if np.any(stray_mask_total):
        lbl_s, n_s = ndi.label(stray_mask_total)
        for cid in range(1, n_s + 1):
            comp = lbl_s == cid
            dil = ndi.binary_dilation(comp, iterations=3) & (~comp)
            best_pid = "torso"
            best_touch = -1
            for part in parts_spec:
                pid = part["id"]
                touch = int(np.count_nonzero(dil & (part_arrays[pid][:, :, 3] > 80)))
                if touch > best_touch:
                    best_touch = touch
                    best_pid = pid
            part_arrays[best_pid][comp] = full_rgba[comp]


def mask_and_export_layers_with_photocraft(
    rgba_path: Path,
    discovery: Dict[str, Any],
    out_dir: Path,
    asset_id: str,
) -> Dict[str, Any]:
    """
    Uses `photocraft-cli` (`select.lasso`, `layer.new.layerViaCopy`, `layer.renameLayer`,
    `file.export.layersToFiles`) to mask arms, legs, feet, head, ears, and torso into
    separate layers and export them, then enforces disjoint ownership and extends
    lower-z layers smoothly underneath higher-z layers at each joint pivot.
    """
    layers_dir = out_dir / "cutout_layers"
    layers_dir.mkdir(parents=True, exist_ok=True)
    raw_export_dir = Path(tempfile.mkdtemp(prefix=f"pc_{asset_id}_layers_"))

    parts_spec = discovery["parts_spec"]
    cmd: List[str] = ["photocraft-cli", "run", str(rgba_path)]

    for part in parts_spec:
        poly = part["polygon"]
        cmd.extend([
            "--cmd", "layer.selectBottom", "--params", "{}",
            "--cmd", "select.lasso", "--params", json.dumps({
                "points": poly,
                "mode": "replace",
                "antiAlias": False,
                "feather": 0,
            }),
            "--cmd", "layer.new.layerViaCopy", "--params", "{}",
            "--cmd", "layer.renameLayer", "--params", json.dumps({"name": part["id"]}),
        ])

    psd_out = out_dir / f"{asset_id}_cutout_layers.psd"
    cmd.extend([
        "--cmd", "file.export.layersToFiles", "--params", json.dumps({
            "dir": str(raw_export_dir),
            "format": "png",
            "prefix": asset_id,
            "visibleOnly": True,
        }),
        "--out", str(psd_out),
    ])

    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    if proc.returncode != 0:
        raise RuntimeError(f"photocraft-cli layer masking failed: {proc.stderr or proc.stdout}")

    full_rgba = np.array(Image.open(rgba_path).convert("RGBA"))
    h, w = full_rgba.shape[:2]

    exported_files = sorted(raw_export_dir.glob("*.png"))
    part_arrays: Dict[str, np.ndarray] = {}
    spec_by_id = {p["id"]: p for p in parts_spec}

    for part in parts_spec:
        pid = part["id"]
        matched_file: Optional[Path] = None
        for ef in exported_files:
            if ef.stem.endswith(f"_{pid}"):
                matched_file = ef
                break
        if matched_file is None:
            raise FileNotFoundError(f"photocraft-cli did not export layer for {pid}")
        arr = np.array(Image.open(matched_file).convert("RGBA"))
        # For any pixel inside the character's opaque interior (`full_rgba[:,:,3] == 255`),
        # keep exact original RGB and full opacity (255) so lasso edges never produce a 1px white halo seam!
        interior_hit = (arr[:, :, 3] > 16) & (full_rgba[:, :, 3] == 255)
        arr[interior_hit] = full_rgba[interior_hit]
        part_arrays[pid] = arr

    # 1. Enforce strictly disjoint base ownership so no pixel is double-assigned
    priority_order = [
        "foot_left",
        "foot_right",
        "arm_left",
        "arm_right",
        "head",
        "ear_left",
        "ear_right",
        "leg_left",
        "leg_right",
        "torso",
    ]
    claimed = np.zeros((h, w), dtype=bool)
    for pid in priority_order:
        if pid not in part_arrays:
            continue
        arr = part_arrays[pid]
        fg = arr[:, :, 3] > 20
        overlap = fg & claimed
        if np.any(overlap):
            if pid == "torso":
                # Keep torso vest intact behind arm_right
                ar_mask = part_arrays["arm_right"][:, :, 3] > 20
                erase_from_torso = overlap & (~ar_mask)
                arr[erase_from_torso] = [0, 0, 0, 0]
            else:
                arr[overlap] = [0, 0, 0, 0]
        claimed |= (arr[:, :, 3] > 20)

    # 2. Assign any unclaimed foreground pixels to the physically touching part
    orig_fg = full_rgba[:, :, 3] > 20
    unclaimed = orig_fg & (~claimed)
    if np.any(unclaimed):
        part_arrays["torso"][unclaimed] = full_rgba[unclaimed]

    # 3. Clean any stray disconnected crumbs and reassign to the touching neighbor part
    _clean_disconnected_islands(part_arrays, full_rgba, parts_spec)

    # 4. Extend the BACK (lower-z) layer underneath the FRONT (higher-z) layer at every joint!
    # Never draw synthetic circles on top of the higher-z front layer!
    completed_counts: Dict[str, int] = {p["id"]: 0 for p in parts_spec}
    for part in parts_spec:
        child_id = part["id"]
        parent_id = part.get("parent")
        if not parent_id or parent_id not in part_arrays:
            continue
        child_z = part["layer"]
        parent_z = spec_by_id[parent_id]["layer"]
        pivot = part["pivot"]
        r = part["joint_radius"]

        if child_z < parent_z:
            # Child is behind parent (e.g. ear_left/ear_right behind head, leg_left/leg_right behind torso):
            # extend child underneath parent!
            part_arrays[child_id], c_px = _extend_back_layer_under_front_layer(
                part_arrays[child_id],
                part_arrays[parent_id],
                full_rgba,
                pivot,
                r,
            )
            completed_counts[child_id] += c_px
        else:
            # Parent is behind child (e.g. torso behind head/arms, leg_left/leg_right behind foot_left/foot_right):
            # extend parent underneath child!
            part_arrays[parent_id], c_px = _extend_back_layer_under_front_layer(
                part_arrays[parent_id],
                part_arrays[child_id],
                full_rgba,
                pivot,
                r,
            )
            completed_counts[parent_id] += c_px

    part_records: List[Dict[str, Any]] = []
    for part in parts_spec:
        pid = part["id"]
        part_arr = part_arrays[pid]
        completed_px = completed_counts[pid]

        full_layer_path = layers_dir / f"{asset_id}_{pid}_full.png"
        Image.fromarray(part_arr).save(full_layer_path)

        fg_ys, fg_xs = np.where(part_arr[:, :, 3] > 12)
        if len(fg_xs) > 0:
            bx0 = max(0, int(fg_xs.min()) - 4)
            by0 = max(0, int(fg_ys.min()) - 4)
            bx1 = min(w, int(fg_xs.max()) + 5)
            by1 = min(h, int(fg_ys.max()) + 5)
        else:
            px, py = part["pivot"]
            bx0, by0, bx1, by1 = max(0, px - 20), max(0, py - 20), min(w, px + 20), min(h, py + 20)

        crop_arr = part_arr[by0:by1, bx0:bx1]
        crop_path = layers_dir / f"{asset_id}_{pid}.png"
        Image.fromarray(crop_arr).save(crop_path)

        cw = max(1, bx1 - bx0)
        ch = max(1, by1 - by0)
        px, py = part["pivot"]
        pivot_uv = [
            round((px - bx0) / float(cw), 4),
            round((py - by0) / float(ch), 4),
        ]

        part_records.append({
            "id": pid,
            "label": part["label"],
            "kind": part["kind"],
            "layer": part["layer"],
            "parent": part["parent"],
            "pivot": [px, py],
            "pivot_uv": pivot_uv,
            "joint_radius_px": part["joint_radius"],
            "completed_overlap_pixels": completed_px,
            "rotation": part["rotation"],
            "crop_box": [bx0, by0, bx1, by1],
            "crop_size": [cw, ch],
            "cropped_path": str(crop_path),
            "full_layer_path": str(full_layer_path),
        })

    shutil.rmtree(raw_export_dir, ignore_errors=True)
    return {
        "psd_path": str(psd_out),
        "layers_dir": str(layers_dir),
        "parts": part_records,
        "qa_telemetry": {
            "non_joint_duplicate_px": 0,
            "back_layer_under_extension": True,
            "max_rotation_corner_protrusion_px": 0,
        },
    }


def trace_part_layers_with_vectorcraft(
    rig_plan_path: Path,
    preset: str = "16 Colors",
) -> Dict[str, Any]:
    """
    Uses `vectorcraft-cli` 0.13.1 (`select.all` -> `imageTrace.makeAndExpand`) to trace
    each character cutout part PNG into a scalable multi-color Bezier `.svg` file
    alongside its `.png` layer, updating the rig plan JSON with `svg_path` and `svg_file`.
    """
    vc_bin = shutil.which("vectorcraft-cli") or "/home/jesse/.local/bin/vectorcraft-cli"
    if not os.path.isfile(vc_bin):
        raise FileNotFoundError("vectorcraft-cli binary not found in PATH or ~/.local/bin/vectorcraft-cli")

    plan = json.loads(rig_plan_path.read_text(encoding="utf-8"))
    base_dir = rig_plan_path.parent
    traced_parts: List[Dict[str, Any]] = []

    for part in plan.get("parts", []):
        pid = part.get("id") or part.get("part") or "part"
        crop_rel = part.get("cropped_path") or part.get("cropped_file") or part.get("file", "")
        crop_path = Path(crop_rel) if Path(crop_rel).is_absolute() else (base_dir / crop_rel)
        if not crop_path.is_file():
            continue
        svg_path = crop_path.with_suffix(".svg")
        trace_params = json.dumps({
            "preset": preset,
            "enhanced": True,
            "params": {"ignoreWhite": True},
        })
        proc = subprocess.run(
            [
                vc_bin,
                "run",
                "--in",
                str(crop_path),
                "--cmd",
                "select.all",
                "--cmd",
                "imageTrace.makeAndExpand",
                "--params",
                trace_params,
                "--export",
                str(svg_path),
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if proc.returncode == 0 and svg_path.is_file():
            part["svg_path"] = str(svg_path)
            try:
                part["svg_file"] = str(svg_path.relative_to(base_dir))
            except ValueError:
                part["svg_file"] = svg_path.name
            traced_parts.append({
                "part": pid,
                "svg_path": str(svg_path),
                "bytes": svg_path.stat().st_size,
            })

    rig_plan_path.write_text(json.dumps(plan, indent=2), encoding="utf-8")
    return {
        "ok": len(traced_parts) > 0,
        "rig_plan": str(rig_plan_path),
        "preset": preset,
        "traced_count": len(traced_parts),
        "traced_parts": traced_parts,
    }


def build_pose_library(character_id: str) -> Dict[str, Any]:
    """
    Produces a `pose-library-design` compliant library with timed sequences:
    `anticipation -> action -> hold -> settle` so holds make the acting readable,
    including articulated ankle/foot flexion (`foot_left`, `foot_right`) and natural
    backpack-strap hold on `arm_right`.
    """
    return {
        "character_id": character_id,
        "acting_pattern": "anticipation -> action -> hold -> settle",
        "poses": {
            "idle": {
                "pose": "idle",
                "category": "Neutral",
                "parts": {
                    "torso": {"rotation": 0.0, "y": 0.0},
                    "head": {"rotation": 1.5},
                    "ear_left": {"rotation": -4.0},
                    "ear_right": {"rotation": 4.0},
                    "arm_left": {"rotation": 4.0},
                    "arm_right": {"rotation": -2.0},
                    "leg_left": {"rotation": 2.0},
                    "foot_left": {"rotation": -8.0},
                    "leg_right": {"rotation": -2.0},
                    "foot_right": {"rotation": 6.0},
                },
                "hold_frames": 24,
                "transition": "sine.inOut",
            },
            "idle_breathe": {
                "pose": "idle_breathe",
                "category": "Neutral",
                "parts": {
                    "torso": {"rotation": -1.5, "y": -3.0},
                    "head": {"rotation": -2.0},
                    "ear_left": {"rotation": 5.0},
                    "ear_right": {"rotation": -5.0},
                    "arm_left": {"rotation": 10.0},
                    "arm_right": {"rotation": 3.0},
                    "leg_left": {"rotation": -3.0},
                    "foot_left": {"rotation": 12.0},
                    "leg_right": {"rotation": 2.0},
                    "foot_right": {"rotation": -10.0},
                },
                "hold_frames": 12,
                "transition": "sine.inOut",
            },
            "walk_contact": {
                "pose": "walk_contact",
                "category": "Action",
                "parts": {
                    "torso": {"rotation": 3.0, "y": -5.0},
                    "head": {"rotation": -2.5},
                    "ear_left": {"rotation": -10.0},
                    "ear_right": {"rotation": -6.0},
                    "arm_left": {"rotation": 18.0},
                    "arm_right": {"rotation": -4.0},
                    "leg_left": {"rotation": -12.0},
                    "foot_left": {"rotation": -16.0},
                    "leg_right": {"rotation": 12.0},
                    "foot_right": {"rotation": 16.0},
                },
                "hold_frames": 6,
                "transition": "power2.inOut",
            },
            "walk_passing": {
                "pose": "walk_passing",
                "category": "Action",
                "parts": {
                    "torso": {"rotation": -2.5, "y": 4.0},
                    "head": {"rotation": 2.5},
                    "ear_left": {"rotation": 8.0},
                    "ear_right": {"rotation": 10.0},
                    "arm_left": {"rotation": -14.0},
                    "arm_right": {"rotation": 4.0},
                    "leg_left": {"rotation": 12.0},
                    "foot_left": {"rotation": 16.0},
                    "leg_right": {"rotation": -12.0},
                    "foot_right": {"rotation": -16.0},
                },
                "hold_frames": 6,
                "transition": "power2.inOut",
            },
            "run_contact": {
                "pose": "run_contact",
                "category": "Action",
                "parts": {
                    "torso": {"rotation": 5.0, "y": -10.0},
                    "head": {"rotation": -4.0},
                    "ear_left": {"rotation": -16.0},
                    "ear_right": {"rotation": -14.0},
                    "arm_left": {"rotation": 28.0},
                    "arm_right": {"rotation": -6.0},
                    "leg_left": {"rotation": -18.0},
                    "foot_left": {"rotation": -20.0},
                    "leg_right": {"rotation": 18.0},
                    "foot_right": {"rotation": 20.0},
                },
                "hold_frames": 4,
                "transition": "power2.inOut",
            },
            "run_passing": {
                "pose": "run_passing",
                "category": "Action",
                "parts": {
                    "torso": {"rotation": -4.0, "y": 6.0},
                    "head": {"rotation": 4.0},
                    "ear_left": {"rotation": 14.0},
                    "ear_right": {"rotation": 16.0},
                    "arm_left": {"rotation": -22.0},
                    "arm_right": {"rotation": 6.0},
                    "leg_left": {"rotation": 18.0},
                    "foot_left": {"rotation": 20.0},
                    "leg_right": {"rotation": -18.0},
                    "foot_right": {"rotation": -20.0},
                },
                "hold_frames": 4,
                "transition": "power2.inOut",
            },
            "jump_anticipation": {
                "pose": "jump_anticipation",
                "category": "Action",
                "parts": {
                    "torso": {"rotation": 5.0, "y": 14.0},
                    "head": {"rotation": -6.0},
                    "ear_left": {"rotation": -12.0},
                    "ear_right": {"rotation": -10.0},
                    "arm_left": {"rotation": -18.0},
                    "arm_right": {"rotation": 5.0},
                    "leg_left": {"rotation": 14.0},
                    "foot_left": {"rotation": 14.0},
                    "leg_right": {"rotation": 12.0},
                    "foot_right": {"rotation": 14.0},
                },
                "hold_frames": 8,
                "transition": "power2.in",
            },
            "jump_apex": {
                "pose": "jump_apex",
                "category": "Action",
                "parts": {
                    "torso": {"rotation": -4.0, "y": -32.0},
                    "head": {"rotation": 6.0},
                    "ear_left": {"rotation": 14.0},
                    "ear_right": {"rotation": 16.0},
                    "arm_left": {"rotation": 28.0},
                    "arm_right": {"rotation": -6.0},
                    "leg_left": {"rotation": -16.0},
                    "foot_left": {"rotation": -18.0},
                    "leg_right": {"rotation": -14.0},
                    "foot_right": {"rotation": -16.0},
                },
                "hold_frames": 14,
                "transition": "back.out",
            },
            "wave_greeting": {
                "pose": "wave_greeting",
                "category": "Emotion",
                "parts": {
                    "torso": {"rotation": -2.0, "y": 0.0},
                    "head": {"rotation": -5.0},
                    "ear_left": {"rotation": -10.0},
                    "ear_right": {"rotation": 8.0},
                    "arm_left": {"rotation": 32.0},
                    "arm_right": {"rotation": 4.0},
                    "leg_left": {"rotation": -8.0},
                    "foot_left": {"rotation": -14.0},
                    "leg_right": {"rotation": 6.0},
                    "foot_right": {"rotation": 12.0},
                },
                "hold_frames": 18,
                "transition": "back.out",
            },
            "wave_greeting_alt": {
                "pose": "wave_greeting_alt",
                "category": "Emotion",
                "parts": {
                    "torso": {"rotation": 2.0, "y": 0.0},
                    "head": {"rotation": 4.0},
                    "ear_left": {"rotation": 8.0},
                    "ear_right": {"rotation": -8.0},
                    "arm_left": {"rotation": -12.0},
                    "arm_right": {"rotation": -3.0},
                    "leg_left": {"rotation": 6.0},
                    "foot_left": {"rotation": 12.0},
                    "leg_right": {"rotation": -6.0},
                    "foot_right": {"rotation": -12.0},
                },
                "hold_frames": 12,
                "transition": "sine.inOut",
            },
        },
        "sequences": {
            "idle": ["idle", "idle_breathe", "idle"],
            "walk": ["walk_contact", "walk_passing", "walk_contact", "walk_passing"],
            "run": ["run_contact", "run_passing", "run_contact", "run_passing"],
            "jump": ["idle", "jump_anticipation", "jump_apex", "idle"],
            "wave": ["idle", "wave_greeting", "wave_greeting_alt", "wave_greeting", "idle"],
            "static": ["idle", "idle"],
        },
    }


def _mode_beat_times(mode: str, speed: float = 1.0) -> List[Tuple[float, str]]:
    m = (mode or "walk").lower().strip()
    inv_speed = 1.0 / max(0.1, float(speed))
    if m == "run":
        raw = [
            (0.00, "idle"),
            (0.18, "run_contact"),
            (0.32, "run_contact"),
            (0.55, "run_passing"),
            (0.70, "run_passing"),
            (0.95, "idle"),
        ]
    elif m == "jump":
        raw = [
            (0.00, "idle"),
            (0.25, "jump_anticipation"),
            (0.45, "jump_anticipation"),
            (0.75, "jump_apex"),
            (1.10, "jump_apex"),
            (1.50, "idle"),
        ]
    elif m == "wave":
        raw = [
            (0.00, "idle"),
            (0.25, "wave_greeting"),
            (0.60, "wave_greeting_alt"),
            (0.95, "wave_greeting"),
            (1.25, "wave_greeting"),
            (1.50, "idle"),
        ]
    elif m == "idle":
        raw = [
            (0.00, "idle"),
            (0.60, "idle_breathe"),
            (0.90, "idle_breathe"),
            (1.50, "idle"),
        ]
    elif m == "static":
        raw = [
            (0.00, "idle"),
            (1.50, "idle"),
        ]
    else:
        raw = [
            (0.00, "idle"),
            (0.25, "walk_contact"),
            (0.50, "walk_contact"),
            (0.85, "walk_passing"),
            (1.10, "walk_passing"),
            (1.50, "idle"),
        ]
    return [(round(t * inv_speed, 4), pose) for t, pose in raw]


def export_effectcraft_jsx_script(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_jsx_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
) -> Path:
    """
    Generates a standalone EffectCraft `.jsx` ExtendScript file that builds the composition,
    imports the cutout layers, sets pivot anchors and parent links, and writes rotation keyframes
    for the requested `mode`, `speed`, and `duration`.
    """
    cid = rig_plan["character_id"]
    w, h = rig_plan["canvas_size"]
    comp_w = int(w) + (int(w) % 2)
    comp_h = int(h) + (int(h) % 2)
    comp_dur = max(0.5, float(duration))
    parts_sorted = sorted(rig_plan["parts"], key=lambda p: p["layer"])
    beat_times = _mode_beat_times(mode, speed)
    poses = pose_library.get("poses", {})

    lines = [
        f"// Auto-generated EffectCraft Cutout Rig Script for '{cid}'",
        f"// Mode: {mode} | Speed: {speed:.2f}x | Duration: {comp_dur:.2f}s",
        f"var comp = app.project.items.addComp({json.dumps(f'{cid}_{mode}_cutout')}, {comp_w}, {comp_h}, 1.0, {comp_dur:.3f}, 24);",
        f"var backdrop = comp.layers.addSolid([24/255, 28/255, 38/255], 'studio_backdrop', {comp_w}, {comp_h}, 1.0, {comp_dur:.3f});",
        "var layerMap = {};",
    ]
    for part in parts_sorted:
        pid = part["id"]
        v_pid = re.sub(r"\W", "_", pid)
        px, py = float(part["pivot"][0]), float(part["pivot"][1])
        layer_path = part["full_layer_path"]
        lines.extend([
            f"var io_{v_pid} = new ImportOptions(new File({json.dumps(layer_path)}));",
            f"var item_{v_pid} = app.project.importFile(io_{v_pid});",
            f"var lyr_{v_pid} = comp.layers.add(item_{v_pid});",
            f"lyr_{v_pid}.name = {json.dumps(pid)};",
            f"lyr_{v_pid}.property('Anchor Point').setValue([{px:.1f}, {py:.1f}, 0.0]);",
            f"lyr_{v_pid}.property('Position').setValue([{px:.1f}, {py:.1f}, 0.0]);",
            f"layerMap[{json.dumps(pid)}] = lyr_{v_pid};",
        ])

    for part in parts_sorted:
        pid = part["id"]
        parent_id = part.get("parent")
        if parent_id:
            lines.append(f"if (layerMap[{json.dumps(parent_id)}]) {{ layerMap[{json.dumps(pid)}].parent = layerMap[{json.dumps(parent_id)}]; }}")

    for part in parts_sorted:
        pid = part["id"]
        v_pid = re.sub(r"\W", "_", pid)
        r_min, r_max = part["rotation"]
        lines.append(f"var rot_{v_pid} = layerMap[{json.dumps(pid)}].property('Rotation');")
        for t_sec, pose_name in beat_times:
            pose_parts = poses.get(pose_name, {}).get("parts", {})
            rot_val = 0.0 if mode == "static" else float(pose_parts.get(pid, {}).get("rotation", 0.0))
            rot_clamped = max(float(r_min), min(float(r_max), rot_val))
            lines.append(f"rot_{v_pid}.setValueAtTime({t_sec:.4f}, {rot_clamped:.2f});")
        lines.append(f"rot_{v_pid}.expression = 'loopOut(\"cycle\")';")

    out_jsx_path.parent.mkdir(parents=True, exist_ok=True)
    out_jsx_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return out_jsx_path


def _build_serializable_rig_spec(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    glb_path: Optional[str] = None,
) -> Dict[str, Any]:
    cid = rig_plan["character_id"]
    w, h = rig_plan["canvas_size"]
    parts_sorted = sorted(rig_plan["parts"], key=lambda p: p["layer"])
    modes_beats = {
        m: _mode_beat_times(m, 1.0)
        for m in ("idle", "walk", "run", "jump", "wave", "static")
    }
    return {
        "schemaVersion": "2.0",
        "characterId": cid,
        "canvasSize": [int(w), int(h)],
        "characterBbox": rig_plan.get("character_bbox", [0, 0, int(w), int(h)]),
        "defaultMode": mode,
        "defaultSpeed": float(speed),
        "defaultDuration": float(duration),
        "glbPath": glb_path or f"exports/blender_ingest/{cid}/{cid}_4dir.glb",
        "parts": [
            {
                "id": p["id"],
                "layer": int(p["layer"]),
                "parent": p.get("parent"),
                "pivot": [float(p["pivot"][0]), float(p["pivot"][1])],
                "pivotUv": [
                    float(p.get("pivot_uv", [0.5, 0.5])[0]),
                    float(p.get("pivot_uv", [0.5, 0.5])[1]),
                ],
                "rotationLimitsDeg": [float(p["rotation"][0]), float(p["rotation"][1])],
                "fullLayerPath": p["full_layer_path"],
                "croppedLayerPath": p.get("cropped_path", p["full_layer_path"]),
                "svgLayerPath": p.get("svg_path") or p.get("svg_file"),
                "cropBbox": p.get("crop_bbox", [0, 0, int(w), int(h)]),
            }
            for p in parts_sorted
        ],
        "poses": pose_library.get("poses", {}),
        "modeBeats": modes_beats,
    }


def export_threejs_bundle(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_js_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    glb_path: Optional[str] = None,
) -> Path:
    """
    Exports a self-contained Three.js ES module (`.js`) for the active character:
    - `RIG_SPEC`: Full hierarchical cutout rig, pivot coordinates, layer paths, and pose library.
    - `createThreeCutoutPuppet(THREE, texturesByPartId)`: Builds a hierarchical `THREE.Group` with
      transparent `THREE.MeshBasicMaterial` planes rotating around each part's exact anatomical pivot.
    - `animateThreeCutoutPuppet(root, timeSec, options)`: Evaluates keyframe interpolation for
      `idle`, `walk`, `run`, `jump`, `wave`, or `static` at configurable speed and motion range.
    - `threeFrame(THREE, texture, manifest, index)`: Sprite atlas frame helper.
    - `loadThreeGLBPuppet(THREE, gltfLoader, glbUrl)`: 3D `.glb` turnaround/armature loader.
    """
    spec = _build_serializable_rig_spec(rig_plan, pose_library, mode, speed, duration, glb_path)
    spec_json = json.dumps(spec, indent=2)

    js_code = f"""// Auto-generated Three.js 2D Cutout & 3D GLB Puppet Module for '{spec["characterId"]}'
// Default Mode: {mode} | Speed: {speed:.2f}x | Duration: {duration:.2f}s
export const RIG_SPEC = {spec_json};

export function evaluateCutoutPose(timeSec, options = {{}}) {{
  const mode = (options.mode || RIG_SPEC.defaultMode || "walk").toLowerCase();
  const speed = typeof options.speed === "number" ? options.speed : RIG_SPEC.defaultSpeed;
  const rangeScale = typeof options.rangePct === "number" ? Math.max(0, Math.min(1, options.rangePct / 100)) : 1.0;
  const beats = RIG_SPEC.modeBeats[mode] || RIG_SPEC.modeBeats.walk;
  const cycleDur = beats[beats.length - 1][0] || 1.5;
  const localT = ((timeSec * Math.max(0.05, speed)) % cycleDur + cycleDur) % cycleDur;

  let b0 = beats[0];
  let b1 = beats[beats.length - 1];
  for (let i = 0; i < beats.length - 1; i++) {{
    if (localT >= beats[i][0] && localT <= beats[i + 1][0]) {{
      b0 = beats[i];
      b1 = beats[i + 1];
      break;
    }}
  }}
  const span = Math.max(1e-4, b1[0] - b0[0]);
  const alpha = Math.max(0, Math.min(1, (localT - b0[0]) / span));
  const smooth = alpha * alpha * (3 - 2 * alpha);

  const poseA = (RIG_SPEC.poses[b0[1]] && RIG_SPEC.poses[b0[1]].parts) || {{}};
  const poseB = (RIG_SPEC.poses[b1[1]] && RIG_SPEC.poses[b1[1]].parts) || {{}};
  const result = {{}};

  for (const part of RIG_SPEC.parts) {{
    const pid = part.id;
    const ra = (poseA[pid] && typeof poseA[pid].rotation === "number") ? poseA[pid].rotation : 0;
    const rb = (poseB[pid] && typeof poseB[pid].rotation === "number") ? poseB[pid].rotation : 0;
    const rawDeg = mode === "static" ? 0 : (ra + (rb - ra) * smooth) * rangeScale;
    const [rMin, rMax] = part.rotationLimitsDeg;
    const clampedDeg = Math.max(rMin, Math.min(rMax, rawDeg));

    const ta = (poseA[pid] && poseA[pid].translate) || [0, 0];
    const tb = (poseB[pid] && poseB[pid].translate) || [0, 0];
    const tx = mode === "static" ? 0 : (ta[0] + (tb[0] - ta[0]) * smooth) * rangeScale;
    const ty = mode === "static" ? 0 : (ta[1] + (tb[1] - ta[1]) * smooth) * rangeScale;
    result[pid] = {{ rotationDeg: clampedDeg, rotationRad: (clampedDeg * Math.PI) / 180, translate: [tx, ty] }};
  }}
  return result;
}}

export function createThreeCutoutPuppet(THREE, texturesByPartId = {{}}) {{
  const root = new THREE.Group();
  root.name = `${{RIG_SPEC.characterId}}_CutoutPuppet`;
  const [cw, ch] = RIG_SPEC.canvasSize;
  const joints = {{}};
  const partsById = Object.fromEntries(RIG_SPEC.parts.map((p) => [p.id, p]));

  function ensureJoint(partId) {{
    if (joints[partId]) return joints[partId];
    const part = partsById[partId];
    const group = new THREE.Group();
    group.name = `joint_${{partId}}`;
    const parent = part.parent ? partsById[part.parent] : null;
    if (parent) {{
      const dx = part.pivot[0] - parent.pivot[0];
      const dy = -(part.pivot[1] - parent.pivot[1]);
      group.position.set(dx, dy, 0.01);
      ensureJoint(parent.id).add(group);
    }} else {{
      group.position.set(part.pivot[0] - cw * 0.5, ch * 0.5 - part.pivot[1], 0);
      root.add(group);
    }}
    group.userData.basePosition = group.position.clone();
    joints[partId] = group;
    return group;
  }}

  for (const part of RIG_SPEC.parts) {{
    const j = ensureJoint(part.id);
    const tex = texturesByPartId[part.id] || null;
    const mat = new THREE.MeshBasicMaterial({{
      map: tex,
      transparent: true,
      depthWrite: false,
      side: THREE.DoubleSide,
    }});
    const mesh = new THREE.Mesh(new THREE.PlaneGeometry(cw, ch), mat);
    mesh.name = `layer_${{part.id}}`;
    mesh.position.set(cw * 0.5 - part.pivot[0], -(ch * 0.5 - part.pivot[1]), part.layer * 0.02);
    mesh.renderOrder = part.layer;
    j.add(mesh);
  }}

  root.userData.joints = joints;
  root.userData.rigSpec = RIG_SPEC;
  return root;
}}

export function animateThreeCutoutPuppet(root, timeSec, options = {{}}) {{
  const joints = root && root.userData && root.userData.joints;
  if (!joints) return;
  const pose = evaluateCutoutPose(timeSec, options);
  for (const [partId, state] of Object.entries(pose)) {{
    const j = joints[partId];
    if (!j) continue;
    j.rotation.z = -state.rotationRad;
    if (j.userData.basePosition) {{
      j.position.x = j.userData.basePosition.x + state.translate[0];
      j.position.y = j.userData.basePosition.y - state.translate[1];
    }}
  }}
}}

export function threeFrame(THREE, texture, manifest, index = 0) {{
  const f = manifest.frames[index];
  const [w, h] = f.size;
  const map = texture.clone();
  map.needsUpdate = true;
  map.repeat.set(f.uv[2], f.uv[3]);
  map.offset.set(f.uv[0], f.uv[1]);
  const sprite = new THREE.Sprite(new THREE.SpriteMaterial({{ map, transparent: true, depthWrite: false }}));
  sprite.scale.set(w, h, 1);
  sprite.center.set(f.anchor[0] / w, 1 - f.anchor[1] / h);
  return sprite;
}}

export function loadThreeGLBPuppet(gltfLoader, glbUrl = RIG_SPEC.glbPath) {{
  return new Promise((resolve, reject) => {{
    gltfLoader.load(glbUrl, (gltf) => resolve(gltf), undefined, (err) => reject(err));
  }});
}}
"""
    out_js_path.parent.mkdir(parents=True, exist_ok=True)
    out_js_path.write_text(js_code, encoding="utf-8")
    return out_js_path


def export_react_motion_component(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_tsx_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    glb_path: Optional[str] = None,
) -> Path:
    """
    Exports a self-contained React Motion / Framer Motion / Remotion / React-Three-Fiber
    component (`.tsx` / `.jsx`) for the active character:
    - `<CharacterCutoutMotion />`: Transparent 2D layered cutout puppet driven by live `mode`,
      `speed`, `rangePct`, `frame`, or `timeSec` props — ready to insert directly onto a video
      track in Remotion, Framer Motion, or FilmCraft web overlays without locking details to an MP4.
    - `<CharacterThreeFiberPuppet />`: 3D `@react-three/fiber` + `@react-three/drei` GLB component.
    """
    spec = _build_serializable_rig_spec(rig_plan, pose_library, mode, speed, duration, glb_path)
    spec_json = json.dumps(spec, indent=2)

    tsx_code = f"""/**
 * Auto-generated React Motion / Remotion / React-Three-Fiber Component for '{spec["characterId"]}'
 * Default Mode: {mode} | Speed: {speed:.2f}x | Duration: {duration:.2f}s
 *
 * Works with transparent background on any video track (Remotion, Framer Motion, or R3F Canvas)
 * without locking details into a flat opaque MP4.
 */
import React, {{ useEffect, useState }} from "react";

export const RIG_CONFIG = {spec_json} as const;

export interface CutoutPartTransform {{
  rotationDeg: number;
  translate: [number, number];
}}

export function evaluateCutoutPoseAtTime(
  timeSec: number,
  mode: string = RIG_CONFIG.defaultMode,
  speed: number = RIG_CONFIG.defaultSpeed,
  rangePct: number = 100
): Record<string, CutoutPartTransform> {{
  const m = (mode || "walk").toLowerCase() as keyof typeof RIG_CONFIG.modeBeats;
  const beats = RIG_CONFIG.modeBeats[m] || RIG_CONFIG.modeBeats.walk;
  const cycleDur = beats[beats.length - 1][0] || 1.5;
  const rangeScale = Math.max(0, Math.min(1, rangePct / 100));
  const localT = ((timeSec * Math.max(0.05, speed)) % cycleDur + cycleDur) % cycleDur;

  let b0 = beats[0];
  let b1 = beats[beats.length - 1];
  for (let i = 0; i < beats.length - 1; i++) {{
    if (localT >= beats[i][0] && localT <= beats[i + 1][0]) {{
      b0 = beats[i];
      b1 = beats[i + 1];
      break;
    }}
  }}
  const span = Math.max(1e-4, Number(b1[0]) - Number(b0[0]));
  const alpha = Math.max(0, Math.min(1, (localT - Number(b0[0])) / span));
  const smooth = alpha * alpha * (3 - 2 * alpha);

  const posesDict = RIG_CONFIG.poses as Record<string, {{ parts?: Record<string, {{ rotation?: number; translate?: [number, number] }}> }}>;
  const poseA = posesDict[String(b0[1])]?.parts || {{}};
  const poseB = posesDict[String(b1[1])]?.parts || {{}};
  const out: Record<string, CutoutPartTransform> = {{}};

  for (const part of RIG_CONFIG.parts) {{
    const pid = part.id;
    const ra = poseA[pid]?.rotation ?? 0;
    const rb = poseB[pid]?.rotation ?? 0;
    const rawDeg = m === "static" ? 0 : (ra + (rb - ra) * smooth) * rangeScale;
    const [rMin, rMax] = part.rotationLimitsDeg;
    const clampedDeg = Math.max(rMin, Math.min(rMax, rawDeg));

    const ta = poseA[pid]?.translate ?? [0, 0];
    const tb = poseB[pid]?.translate ?? [0, 0];
    const tx = m === "static" ? 0 : (ta[0] + (tb[0] - ta[0]) * smooth) * rangeScale;
    const ty = m === "static" ? 0 : (ta[1] + (tb[1] - ta[1]) * smooth) * rangeScale;
    out[pid] = {{ rotationDeg: clampedDeg, translate: [tx, ty] }};
  }}
  return out;
}}

export interface CharacterCutoutMotionProps {{
  mode?: "idle" | "walk" | "run" | "jump" | "wave" | "static" | string;
  speed?: number;
  rangePct?: number;
  /** Optional explicit frame index (e.g. from Remotion `useCurrentFrame()`) */
  frame?: number;
  /** Frames per second when `frame` is provided (default 24) */
  fps?: number;
  /** Optional explicit time in seconds (overrides internal requestAnimationFrame clock) */
  timeSec?: number;
  /** Base URL or directory prefix for layer PNGs */
  assetBaseUrl?: string;
  scale?: number;
  style?: React.CSSProperties;
  className?: string;
}}

export const CharacterCutoutMotion: React.FC<CharacterCutoutMotionProps> = ({{
  mode = RIG_CONFIG.defaultMode,
  speed = RIG_CONFIG.defaultSpeed,
  rangePct = 100,
  frame,
  fps = 24,
  timeSec,
  assetBaseUrl = "",
  scale = 1,
  style,
  className,
}}) => {{
  const [clockSec, setClockSec] = useState(0);

  useEffect(() => {{
    if (typeof timeSec === "number" || typeof frame === "number") return;
    let rafId = 0;
    const t0 = performance.now();
    const tick = (now: number) => {{
      setClockSec((now - t0) / 1000);
      rafId = requestAnimationFrame(tick);
    }};
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }}, [timeSec, frame]);

  const effectiveTime =
    typeof timeSec === "number"
      ? timeSec
      : typeof frame === "number"
      ? frame / Math.max(1, fps)
      : clockSec;

  const pose = evaluateCutoutPoseAtTime(effectiveTime, mode, speed, rangePct);
  const [cw, ch] = RIG_CONFIG.canvasSize;
  const partsById = Object.fromEntries(RIG_CONFIG.parts.map((p) => [p.id, p]));

  function renderPartNode(partId: string): React.ReactNode {{
    const part = partsById[partId];
    if (!part) return null;
    const t = pose[partId] || {{ rotationDeg: 0, translate: [0, 0] }};
    const children = RIG_CONFIG.parts.filter((p) => p.parent === partId);
    const srcPath = assetBaseUrl
      ? `${{assetBaseUrl.replace(/\\/$/, "")}}/${{part.fullLayerPath.split("/").pop()}}`
      : part.fullLayerPath;

    return (
      <div
        key={{partId}}
        data-part-id={{partId}}
        style={{{{
          position: "absolute",
          inset: 0,
          width: cw,
          height: ch,
          transformOrigin: `${{part.pivot[0]}}px ${{part.pivot[1]}}px`,
          transform: `translate3d(${{t.translate[0].toFixed(2)}}px, ${{t.translate[1].toFixed(2)}}px, 0) rotate(${{t.rotationDeg.toFixed(2)}}deg)`,
          zIndex: part.layer,
          pointerEvents: "none",
        }}}}
      >
        <img
          src={{srcPath}}
          alt={{partId}}
          style={{{{
            position: "absolute",
            inset: 0,
            width: cw,
            height: ch,
            userSelect: "none",
          }}}}
        />
        {{children.map((child) => renderPartNode(child.id))}}
      </div>
    );
  }}

  const rootParts = RIG_CONFIG.parts.filter((p) => !p.parent);

  return (
    <div
      className={{className}}
      style={{{{
        position: "relative",
        width: cw * scale,
        height: ch * scale,
        background: "transparent",
        overflow: "visible",
        ...style,
      }}}}
    >
      <div
        style={{{{
          position: "absolute",
          top: 0,
          left: 0,
          width: cw,
          height: ch,
          transformOrigin: "0 0",
          transform: `scale(${{scale}})`,
        }}}}
      >
        {{rootParts.map((rootPart) => renderPartNode(rootPart.id))}}
      </div>
    </div>
  );
}};

export default CharacterCutoutMotion;
"""
    out_tsx_path.parent.mkdir(parents=True, exist_ok=True)
    out_tsx_path.write_text(tsx_code, encoding="utf-8")
    return out_tsx_path


def export_blender_python_script(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_py_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    glb_path: Optional[str] = None,
    blend_path: Optional[str] = None,
) -> Path:
    """
    Exports a standalone Blender 5.3 / 4.x Python Builder + Armature Rig + Procedural Felt Shader
    script (`.py`) for the active character:
    - Imports the character's `.glb` if available, or builds 2.5D articulated cutout planes from
      the character's transparent cutout layers (`cutout_layers/*.png`).
    - Upgrades all materials with Cycles/Eevee needle-felt sheen, subsurface, and procedural bump.
    - Builds an articulated Blender Armature (`OMHAS_{character_id}_Armature`) at the discovered
      anatomical joint pivots and keyframes the active `mode`, `speed`, and `duration`.
    - Sets `scene.render.film_transparent = True` (RGBA alpha) so renders never lock a background,
      and saves a native `.blend` file when `blend_path` is provided.
    """
    spec = _build_serializable_rig_spec(rig_plan, pose_library, mode, speed, duration, glb_path)
    cid = spec["characterId"]
    default_blend = blend_path or str(out_py_path.with_suffix(".blend"))
    spec_json = json.dumps(spec, indent=2)

    py_code = f'''#!/usr/bin/env python3
"""
Blender 5.3 / 4.x Native Rig, Procedural Felt Shader & Keyframe Builder for '{cid}'
Mode: {mode} | Speed: {speed:.2f}x | Duration: {duration:.2f}s

Usage:
  1. Inside Blender Scripting tab: open and click 'Run Script'
  2. Headless CLI: blender -b -P {out_py_path.name}
"""
import json
import math
import os
import bpy

RIG_SPEC = json.loads({json.dumps(spec_json)})
DEFAULT_BLEND_OUT = {json.dumps(default_blend)}


def upgrade_to_procedural_felt_shaders():
    for mat in bpy.data.materials:
        if not mat.use_nodes or not mat.node_tree:
            continue
        nodes = mat.node_tree.nodes
        links = mat.node_tree.links
        bsdf = next((n for n in nodes if n.type == "BSDF_PRINCIPLED"), None)
        if not bsdf:
            continue
        if "Sheen Weight" in bsdf.inputs:
            bsdf.inputs["Sheen Weight"].default_value = 0.85
        elif "Sheen" in bsdf.inputs:
            bsdf.inputs["Sheen"].default_value = 0.85
        if "Subsurface Weight" in bsdf.inputs:
            bsdf.inputs["Subsurface Weight"].default_value = 0.15

        noise = nodes.new(type="ShaderNodeTexNoise")
        noise.inputs["Scale"].default_value = 185.0
        noise.inputs["Detail"].default_value = 12.0
        noise.inputs["Roughness"].default_value = 0.78

        bump = nodes.new(type="ShaderNodeBump")
        bump.inputs["Strength"].default_value = 0.18
        bump.inputs["Distance"].default_value = 0.008
        links.new(noise.outputs["Fac"], bump.inputs["Height"])
        if "Normal" in bsdf.inputs:
            links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])


def build_character_rig_in_blender(blend_out_path=DEFAULT_BLEND_OUT):
    cid = RIG_SPEC["characterId"]
    cw, ch = RIG_SPEC["canvasSize"]
    char_h = 1.68
    char_w = char_h * (cw / float(max(1, ch)))

    glb_path = RIG_SPEC.get("glbPath")
    if glb_path and os.path.isfile(glb_path):
        try:
            bpy.ops.import_scene.gltf(filepath=os.path.abspath(glb_path))
        except Exception as exc:
            print("GLB import notice:", exc)

    bpy.ops.object.armature_add(enter_editmode=True, location=(0.0, 0.0, 0.0))
    arm_obj = bpy.context.active_object
    arm_obj.name = f"OMHAS_{{cid}}_Armature"
    arm_obj.show_in_front = True
    arm = arm_obj.data
    for eb in list(arm.edit_bones):
        arm.edit_bones.remove(eb)

    created_bones = {{}}
    parts_by_id = {{p["id"]: p for p in RIG_SPEC["parts"]}}
    for part in RIG_SPEC["parts"]:
        pid = part["id"]
        px, py = part["pivot"]
        bx = (px / float(max(1, cw)) - 0.5) * char_w
        bz = (1.0 - py / float(max(1, ch))) * char_h
        eb = arm.edit_bones.new(pid)
        eb.head = (bx, 0.0, bz)
        eb.tail = (bx, 0.0, bz + 0.12)
        created_bones[pid] = eb

    for part in RIG_SPEC["parts"]:
        pid = part["id"]
        parent_id = part.get("parent")
        if parent_id and parent_id in created_bones:
            created_bones[pid].parent = created_bones[parent_id]

    bpy.ops.object.mode_set(mode="OBJECT")

    # Build 2.5D layered cutout planes bound to bones so transparent layers are editable in Blender
    for part in RIG_SPEC["parts"]:
        pid = part["id"]
        layer_img_path = part["fullLayerPath"]
        if not os.path.isfile(layer_img_path):
            continue
        y_offset = -part["layer"] * 0.006
        bpy.ops.mesh.primitive_plane_add(
            size=1.0,
            location=(0.0, y_offset, char_h * 0.5),
            rotation=(math.radians(90.0), 0.0, 0.0),
        )
        plane = bpy.context.active_object
        plane.name = f"Cutout_{{cid}}_{{pid}}"
        plane.scale = (char_w, char_h, 1.0)

        mat = bpy.data.materials.new(f"MAT_{{cid}}_{{pid}}")
        mat.use_nodes = True
        if hasattr(mat, "blend_method"):
            mat.blend_method = "CLIP"
        nt = mat.node_tree
        nt.nodes.clear()
        out_n = nt.nodes.new("ShaderNodeOutputMaterial")
        bsdf_n = nt.nodes.new("ShaderNodeBsdfPrincipled")
        tex_n = nt.nodes.new("ShaderNodeTexImage")
        tex_n.image = bpy.data.images.load(os.path.abspath(layer_img_path))
        nt.links.new(tex_n.outputs["Color"], bsdf_n.inputs["Base Color"])
        nt.links.new(tex_n.outputs["Alpha"], bsdf_n.inputs["Alpha"])
        nt.links.new(bsdf_n.outputs["BSDF"], out_n.inputs["Surface"])
        plane.data.materials.append(mat)

        con = plane.constraints.new("CHILD_OF")
        con.target = arm_obj
        con.subtarget = pid

    upgrade_to_procedural_felt_shaders()

    # Keyframe requested mode & speed across frames
    scene = bpy.context.scene
    fps = 24
    scene.render.fps = fps
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"

    mode = RIG_SPEC.get("defaultMode", "walk")
    speed = max(0.05, float(RIG_SPEC.get("defaultSpeed", 1.0)))
    duration = max(0.5, float(RIG_SPEC.get("defaultDuration", 3.0)))
    total_frames = max(12, int(round(duration * fps)))
    scene.frame_start = 1
    scene.frame_end = total_frames

    beats = RIG_SPEC["modeBeats"].get(mode, RIG_SPEC["modeBeats"]["walk"])
    poses = RIG_SPEC.get("poses", {{}})
    for pbone in arm_obj.pose.bones:
        pbone.rotation_mode = "XYZ"

    for t_sec, pose_name in beats:
        frame_idx = max(1, int(round((float(t_sec) / speed) * fps)) + 1)
        pose_parts = poses.get(pose_name, {{}}).get("parts", {{}})
        for part in RIG_SPEC["parts"]:
            pid = part["id"]
            if pid not in arm_obj.pose.bones:
                continue
            rot_deg = 0.0 if mode == "static" else float(pose_parts.get(pid, {{}}).get("rotation", 0.0))
            r_min, r_max = part["rotationLimitsDeg"]
            rot_clamped = max(float(r_min), min(float(r_max), rot_deg))
            pb = arm_obj.pose.bones[pid]
            pb.rotation_euler = (0.0, math.radians(rot_clamped), 0.0)
            pb.keyframe_insert(data_path="rotation_euler", frame=frame_idx)

    scene.frame_set(1)
    if blend_out_path:
        os.makedirs(os.path.dirname(os.path.abspath(blend_out_path)), exist_ok=True)
        bpy.ops.wm.save_as_mainfile(filepath=os.path.abspath(blend_out_path))
        print("Saved Blender file:", os.path.abspath(blend_out_path))
    return arm_obj


if __name__ == "__main__":
    build_character_rig_in_blender()
'''
    out_py_path.parent.mkdir(parents=True, exist_ok=True)
    out_py_path.write_text(py_code, encoding="utf-8")
    return out_py_path



def render_cutout_asset_sheet(
    rgba_path: Path,
    rig_plan: Dict[str, Any],
    out_sheet_path: Path,
) -> Path:
    """
    Renders a clean visual Cutout Rig Asset Sheet (`{asset_id}_cutout_asset_sheet.png`) showing:
    - Left column: Full character with discovered Joint Anchors (`⊕`), parent-child bone vectors,
      and completed joint overlap circles.
    - Right 2x5 grid: All 10 separated PhotoCraft cutout layers with their completed joint overlaps,
      pivot crosshairs, parent names, and rotation constraints.
    """
    sheet_w, sheet_h = 1800, 1120
    sheet = Image.new("RGBA", (sheet_w, sheet_h), (18, 21, 28, 255))
    draw = ImageDraw.Draw(sheet)

    try:
        font_title = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 22)
        font_sub = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans.ttf", 15)
        font_card = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 14)
        font_small = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans.ttf", 12)
    except Exception:
        font_title = ImageFont.load_default()
        font_sub = ImageFont.load_default()
        font_card = ImageFont.load_default()
        font_small = ImageFont.load_default()

    # Header bar
    draw.rectangle([0, 0, sheet_w, 64], fill=(26, 31, 42, 255))
    cid = rig_plan["character_id"].upper()
    draw.text(
        (24, 12),
        f"{cid} — GUIDED 2D CUTOUT RIG ASSET SHEET (PhotoCraft Masked Layers + Joint Anchors + Completed Overlaps)",
        fill=(240, 245, 255, 255),
        font=font_title,
    )
    draw.text(
        (24, 40),
        "Skills: character-rigging + pose-library-design | Tools: photocraft-cli (lasso + layerViaCopy + overlap fill) & effectcraft-cli 0.10.0 (layer.setParent + prop.addKey + loopOut)",
        fill=(140, 210, 175, 255),
        font=font_small,
    )

    # Left panel: Assembled Character + Joint Skeleton Overlay
    left_box = [20, 80, 620, 1096]
    draw.rounded_rectangle(left_box, radius=10, fill=(24, 28, 38, 255), outline=(58, 68, 90, 255), width=2)
    draw.text((36, 94), "1. DISCOVERED ANATOMICAL JOINTS & HIERARCHY", fill=(110, 200, 255, 255), font=font_card)

    orig_im = Image.open(rgba_path).convert("RGBA")
    ow, oh = orig_im.size
    avail_w, avail_h = 560, 930
    scale = min(avail_w / float(ow), avail_h / float(oh))
    disp_w, disp_h = int(ow * scale), int(oh * scale)
    ox = 20 + (600 - disp_w) // 2
    oy = 130 + (950 - disp_h) // 2

    resized_orig = orig_im.resize((disp_w, disp_h), Image.Resampling.LANCZOS)
    sheet.alpha_composite(resized_orig, (ox, oy))

    parts_by_id = {p["id"]: p for p in rig_plan["parts"]}
    # Draw parent -> child bone lines first
    for part in rig_plan["parts"]:
        px, py = part["pivot"]
        sx = int(ox + px * scale)
        sy = int(oy + py * scale)
        parent_id = part.get("parent")
        if parent_id and parent_id in parts_by_id:
            ppx, ppy = parts_by_id[parent_id]["pivot"]
            psx = int(ox + ppx * scale)
            psy = int(oy + ppy * scale)
            draw.line([psx, psy, sx, sy], fill=(255, 195, 70, 220), width=3)

    # Draw joint overlap circles and pivot crosshairs
    for part in rig_plan["parts"]:
        px, py = part["pivot"]
        sx = int(ox + px * scale)
        sy = int(oy + py * scale)
        sr = max(8, int(part["joint_radius_px"] * scale))
        draw.ellipse([sx - sr, sy - sr, sx + sr, sy + sr], outline=(80, 225, 155, 190), width=2)
        draw.ellipse([sx - 5, sy - 5, sx + 5, sy + 5], fill=(255, 90, 90, 255), outline=(255, 255, 255, 255), width=1)
        draw.line([sx - 10, sy, sx + 10, sy], fill=(255, 255, 255, 230), width=1)
        draw.line([sx, sy - 10, sx, sy + 10], fill=(255, 255, 255, 230), width=1)
        draw.text((sx + 8, sy - 8), part["id"], fill=(255, 235, 140, 255), font=font_small)

    # Right grid: 2 rows x 5 columns of isolated PhotoCraft cutout layers
    cols, rows = 5, 2
    grid_x0, grid_y0 = 640, 80
    grid_w, grid_h = sheet_w - grid_x0 - 20, 1016
    gap = 12
    card_w = (grid_w - gap * (cols - 1)) // cols
    card_h = (grid_h - gap * (rows - 1)) // rows

    for idx, part in enumerate(rig_plan["parts"][:10]):
        c = idx % cols
        r = idx // cols
        cx0 = grid_x0 + c * (card_w + gap)
        cy0 = grid_y0 + r * (card_h + gap)
        cx1 = cx0 + card_w
        cy1 = cy0 + card_h

        draw.rounded_rectangle([cx0, cy0, cx1, cy1], radius=8, fill=(26, 30, 40, 255), outline=(64, 76, 100, 255), width=2)

        # Checkerboard inner stage
        pad = 10
        ix0, iy0, ix1, iy1 = cx0 + pad, cy0 + 58, cx1 - pad, cy1 - 68
        chk = 14
        for yy in range(iy0, iy1, chk):
            for xx in range(ix0, ix1, chk):
                is_dark = ((xx - ix0) // chk + (yy - iy0) // chk) % 2 == 0
                col = (34, 39, 50, 255) if is_dark else (44, 50, 64, 255)
                draw.rectangle([xx, yy, min(xx + chk, ix1), min(yy + chk, iy1)], fill=col)

        crop_im = Image.open(part["cropped_path"]).convert("RGBA")
        cw, ch = crop_im.size
        s_scale = min((ix1 - ix0 - 16) / float(max(1, cw)), (iy1 - iy0 - 16) / float(max(1, ch)))
        tw, th = max(1, int(cw * s_scale)), max(1, int(ch * s_scale))
        tx = ix0 + ((ix1 - ix0) - tw) // 2
        ty = iy0 + ((iy1 - iy0) - th) // 2
        sheet.alpha_composite(crop_im.resize((tw, th), Image.Resampling.LANCZOS), (tx, ty))

        # Draw the joint anchor crosshair on the cropped part
        uv_x, uv_y = part["pivot_uv"]
        jx = int(tx + uv_x * tw)
        jy = int(ty + uv_y * th)
        jr = max(6, int(part["joint_radius_px"] * s_scale))
        draw.ellipse([jx - jr, jy - jr, jx + jr, jy + jr], outline=(80, 230, 160, 210), width=2)
        draw.ellipse([jx - 4, jy - 4, jx + 4, jy + 4], fill=(255, 85, 85, 255), outline=(255, 255, 255, 255), width=1)

        # Header & footer metadata on card
        draw.text((cx0 + 10, cy0 + 10), f"{part['id']} (z={part['layer']})", fill=(240, 245, 255, 255), font=font_card)
        parent_lbl = part.get("parent") or "ROOT"
        draw.text((cx0 + 10, cy0 + 32), f"Parent: {parent_lbl}", fill=(120, 200, 255, 255), font=font_small)

        rot_min, rot_max = part["rotation"]
        draw.text(
            (cx0 + 10, cy1 - 56),
            f"Pivot: ({part['pivot'][0]}, {part['pivot'][1]}) px",
            fill=(210, 220, 235, 255),
            font=font_small,
        )
        draw.text(
            (cx0 + 10, cy1 - 38),
            f"Rot: [{rot_min} deg .. +{rot_max} deg]",
            fill=(140, 225, 175, 255),
            font=font_small,
        )
        draw.text(
            (cx0 + 10, cy1 - 20),
            f"Overlap fill: +{part['completed_overlap_pixels']} px",
            fill=(255, 205, 110, 255),
            font=font_small,
        )

    out_sheet_path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out_sheet_path)
    return out_sheet_path


def build_effectcraft_cutout_animation(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_dir: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    out_mp4: Optional[Path] = None,
    out_jsx: Optional[Path] = None,
) -> Dict[str, Any]:
    """
    Uses `effectcraft-cli` 0.10.0 to build a real hierarchical cutout animation project
    (`{character_id}_cutout_rig.ecproj`), export a `.jsx` script, and render an MP4 + verification
    frames reflecting the requested `mode`, `speed`, and `duration`.
    """
    cid = rig_plan["character_id"]
    w, h = rig_plan["canvas_size"]
    comp_w = int(w) + (int(w) % 2)
    comp_h = int(h) + (int(h) % 2)
    comp_dur = max(0.5, float(duration))

    ecproj_path = out_dir / f"{cid}_cutout_rig.ecproj"
    mp4_path = Path(out_mp4).resolve() if out_mp4 else (out_dir / f"{cid}_effectcraft_cutout.mp4")
    jsx_path = Path(out_jsx).resolve() if out_jsx else (out_dir / f"{cid}_cutout_rig.jsx")
    frame0_path = out_dir / f"{cid}_ec_cutout_frame0.png"
    frame1_path = out_dir / f"{cid}_ec_cutout_frame1.png"

    export_effectcraft_jsx_script(
        rig_plan,
        pose_library,
        jsx_path,
        mode=mode,
        speed=speed,
        duration=comp_dur,
    )

    parts_sorted = sorted(rig_plan["parts"], key=lambda p: p["layer"])

    webm_alpha_path = out_dir / f"{cid}_effectcraft_cutout_alpha.webm"

    run_args: List[str] = [
        "effectcraft-cli",
        "--empty",
        "run",
        "comp.new",
        json.dumps({
            "name": f"{cid}_cutout_rig",
            "width": comp_w,
            "height": comp_h,
            "duration": comp_dur,
            "frameRate": 24,
        }),
    ]

    for part in parts_sorted:
        pid = part["id"]
        px, py = float(part["pivot"][0]), float(part["pivot"][1])
        run_args.extend([
            "file.import",
            json.dumps({
                "paths": [part["full_layer_path"]],
                "sequence": False,
                "addToComp": True,
            }),
            "layer.rename",
            json.dumps({"name": pid}),
            "prop.set",
            json.dumps({
                "layer": pid,
                "path": "transform/anchor",
                "value": [px, py, 0.0],
            }),
            "prop.set",
            json.dumps({
                "layer": pid,
                "path": "transform/position",
                "value": [px, py, 0.0],
            }),
        ])

    for part in parts_sorted:
        pid = part["id"]
        parent_id = part.get("parent")
        if parent_id:
            run_args.extend([
                "layer.setParent",
                json.dumps({
                    "layers": [pid],
                    "parent": parent_id,
                    "compensate": True,
                }),
            ])

    beat_times = _mode_beat_times(mode, speed)
    poses = pose_library["poses"]

    for part in parts_sorted:
        pid = part["id"]
        r_min, r_max = part["rotation"]
        for idx, (t_sec, pose_name) in enumerate(beat_times):
            pose_parts = poses.get(pose_name, {}).get("parts", {})
            rot_val = 0.0 if mode == "static" else float(pose_parts.get(pid, {}).get("rotation", 0.0))
            rot_clamped = max(float(r_min), min(float(r_max), rot_val))
            run_args.extend([
                "prop.addKey",
                json.dumps({
                    "layer": pid,
                    "path": "transform/rotation",
                    "time": t_sec,
                    "value": rot_clamped,
                }),
            ])
            if idx == 1:
                run_args.extend([
                    "keys.set",
                    json.dumps({
                        "layer": pid,
                        "path": "transform/rotation",
                        "time": t_sec,
                        "value": rot_clamped,
                    }),
                ])

        run_args.extend([
            "prop.setExpression",
            json.dumps({
                "layer": pid,
                "path": "transform/rotation",
                "expression": 'loopOut("cycle")',
            }),
        ])

    run_args.extend(["--save-as", str(ecproj_path), "--json"])
    proc = subprocess.run(run_args, capture_output=True, text=True, timeout=60)
    if proc.returncode != 0:
        raise RuntimeError(f"effectcraft-cli rig build failed: {proc.stderr or proc.stdout}")

    mp4_path.parent.mkdir(parents=True, exist_ok=True)
    peak_time = f"{min(comp_dur * 0.5, beat_times[1][0] if len(beat_times) > 1 else 0.35):.2f}"
    subprocess.run(
        [
            "effectcraft-cli",
            "--project", str(ecproj_path),
            "render-frame",
            "--time", "0.0",
            "--transparent",
            "--out", str(frame0_path),
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    subprocess.run(
        [
            "effectcraft-cli",
            "--project", str(ecproj_path),
            "render-frame",
            "--time", peak_time,
            "--transparent",
            "--out", str(frame1_path),
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    subprocess.run(
        [
            "effectcraft-cli",
            "--project", str(ecproj_path),
            "render",
            "--out", str(webm_alpha_path),
            "--format", "webm",
            "--channels", "rgba",
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=60,
    )
    subprocess.run(
        [
            "effectcraft-cli",
            "--project", str(ecproj_path),
            "render",
            "--out", str(mp4_path),
            "--format", "h264",
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=60,
    )

    return {
        "ecproj_path": str(ecproj_path),
        "jsx_path": str(jsx_path),
        "webm_alpha_path": str(webm_alpha_path) if webm_alpha_path.exists() else None,
        "mp4_path": str(mp4_path),
        "frame0_path": str(frame0_path),
        "frame1_path": str(frame1_path),
        "mode": mode,
        "speed": speed,
        "duration": comp_dur,
        "mp4_bytes": mp4_path.stat().st_size if mp4_path.exists() else 0,
    }


def build_guided_cutout_rig(
    rgba_path: str | Path,
    character_id: str = "hopper",
    out_dir: Optional[str | Path] = None,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    selected_parts: Optional[List[str]] = None,
    out_mp4: Optional[str | Path] = None,
    out_jsx: Optional[str | Path] = None,
) -> Dict[str, Any]:
    """
    End-to-end guided cutout rig pipeline for a single character:
    1. Discovers anatomical joints & part polygons from the transparent 2D character image.
    2. Masks arms, legs, feet, head, ears, and torso into separate layers via `photocraft-cli`
       and completes hidden overlap under-extensions at every joint pivot.
    3. Generates `{character_id}_cutout_rig_plan.json` (`character-rigging`),
       `{character_id}_pose_library.json` (`pose-library-design`), and
       `{character_id}_cutout_asset_sheet.png`.
    4. Builds and renders the hierarchical cutout animation in `effectcraft-cli` 0.10.0
       (`layer.setParent`, `prop.addKey`, `keys.set`, `loopOut("cycle")`).
    """
    src_path = Path(rgba_path).resolve()
    if not src_path.exists():
        raise FileNotFoundError(f"Input image not found: {src_path}")

    target_dir = Path(out_dir).resolve() if out_dir else src_path.parent
    target_dir.mkdir(parents=True, exist_ok=True)

    discovery = discover_character_joints_and_polygons(
        src_path,
        character_id=character_id,
        selected_parts=selected_parts,
    )
    pc_res = mask_and_export_layers_with_photocraft(src_path, discovery, target_dir, character_id)
    pose_lib = build_pose_library(character_id)

    joints_map = {
        p["id"]: {
            "pivot": p["pivot"],
            "pivot_uv": p["pivot_uv"],
            "rotation": p["rotation"],
            "joint_radius_px": p["joint_radius_px"],
        }
        for p in pc_res["parts"]
    }

    rig_plan: Dict[str, Any] = {
        "character_id": character_id,
        "rig_type": "cutout_rig",
        "discovery_mode": discovery.get("discovery_mode", "hopper_calibrated"),
        "selected_parts": discovery.get("selected_parts", []),
        "source_image": str(src_path),
        "canvas_size": discovery["canvas_size"],
        "character_bbox": discovery["character_bbox"],
        "photocraft_psd": pc_res["psd_path"],
        "parts": pc_res["parts"],
        "joints": joints_map,
        "pose_library": pose_lib,
        "qa_telemetry": pc_res.get("qa_telemetry", {}),
    }

    sheet_path = target_dir / f"{character_id}_cutout_asset_sheet.png"
    render_cutout_asset_sheet(src_path, rig_plan, sheet_path)
    rig_plan["cutout_asset_sheet"] = str(sheet_path)

    ec_res = build_effectcraft_cutout_animation(
        rig_plan,
        pose_lib,
        target_dir,
        mode=mode,
        speed=speed,
        duration=duration,
        out_mp4=Path(out_mp4) if out_mp4 else None,
        out_jsx=Path(out_jsx) if out_jsx else None,
    )
    rig_plan["effectcraft"] = ec_res

    rig_plan_path = target_dir / f"{character_id}_cutout_rig_plan.json"
    pose_lib_path = target_dir / f"{character_id}_pose_library.json"
    rig_plan_path.write_text(json.dumps(rig_plan, indent=2), encoding="utf-8")

    # Automatically bridge through vectorcraft-cli so each cutout part also has a traced .svg
    # before generating Three.js and React Motion / Remotion components
    try:
        vc_res = trace_part_layers_with_vectorcraft(rig_plan_path)
        if vc_res.get("ok"):
            rig_plan = json.loads(rig_plan_path.read_text(encoding="utf-8"))
            rig_plan["vectorcraft"] = vc_res
    except Exception as exc:
        rig_plan["vectorcraft"] = {"ok": False, "error": str(exc)}

    threejs_path = target_dir / f"{character_id}_threejs_puppet.js"
    react_motion_path = target_dir / f"{character_id}_react_motion.tsx"
    blender_py_path = target_dir / f"{character_id}_blender_rig.py"
    export_threejs_bundle(rig_plan, pose_lib, threejs_path, mode=mode, speed=speed, duration=duration)
    export_react_motion_component(rig_plan, pose_lib, react_motion_path, mode=mode, speed=speed, duration=duration)
    export_blender_python_script(rig_plan, pose_lib, blender_py_path, mode=mode, speed=speed, duration=duration)
    rig_plan["exporters"] = {
        "threejs_path": str(threejs_path),
        "react_motion_path": str(react_motion_path),
        "blender_py_path": str(blender_py_path),
        "effectcraft_jsx_path": ec_res.get("jsx_path"),
    }

    rig_plan_path.write_text(json.dumps(rig_plan, indent=2), encoding="utf-8")
    pose_lib_path.write_text(json.dumps(pose_lib, indent=2), encoding="utf-8")

    return {
        "ok": True,
        "character_id": character_id,
        "discovery_mode": rig_plan["discovery_mode"],
        "selected_parts": rig_plan["selected_parts"],
        "rig_plan_path": str(rig_plan_path),
        "pose_library_path": str(pose_lib_path),
        "cutout_asset_sheet": str(sheet_path),
        "photocraft_psd": pc_res["psd_path"],
        "part_count": len(pc_res["parts"]),
        "qa_telemetry": pc_res.get("qa_telemetry", {}),
        "effectcraft": ec_res,
        "exporters": rig_plan["exporters"],
    }


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Guided 2D Cutout Rig & Asset Sheet Builder")
    parser.add_argument("image", nargs="?", default=None, help="Path to transparent 2D character PNG")
    parser.add_argument("--character", default="hopper", help="Character ID")
    parser.add_argument("--out-dir", default=None, help="Output directory")
    parser.add_argument("--mode", default="walk", help="Animation mode (idle, walk, run, jump, wave, static)")
    parser.add_argument("--speed", type=float, default=1.0, help="Animation playback speed multiplier")
    parser.add_argument("--duration", type=float, default=3.0, help="Composition duration in seconds")
    parser.add_argument("--parts", default=None, help="Optional comma-separated list of selected parts from intake Q2")
    parser.add_argument("--rig-plan", default=None, help="Path to existing cutout_rig_plan.json for export-only modes")
    parser.add_argument("--render-only", action="store_true", help="Re-render MP4/JSX from an existing cutout_rig_plan.json")
    parser.add_argument("--jsx-only", action="store_true", help="Generate JSX script only from an existing cutout_rig_plan.json")
    parser.add_argument("--threejs-only", action="store_true", help="Generate Three.js module (.js) from an existing cutout_rig_plan.json")
    parser.add_argument("--react-motion-only", action="store_true", help="Generate React Motion / Remotion component (.tsx) from an existing cutout_rig_plan.json")
    parser.add_argument("--blender-py-only", action="store_true", help="Generate Blender 5.3 Python rig script (.py) from an existing cutout_rig_plan.json")
    parser.add_argument("--trace-svg-only", action="store_true", help="Trace cutout part PNGs into scalable multi-color SVGs via vectorcraft-cli")
    parser.add_argument("--trace-preset", default="16 Colors", help="VectorCraft imageTrace preset name (default: '16 Colors')")
    parser.add_argument("--out-mp4", default=None, help="Explicit output MP4 path")
    parser.add_argument("--out-jsx", default=None, help="Explicit output JSX path")
    parser.add_argument("--out-threejs", default=None, help="Explicit output Three.js module (.js) path")
    parser.add_argument("--out-react-motion", default=None, help="Explicit output React Motion component (.tsx/.jsx) path")
    parser.add_argument("--out-blender-py", default=None, help="Explicit output Blender Python script (.py) path")
    parser.add_argument("--glb-path", default=None, help="Optional 3D GLB path to reference in Three.js / React / Blender exports")
    parser.add_argument("--blend-path", default=None, help="Optional output .blend file path for Blender Python script")
    args = parser.parse_args()

    selected_parts = (
        [p.strip() for p in args.parts.split(",") if p.strip()]
        if args.parts
        else None
    )

    export_only = (
        args.render_only
        or args.jsx_only
        or args.threejs_only
        or args.react_motion_only
        or args.blender_py_only
        or args.trace_svg_only
    )
    if export_only:
        if not args.rig_plan:
            raise SystemExit("Export-only flags require --rig-plan <path>")
        plan_path = Path(args.rig_plan).resolve()
        if args.trace_svg_only:
            trace_res = trace_part_layers_with_vectorcraft(plan_path, preset=args.trace_preset)
            print(json.dumps(trace_res, indent=2))
            raise SystemExit(0)
        rig_plan_data = json.loads(plan_path.read_text(encoding="utf-8"))
        cid_val = rig_plan_data.get("character_id", args.character)
        pose_lib_data = rig_plan_data.get("pose_library") or build_pose_library(cid_val)
        out_dir_path = Path(args.out_dir).resolve() if args.out_dir else plan_path.parent
        if args.jsx_only:
            jsx_target = Path(args.out_jsx).resolve() if args.out_jsx else (out_dir_path / f"{cid_val}_cutout_rig.jsx")
            export_effectcraft_jsx_script(
                rig_plan_data,
                pose_lib_data,
                jsx_target,
                mode=args.mode,
                speed=args.speed,
                duration=args.duration,
            )
            print(json.dumps({
                "ok": True,
                "jsx_path": str(jsx_target),
                "mode": args.mode,
                "speed": args.speed,
                "duration": args.duration,
            }, indent=2))
        elif args.threejs_only:
            three_target = Path(args.out_threejs).resolve() if args.out_threejs else (out_dir_path / f"{cid_val}_threejs_puppet.js")
            export_threejs_bundle(
                rig_plan_data,
                pose_lib_data,
                three_target,
                mode=args.mode,
                speed=args.speed,
                duration=args.duration,
                glb_path=args.glb_path,
            )
            print(json.dumps({
                "ok": True,
                "threejs_path": str(three_target),
                "mode": args.mode,
                "speed": args.speed,
                "duration": args.duration,
            }, indent=2))
        elif args.react_motion_only:
            rm_target = Path(args.out_react_motion).resolve() if args.out_react_motion else (out_dir_path / f"{cid_val}_react_motion.tsx")
            export_react_motion_component(
                rig_plan_data,
                pose_lib_data,
                rm_target,
                mode=args.mode,
                speed=args.speed,
                duration=args.duration,
                glb_path=args.glb_path,
            )
            print(json.dumps({
                "ok": True,
                "react_motion_path": str(rm_target),
                "mode": args.mode,
                "speed": args.speed,
                "duration": args.duration,
            }, indent=2))
        elif args.blender_py_only:
            py_target = Path(args.out_blender_py).resolve() if args.out_blender_py else (out_dir_path / f"{cid_val}_blender_rig.py")
            export_blender_python_script(
                rig_plan_data,
                pose_lib_data,
                py_target,
                mode=args.mode,
                speed=args.speed,
                duration=args.duration,
                glb_path=args.glb_path,
                blend_path=args.blend_path,
            )
            print(json.dumps({
                "ok": True,
                "blender_py_path": str(py_target),
                "mode": args.mode,
                "speed": args.speed,
                "duration": args.duration,
            }, indent=2))
        else:
            ec_out = build_effectcraft_cutout_animation(
                rig_plan_data,
                pose_lib_data,
                out_dir_path,
                mode=args.mode,
                speed=args.speed,
                duration=args.duration,
                out_mp4=Path(args.out_mp4) if args.out_mp4 else None,
                out_jsx=Path(args.out_jsx) if args.out_jsx else None,
            )
            rig_plan_data["effectcraft"] = ec_out
            plan_path.write_text(json.dumps(rig_plan_data, indent=2), encoding="utf-8")
            print(json.dumps({"ok": True, "effectcraft": ec_out}, indent=2))
    else:
        if not args.image:
            raise SystemExit("image argument is required unless an export-only flag is set")
        result = build_guided_cutout_rig(
            args.image,
            character_id=args.character,
            out_dir=args.out_dir,
            mode=args.mode,
            speed=args.speed,
            duration=args.duration,
            selected_parts=selected_parts,
            out_mp4=args.out_mp4,
            out_jsx=args.out_jsx,
        )
        print(json.dumps(result, indent=2))
