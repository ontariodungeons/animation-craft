#!/usr/bin/env python3
"""Stage 3A: Orthographic 5-view turnaround registration & vertical drift alignment.

Measures each turnaround view's crown and foot baseline at the given alpha threshold,
uniformly scales views to the reference view's height, aligns foot-center X and crown/foot Y
on a shared padded canvas, and writes `registered_turnaround/manifest.json`, `atlas.png`,
`contact_sheet.jpg`, and `turnaround_registration.json`.
"""
from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
import json
import math
import os
from pathlib import Path
from PIL import Image, ImageDraw

SCHEMA_VERSION = 1
DIRECTIONS = ("front", "quarter_front", "side", "opposite_quarter", "back")


def write_json_atomic(path: str | Path, value: dict) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    try:
        temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def read_sheet_frames(directory: str | Path) -> tuple[list[dict], dict[str, str]]:
    root = Path(directory).resolve()
    manifest_path = root / "manifest.json"
    manifest_data = manifest_path.read_bytes()
    manifest = json.loads(manifest_data)
    frames = []
    hashes = {str(manifest_path): sha256(manifest_data).hexdigest()}
    for item in manifest.get("frames", []):
        path = (root / item["file"]).resolve()
        data = path.read_bytes()
        hashes[str(path)] = sha256(data).hexdigest()
        with Image.open(BytesIO(data)) as src:
            image = src.convert("RGBA")
        frames.append({"meta": item, "path": path, "image": image})
    if not frames:
        raise ValueError(f"No frames in {manifest_path}")
    return frames, hashes


def measure_silhouette(image: Image.Image, threshold: int = 128) -> dict:
    alpha = image.getchannel("A").point(lambda a: 255 if a >= threshold else 0)
    bbox = alpha.getbbox()
    if not bbox:
        raise ValueError("Blank foreground at selected alpha threshold")
    left, top, right, bottom = bbox
    foot_band = alpha.crop((left, max(top, bottom - max(2, round((bottom - top) * 0.08))), right, bottom))
    fb_box = foot_band.getbbox() or (0, 0, right - left, 1)
    foot_x = left + (fb_box[0] + fb_box[2]) * 0.5
    area_px = sum(alpha.histogram()[1:])
    return {
        "bbox": [left, top, right, bottom],
        "crownY": top,
        "footBaselineY": bottom,
        "heightPx": bottom - top,
        "widthPx": right - left,
        "footCenterX": foot_x,
        "centroid": [(left + right) * 0.5, (top + bottom) * 0.5],
        "areaPx": area_px,
        "ignoredOpaquePixels": 0,
        "softAlphaPixels": 0,
    }


