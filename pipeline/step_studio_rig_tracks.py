#!/usr/bin/env python3
"""Stage 2D / 4E: Magic Studio Native `rig.json` Timed Track Compiler (`step_studio_rig_tracks.py`).

Writes a Magic Studio native `rig.json` (`schemaVersion: 1`, `canvas`, `layers`, `tracks`)
compatible with `crates/engine/src/motion.rs` (`eval_keys_term`), `project.add_asset kind=cutout_rig`,
`motion.tracks`, `motion.part.set`, and `motion.theatre` for interactive human fine-tuning in the app.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import numpy as np
from PIL import Image
import scipy.ndimage as ndi


def _extend_back_under_front(
    back_rgba: np.ndarray,
    front_rgba: np.ndarray,
    full_rgba: np.ndarray,
    pivot: list[float],
    radius: int = 18,
    dilate_iters: int = 12,
) -> np.ndarray:
    """Extend the lower-z back layer underneath the higher-z front layer around the joint pivot so rotation never opens slits."""
    h, w = back_rgba.shape[:2]
    px, py = int(round(pivot[0])), int(round(pivot[1]))
    r = max(22, int(radius * 1.6))
    y0, y1 = max(0, py - r), min(h, py + r)
    x0, x1 = max(0, px - r), min(w, px + r)
    if y0 >= y1 or x0 >= x1:
        return back_rgba

    sub_back = back_rgba[y0:y1, x0:x1].copy()
    sub_front = front_rgba[y0:y1, x0:x1]
    sub_full = full_rgba[y0:y1, x0:x1]

    back_fg = sub_back[:, :, 3] >= 128
    back_core = ndi.binary_erosion(sub_back[:, :, 3] >= 220, iterations=1)
    if not np.any(back_core):
        back_core = back_fg
    if not np.any(back_core):
        return back_rgba

    front_fg = sub_front[:, :, 3] > 80
    full_fg = sub_full[:, :, 3] > 80

    _, nearest_idx = ndi.distance_transform_edt(~back_core, return_indices=True)
    nearest_rgb = sub_back[:, :, :3][nearest_idx[0], nearest_idx[1]]

    dilated_back = ndi.binary_dilation(back_fg, iterations=dilate_iters)
    under_ext = dilated_back & (~back_fg) & front_fg & full_fg
    full_interior = ndi.binary_erosion(full_fg, iterations=6)
    if np.any(full_interior):
        under_ext = under_ext & full_interior
    combined_mask = back_fg | under_ext
    lbl_c, n_c = ndi.label(combined_mask, structure=np.ones((3, 3), dtype=int))
    if n_c > 1:
        core_labels = set(np.unique(lbl_c[back_fg])) - {0}
        under_ext = under_ext & np.isin(lbl_c, list(core_labels))
    if np.any(under_ext):
        sub_back[under_ext, :3] = nearest_rgb[under_ext]
        sub_back[under_ext, 3] = 255
        back_rgba[y0:y1, x0:x1] = sub_back
    return back_rgba


def compile_studio_rig_json(
    character_id: str,
    canvas_size: list[int],
    layers_meta: list[dict],
    timed_trajectories: dict | None = None,
    out_path: str | Path | None = None,
) -> dict:
    """Compile a native Magic Studio `rig.json` with 6 timed tracks (`idle`, `walk`, `run`, `jump`, `wave`, `talk_express`)."""
    cw, ch = int(canvas_size[0]), int(canvas_size[1])
    studio_layers: list[dict] = []
    root_id: str | None = None
    out_dir = Path(out_path).parent if out_path is not None else None
    canvas_layers_dir = (out_dir / "canvas_layers") if out_dir is not None else None
    if canvas_layers_dir is not None:
        canvas_layers_dir.mkdir(parents=True, exist_ok=True)

    canvas_arrays: dict[str, np.ndarray] = {}
    for item in layers_meta:
        pid = item.get("id") or item.get("name") or "part"
        file_rel = item.get("file") or f"{pid}.png"
        parent = item.get("parent")
        if not parent and root_id is None:
            root_id = pid
        bind_pos = item.get("bindPosition") or item.get("center") or [cw / 2.0, ch / 2.0]
        local_piv = item.get("pivot") or [0.0, 0.0]
        canvas_piv = list(bind_pos)

        already_ext = False
        if out_dir is not None and canvas_layers_dir is not None:
            src_png = (out_dir / file_rel).resolve()
            if src_png.is_file():
                with Image.open(src_png) as im:
                    rgba = im.convert("RGBA")
                if rgba.size != (cw, ch):
                    full_im = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
                    ox = int(round(float(bind_pos[0]) - float(local_piv[0])))
                    oy = int(round(float(bind_pos[1]) - float(local_piv[1])))
                    full_im.paste(rgba, (ox, oy), rgba.getchannel("A"))
                    full_rel = f"canvas_layers/{pid}.png"
                    canvas_arrays[pid] = np.array(full_im)
                    file_rel = full_rel
                else:
                    already_ext = True
                    canvas_piv = [float(local_piv[0]), float(local_piv[1])]
                    full_rel = f"canvas_layers/{pid}.png"
                    canvas_arrays[pid] = np.array(rgba)
                    file_rel = full_rel

        studio_layers.append({
            "id": pid,
            "file": file_rel,
            "center": [round(float(bind_pos[0]), 1), round(float(bind_pos[1]), 1)],
            "pivot": [round(float(canvas_piv[0]), 1), round(float(canvas_piv[1]), 1)],
            "parent": parent,
            "scale": 100.0,
            "rotation": 0.0,
            "opacity": 100.0,
            "_already_extended": already_ext,
        })

    # Reorder layers back-to-front (`ears`, `leg_L`, `leg_R`, `torso`, `head`, `arm_L`, `arm_R`)
    # and extend lower-z layers underneath higher-z layers at each joint pivot so rotation never opens slits.
    def _layer_z(lid: str) -> int:
        l = lid.lower()
        if "ear" in l or "tail" in l:
            return 10
        if "leg" in l:
            return 14
        if "foot" in l:
            return 16
        if "torso" in l or "body" in l:
            return 20
        if "head" in l:
            return 30
        if "arm" in l or "wing" in l or "hand" in l:
            return 35
        return 25

    studio_layers.sort(key=lambda lyr: _layer_z(lyr["id"]))
    if canvas_arrays and out_dir is not None:
        comp_im = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
        for lyr in studio_layers:
            pid = lyr["id"]
            if pid in canvas_arrays:
                comp_im.alpha_composite(Image.fromarray(canvas_arrays[pid]))
        full_rgba = np.array(comp_im)
        z_by_id = {lyr["id"]: i for i, lyr in enumerate(studio_layers)}
        for lyr in studio_layers:
            child_id = lyr["id"]
            parent_id = lyr.get("parent")
            if not parent_id or child_id not in canvas_arrays or parent_id not in canvas_arrays:
                continue
            piv = lyr["pivot"]
            if lyr.get("_already_extended"):
                continue
            if z_by_id[child_id] < z_by_id[parent_id]:
                ext_iters = 8 if child_id in ("ear_left", "ear_right") else 12
                canvas_arrays[child_id] = _extend_back_under_front(
                    canvas_arrays[child_id], canvas_arrays[parent_id], full_rgba, piv, dilate_iters=ext_iters
                )
            else:
                canvas_arrays[parent_id] = _extend_back_under_front(
                    canvas_arrays[parent_id], canvas_arrays[child_id], full_rgba, piv, dilate_iters=12
                )
        for lyr in studio_layers:
            lyr.pop("_already_extended", None)
            pid = lyr["id"]
            if pid in canvas_arrays and lyr["file"].startswith("canvas_layers/"):
                Image.fromarray(canvas_arrays[pid]).save(out_dir / lyr["file"])

    if root_id is None and studio_layers:
        root_id = studio_layers[0]["id"]

    traj_tracks = (timed_trajectories or {}).get("tracks", {})
    studio_tracks: list[dict] = [
        {"id": "rest", "label": "Rest", "duration": 2.0, "loop": True, "motion": {}},
    ]

    action_ids = ["idle", "walk", "run", "jump", "wave", "talk_express"]
    for act_id in action_ids:
        t_spec = traj_tracks.get(act_id)
        if not t_spec:
            continue
        dur = float(t_spec.get("duration") or 1.6)
        kfs = t_spec.get("keyframes", [])
        motion_map: dict[str, dict] = {}
        for lyr in studio_layers:
            pid = lyr["id"]
            low = pid.lower()
            if pid == root_id:
                pos_keys = [[float(kf["t"]), [0.0, float(kf.get("rootDyPx", 0.0))]] for kf in kfs]
                motion_map[pid] = {
                    "position": [{"kind": "keys", "ease": "smooth", "keys": pos_keys}],
                }
            elif "head" in low:
                rot_keys = [[float(kf["t"]), float(kf.get("headRotDeg", 0.0))] for kf in kfs]
                motion_map[pid] = {
                    "rotation": [{"kind": "keys", "ease": "smooth", "keys": rot_keys}],
                }
            elif "arm" in low or "wing" in low or "hand" in low:
                is_right = "_r" in low or "right" in low
                key_field = "armRRotDeg" if is_right else "armLRotDeg"
                rot_keys = [[float(kf["t"]), round(0.65 * float(kf.get(key_field, 0.0)), 2)] for kf in kfs]
                motion_map[pid] = {
                    "rotation": [{"kind": "keys", "ease": "smooth", "keys": rot_keys}],
                }
            elif "leg" in low:
                is_right = "_r" in low or "right" in low
                key_field = "legRRotDeg" if is_right else "legLRotDeg"
                rot_keys = [[float(kf["t"]), round(0.55 * float(kf.get(key_field, 0.0)), 2)] for kf in kfs]
                motion_map[pid] = {
                    "rotation": [{"kind": "keys", "ease": "smooth", "keys": rot_keys}],
                }
            elif "foot" in low:
                is_right = "_r" in low or "right" in low
                key_field = "legRRotDeg" if is_right else "legLRotDeg"
                rot_keys = [[float(kf["t"]), round(0.25 * float(kf.get(key_field, 0.0)), 2)] for kf in kfs]
                motion_map[pid] = {
                    "rotation": [{"kind": "keys", "ease": "smooth", "keys": rot_keys}],
                }
            elif any(tag in low for tag in ("ear", "tail", "hair", "prop", "hat", "cape")):
                # If `ear_left` and `ear_right` share a close center crown pivot (< 12% canvas width apart),
                # rotate them in the same direction so a center tuft/crest sways together without splitting.
                ear_l = next((x for x in studio_layers if x["id"] == "ear_left"), None)
                ear_r = next((x for x in studio_layers if x["id"] == "ear_right"), None)
                shared_crest = (
                    ear_l is not None
                    and ear_r is not None
                    and abs(float(ear_r["pivot"][0]) - float(ear_l["pivot"][0])) < 0.12 * cw
                )
                sign = 1.0 if shared_crest else (-1.0 if ("_r" in low or "right" in low) else 1.0)
                rot_keys = [[float(kf["t"]), round(0.45 * sign * float(kf.get("secondaryRotDeg", 0.0)), 2)] for kf in kfs]
                motion_map[pid] = {
                    "rotation": [{"kind": "keys", "ease": "smooth", "keys": rot_keys}],
                }
        studio_tracks.append({
            "id": act_id,
            "label": act_id.replace("_", " ").title(),
            "duration": dur,
            "loop": bool(t_spec.get("loop", True)),
            "motion": motion_map,
        })

    rig_doc = {
        "schemaVersion": 1,
        "id": character_id,
        "name": character_id,
        "canvas": [cw, ch],
        "fps": 30,
        "duration": 2.0,
        "loop": True,
        "background": None,
        "layers": studio_layers,
        "tracks": studio_tracks,
    }
    if out_path is not None:
        p = Path(out_path)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps(rig_doc, indent=2) + "\n", encoding="utf-8")
    return rig_doc


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--character", required=True)
    ap.add_argument("--timeline-json", required=True, help="Path to <slug>_6actions_timeline.json")
    ap.add_argument("--timed-trajectories", default=None, help="Optional path to timed_trajectories.json")
    ap.add_argument("--out", required=True, help="Output rig.json path")
    args = ap.parse_args(argv)

    tl = json.loads(Path(args.timeline_json).read_text())
    traj = json.loads(Path(args.timed_trajectories).read_text()) if args.timed_trajectories else None
    sheet_path = tl.get("outputs", {}).get("layersSheet")
    canvas = [560, 720]
    if sheet_path and Path(sheet_path).is_file():
        with Image.open(sheet_path) as im:
            canvas = [im.width, im.height]
    doc = compile_studio_rig_json(
        character_id=args.character,
        canvas_size=canvas,
        layers_meta=tl.get("layers", []),
        timed_trajectories=traj,
        out_path=args.out,
    )
    track_ids = [t.get("id") for t in doc["tracks"]] if isinstance(doc["tracks"], list) else list(doc["tracks"].keys())
    print(json.dumps({"ok": True, "rig": args.out, "tracks": track_ids, "layers": len(doc["layers"])}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
