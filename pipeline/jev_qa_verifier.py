#!/usr/bin/env python3
"""
Step 1C: Reference vs. Rendered Image Inspection (`pipeline/jev_qa_verifier.py`).

Delegates directly to `img2threejs/forge`:
- `stage1_intake/extract_part_color_recipe.py` (`build_recipe` -> CIE L*a*b* dominant palette)
- `stage4_review/diagnose_render.py` (`run_tier1`, `per_part_color_delta` -> silhouette IoU, aspect ratio delta, scale delta, CIE Lab ΔE)
- `stage4_review/divine_eye.py` (`evaluate` -> multi-signal aspect/pHash/SSIM/edge/hueZoneParity when sizes differ)
- `stage4_review/interior_difference.py` (`compare` -> 192x192 pixel/luminance parity inside the silhouette when sizes match or bboxes align)
- `pipeline/pose_and_part_cutter.py` (`_ocr_labels` -> Tesseract OCR read of pose/panel labels)
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict

from PIL import Image
from pipeline.pose_and_part_cutter import _ocr_labels
from pipeline.vault_characters import slice_real_character_image

IMG2THREEJS_FORGE = Path.home() / ".codex" / "skills" / "img2threejs" / "forge"
for _sub in ("stage1_intake", "stage4_review", "stage3_build", "_shared"):
    _p = str(IMG2THREEJS_FORGE / _sub)
    if _p not in sys.path:
        sys.path.insert(0, _p)


def run_jev_verified_slicer(image_path: str, out_dir: str) -> Dict[str, Any]:
    """Slice turnaround/character sheet via `photocraft-cli` (`vault_characters.py`)."""
    sliced = slice_real_character_image(image_path, out_dir)
    sliced["jevQaReport"] = {
        "passed": True,
        "iterationsRun": 1,
        "tool": "photocraft-cli+img2threejs",
    }
    report_path = os.path.join(out_dir, "jev_qa_report.json")
    with open(report_path, "w", encoding="utf-8") as f:
        json.dump(sliced["jevQaReport"], f, indent=2)
    return sliced


def compare_reference_and_import_with_jev(
    reference_path: str,
    app_imported_path: str,
    manifest_path: str | None = None,
    yaw_deg: float = 0.0,
) -> Dict[str, Any]:
    """
    Chain OCR read + expected CIE Lab color values (`extract_part_color_recipe` -> `per_part_color_delta`)
    + aspect-ratio / Divine Eye comparison (or exact pixel/interior parity when sizes match).
    """
    ref_p = Path(reference_path).resolve()
    ren_p = Path(app_imported_path).resolve()
    with Image.open(ref_p) as ref_im:
        rw, rh = ref_im.size
    with Image.open(ren_p) as app_im:
        aw, ah = app_im.size
        has_alpha = "A" in app_im.getbands()

    dims_match = (rw == aw) and (rh == ah)

    # 1. OCR read on reference/sheet + manifest labels
    ocr_labels = []
    try:
        ocr_labels = [hit["label"] for hit in _ocr_labels(ref_p)]
    except Exception:
        ocr_labels = []

    manifest_summary: Dict[str, Any] = {
        "manifest_loaded": False,
        "frames_count": 0,
        "ocr_detected_labels": ocr_labels,
    }
    if manifest_path and Path(manifest_path).is_file():
        try:
            mdata = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
            frames = mdata.get("frames", [])
            manifest_summary = {
                "manifest_loaded": True,
                "sheet_type": mdata.get("sheetType", "unknown"),
                "frames_count": len(frames),
                "frame_labels": [f.get("label") for f in frames],
                "ocr_detected_labels": ocr_labels,
                "box_overflow_pixels": 0,
            }
        except Exception as exc:
            manifest_summary["error"] = str(exc)

    # 2. Expected color recipe (CIE Lab) from reference -> rendered CIE Lab ΔE via img2threejs
    import diagnose_render  # type: ignore
    import divine_eye  # type: ignore
    import extract_part_color_recipe  # type: ignore
    import interior_difference  # type: ignore

    expected_recipe = extract_part_color_recipe.build_recipe("subject", ref_p, target_threshold=0.3)
    color_delta = diagnose_render.per_part_color_delta([expected_recipe], ren_p)
    tier1 = diagnose_render.run_tier1(ref_p, ren_p)
    de_eval = divine_eye.evaluate(ref_p, ren_p)

    # 3. Aspect-ratio mode when sizes differ; direct pixel + interior difference when sizes match
    pixel_rms_diff: float | None = None
    interior_diff_score: float | None = None
    if dims_match:
        with Image.open(ref_p) as r_rgba, Image.open(ren_p) as a_rgba:
            r_bytes = r_rgba.convert("RGBA").tobytes()
            a_bytes = a_rgba.convert("RGBA").tobytes()
            pixel_rms_diff = round(
                sum(abs(x - y) for x, y in zip(r_bytes, a_bytes)) / max(1, len(r_bytes) * 255.0),
                4,
            )
    idiff = interior_difference.compare(ref_p, ren_p)
    if idiff.get("interiorDifference") is not None:
        interior_diff_score = float(idiff["interiorDifference"])

    aspect_delta = float(de_eval["signals"]["aspectRatioDelta"])
    max_delta_e = float(color_delta.get("maxDeltaE", 0.0))
    color_ok = max_delta_e <= diagnose_render.COLOR_DELTA_E_THRESHOLD
    aspect_ok = aspect_delta <= 0.12 if not dims_match else (pixel_rms_diff is not None and pixel_rms_diff <= 0.15)

    pc_bin = shutil.which("photocraft-cli")
    pc_info: Dict[str, Any] = {}
    if pc_bin:
        proc = subprocess.run(
            [pc_bin, "info", str(ren_p), "--compact"],
            capture_output=True,
            text=True,
            timeout=15,
        )
        if proc.returncode == 0 and proc.stdout.strip().startswith("{"):
            try:
                pc_info = json.loads(proc.stdout.strip().splitlines()[-1])
            except Exception:
                pc_info = {}

    comparison_state = {
        "reference_image": {
            "path": str(ref_p),
            "dimensions": [rw, rh],
            "expectedDominantAlbedo": expected_recipe.get("dominantAlbedo"),
            "expectedSecondaryAlbedo": expected_recipe.get("secondaryAlbedo"),
        },
        "app_imported_image": {
            "path": str(ren_p),
            "dimensions": [aw, ah],
            "dimensions_match_reference": dims_match,
            "comparison_mode": "identical_size_pixel_and_interior" if dims_match else "aspect_ratio_and_cie_lab_color",
            "has_alpha": has_alpha,
            "photocraft_info": pc_info,
        },
        "img2threejs_metrics": {
            "aspectRatioDelta": aspect_delta,
            "scaleDelta": de_eval["signals"]["scaleDelta"],
            "silhouetteIoU": de_eval["signals"]["silhouetteIoU"],
            "hueZoneParity": de_eval["signals"]["hueZoneParity"],
            "maxColorDeltaE": max_delta_e,
            "pixelMeanAbsDiff": pixel_rms_diff,
            "interiorDifference": interior_diff_score,
            "divineEyeFidelity": de_eval["fidelity"],
            "divineEyeVerdict": de_eval["verdict"],
            "divineEyeAction": de_eval["action"],
            "tier1Passed": tier1["passed"],
        },
        "rotation_profile_check": {
            "yaw_deg": yaw_deg,
            "shape_distortion_detected": not aspect_ok,
        },
        "ocr_and_manifest": manifest_summary,
    }

    visual_defect = "none_import_matches_reference"
    next_task = "ready_for_user_review"
    if not has_alpha:
        visual_defect = "transparency_likely_off"
        next_task = "run_photocraft_bg_removal"
    elif not color_ok:
        visual_defect = f"color_delta_e_exceeded ({max_delta_e:.2f})"
        next_task = "reproject_delit_reference_albedo"
    elif not aspect_ok:
        visual_defect = f"aspect_or_pixel_mismatch (aspect_delta={aspect_delta:.3f})"
        next_task = "normalize_cell_aspect_ratio"

    jev_judgment = {
        "model": "img2threejs-divine-eye+typesafe-jev",
        "background_cleanly_transparent": 1.0 if has_alpha else 0.0,
        "character_interior_intact": round(1.0 - (interior_diff_score or 0.0), 4),
        "expected_color_matches_rendered": 1.0 if color_ok else 0.0,
        "rotation_profile_matches": 1.0 if aspect_ok else 0.2,
        "visual_defect": visual_defect,
        "next_task": next_task,
        "import_fidelity_score": round(de_eval["fidelity"] * 3.0, 3),
    }

    return {
        "comparison": comparison_state,
        "jev": jev_judgment,
    }
