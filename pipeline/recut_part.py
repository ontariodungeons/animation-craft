#!/usr/bin/env python3
"""Re-cut one rigged part from its clean source art with PhotoCraft (headless photocraft-cli).

The old part file gives the shape and position; PhotoCraft gives the clean edge:
  1. scale the source to the rig's size and place the old part where the rig puts it;
  2. take the part's shape, smooth and grow it (recovers clipped edges);
  3. intersect with PhotoCraft's subject outline (drops background) and copy it to its own layer;
  4. where the part sits behind other parts (e.g. a fin behind the body), content-aware fill that
     hidden strip from the part's own texture, so rotation never shows a smear or the body;
  5. crop back to the part's box (same size, so the rig data stays valid).
No pixels are edited outside PhotoCraft. Prints one JSON result.

  recut_part.py --source lineup.png --scale 2 --part fin.png --at 659,722 \
      --behind body.png:362,265 --out fin_new.png
"""
import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


def xy(text):
    x, y = text.split(",")
    return int(float(x)), int(float(y))


def size(path):
    # PNG header: width and height are big-endian u32 at bytes 16..24.
    with open(path, "rb") as f:
        head = f.read(24)
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(json.dumps({"ok": False, "error": f"not a PNG: {path}"}))
    return int.from_bytes(head[16:20], "big"), int.from_bytes(head[20:24], "big")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", required=True, help="clean source art the part was cut from")
    ap.add_argument("--scale", type=float, default=1.0, help="rig pixels per source pixel")
    ap.add_argument("--part", required=True, help="the existing part PNG (shape and box)")
    ap.add_argument("--at", required=True, type=xy, help="part's top-left in the scaled source, x,y")
    ap.add_argument("--behind", action="append", default=[], help="part PNG this one sits behind, as path:x,y")
    ap.add_argument("--expand", type=int, default=6, help="pixels to grow the old shape")
    ap.add_argument("--smooth", type=int, default=40, help="edge smoothing 0..100")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    w, h = size(a.part)
    x, y = a.at
    cmds = [
        ("image.imageSize", {"percent": a.scale * 100}),
        ("file.placeEmbedded", {"path": a.part, "scale": 100, "fit": False, "smartObject": False, "center": [x + w / 2, y + h / 2]}),
        ("select.loadSelection", {"channel": "transparency"}),
        ("select.refineEdge", {"smooth": a.smooth, "output": "selection"}),
        ("select.modify.expand", {"radius": max(1, a.expand)}),
        ("layer.delete", {}),
        ("select.subject", {"mode": "intersect", "sampleAllLayers": True}),
        ("layer.new.layerViaCopy", {}),
    ]
    for spec in a.behind:
        path, pos = spec.rsplit(":", 1)
        bx, by = xy(pos)
        bw, bh = size(path)
        cmds += [
            # the hidden strip = this part ∩ the part in front of it
            ("select.loadSelection", {"channel": "transparency"}),
            ("file.placeEmbedded", {"path": path, "scale": 100, "fit": False, "smartObject": False, "center": [bx + bw / 2, by + bh / 2]}),
            ("select.loadSelection", {"channel": "transparency", "operation": "intersect"}),
            ("layer.delete", {}),
            ("layer.select", {"name": "Layer 1"}),
            # sample only the part's own pixels (its box), not the transparent surroundings
            ("edit.contentAwareFill", {"sampling": "custom", "area": [x, y, w, h], "output": "current"}),
            ("select.deselect", {}),
        ]
    cmds += [
        ("layer.select", {"name": "Background"}),
        ("layer.delete", {}),
        ("image.crop", {"x": x, "y": y, "width": w, "height": h}),
    ]
    argv = [shutil.which("photocraft-cli") or "photocraft-cli", "run", a.source]
    for cid, params in cmds:
        argv += ["--cmd", cid, "--params", json.dumps(params)]
    argv += ["--out", a.out]
    run = subprocess.run(argv, capture_output=True, text=True)
    steps = [json.loads(l) for l in run.stdout.splitlines() if l.strip().startswith("{")]
    ok = run.returncode == 0 and Path(a.out).is_file()
    out_size = size(a.out) if ok else None
    print(json.dumps({"ok": ok and out_size == (w, h), "out": a.out, "size": out_size, "expected_size": [w, h],
                      "steps": len(steps), "error": None if ok else (run.stderr.strip() or run.stdout.strip())[-600:]}))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
