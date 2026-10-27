#!/usr/bin/env python3
"""Stage 4 Orchestrator: Export shareable 2D character package (`step_turnaround_register` + `step_motion_range` + `step_six_action_track`)."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.step_turnaround_register import register_turnaround
from pipeline.step_motion_range import measure_motion_range
from pipeline.step_six_action_track import SIX_ACTIONS, build_six_action_package

ROOT = Path(__file__).resolve().parents[1]
POSE_PARTS_ROOT = ROOT / "exports/pose_and_parts"
SHARE_ROOT = ROOT / "exports/design_share"

CHARACTER_PRESETS: dict[str, dict[str, str]] = {
    "hopper": {
        "title": "Hopper (Cream Rabbit Student)",
        "poses_dir": str(POSE_PARTS_ROOT / "hopper_poses"),
        "turnaround_dir": str(POSE_PARTS_ROOT / "hopper_turnaround"),
    },
    "whiskers": {
        "title": "Whiskers (Grey Tabby Kitten)",
        "poses_dir": str(POSE_PARTS_ROOT / "whiskers_poses"),
        "turnaround_dir": str(POSE_PARTS_ROOT / "Character_turnaround_sheet_anima\u2026_202607180130_17"),
    },
    "puddles": {
        "title": "Puddles (Yellow Duckling)",
        "poses_dir": str(POSE_PARTS_ROOT / "Create_pose_sheet_for_puppet_202607180130_13"),
        "turnaround_dir": str(POSE_PARTS_ROOT / "Character_turnaround_sheet_anima\u2026_202607180130_12"),
    },
    "jenny": {
        "title": "Jenny (Yellow Chick)",
        "poses_dir": str(POSE_PARTS_ROOT / "Create_pose_sheet_for_puppet_202607180130_3"),
        "turnaround_dir": str(POSE_PARTS_ROOT / "Character_turnaround_sheet_anima\u2026_202607180130_9"),
    },
    "rusty": {
        "title": "Rusty (Brown Foal)",
        "poses_dir": str(POSE_PARTS_ROOT / "Create_pose_sheet_for_puppet_202607180130_24"),
        "turnaround_dir": str(POSE_PARTS_ROOT / "Character_turnaround_sheet_anima\u2026_202607180130_19"),
    },
    "miss_hayley": {
        "title": "Miss Hayley (Farm School Teacher)",
        "poses_dir": str(POSE_PARTS_ROOT / "miss_hayley_poses"),
        "turnaround_dir": str(POSE_PARTS_ROOT / "miss_hayley_turnaround"),
    },
    "old_mac": {
        "title": "Old Mac (Farm School Principal)",
        "poses_dir": str(POSE_PARTS_ROOT / "Create_pose_sheet_for_puppet_202607180130_27"),
        "turnaround_dir": str(POSE_PARTS_ROOT / "old_mac_turnaround"),
    },
}


def export_character_iteration(
    char_id: str,
    poses_dir: str | Path | None = None,
    turnaround_dir: str | Path | None = None,
    out_dir: str | Path | None = None,
) -> dict:
    slug = char_id.lower().strip()
    preset = CHARACTER_PRESETS.get(slug, {})
    title = preset.get("title", slug.replace("_", " ").title())
    p_dir = Path(poses_dir or preset.get("poses_dir", POSE_PARTS_ROOT / f"{slug}_poses")).resolve()
    t_raw = turnaround_dir or preset.get("turnaround_dir")
    t_dir = Path(t_raw).resolve() if t_raw else None
    dest = Path(out_dir or (SHARE_ROOT / slug)).resolve()
    dest.mkdir(parents=True, exist_ok=True)

    registration = None
    if t_dir and (t_dir / "manifest.json").is_file():
        registration = register_turnaround(t_dir, dest)
    motion = measure_motion_range(p_dir, dest, registration=registration)
    return build_six_action_package(
        slug=slug,
        title=title,
        p_dir=p_dir,
        dest=dest,
        motion=motion,
        registration=registration,
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--character", default="hopper")
    parser.add_argument("--poses-dir", default=None)
    parser.add_argument("--turnaround-dir", default=None)
    parser.add_argument("--out-dir", default=None)
    parser.add_argument("--out-root", default=None)
    args = parser.parse_args(argv)
    root_dir = Path(args.out_root).resolve() if args.out_root else SHARE_ROOT
    if args.character.lower() == "all":
        print(json.dumps({"ok": True, "iterations": [export_character_iteration(c, out_dir=root_dir / c) for c in CHARACTER_PRESETS]}, indent=2))
        return 0
    target_out = args.out_dir or str(root_dir / args.character.lower().strip())
    print(json.dumps(export_character_iteration(args.character, args.poses_dir, args.turnaround_dir, target_out), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
