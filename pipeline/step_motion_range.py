#!/usr/bin/env python3
"""Stage 3B: Observed silhouette motion-range measurement & timed trajectory compiler.

Measures each pose frame's crown, foot baseline, lift (`footLiftPx`), and centroid offsets
relative to the standing rest pose, and compiles timed contact/lift/apex_hold/landing/rest
trajectories scaled into rig coordinates (`motion_range.json` + `timed_trajectories.json`).
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.step_turnaround_register import (
    SCHEMA_VERSION,
    measure_silhouette,
    read_sheet_frames,
    write_json_atomic,
)

REQUESTED_POSES = (
    "standing",
    "walking",
    "running",
    "jumping",
    "waving",
    "sitting",
    "looking_up",
    "looking_down",
)


def _rows(frames: list[dict]) -> list[dict]:
    """Cluster source boxes by overlapping row centers, not filename indices."""
    groups: list[dict] = []
    for f in sorted(frames, key=lambda fr: sum(fr["meta"].get("sourceBox", [0, 0, 0, 0])[1::2]) / 2):
        meta = f["meta"]
        if "sourceBox" not in meta or "sourceOffset" not in meta:
            f["row"] = None
            continue
        box = meta["sourceBox"]
        cy = (box[1] + box[3]) / 2
        height = box[3] - box[1]
        match = next((g for g in groups if abs(cy - g["center"]) < 0.55 * min(height, g["height"])), None)
        if match is None:
            match = {"center": cy, "height": height, "frames": []}
            groups.append(match)
        match["frames"].append(f)
        f["row"] = groups.index(match)
    return groups


def compile_timed_trajectories(still_height_px: float, limits: dict, poses: dict) -> dict:
    """Compile timed contact/lift/hold/rest trajectories scaled from observed pose metrics."""
    foot_lim = limits.get("footLiftPx") or {}
    max_lift_px = float(foot_lim.get("max") or 0.0)
    norm_lift = min(0.28, max(0.04, max_lift_px / max(1.0, float(still_height_px))))
    walk_lift_px = round(max(4.0, still_height_px * norm_lift * 0.45), 2)
    run_lift_px = round(max(7.0, still_height_px * norm_lift * 0.75), 2)
    jump_lift_px = round(max(12.0, still_height_px * norm_lift * 1.15), 2)

    return {
        "schemaVersion": SCHEMA_VERSION,
        "fps": 24,
        "referenceHeightPx": still_height_px,
        "normalizedLiftEnvelope": round(norm_lift, 4),
        "tracks": {
            "idle": {
                "duration": 2.0,
                "loop": True,
                "keyframes": [
                    {"t": 0.0, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 0.0, "armRRotDeg": 0.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 0.0},
                    {"t": 0.5, "phase": "lift", "pose": "looking_up" if "looking_up" in poses else "standing", "rootDyPx": -round(walk_lift_px * 0.25, 2), "headRotDeg": -3.0, "armLRotDeg": 2.5, "armRRotDeg": -2.5, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 4.0},
                    {"t": 1.0, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 0.0, "armRRotDeg": 0.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": -2.0},
                    {"t": 1.5, "phase": "contact", "pose": "looking_down" if "looking_down" in poses else "standing", "rootDyPx": round(walk_lift_px * 0.15, 2), "headRotDeg": 2.5, "armLRotDeg": -2.0, "armRRotDeg": 2.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": -3.5},
                    {"t": 2.0, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 0.0, "armRRotDeg": 0.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 0.0},
                ],
            },
            "walk": {
                "duration": 1.6,
                "loop": True,
                "keyframes": [
                    {"t": 0.0, "phase": "contact", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 16.0, "armRRotDeg": -16.0, "legLRotDeg": -18.0, "legRRotDeg": 18.0, "secondaryRotDeg": -6.0},
                    {"t": 0.4, "phase": "lift", "pose": "walking", "rootDyPx": -walk_lift_px, "headRotDeg": -3.5, "armLRotDeg": 0.0, "armRRotDeg": 0.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 8.0},
                    {"t": 0.8, "phase": "contact", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": -16.0, "armRRotDeg": 16.0, "legLRotDeg": 18.0, "legRRotDeg": -18.0, "secondaryRotDeg": 6.0},
                    {"t": 1.2, "phase": "lift", "pose": "walking", "rootDyPx": -walk_lift_px, "headRotDeg": 3.5, "armLRotDeg": 0.0, "armRRotDeg": 0.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": -8.0},
                    {"t": 1.6, "phase": "contact", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 16.0, "armRRotDeg": -16.0, "legLRotDeg": -18.0, "legRRotDeg": 18.0, "secondaryRotDeg": -6.0},
                ],
            },
            "run": {
                "duration": 1.2,
                "loop": True,
                "keyframes": [
                    {"t": 0.0, "phase": "contact", "pose": "walking", "rootDyPx": 2.0, "headRotDeg": 2.0, "armLRotDeg": 26.0, "armRRotDeg": -26.0, "legLRotDeg": -28.0, "legRRotDeg": 28.0, "secondaryRotDeg": -10.0},
                    {"t": 0.3, "phase": "apex_hold", "pose": "running" if "running" in poses else "jumping", "rootDyPx": -run_lift_px, "headRotDeg": -4.0, "armLRotDeg": 6.0, "armRRotDeg": -6.0, "legLRotDeg": -8.0, "legRRotDeg": 8.0, "secondaryRotDeg": 12.0},
                    {"t": 0.6, "phase": "contact", "pose": "walking", "rootDyPx": 2.0, "headRotDeg": -2.0, "armLRotDeg": -26.0, "armRRotDeg": 26.0, "legLRotDeg": 28.0, "legRRotDeg": -28.0, "secondaryRotDeg": 10.0},
                    {"t": 0.9, "phase": "apex_hold", "pose": "running" if "running" in poses else "jumping", "rootDyPx": -run_lift_px, "headRotDeg": 4.0, "armLRotDeg": -6.0, "armRRotDeg": 6.0, "legLRotDeg": 8.0, "legRRotDeg": -8.0, "secondaryRotDeg": -12.0},
                    {"t": 1.2, "phase": "contact", "pose": "walking", "rootDyPx": 2.0, "headRotDeg": 2.0, "armLRotDeg": 26.0, "armRRotDeg": -26.0, "legLRotDeg": -28.0, "legRRotDeg": 28.0, "secondaryRotDeg": -10.0},
                ],
            },
            "jump": {
                "duration": 1.6,
                "loop": True,
                "keyframes": [
                    {"t": 0.0, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 0.0, "armRRotDeg": 0.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 0.0},
                    {"t": 0.3, "phase": "contact", "pose": "standing", "rootDyPx": round(jump_lift_px * 0.25, 2), "headRotDeg": 3.0, "armLRotDeg": 14.0, "armRRotDeg": -14.0, "legLRotDeg": 10.0, "legRRotDeg": -10.0, "secondaryRotDeg": -6.0},
                    {"t": 0.75, "phase": "apex_hold", "pose": "jumping", "rootDyPx": -jump_lift_px, "headRotDeg": -5.0, "armLRotDeg": -26.0, "armRRotDeg": 26.0, "legLRotDeg": -14.0, "legRRotDeg": 14.0, "secondaryRotDeg": 14.0},
                    {"t": 1.2, "phase": "landing", "pose": "standing", "rootDyPx": round(jump_lift_px * 0.2, 2), "headRotDeg": 2.5, "armLRotDeg": 8.0, "armRRotDeg": -8.0, "legLRotDeg": 8.0, "legRRotDeg": -8.0, "secondaryRotDeg": -8.0},
                    {"t": 1.6, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 0.0, "armRRotDeg": 0.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 0.0},
                ],
            },
            "wave": {
                "duration": 1.6,
                "loop": True,
                "keyframes": [
                    {"t": 0.0, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 0.0, "armRRotDeg": -24.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 0.0},
                    {"t": 0.4, "phase": "lift", "pose": "waving", "rootDyPx": -round(walk_lift_px * 0.35, 2), "headRotDeg": -5.0, "armLRotDeg": 4.0, "armRRotDeg": -42.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 9.0},
                    {"t": 0.8, "phase": "apex_hold", "pose": "waving", "rootDyPx": 0.0, "headRotDeg": 4.0, "armLRotDeg": -3.0, "armRRotDeg": -16.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": -7.0},
                    {"t": 1.2, "phase": "lift", "pose": "waving", "rootDyPx": -round(walk_lift_px * 0.35, 2), "headRotDeg": -4.0, "armLRotDeg": 4.0, "armRRotDeg": -40.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 8.0},
                    {"t": 1.6, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": 0.0, "armRRotDeg": -24.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 0.0},
                ],
            },
            "talk_express": {
                "duration": 1.6,
                "loop": True,
                "keyframes": [
                    {"t": 0.0, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": -8.0, "armRRotDeg": 8.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 0.0},
                    {"t": 0.4, "phase": "lift", "pose": "pointing" if "pointing" in poses else "waving", "rootDyPx": -round(walk_lift_px * 0.3, 2), "headRotDeg": -4.5, "armLRotDeg": -20.0, "armRRotDeg": 18.0, "legLRotDeg": -4.0, "legRRotDeg": 4.0, "secondaryRotDeg": 7.0},
                    {"t": 0.8, "phase": "contact", "pose": "clapping" if "clapping" in poses else "standing", "rootDyPx": 0.0, "headRotDeg": 3.5, "armLRotDeg": 12.0, "armRRotDeg": -12.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": -6.0},
                    {"t": 1.2, "phase": "lift", "pose": "dancing" if "dancing" in poses else "waving", "rootDyPx": -round(walk_lift_px * 0.35, 2), "headRotDeg": -3.0, "armLRotDeg": -16.0, "armRRotDeg": 22.0, "legLRotDeg": 4.0, "legRRotDeg": -4.0, "secondaryRotDeg": 8.0},
                    {"t": 1.6, "phase": "rest", "pose": "standing", "rootDyPx": 0.0, "headRotDeg": 0.0, "armLRotDeg": -8.0, "armRRotDeg": 8.0, "legLRotDeg": 0.0, "legRRotDeg": 0.0, "secondaryRotDeg": 0.0},
                ],
            },
        },
    }


def measure_motion_range(
    poses_dir: str | Path,
    out_dir: str | Path,
    registration: dict | None = None,
    alpha_threshold: int = 128,
    _loaded: tuple[list[dict], dict[str, str]] | None = None,
) -> dict:
    import numpy as np

    frames, hashes = _loaded if _loaded is not None else read_sheet_frames(poses_dir)
    labelled = {}
    for f in frames:
        label = f["meta"]["label"]
        if label in labelled:
            label = f"{label}_{f['meta'].get('index', len(labelled))}"
        labelled[label] = f
        f["measure"] = measure_silhouette(f["image"], alpha_threshold)
    if "standing" not in labelled:
        raise ValueError("Missing standing (STILL) reference")
    still = labelled["standing"]
    height = max(1, still["measure"]["heightPx"])
    rows = _rows(frames)
    row_info = []
    for i, row in enumerate(rows):
        grounds = [f["measure"]["footBaselineY"] - f["meta"]["sourceOffset"][1] + f["meta"]["sourceBox"][1] for f in row["frames"]]
        row_info.append({
            "row": i,
            "baselineSourceY": float(np.median(grounds)),
            "method": "median measured foot baseline of source-sheet row",
            "poseLabels": [f["meta"]["label"] for f in row["frames"]],
            "baselineSpreadPx": float(max(grounds) - min(grounds)),
        })

    def values(frame: dict) -> tuple[dict, list[float], float, str]:
        m = frame["measure"]
        meta = frame["meta"]
        left, top, right, bottom = m["bbox"]
        anchor_x = float(meta.get("anchor", [frame["image"].width / 2, 0])[0])
        if frame.get("row") is not None:
            ground = row_info[frame["row"]]["baselineSourceY"]
            dy = meta["sourceBox"][1] - meta["sourceOffset"][1] - ground
            row_lift = -(bottom + dy)
            # When an entire sheet row is bottom-aligned (baselineSpreadPx <= 3.0),
            # measure single-leg stance lift inside the lower 35% leg zone.
            leg_diff = 0.0
            if row_info[frame["row"]]["baselineSpreadPx"] <= 3.0 and meta.get("label") in ("walking", "running", "jumping"):
                alpha_arr = np.array(frame["image"].getchannel("A")) >= alpha_threshold
                leg_top = int(round(bottom - max(8, (bottom - top) * 0.35)))
                leg_band = alpha_arr[max(0, leg_top):bottom, :]
                yy_fg, xx_fg = np.where(leg_band)
                if len(xx_fg) > 10:
                    mid_x = 0.5 * (float(xx_fg.min()) + float(xx_fg.max()))
                    l_yy = yy_fg[xx_fg < mid_x]
                    r_yy = yy_fg[xx_fg >= mid_x]
                    if len(l_yy) > 5 and len(r_yy) > 5:
                        leg_diff = float(abs(int(l_yy.max()) - int(r_yy.max())))
            lift = max(float(row_lift), leg_diff) if row_lift >= 0 else float(row_lift)
            origin = "source_row_median_baseline"
        else:
            dy = -m["footBaselineY"]
            lift = None
            origin = "own_feet; source row unavailable"
        bounds = [left - anchor_x, top + dy, right - anchor_x, bottom + dy]
        metrics = {
            "widthPx": m["widthPx"],
            "heightPx": m["heightPx"],
            "heightRatioToStill": m["heightPx"] / height,
            "areaPx": m["areaPx"],
            "areaRatioToStill": m["areaPx"] / max(1, still["measure"]["areaPx"]),
            "leftPx": bounds[0],
            "topPx": bounds[1],
            "rightPx": bounds[2],
            "bottomPx": bounds[3],
            "centroidXPx": m["centroid"][0] - anchor_x,
            "centroidYPx": m["centroid"][1] + dy,
            "footLiftPx": lift,
        }
        return metrics, bounds, dy, origin

    still_metrics, _, _, _ = values(still)
    poses = {}
    for label, frame in labelled.items():
        metrics, bounds, dy, origin = values(frame)
        profile = []
        mask = np.array(frame["image"].getchannel("A")) >= alpha_threshold
        for h in (0.10, 0.25, 0.50, 0.75, 0.90):
            y = int(round(-h * height - dy))
            xs = np.flatnonzero(mask[y]) if 0 <= y < mask.shape[0] else np.array([])
            cx = float(frame["meta"].get("anchor", [frame["image"].width / 2, 0])[0])
            profile.append({
                "heightAboveGroundUnits": h,
                "leftUnits": float((xs.min() - cx) / height) if len(xs) else None,
                "rightUnits": float((xs.max() + 1 - cx) / height) if len(xs) else None,
            })
        poses[label] = {
            "sourceFile": str(frame["path"]),
            "frameIndex": frame["meta"].get("index", 0),
            "sourceMeasurement": frame["measure"],
            "row": frame.get("row"),
            "verticalOrigin": origin,
            "metrics": metrics,
            "deltaFromStill": {
                k: (v - still_metrics[k] if v is not None and still_metrics[k] is not None else None)
                for k, v in metrics.items()
            },
            "bboxStandingUnits": [v / height for v in bounds],
            "profile": profile,
            "jointAnglesRadians": None,
        }
    limits = {}
    for key in still_metrics:
        observed = [(p["metrics"][key], label) for label, p in poses.items() if p["metrics"][key] is not None]
        if not observed:
            limits[key] = None
            continue
        lo, low_label = min(observed, key=lambda v: v[0])
        hi, high_label = max(observed, key=lambda v: v[0])
        limits[key] = {
            "min": lo,
            "max": hi,
            "minPose": low_label,
            "maxPose": high_label,
            "still": still_metrics[key],
            "minDeltaFromStill": lo - still_metrics[key] if still_metrics[key] is not None else None,
            "maxDeltaFromStill": hi - still_metrics[key] if still_metrics[key] is not None else None,
        }
    timed_trajectories = compile_timed_trajectories(height, limits, poses)
    result = {
        "schemaVersion": SCHEMA_VERSION,
        "kind": "observed_silhouette_motion_range",
        "alphaThreshold": alpha_threshold,
        "still": {
            "label": "standing",
            "file": str(still["path"]),
            "heightPx": height,
            "metrics": still_metrics,
        },
        "poses": poses,
        "limits": limits,
        "timedTrajectories": timed_trajectories,
        "units": {
            "pixels": "original pose frame pixels; no per-pose resizing",
            "standingUnits": "one standing crown-to-foot silhouette height = 1",
            "axes": "x right, y down; y=0 is estimated row ground; image edges exclusive",
        },
        "requestedPoses": list(REQUESTED_POSES),
        "missingRequestedPoses": [p for p in REQUESTED_POSES if p not in poses],
        "rowBaselines": row_info,
        "jointAngleLimits": None,
        "rootTrajectory": None,
        "sourceHashes": hashes,
        "registrationFile": "turnaround_registration.json" if registration else None,
        "turnaroundHeightPx": registration["target"]["heightPx"] if registration else None,
        "poseToTurnaroundScale": registration["target"]["heightPx"] / height if registration else None,
        "warnings": [
            "These are sampled 2D silhouette envelopes, not physiological joint limits or temporal motion.",
            "Pose artwork includes props/costume and viewpoint changes; envelopes include their pixels.",
            "Row-median ground is a sheet-layout estimate; hovering whole rows cannot establish ground.",
            "Horizontal root translation cannot be recovered from independent centered pose drawings.",
            "CLI, browser and Blender must explicitly read this file; existing consumers were not edited.",
        ],
    }
    out = Path(out_dir)
    write_json_atomic(out / "motion_range.json", result)
    write_json_atomic(out / "timed_trajectories.json", timed_trajectories)
    return result


def load_motion_range(path: str | Path) -> dict:
    value = json.loads(Path(path).read_text())
    if value.get("schemaVersion") != SCHEMA_VERSION or value.get("kind") != "observed_silhouette_motion_range":
        raise ValueError("Unsupported motion range schema")
    return value


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--poses-dir", required=True)
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--alpha-threshold", type=int, default=128)
    args = ap.parse_args(argv)
    res = measure_motion_range(args.poses_dir, args.out_dir, alpha_threshold=args.alpha_threshold)
    print(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
