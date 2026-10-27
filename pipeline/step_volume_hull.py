#!/usr/bin/env python3
"""Stage 4A: 2D Turnaround + Motion Range -> 3D Visual Hull (`visual_hull.npz`).

First sequential stage of the refactored 3D+2D animation pipeline:
reads 2D turnaround views + measured 2D `motion_range.json` (`footLiftPx`)
and carves a closed 3D visual hull (`visual_hull.npz`) with view-aligned projection textures.
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any, Dict

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.character_iteration_exporter import (
    CHARACTER_PRESETS,
    POSE_PARTS_ROOT,
    SHARE_ROOT,
)

ROOT = Path(__file__).resolve().parents[1]
SCHOOL_BG = Path("/home/jesse/main-vault/Creative/Old MacDonald/oldmac-school.png")
CLASS_BG = Path("/home/jesse/main-vault/Creative/old-macs-class.png")

SIX_CAST_ORDER = [
    "miss_hayley",
    "old_mac",
    "hopper",
    "whiskers",
    "puddles",
    "rusty",
]

CHARACTER_STAGE_SPECS: Dict[str, Dict[str, Any]] = {
    "miss_hayley": {
        "role": "teacher",
        "height_m": 1.82,
        "action_style": "teacher_welcome",
        "start_xy": (-2.10, 0.88),
        "end_xy": (-2.00, 0.82),
        "start_yaw_deg": 16.0,
        "facing_yaw_deg": 12.0,
    },
    "old_mac": {
        "role": "teacher",
        "height_m": 1.84,
        "action_style": "principal_beckon",
        "start_xy": (2.10, 0.88),
        "end_xy": (2.00, 0.82),
        "start_yaw_deg": -16.0,
        "facing_yaw_deg": -12.0,
    },
    "hopper": {
        "role": "student",
        "height_m": 1.34,
        "action_style": "bunny_hop_walk",
        "start_xy": (-1.78, -0.85),
        "end_xy": (-1.26, -0.15),
        "start_yaw_deg": 22.0,
        "facing_yaw_deg": 0.0,
    },
    "whiskers": {
        "role": "student",
        "height_m": 1.26,
        "action_style": "cat_walk_wave",
        "start_xy": (-0.78, -1.00),
        "end_xy": (-0.42, -0.15),
        "start_yaw_deg": 14.0,
        "facing_yaw_deg": 0.0,
    },
    "puddles": {
        "role": "student",
        "height_m": 1.20,
        "action_style": "duck_waddle_hop",
        "start_xy": (0.78, -1.00),
        "end_xy": (0.42, -0.15),
        "start_yaw_deg": -14.0,
        "facing_yaw_deg": 0.0,
    },
    "rusty": {
        "role": "student",
        "height_m": 1.40,
        "action_style": "foal_trot_wave",
        "start_xy": (1.78, -0.85),
        "end_xy": (1.26, -0.15),
        "start_yaw_deg": -22.0,
        "facing_yaw_deg": 0.0,
    },
    "jenny": {
        "role": "student",
        "height_m": 1.15,
        "action_style": "chick_hop_wave",
        "start_xy": (0.0, -1.05),
        "end_xy": (0.0, -0.18),
        "start_yaw_deg": 10.0,
        "facing_yaw_deg": 0.0,
    },
}

VIEW_ANGLES: Dict[str, list[int]] = {
    "hopper": [0, -45, -90, 45, 180],
    "whiskers": [0, -45, 90, 135, 180],
    "puddles": [0, -45, -90, 45, 180],
    "rusty": [0, 45, 90, -45, 180],
    "jenny": [0, -45, 90, 45, 180],
    "miss_hayley": [0, -45, -90, 45, 180],
    "old_mac": [0, -45, -90, 45, 180],
}

ANATOMY: Dict[str, Dict[str, float]] = {
    "hopper": dict(hip=0.265, knee=0.155, ankle=0.055, neck=0.535, shoulder=0.49, elbow=0.37, wrist=0.29, ear=0.79, leg_x=0.075, arm_x=0.16),
    "whiskers": dict(hip=0.28, knee=0.16, ankle=0.045, neck=0.57, shoulder=0.52, elbow=0.40, wrist=0.32, ear=0.89, leg_x=0.075, arm_x=0.17),
    "puddles": dict(hip=0.19, knee=0.12, ankle=0.045, neck=0.56, shoulder=0.49, elbow=0.37, wrist=0.29, ear=0.91, leg_x=0.085, arm_x=0.16),
    "rusty": dict(hip=0.33, knee=0.19, ankle=0.055, neck=0.60, shoulder=0.54, elbow=0.43, wrist=0.34, ear=0.88, leg_x=0.065, arm_x=0.14),
    "jenny": dict(hip=0.17, knee=0.10, ankle=0.045, neck=0.55, shoulder=0.48, elbow=0.38, wrist=0.30, ear=0.90, leg_x=0.075, arm_x=0.24),
    "miss_hayley": dict(hip=0.36, knee=0.19, ankle=0.075, neck=0.70, shoulder=0.64, elbow=0.49, wrist=0.37, ear=0.91, leg_x=0.065, arm_x=0.165),
    "old_mac": dict(hip=0.28, knee=0.16, ankle=0.065, neck=0.60, shoulder=0.55, elbow=0.40, wrist=0.28, ear=0.88, leg_x=0.085, arm_x=0.19),
}

PERFORMANCES: Dict[str, Dict[str, Any]] = {
    "hopper": dict(period=10, offset=0, lift=0.80, sway=0.010, gesture="ear_perk"),
    "whiskers": dict(period=12, offset=3, lift=0.55, sway=0.006, gesture="small_wave"),
    "puddles": dict(period=8, offset=1, lift=0.65, sway=0.024, gesture="wing_greeting"),
    "rusty": dict(period=10, offset=2, lift=0.70, sway=0.012, gesture="confident_wave"),
    "jenny": dict(period=8, offset=1, lift=0.70, sway=0.020, gesture="wing_greeting"),
    "miss_hayley": dict(period=12, offset=0, lift=0.0, sway=0.003, gesture="welcome"),
    "old_mac": dict(period=14, offset=4, lift=0.0, sway=0.002, gesture="beckon"),
}


def resolve_character_assets(slug: str, reference_root: Path | None = None) -> dict:
    """Read existing 2D turnaround views and measured 2D motion_range.json."""
    if slug not in CHARACTER_STAGE_SPECS:
        raise ValueError(f"Unknown character: {slug}")
    preset = CHARACTER_PRESETS.get(slug, {"title": slug})
    motion_path = SHARE_ROOT / slug / "motion_range.json"
    motion = json.loads(motion_path.read_text())
    lift = motion.get("limits", {}).get("footLiftPx", {}).get("max")
    still_h = motion.get("still", {}).get("heightPx")
    if lift is None or still_h is None or not math.isfinite(float(lift)) or float(still_h) <= 0:
        raise ValueError(f"{slug}: measured footLiftPx.max and standing height are required: {motion_path}")
    views = []
    if slug in ("miss_hayley", "old_mac"):
        reference_root = reference_root or ROOT / "exports/farm_school_milestone/references"
        sheet = reference_root / slug / "turnaround.png"
        provenance = reference_root / slug / "provenance.json"
        if not sheet.is_file() or not provenance.is_file():
            raise ValueError(f"{slug}: a genuine or explicitly generated five-view sheet with provenance is required at {sheet}")
        for i, (label, angle) in enumerate(zip(["front", "quarter_front", "side", "opposite_quarter", "back"], VIEW_ANGLES[slug])):
            views.append(dict(label=label, angle=angle, sheet=str(sheet), cell=i))
        provenance_data = json.loads(provenance.read_text())
    else:
        reg_path = SHARE_ROOT / slug / "turnaround_registration.json"
        reg = json.loads(reg_path.read_text())
        for fr, angle in zip(reg["frames"], VIEW_ANGLES[slug]):
            views.append(dict(
                label=fr["label"],
                angle=angle,
                file=str(Path(reg["assetRoot"]) / fr["file"]),
                baseline=fr["registered"]["footBaselineY"],
                crown=fr["registered"]["crownY"],
                center=fr["registered"]["footCenterX"],
            ))
        provenance_data = dict(
            kind="existing_turnaround_artwork",
            registration=str(reg_path),
            source=json.loads((Path(preset["turnaround_dir"]) / "manifest.json").read_text())["sourceImage"],
        )
    return dict(
        slug=slug,
        title=preset.get("title", slug),
        **CHARACTER_STAGE_SPECS[slug],
        anatomy=ANATOMY[slug],
        performance=PERFORMANCES[slug],
        views=views,
        foot_lift_max_px=float(lift),
        standing_height_px=float(still_h),
        foot_lift_max_m=max(0.0, float(lift)) / float(still_h) * CHARACTER_STAGE_SPECS[slug]["height_m"],
        motion_source=str(motion_path),
        motion_kind=motion.get("kind"),
        provenance=provenance_data,
    )


def _strip_enclosed_background_with_photocraft(image: "Image.Image") -> "Image.Image":
    """Strip any enclosed studio background pocket (e.g. #dcdcdc between legs) via photocraft-cli."""
    import numpy as np
    import shutil
    import subprocess
    import tempfile

    arr = np.array(image)
    alpha = arr[..., 3]
    rgb = arr[..., :3].astype(np.int16)
    chroma = rgb.max(axis=2) - rgb.min(axis=2)
    luma = rgb.min(axis=2)
    neutral_light = (alpha > 180) & (chroma <= 18) & (luma >= 195)
    if int(neutral_light.sum()) < 20:
        return image
    med = np.median(arr[..., :3][neutral_light], axis=0).astype(int)
    bg_hex = f"#{int(med[0]):02x}{int(med[1]):02x}{int(med[2]):02x}"
    pc_bin = shutil.which("photocraft-cli")
    if not pc_bin:
        arr[neutral_light, 3] = 0
        from PIL import Image
        return Image.fromarray(arr)
    with tempfile.TemporaryDirectory(prefix="pc_hull_") as td:
        in_p, out_p = Path(td) / "in.png", Path(td) / "out.png"
        image.save(in_p)
        proc = subprocess.run([
            pc_bin, "run", str(in_p),
            "--cmd", "layer.removeBackground", "--params", json.dumps({"refine": True}),
            "--cmd", "select.colorRange", "--params", json.dumps({"color": bg_hex, "fuzziness": 22}),
            "--cmd", "layer.layerMask.hideSelection", "--params", "{}",
            "--cmd", "layer.matting.colorDecontaminate", "--params", json.dumps({"amount": 50}),
            "--out", str(out_p),
        ], capture_output=True, text=True, timeout=60)
        if proc.returncode == 0 and out_p.is_file():
            from PIL import Image
            with Image.open(out_p) as cleaned:
                return cleaned.convert("RGBA")
    return image


def prepare_volume(spec: dict, out_dir: Path, resolution: int = 144) -> dict:
    """Intersect the supplied silhouette cones; retain real image projections on the closed hull."""
    import numpy as np
    from PIL import Image
    from scipy import ndimage

    out_dir.mkdir(parents=True, exist_ok=True)
    height = spec["height_m"]
    textures = []
    for vi, view in enumerate(spec["views"]):
        if "sheet" in view:
            sheet = Image.open(view["sheet"]).convert("RGBA")
            sheet_rgba = np.array(sheet)
            components, _ = ndimage.label(sheet_rgba[..., 3] >= 128)
            sizes = np.bincount(components.ravel())
            sizes[0] = 0
            ids = sorted(np.argsort(sizes)[-5:], key=lambda k: float(np.where(components == k)[1].mean()))
            if len(ids) != 5 or min(sizes[k] for k in ids) < 1000:
                raise ValueError(f"{spec['slug']}: teacher sheet must contain five separate full-body figures")
            component = components == ids[vi]
            yy0, xx0 = np.where(component)
            sheet_rgba[..., 3] = np.where(component, sheet_rgba[..., 3], 0)
            image = Image.fromarray(sheet_rgba).crop((max(0, int(xx0.min()) - 4), 0, min(sheet.width, int(xx0.max()) + 5), sheet.height))
        else:
            image = _strip_enclosed_background_with_photocraft(Image.open(view["file"]).convert("RGBA"))
        rgba = np.array(image)
        mask = rgba[..., 3] >= 128
        labels, count = ndimage.label(mask)
        if count == 0:
            raise ValueError(f"{spec['slug']} {view['label']}: empty silhouette")
        areas = np.bincount(labels.ravel())
        areas[0] = 0
        mask = labels == areas.argmax()
        yy, xx = np.where(mask)
        crown, baseline = int(yy.min()), int(yy.max()) + 1
        if "sheet" in view:
            foot = mask[max(crown, baseline - max(2, (baseline - crown) // 10)):baseline]
            fx = np.where(foot)[1]
            center = (float(fx.min()) + float(fx.max())) / 2 if len(fx) else float(np.median(xx))
        else:
            crown, baseline, center = view["crown"], view["baseline"], view["center"]
        scale = (baseline - crown) / height
        eroded = ndimage.binary_erosion(mask, iterations=2)
        color_src = eroded if int(eroded.sum()) >= 100 else mask
        nearest = ndimage.distance_transform_edt(~color_src, return_distances=False, return_indices=True)
        rgb = rgba[..., :3][tuple(nearest)]
        path = out_dir / f"{vi:02d}_{view['label']}_projection.png"
        Image.fromarray(rgb.astype("uint8")).save(path)
        textures.append(dict(
            view,
            texture=str(path.resolve()),
            center=center,
            baseline=baseline,
            pixels_per_m=scale,
            size=list(image.size),
            silhouette=mask,
            distance=ndimage.distance_transform_edt(mask) - ndimage.distance_transform_edt(~mask),
        ))
    front = textures[0]
    fy, fx = np.where(front["silhouette"])
    half_x = max(abs(fx - front["center"])) / front["pixels_per_m"] + height * 0.025
    profile = next(v for v in textures if abs(v["angle"]) == 90)
    py, px = np.where(profile["silhouette"])
    half_y = max(abs(px - profile["center"])) / profile["pixels_per_m"] + height * 0.025
    step = height / resolution
    axes = [
        np.arange(-half_x, half_x + step, step, dtype=np.float32),
        np.arange(-half_y, half_y + step, step, dtype=np.float32),
        np.arange(-step * 2, height + step * 3, step, dtype=np.float32),
    ]
    x, y, z = np.meshgrid(*axes, indexing="ij")
    field = np.full(x.shape, 100.0, dtype=np.float32)
    for view in textures:
        angle = math.radians(view["angle"])
        u = view["center"] + (x * math.cos(angle) + y * math.sin(angle)) * view["pixels_per_m"]
        v = view["baseline"] - z * view["pixels_per_m"]
        sdf = ndimage.map_coordinates(view["distance"], [v, u], order=1, mode="constant", cval=-10000)
        field = np.minimum(field, sdf / view["pixels_per_m"])
    occupied = ndimage.gaussian_filter(field, 0.65) > -step * 0.35
    occupied[[0, -1], :, :] = False
    occupied[:, [0, -1], :] = False
    occupied[:, :, [0, -1]] = False
    labs, _ = ndimage.label(occupied)
    counts = np.bincount(labs.ravel())
    counts[0] = 0
    occupied = labs == counts.argmax()
    if occupied.sum() < 100:
        raise ValueError(f"{spec['slug']}: supplied views do not produce a usable common volume")
    face_corners = [
        [(0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0)], [(1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)],
        [(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)], [(0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0)],
        [(0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)], [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)],
    ]
    faces = []
    for axis in range(3):
        for positive in [False, True]:
            neighbor = np.roll(occupied, -1 if positive else 1, axis=axis)
            cells = np.argwhere(occupied & ~neighbor)
            faces.append(cells[:, None, :] + np.array(face_corners[axis * 2 + positive])[None, :, :])
    corners = np.concatenate(faces).reshape(-1, 3)
    unique, inverse = np.unique(corners, axis=0, return_inverse=True)
    vertices = unique.astype(np.float32) * step + np.array([a[0] - step / 2 for a in axes])
    quads = inverse.reshape(-1, 4)
    mesh_path = out_dir / "visual_hull.npz"
    np.savez_compressed(mesh_path, vertices=vertices.astype(np.float32), faces=quads.astype(np.int32))
    clean_views = [{k: v for k, v in t.items() if k not in ("silhouette", "distance")} for t in textures]
    spec = dict(
        spec,
        views=clean_views,
        mesh_file=str(mesh_path.resolve()),
        output_dir=str(out_dir.resolve()),
        reconstruction=dict(
            method="five_view_silhouette_intersection",
            resolution=resolution,
            vertices=len(vertices),
            faces=len(quads),
            joint_landmarks="authored_from_reference_proportions",
        ),
    )
    (out_dir / "asset_sources.json").write_text(json.dumps(spec, indent=2) + "\n")
    return spec
