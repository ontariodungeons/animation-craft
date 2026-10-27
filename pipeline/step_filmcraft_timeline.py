#!/usr/bin/env python3
"""Stage 4E: FilmCraft Multi-Track NLE Sequence & Action Marker Builder (`step_filmcraft_timeline.py`).

Builds a FilmCraft `.fcproj` sequence from the character's rendered EffectCraft `.mp4` and cutout layer PNGs,
places named program timeline markers (`markers.add`) for each action segment (`idle`, `walk`, `run`, `jump`,
`wave`, `talk_express`), exports `<slug>_markers.csv` (`markers.exportCsv`), and renders a verified frame.
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.filmcraft_bridge import load_animation_into_filmcraft

TICKS_PER_SECOND = 254016000000  # Premiere/FilmCraft standard tick rate


def build_filmcraft_marked_sequence(
    slug: str,
    media_path: str | Path,
    project_out: str | Path,
    frame_png_out: str | Path,
    action_segments: list[dict[str, Any]] | None = None,
    extra_assets: list[str] | None = None,
) -> dict[str, Any]:
    """Import media into FilmCraft, stamp named action markers on the sequence, and export marker CSV."""
    base_res = load_animation_into_filmcraft(
        media_path=str(media_path),
        project_out=str(project_out),
        frame_png_out=str(frame_png_out),
        extra_assets=extra_assets,
    )
    fc_bin = shutil.which("filmcraft-cli") or "filmcraft-cli"
    abs_proj = Path(project_out).resolve()
    markers_csv = abs_proj.with_name(f"{slug}_markers.csv")

    segments = action_segments or [
        {"id": "idle", "title": "1. IDLE", "startSec": 0.0, "durationFrames": 19},
        {"id": "walk", "title": "2. WALK (R/L)", "startSec": 0.8, "durationFrames": 19},
        {"id": "run", "title": "3. RUN", "startSec": 1.6, "durationFrames": 19},
        {"id": "jump", "title": "4. JUMP", "startSec": 2.4, "durationFrames": 19},
        {"id": "wave", "title": "5. WAVE", "startSec": 3.2, "durationFrames": 19},
        {"id": "talk_express", "title": "6. TALK / EXPRESS", "startSec": 4.0, "durationFrames": 19},
    ]

    marker_receipts: list[dict[str, Any]] = []
    for seg in segments:
        start_sec = float(seg.get("startSec", 0.0))
        ticks = int(round(start_sec * TICKS_PER_SECOND))
        dur_frames = int(seg.get("durationFrames", 19))
        name = str(seg.get("title") or seg.get("id") or "Action")
        comment = f"{slug}:{seg.get('id', 'action')}"
        proc = subprocess.run(
            [
                fc_bin,
                "--project",
                str(abs_proj),
                "--save",
                "exec",
                "markers.add",
                "target=program",
                f"time={ticks}",
                f"name={name}",
                f"comment={comment}",
                f"durationFrames={dur_frames}",
            ],
            capture_output=True,
            text=True,
            timeout=15,
        )
        marker_receipts.append({
            "id": seg.get("id"),
            "name": name,
            "startSec": start_sec,
            "durationFrames": dur_frames,
            "ok": proc.returncode == 0,
        })

    subprocess.run(
        [
            fc_bin,
            "--project",
            str(abs_proj),
            "exec",
            "markers.exportCsv",
            f"path={markers_csv}",
        ],
        capture_output=True,
        text=True,
        timeout=15,
    )

    base_res["markers"] = marker_receipts
    base_res["markersCsv"] = str(markers_csv) if markers_csv.is_file() else None
    base_res["verifiedFrame"] = base_res.get("programFramePng")
    return base_res


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--character", required=True)
    ap.add_argument("--media", required=True)
    ap.add_argument("--project-out", required=True)
    ap.add_argument("--frame-out", required=True)
    args = ap.parse_args(argv)
    res = build_filmcraft_marked_sequence(
        slug=args.character,
        media_path=args.media,
        project_out=args.project_out,
        frame_png_out=args.frame_out,
    )
    print(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