def register_turnaround(
    turnaround_dir: str | Path,
    out_dir: str | Path,
    alpha_threshold: int = 128,
    reference: str = "front",
    _loaded: tuple[list[dict], dict[str, str]] | None = None,
) -> dict:
    frames, hashes = _loaded if _loaded is not None else read_sheet_frames(turnaround_dir)
    by_label = {f["meta"]["label"]: f for f in frames if f["meta"]["label"] in DIRECTIONS}
    if not by_label:
        by_label = {f["meta"].get("label", f"view_{i}"): f for i, f in enumerate(frames)}
    if reference not in by_label:
        reference = next(iter(by_label))
    for f in frames:
        f["measure"] = measure_silhouette(f["image"], alpha_threshold)
    target_height = by_label[reference]["measure"]["heightPx"]
    transformed = []
    ordered_labels = [d for d in DIRECTIONS if d in by_label] or list(by_label.keys())
    for label in ordered_labels:
        f = by_label[label]
        orig_h = max(1, f["measure"]["heightPx"])
        scale = target_height / orig_h
        nw = max(1, round(f["image"].width * scale))
        nh = max(1, round(f["image"].height * scale))
        scaled = f["image"].resize((nw, nh), Image.Resampling.BICUBIC)
        measured = measure_silhouette(scaled, alpha_threshold)
        transformed.append((label, f, scaled, measured, scale))

    padding = 16
    left_room = math.ceil(max(m["footCenterX"] for _, _, _, m, _ in transformed))
    right_room = math.ceil(max(im.width - m["footCenterX"] for _, _, im, m, _ in transformed))
    top_room = max(m["crownY"] for _, _, _, m, _ in transformed)
    bottom_room = max(im.height - m["footBaselineY"] for _, _, im, m, _ in transformed)
    crown = padding + top_room
    baseline = crown + target_height
    center = padding + left_room
    size = (padding * 2 + left_room + right_room, baseline + bottom_room + padding)

    output = Path(out_dir).resolve()
    destination = output / "registered_turnaround"
    (destination / "frames").mkdir(parents=True, exist_ok=True)
    result_frames = []
    atlas = Image.new("RGBA", (size[0] * len(transformed), size[1]))
    contact = Image.new("RGB", (atlas.width, size[1] + 24), "#60636a")
    contact_draw = ImageDraw.Draw(contact)

    for i, (label, frame, image, measure, scale) in enumerate(transformed):
        tx = max(0, min(size[0] - image.width, round(center - measure["footCenterX"])))
        ty = max(0, min(size[1] - image.height, crown - measure["crownY"]))
        aligned = Image.new("RGBA", size)
        aligned.paste(image, (tx, ty))
        final = measure_silhouette(aligned, alpha_threshold)
        filename = f"frames/{i:02d}_{label}.png"
        aligned.save(destination / filename, compress_level=3)
        atlas.paste(aligned, (i * size[0], 0))
        contact.paste(aligned, (i * size[0], 0), aligned.getchannel("A"))
        contact_draw.text((i * size[0] + 5, size[1] + 3), label, fill="white")
        iw, ih = frame["image"].size
        result_frames.append({
            "index": i,
            "label": label,
            "file": filename,
            "size": list(size),
            "sourceFile": str(frame["path"]),
            "sourceSize": [iw, ih],
            "sourceBox": frame["meta"].get("sourceBox"),
            "input": frame["measure"],
            "registered": final,
            "uniformScale": scale,
            "anchor": [center, baseline],
            "atlasRect": [i * size[0], 0, *size],
            "uv": [i / len(transformed), 0, 1 / len(transformed), 1],
        })

    atlas.save(destination / "atlas.png", compress_level=3)
    contact.save(destination / "contact_sheet.jpg", quality=92)
    crown_vals = [f["registered"]["crownY"] for f in result_frames]
    foot_vals = [f["registered"]["footBaselineY"] for f in result_frames]
    drift = float(max(max(crown_vals) - min(crown_vals), max(foot_vals) - min(foot_vals)))
    before_crown = [f["input"]["crownY"] for f in result_frames]
    before_foot = [f["input"]["footBaselineY"] for f in result_frames]
    result = {
        "schemaVersion": SCHEMA_VERSION,
        "kind": "registered_turnaround",
        "assetRoot": str(destination),
        "alphaThreshold": alpha_threshold,
        "referenceDirection": reference,
        "frames": result_frames,
        "target": {
            "size": list(size),
            "crownY": crown,
            "footBaselineY": baseline,
            "heightPx": target_height,
            "footCenterX": center,
        },
        "before": {
            "crownDriftPx": max(before_crown) - min(before_crown),
            "footDriftPx": max(before_foot) - min(before_foot),
        },
        "maxVerticalDriftPx": drift,
        "rasterRegistrationPassed": drift <= 2.0,
        "atlas": {"file": "atlas.png", "size": list(atlas.size)},
        "sourceHashes": hashes,
    }
    write_json_atomic(destination / "manifest.json", result)
    write_json_atomic(output / "turnaround_registration.json", result)
    return result


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--turnaround-dir", required=True)
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--alpha-threshold", type=int, default=128)
    ap.add_argument("--reference", default="front")
    args = ap.parse_args(argv)
    res = register_turnaround(args.turnaround_dir, args.out_dir, args.alpha_threshold, args.reference)
    print(json.dumps(res, indent=2))
    return 0 if res["rasterRegistrationPassed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
