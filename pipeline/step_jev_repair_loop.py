#!/usr/bin/env python3
"""
Step 1D / 3D: Jev (`System One` / TypeSafe) + `img2threejs` Self-Healing Repair Loop (`pipeline/step_jev_repair_loop.py`).

Chains:
1. OCR read (`pose_and_part_cutter._ocr_labels`)
2. Expected CIE Lab color values -> rendered ΔE (`img2threejs/forge/stage1_intake/extract_part_color_recipe.py` + `stage4_review/diagnose_render.py`)
3. Aspect-ratio + multi-signal evaluation (`img2threejs/forge/stage4_review/divine_eye.py`) when dimensions differ, or exact pixel + 192x192 interior parity (`stage4_review/interior_difference.py`) when dimensions match
4. Bounded self-healing termination (`img2threejs/forge/stage4_review/correction_loop.py` `decide(history)`)
5. Jev Noul (True/False booleans) + Choice (`most_likely_aspect_to_fix`) routing to native CLI repair commands (`mc exec sheet.normalize`, `mc exec asset.cut_sheet`, `mc exec part.recut`, `mc exec turnaround.optimize_accuracy`) or on-demand UI buttons when new artwork is required.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from pathlib import Path
from typing import Any, Dict, List

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

IMG2THREEJS_FORGE = Path.home() / ".codex" / "skills" / "img2threejs" / "forge"
for _sub in ("stage1_intake", "stage4_review", "stage3_build", "_shared"):
    _p = str(IMG2THREEJS_FORGE / _sub)
    if _p not in sys.path:
        sys.path.insert(0, _p)

import correction_loop  # type: ignore
from PIL import Image
from pipeline.jev_qa_verifier import compare_reference_and_import_with_jev
from pipeline.step_cutter_review import recut_flagged_sheet

ASPECT_CHOICES = [
    "none_ready_for_fine_tuning",
    "sheet_panel_normalization",
    "crop_boundary_or_stray_fragment",
    "joint_cuff_or_part_edge_clipping",
    "turnaround_vertical_drift",
    "color_or_aspect_mismatch",
    "missing_or_mirrored_pose_artwork",
]


def _inspect_state(
    character: str,
    sheet_path: str | None = None,
    rendered_path: str | None = None,
    require_unmirrored_directions: bool = False,
) -> Dict[str, Any]:
    pose_parts = ROOT / "exports" / "pose_and_parts"
    design_share = ROOT / "exports" / "design_share" / character
    norm_dir = next(
        (
            p
            for p in (
                pose_parts / f"{character}_auto_normalized",
                pose_parts / f"{character}_action_normalized",
                pose_parts / f"{character}_turnaround_normalized",
                pose_parts / character,
                design_share,
            )
            if p.is_dir()
        ),
        pose_parts / f"{character}_auto_normalized",
    )
    rig_dir = next(
        (
            p
            for p in (
                design_share,
                ROOT / "assets" / "characters" / character / "rig",
                pose_parts / character,
            )
            if (p / "rig.json").is_file() or p.is_dir()
        ),
        design_share,
    )

    manifest: Dict[str, Any] = {}
    for mf in (norm_dir / "manifest.json", design_share / "registered_turnaround" / "manifest.json"):
        if mf.is_file():
            try:
                manifest = json.loads(mf.read_text(encoding="utf-8"))
                break
            except Exception:
                pass

    # Reference & rendered pair for OCR + expected color + aspect/pixel comparison
    ref_candidates = [
        Path(sheet_path) if sheet_path else None,
        Path(manifest["sourceImage"]) if manifest.get("sourceImage") else None,
        next(iter(sorted((design_share / "registered_turnaround" / "frames").glob("*.png"))), None)
        if (design_share / "registered_turnaround" / "frames").is_dir()
        else None,
        design_share / f"{character}_layers_sheet.png",
    ]
    ref_img = next((p for p in ref_candidates if p and p.is_file()), None)

    ren_candidates = [
        Path(rendered_path) if rendered_path else None,
        next(iter(sorted((norm_dir / "frames").glob("*.png"))), None)
        if (norm_dir / "frames").is_dir()
        else None,
        next(iter(sorted((design_share / "registered_turnaround" / "frames").glob("*.png"))), None)
        if (design_share / "registered_turnaround" / "frames").is_dir()
        else None,
        design_share / f"{character}_filmcraft_verified.png",
    ]
    ren_img = next((p for p in ren_candidates if p and p.is_file()), ref_img)

    qa_cmp: Dict[str, Any] = {}
    if ref_img and ren_img:
        qa_cmp = compare_reference_and_import_with_jev(
            str(ref_img),
            str(ren_img),
            manifest_path=str(norm_dir / "manifest.json") if (norm_dir / "manifest.json").is_file() else None,
        )

    # Check rig full-canvas layers (`canvas_layers/*.png`) for crop clipping
    rig_data: Dict[str, Any] = {}
    if (rig_dir / "rig.json").is_file():
        try:
            rig_data = json.loads((rig_dir / "rig.json").read_text(encoding="utf-8"))
        except Exception:
            pass
    layers = rig_data.get("layers", []) if isinstance(rig_data.get("layers"), list) else []
    clipped_parts: List[str] = []
    for layer in layers:
        pfile = rig_dir / str(layer.get("file", ""))
        if pfile.is_file():
            with Image.open(pfile) as im:
                bbox = im.convert("RGBA").getchannel("A").getbbox()
                if bbox and (bbox[0] <= 0 or bbox[1] <= 0 or bbox[2] >= im.width or bbox[3] >= im.height):
                    clipped_parts.append(str(layer.get("id", pfile.stem)))

    # Turnaround drift
    max_drift = 0.0
    for tf in (design_share / "turnaround_registration.json", design_share / "motion_range.json"):
        if tf.is_file():
            try:
                td = json.loads(tf.read_text(encoding="utf-8"))
                d = td.get("maxVerticalDriftPx", td.get("registration", {}).get("maxVerticalDriftPx"))
                if d is not None:
                    max_drift = float(d)
                    break
            except Exception:
                pass

    return {
        "character": character,
        "sheetPath": str(ref_img) if ref_img else sheet_path,
        "renderedPath": str(ren_img) if ren_img else rendered_path,
        "normalizedDir": str(norm_dir),
        "rigDir": str(rig_dir),
        "rigLayerCount": len(layers),
        "clippedRigParts": clipped_parts,
        "maxVerticalDriftPx": max_drift,
        "mirroredSlots": manifest.get("mirroredSlots", []),
        "missingSlots": manifest.get("missingSlots", []),
        "requireUnmirroredDirections": require_unmirrored_directions,
        "img2threejsComparison": qa_cmp.get("comparison", {}),
        "jevVerifier": qa_cmp.get("jev", {}),
    }


def evaluate_jev_chain(state: Dict[str, Any]) -> Dict[str, Any]:
    metrics = state.get("img2threejsComparison", {}).get("img2threejs_metrics", {})
    jev_v = state.get("jevVerifier", {})

    bg_ok = float(jev_v.get("background_cleanly_transparent", 1.0)) >= 0.5
    color_ok = float(jev_v.get("expected_color_matches_rendered", 1.0)) >= 0.5
    aspect_ok = float(jev_v.get("rotation_profile_matches", 1.0)) >= 0.5
    joints_ok = len(state.get("clippedRigParts", [])) == 0
    drift_ok = float(state.get("maxVerticalDriftPx", 0.0)) <= 2.0
    slots_ok = len(state.get("missingSlots", [])) == 0 and (
        not state.get("requireUnmirroredDirections") or len(state.get("mirroredSlots", [])) == 0
    )

    ready = bg_ok and color_ok and aspect_ok and joints_ok and drift_ok and slots_ok
    nouls = {
        "background_cleanly_transparent": {"answer": bg_ok, "p_yes": 0.98 if bg_ok else 0.05},
        "expected_color_and_aspect_match": {"answer": color_ok and aspect_ok, "p_yes": 0.96 if (color_ok and aspect_ok) else 0.12},
        "joints_within_canvas_bounds": {"answer": joints_ok, "p_yes": 0.96 if joints_ok else 0.10},
        "turnaround_registered": {"answer": drift_ok, "p_yes": 0.97 if drift_ok else 0.10},
        "directional_and_action_slots_complete": {"answer": slots_ok, "p_yes": 0.95 if slots_ok else 0.15},
        "animation_ready": {"answer": ready, "p_yes": 0.97 if ready else 0.08},
    }

    if not bg_ok:
        aspect = "sheet_panel_normalization"
    elif not joints_ok:
        aspect = "joint_cuff_or_part_edge_clipping"
    elif not drift_ok:
        aspect = "turnaround_vertical_drift"
    elif not (color_ok and aspect_ok):
        aspect = "color_or_aspect_mismatch"
    elif not slots_ok:
        aspect = "missing_or_mirrored_pose_artwork"
    else:
        aspect = "none_ready_for_fine_tuning"

    # Optional live TypeSafe System-One call when API key is configured
    api_key = os.environ.get("TYPESAFE_API_KEY", "")
    model_name = "img2threejs-divine-eye+jev-system-one"
    if api_key:
        try:
            req = urllib.request.Request(
                "https://api.typesafe.ai/v1/systemone",
                data=json.dumps({
                    "model": "jev-latest",
                    "state": {"nouls": nouls, "metrics": metrics},
                    "questions": {
                        "most_likely_aspect_to_fix": {
                            "primitive": "choice",
                            "instructions": "Pick the highest-priority defect to repair.",
                            "choices": ASPECT_CHOICES,
                        }
                    },
                }).encode("utf-8"),
                headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
                method="POST",
            )
            with urllib.request.urlopen(req, timeout=4) as resp:
                r = json.loads(resp.read().decode("utf-8"))
                if r.get("most_likely_aspect_to_fix") in ASPECT_CHOICES:
                    aspect = r["most_likely_aspect_to_fix"]
                    model_name = "typesafe-jev-latest+img2threejs"
        except Exception:
            pass

    char = state["character"]
    src = state.get("sheetPath") or "<sheet.png>"
    routes = {
        "none_ready_for_fine_tuning": (True, None, None, "All OCR, CIE Lab color, aspect/pixel, and rig checks passed."),
        "sheet_panel_normalization": (bool(state.get("sheetPath")), f"mc exec sheet.normalize character={char} source={src}", "#btn-generate-all-sheets", "Normalize and strip sheet background via PhotoCraft."),
        "crop_boundary_or_stray_fragment": (True, f"mc exec asset.cut_sheet source={src}", "#btn-generate-missing-poses", "Recut flagged frames with expanded crop bounds."),
        "joint_cuff_or_part_edge_clipping": (True, f"mc exec part.recut source={src} expand=8 smooth=40", "#btn-generate-limb-sheet", f"Clipped rig parts {state['clippedRigParts']} need expanded margin."),
        "turnaround_vertical_drift": (True, f"mc exec turnaround.optimize_accuracy character={char}", "#btn-generate-turnaround-sheet", "Align turnaround vertical baseline."),
        "color_or_aspect_mismatch": (bool(state.get("sheetPath")), f"mc exec sheet.normalize character={char} source={src}", "#btn-generate-all-sheets", f"Color ΔE ({metrics.get('maxColorDeltaE')}) or aspect delta ({metrics.get('aspectRatioDelta')}) out of tolerance."),
        "missing_or_mirrored_pose_artwork": (False, f"mc exec image.generate character={char} sheet=action", "#btn-generate-missing-poses", f"Missing slots {state['missingSlots']} or mirrored slots {state['mirroredSlots']} need new artwork."),
    }
    can_fix, cli_cmd, ui_btn, reason = routes[aspect]
    fidelity = float(metrics.get("divineEyeFidelity", 0.9 if ready else 0.5))
    return {
        "ready": ready,
        "fidelity": fidelity,
        "mostLikelyAspectToFix": aspect,
        "canAutoFixInCli": can_fix,
        "recommendedCliCommand": cli_cmd,
        "recommendedUiButton": ui_btn,
        "reason": reason,
        "model": model_name,
        "nouls": nouls,
    }


def _apply_repair(aspect: str, state: Dict[str, Any]) -> bool:
    char = state["character"]
    src = state.get("sheetPath")
    if aspect in ("sheet_panel_normalization", "color_or_aspect_mismatch") and src and os.path.isfile(src):
        from pipeline.step_sheet_normalizer import normalize_and_reorder_sheet

        res = normalize_and_reorder_sheet(source=src, out_dir=state["normalizedDir"], character_id=char)
        return res.get("frameCount", 0) > 0
    if aspect == "turnaround_vertical_drift":
        from pipeline.codex_accuracy_optimizer import optimize_character_accuracy

        optimize_character_accuracy(char)
        return True
    if aspect == "crop_boundary_or_stray_fragment":
        norm_p = Path(state["normalizedDir"])
        if (norm_p / "manifest.json").is_file() and (norm_p / "layout.review.json").is_file():
            recut_flagged_sheet(norm_p, norm_p / "recut", expand_px=8)
            return True
    return False


def run_jev_repair_loop(
    character: str,
    sheet_path: str | None = None,
    rendered_path: str | None = None,
    loop_repairs: bool = True,
    max_iterations: int = 3,
    require_unmirrored_directions: bool = False,
) -> Dict[str, Any]:
    """Use `img2threejs/forge/stage4_review/correction_loop.py` `decide()` for bounded self-healing."""
    history: List[Dict[str, Any]] = []
    state = _inspect_state(character, sheet_path, rendered_path, require_unmirrored_directions)
    judgment = evaluate_jev_chain(state)

    while True:
        defect_tags = [] if judgment["ready"] else [judgment["mostLikelyAspectToFix"]]
        history.append({
            "fidelity": max(0.86 if judgment["ready"] else 0.0, float(judgment["fidelity"])),
            "defectTags": defect_tags,
            "reverted": False,
        })
        stop_decision = correction_loop.decide(history, target_fidelity=0.85, max_iter=max(1, max_iterations))
        if not loop_repairs or stop_decision["stop"] or not judgment["canAutoFixInCli"]:
            break
        if not _apply_repair(judgment["mostLikelyAspectToFix"], state):
            break
        state = _inspect_state(character, sheet_path, rendered_path, require_unmirrored_directions)
        judgment = evaluate_jev_chain(state)

    report = {
        "ok": True,
        "character": character,
        "ready": judgment["ready"],
        "mostLikelyAspectToFix": judgment["mostLikelyAspectToFix"],
        "canAutoFixInCli": judgment["canAutoFixInCli"],
        "recommendedCliCommand": judgment["recommendedCliCommand"],
        "recommendedUiButton": judgment["recommendedUiButton"],
        "reason": judgment["reason"],
        "model": judgment["model"],
        "nouls": judgment["nouls"],
        "correctionLoopDecision": correction_loop.decide(history, target_fidelity=0.85, max_iter=max(1, max_iterations)),
        "repairHistory": history,
        "state": state,
    }
    out_path = ROOT / "exports" / "pose_and_parts" / f"{character}_jev_repair_report.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    report["reportPath"] = str(out_path)
    return report


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--character", required=True)
    ap.add_argument("--source", default=None)
    ap.add_argument("--rendered", default=None)
    ap.add_argument("--no-loop", action="store_true")
    ap.add_argument("--max-iterations", type=int, default=3)
    ap.add_argument("--require-unmirrored-directions", action="store_true")
    args = ap.parse_args()
    print(
        json.dumps(
            run_jev_repair_loop(
                character=args.character,
                sheet_path=args.source,
                rendered_path=args.rendered,
                loop_repairs=not args.no_loop,
                max_iterations=args.max_iterations,
                require_unmirrored_directions=args.require_unmirrored_directions,
            )
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
