#!/usr/bin/env python3
"""Build a volume-turn layered_motion rig spec from registered turn views.

A volume turn is not a rotation: it is a cross-fade between five drawn views
(right profile, right three-quarter, front, left three-quarter, left profile)
that all share one body pivot on one canvas. The left profile reuses the right
profile's artwork and mirrors it around that pivot with a negative scale.

Inputs are the registered views produced by `image.register_frame` plus the
generation receipt for the pose sheet. The output is the rig spec that
`project.add_asset kind=cutout_rig` consumes, so the Animation tab and
`blender.start` can render the turn.

Usage:
  python3 pipeline/build_volume_turn_rig.py \
    --registered-dir DIR --canvas 700 440 --pivot 350 220 \
    --character-id NAME --source-character ID \
    --source-generation assets/generated-images/ID/generation.json \
    --duration 1.1 --out PATH [--sheet-source NAME --sheet-canvas W H]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# The five views of a turn, in turn order.
VIEWS = ["profile_right", "quarter_right", "front", "quarter_left", "profile_left"]
# Both profile layers read the same artwork; only the left one is mirrored.
PROFILE_SOURCE = {"profile_right": "profile", "profile_left": "profile"}
# The left profile is the right profile's artwork, mirrored around the pivot.
MIRRORED = {"profile_left"}
# The verified little-fish turn's opacity keys, as fractions of the track
# duration: five views, each fading in before its stage and out before the next.
# Index 0 is the right profile, 4 the left profile.
OPACITY_TABLE = [
    [(0.00, 0), (0.08, 0), (0.18, -100), (1.00, -100)],
    [(0.00, -100), (0.08, -100), (0.18, 0), (0.33, 0), (0.40, -100), (1.00, -100)],
    [(0.00, -100), (0.33, -100), (0.40, 0), (0.60, 0), (0.67, -100), (1.00, -100)],
    [(0.00, -100), (0.60, -100), (0.67, 0), (0.82, 0), (0.92, -100), (1.00, -100)],
    [(0.00, -100), (0.82, -100), (0.92, 0), (1.00, 0)],
]
# Every layer rides a gentle bob and sway so the cut-out is not rigid.
POSITION_KEYS = [(0.0, (0, 0)), (0.5, (0, -4)), (1.0, (0, 0))]
ROTATION_KEYS = [(0.0, 0), (0.35, -3), (0.70, 2), (1.0, 0)]
MIRROR_SCALE = (-200, 0)


def fail(message: str) -> None:
    print(f"build_volume_turn_rig: {message}", file=sys.stderr)
    raise SystemExit(1)


def keys(points: list[tuple[float, float]]) -> list[dict]:
    """Keyframe list in the engine's channel encoding."""
    return [{"kind": "keys", "ease": "smooth", "keys": [[t, v] for t, v in points]}]


def opacity_channel(index: int, duration: float, left_to_right: bool) -> list[dict]:
    """Opacity keys for one view: 0 is visible, -100 is hidden."""
    stage = len(VIEWS) - 1 - index if left_to_right else index
    return keys([(fraction * duration, value) for fraction, value in OPACITY_TABLE[stage]])


def verify_registered(registered: Path, canvas: tuple[int, int]) -> dict[str, Path]:
    """Every view must exist, be RGBA and be exactly the shared canvas."""
    from PIL import Image

    found: dict[str, Path] = {}
    for view in VIEWS:
        source = PROFILE_SOURCE.get(view, view)
        path = registered / f"{source}.png"
        if not path.is_file():
            fail(f"missing registered view {path}")
        with Image.open(path) as image:
            size = image.size
            mode = image.mode
        if size != canvas:
            fail(f"{path} is {size}, expected the shared canvas {canvas}")
        if mode != "RGBA":
            fail(f"{path} is {mode}, expected RGBA with transparency")
        found[view] = path
    return found


def build_tracks(duration: float) -> list[dict]:
    tracks = []
    for track_id, label, left_to_right in [
        ("turn-left", "Volume turn · right to left", False),
        ("turn-right", "Volume turn · left to right", True),
    ]:
        motion: dict[str, dict] = {}
        for index, view in enumerate(VIEWS):
            channel = {
                "opacity": opacity_channel(index, duration, left_to_right),
                "position": keys([(t * duration, v) for t, v in POSITION_KEYS]),
                "rotation": keys([(t * duration, v) for t, v in ROTATION_KEYS]),
            }
            if view in MIRRORED:
                channel["scale"] = [{"kind": "keys", "keys": [[0, list(MIRROR_SCALE)], [duration, list(MIRROR_SCALE)]]}]
            motion[view] = channel
        tracks.append(
            {
                "id": track_id,
                "label": label,
                "duration": duration,
                "loop": False,
                "motion": motion,
            }
        )
    return tracks


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--registered-dir", required=True, type=Path)
    parser.add_argument("--canvas", required=True, type=int, nargs=2)
    parser.add_argument("--pivot", required=True, type=int, nargs=2)
    parser.add_argument("--character-id", required=True)
    parser.add_argument("--source-character", required=True)
    parser.add_argument("--source-generation", type=Path)
    parser.add_argument("--duration", type=float, default=1.1)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--sheet-source")
    parser.add_argument("--sheet-canvas", type=int, nargs=2)
    args = parser.parse_args()

    canvas = tuple(args.canvas)
    pivot = list(args.pivot)
    views = verify_registered(args.registered_dir, canvas)

    rig = {
        "rig_type": "layered_motion",
        "character_id": args.character_id,
        "source_character": args.source_character,
        "source_generation": str(args.source_generation) if args.source_generation else None,
        "canvas": list(canvas),
        "fps": 30,
        "layers": [
            {"id": view, "file": str(views[view]), "pivot": list(pivot)} for view in VIEWS
        ],
        "tracks": build_tracks(args.duration),
        "animation_notes": (
            "Volume turn: five drawn views on one shared pivot, cross-faded by opacity. "
            "The left profile reuses the right profile mirrored around the pivot."
        ),
    }
    if rig["source_generation"] is None:
        rig.pop("source_generation")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(rig, indent=1), encoding="utf-8")

    if args.sheet_source or args.sheet_canvas:
        registration = {
            "source": args.sheet_source or "",
            "poses": [
                {
                    "id": view,
                    "pivot": list(pivot),
                    "scale": 1.0,
                }
                for view in VIEWS
                if view not in MIRRORED
            ],
            "sheet_canvas": list(args.sheet_canvas) if args.sheet_canvas else None,
            "method": "registered views share one body pivot; opacity cross-fade between five views",
            "status": "built from registered views",
        }
        if registration["sheet_canvas"] is None:
            registration.pop("sheet_canvas")
        args.out.with_name("registration.json").write_text(
            json.dumps(registration, indent=1), encoding="utf-8"
        )

    print(
        json.dumps(
            {
                "ok": True,
                "rig": str(args.out),
                "character_id": args.character_id,
                "layers": len(rig["layers"]),
                "tracks": [track["id"] for track in rig["tracks"]],
                "canvas": rig["canvas"],
                "pivot": pivot,
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
