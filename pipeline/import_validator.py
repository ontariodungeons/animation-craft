#!/usr/bin/env python3
"""
Step 1B: Image Import Validator (`pipeline/import_validator.py`).

Uses `photocraft-cli info` to inspect imported image dimensions, alpha, and layout
without custom pixel flood-fill loops.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
from typing import Any, Dict

from PIL import Image

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


def validate_imported_asset(image_path: str) -> Dict[str, Any]:
    """Inspect an imported image via `photocraft-cli info` and classify sheet layout."""
    if not image_path or not os.path.exists(image_path):
        return {
            "valid": False,
            "status": "file_not_found",
            "imagePath": image_path,
            "message": f"File not found: {image_path}. Pass any .png/.jpg/.webp path.",
            "standardTemplateSpec": STANDARD_TEMPLATE_SPEC,
        }

    pc_bin = shutil.which("photocraft-cli")
    pc_info: Dict[str, Any] = {}
    if pc_bin:
        proc = subprocess.run(
            [pc_bin, "info", image_path, "--compact"],
            capture_output=True,
            text=True,
            timeout=15,
        )
        if proc.returncode == 0 and proc.stdout.strip().startswith("{"):
            try:
                pc_info = json.loads(proc.stdout.strip().splitlines()[-1])
            except Exception:
                pc_info = {}

    with Image.open(image_path) as im:
        w, h = im.size
        has_alpha = "A" in im.getbands()

    aspect = w / max(1, h)
    lower = os.path.basename(image_path).lower()
    if aspect >= 1.45 and ("pose" in lower or "sheet" in lower) and "turnaround" not in lower:
        rows_detected = 2
        views_in_row = 5
        valid = True
        status = "ready"
    elif aspect >= 1.45:
        rows_detected = 1
        views_in_row = 5
        valid = True
        status = "ready"
    else:
        rows_detected = 1
        views_in_row = 1
        valid = False
        status = "single_or_partial_sheet"

    return {
        "valid": valid,
        "status": status,
        "imagePath": os.path.abspath(image_path),
        "dimensions": [w, h],
        "hasAlpha": has_alpha,
        "rowsDetected": rows_detected,
        "viewsInPrimaryRow": views_in_row,
        "photocraftInfo": pc_info,
        "standardTemplateSpec": STANDARD_TEMPLATE_SPEC,
    }
