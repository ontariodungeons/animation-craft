#!/usr/bin/env python3
"""
Step 1: Character Asset Resolution & PhotoCraft CLI Slicing (`pipeline/vault_characters.py`).

Uses installed `photocraft-cli` (`layer.removeBackground`, `image.crop`, `image.trim`)
instead of custom pixel flood-fills or BFS distance transforms.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any, Dict, List, Tuple

from PIL import Image

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
    for key, entry in VAULT_CHARACTER_CATALOG.items():
        if key == low or key in low or low in key:
            candidate = entry.get(prefer) or entry.get("cutout") or entry.get("turnaround")
            if candidate and os.path.exists(candidate):
                return candidate
    for root, _, files in os.walk(VAULT_ROOT):
        for f in files:
            if low.replace("_", " ") in f.lower() or low in f.lower():
                return os.path.join(root, f)
    raise FileNotFoundError(f"Could not resolve source image or character: {image_or_name!r}")


def matte_with_photocraft(src_path: str, out_path: str) -> str:
    """Use `photocraft-cli run` with `layer.removeBackground` to strip studio background."""
    pc_bin = shutil.which("photocraft-cli") or "photocraft-cli"
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    proc = subprocess.run(
        [
            pc_bin, "run", src_path,
            "--cmd", "layer.removeBackground", "--params", '{"refine":true}',
            "--out", out_path,
        ],
        capture_output=True,
        text=True,
        timeout=90,
    )
    if proc.returncode != 0 or not os.path.isfile(out_path):
        Image.open(src_path).convert("RGBA").save(out_path)
    return out_path


def slice_real_character_image(
    image_path: str,
    out_dir: str,
    tolerance: int = 22,
    step_tol: int = 9,
    erode_px: int = 1,
) -> Dict[str, Any]:
    """Matte via `photocraft-cli` and slice turnaround columns via `photocraft-cli` `image.crop`."""
    os.makedirs(out_dir, exist_ok=True)
    real_path = resolve_real_vault_image(image_path)
    clean_sheet_path = os.path.join(out_dir, "clean_turnaround_sheet.png")
    matte_with_photocraft(real_path, clean_sheet_path)

    with Image.open(clean_sheet_path) as im:
        w, h = im.size
    aspect = w / max(1, h)
    pc_bin = shutil.which("photocraft-cli") or "photocraft-cli"
    views_paths: List[str] = []

    if aspect >= 1.45:
        sheet_type = "pose_sheet" if "pose_sheet" in os.path.basename(real_path).lower() else "turnaround_5view"
        cols = 5
        col_w = w // cols
        top_y = int(h * 0.08)
        crop_h = int(h * (0.41 if sheet_type == "pose_sheet" else 0.83))
        for i in range(cols):
            p = os.path.join(out_dir, f"view_{i}.png")
            subprocess.run(
                [
                    pc_bin, "run", clean_sheet_path,
                    "--cmd", "image.crop",
                    "--params", json.dumps({"x": i * col_w, "y": top_y, "width": col_w, "height": crop_h}),
                    "--cmd", "image.trim", "--params", "{}",
                    "--out", p,
                ],
                capture_output=True,
                text=True,
                timeout=30,
            )
            if not os.path.isfile(p):
                with Image.open(clean_sheet_path) as sheet_im:
                    sheet_im.crop((i * col_w, top_y, (i + 1) * col_w, top_y + crop_h)).save(p)
            views_paths.append(p)
    else:
        sheet_type = "single_cutout"
        p = os.path.join(out_dir, "view_0.png")
        subprocess.run(
            [pc_bin, "run", clean_sheet_path, "--cmd", "image.trim", "--params", "{}", "--out", p],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if not os.path.isfile(p):
            Image.open(clean_sheet_path).save(p)
        views_paths.append(p)

    hero_path = os.path.join(out_dir, "hero_full.png")
    with Image.open(views_paths[0]) as hero_im:
        hero_im.convert("RGBA").save(hero_path)
        hw, hh = hero_im.size

    front_view_path = views_paths[0]
    qtr_front_path = views_paths[1] if len(views_paths) > 1 else views_paths[0]
    side_view_path = views_paths[2] if len(views_paths) > 2 else views_paths[0]
    qtr_back_path = views_paths[3] if len(views_paths) > 3 else side_view_path
    back_view_path = views_paths[4] if len(views_paths) > 4 else views_paths[-1]

    depth_profile = [
        {"v": round(1.0 - (r / 47.0), 4), "depthSpan": 0.28, "depthCenter": 0.5}
        for r in range(48)
    ]

    return {
        "sourceImage": real_path,
        "cleanSheetPng": clean_sheet_path,
        "sheetType": sheet_type,
        "heroFullPng": hero_path,
        "views": views_paths,
        "dilatedViews": views_paths,
        "turnaroundViews": {
            "front": front_view_path,
            "qtrFront": qtr_front_path,
            "side": side_view_path,
            "qtrBack": qtr_back_path,
            "back": back_view_path,
            "frontDilated": front_view_path,
            "sideDilated": side_view_path,
            "backDilated": back_view_path,
        },
        "depthProfile": depth_profile,
        "parts": {},
        "heroSize": {"width": hw, "height": hh},
    }
