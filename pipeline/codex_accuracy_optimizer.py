#!/usr/bin/env python3
"""Stage 3 Orchestrator: Turnaround registration (`step_turnaround_register`) & motion-range trajectories (`step_motion_range`)."""
from __future__ import annotations

import argparse
import json
import shutil
import time
from pathlib import Path

from pipeline.step_turnaround_register import (
    DIRECTIONS,
    SCHEMA_VERSION,
    measure_silhouette as _measure,
    read_sheet_frames as _read_frames,
    register_turnaround,
    write_json_atomic as _write_json,
)
from pipeline.step_motion_range import (
    REQUESTED_POSES,
    compile_timed_trajectories,
    load_motion_range,
    measure_motion_range,
)


def discover_local_tools() -> dict:
    return {
        name: {"paths": [p for p in [shutil.which(f"{name}-cli")] if p]}
        for name in ("photocraft", "vectorcraft", "filmcraft", "effectcraft")
    }


def optimize(turnaround_dir, poses_dir, out_dir, alpha_threshold=128, tools=False) -> dict:
    start = time.perf_counter()
    out = Path(out_dir).resolve()
    turnaround = _read_frames(turnaround_dir)
    poses = _read_frames(poses_dir)
    registration = register_turnaround(turnaround_dir, out, alpha_threshold, _loaded=turnaround)
    motion = measure_motion_range(poses_dir, out, registration, alpha_threshold, _loaded=poses)
    report = {
        "schemaVersion": SCHEMA_VERSION,
        "cacheHit": False,
        "elapsedSeconds": time.perf_counter() - start,
        "frames": {"turnaround": len(registration["frames"]), "poses": len(motion["poses"])},
        "registrationPassed": registration["rasterRegistrationPassed"],
        "maxVerticalDriftPx": registration["maxVerticalDriftPx"],
        "motionRangeFile": str(out / "motion_range.json"),
        "timedTrajectoriesFile": str(out / "timed_trajectories.json"),
        "registrationFile": str(out / "turnaround_registration.json"),
    }
    if tools:
        report["localTools"] = discover_local_tools()
    _write_json(out / "optimizer_report.json", report)
    return report


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--turnaround-dir", default="exports/pose_and_parts/hopper_turnaround")
    parser.add_argument("--poses-dir", default="exports/pose_and_parts/hopper_poses")
    parser.add_argument("--out-dir", default="exports/pose_and_parts/hopper")
    parser.add_argument("--alpha-threshold", type=int, default=128)
    parser.add_argument("--tools", action="store_true")
    args = parser.parse_args(argv)
    result = optimize(args.turnaround_dir, args.poses_dir, args.out_dir, args.alpha_threshold, args.tools)
    print(json.dumps(result, indent=2))
    return 0 if result["registrationPassed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
