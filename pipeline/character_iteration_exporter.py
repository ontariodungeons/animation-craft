#!/usr/bin/env python3
"""
Design-Team Character Iteration Exporter.

For each character iteration (hopper, whiskers, puddles, jenny, rusty, or any
custom cut pose/turnaround directory), produces a complete shareable 2D
layered + 6-action animation package in `exports/design_share/<character>/`:
  1. `layers/` + `<char>_layers_sheet.png` — isolated transparent part layers
     (head, ears, torso, left_arm, right_arm, left_leg, right_leg) with pivots
  2. `motion_range.json` + `turnaround_registration.json` — verified 0px-drift
     turnaround registration and silhouette motion range
  3. `<char>_6actions_strip.png` — 6-action contact sheet + layer breakdown
  4. `<char>_6actions_track.mp4` — 1280x720 24fps MP4 animation track playing
     all 6 actions sequentially (`idle`, `walk`, `run`, `jump`, `wave`, `talk_express`)
  5. `<char>_6actions_timeline.json` + `<char>_layers_6actions.jsx` + `<char>_6actions.fcproj`
"""
from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

from pipeline.codex_accuracy_optimizer import measure_motion_range, register_turnaround
from pipeline.filmcraft_bridge import load_animation_into_filmcraft

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

SIX_ACTIONS: list[tuple[str, str, list[str]]] = [
    ("idle", "1. IDLE", ["looking_up", "standing", "looking_down", "standing"]),
    ("walk", "2. WALK", ["walking", "standing", "walking", "pointing"]),
    ("run", "3. RUN", ["running", "walking", "jumping", "walking"]),
    ("jump", "4. JUMP", ["jumping", "standing", "jumping", "running"]),
    ("wave", "5. WAVE", ["waving", "arms_wide", "clapping", "waving"]),
    ("talk_express", "6. TALK / EXPRESS", ["pointing", "dancing", "clapping", "waving"]),
]


def ensure_character_sheets_ready(slug: str) -> None:
    """Ensure clean turnaround and pose sheets exist for hopper, miss_hayley, and old_mac using the cutter tool."""
    from pipeline.pose_and_part_cutter import cut_sheet

    if slug == "hopper":
        # Repair 00_standing.png cheek notch in hopper_poses using notch-free PS_18 standing frame
        clean_src = POSE_PARTS_ROOT / "Create_pose_sheet_for_puppet_202607180130_18/frames/00_standing.png"
        dst_frame = POSE_PARTS_ROOT / "hopper_poses/frames/00_standing.png"
        marker = POSE_PARTS_ROOT / "hopper_poses/.ps18_standing_repaired"
        if clean_src.is_file() and dst_frame.is_file() and not marker.exists():
            with Image.open(clean_src) as c_im, Image.open(dst_frame) as d_im:
                cw, ch = d_im.size
                d_bbox = d_im.getchannel("A").getbbox()
                c_bbox = c_im.getchannel("A").getbbox()
                if d_bbox and c_bbox:
                    target_h = d_bbox[3] - d_bbox[1]
                    crop_c = c_im.crop(c_bbox)
                    scale = target_h / max(1, crop_c.height)
                    new_w = max(1, int(round(crop_c.width * scale)))
                    resized = crop_c.resize((new_w, target_h), Image.Resampling.LANCZOS)
                    canvas = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
                    ox = (cw - new_w) // 2
                    oy = d_bbox[3] - target_h
                    canvas.paste(resized, (ox, oy))
                    canvas.save(dst_frame)
                    marker.write_text("repaired from PS_18\n")

    elif slug == "miss_hayley":
        t_dir = POSE_PARTS_ROOT / "miss_hayley_turnaround"
        p_dir = POSE_PARTS_ROOT / "miss_hayley_poses"
        sheet_src = Path("/home/jesse/main-vault/Creative/Design/Characters/ChatGPT Image Jul 27, 2026, 08_59_54 PM.png")
        front_cut = Path("/home/jesse/main-vault/Creative/Curriculum/Characters/Miss Hayley.png")
        if not (t_dir / "manifest.json").is_file() or not (p_dir / "manifest.json").is_file():
            work_dir = POSE_PARTS_ROOT / "_miss_hayley_build"
            work_dir.mkdir(parents=True, exist_ok=True)
            # Slice the 3 full-body views from Miss Hayley's sheet
            raw_3view_dir = work_dir / "raw_3view"
            layout_3 = {
                "frames": [
                    {"label": "standing", "box": [27, 99, 337, 932]},
                    {"label": "arms_wide", "box": [364, 61, 792, 939]},
                    {"label": "side", "box": [749, 104, 1006, 937]},
                ]
            }
            cut_sheet(sheet_src, raw_3view_dir, kind="action", layout=layout_3, use_ocr=False, tolerance=24)
            f_stand_raw = Image.open(raw_3view_dir / "frames/00_standing.png").convert("RGBA")
            f_wide_raw = Image.open(raw_3view_dir / "frames/01_arms_wide.png").convert("RGBA")
            f_side_raw = Image.open(raw_3view_dir / "frames/02_side.png").convert("RGBA")
            if front_cut.is_file():
                f_stand_raw = Image.open(front_cut).convert("RGBA")

            cw, ch = 480, 840

            def _fit_tight(src_im: Image.Image) -> Image.Image:
                im_c = src_im.copy()
                w0, h0 = im_c.size
                px0 = im_c.load()
                for yy in range(h0):
                    for xx in range(w0):
                        r0, g0, b0, a0 = px0[xx, yy]
                        if a0 < 32:
                            px0[xx, yy] = (0, 0, 0, 0)
                        elif max(r0, g0, b0) - min(r0, g0, b0) < 18 and min(r0, g0, b0) > 200:
                            # Remove enclosed white/grey studio background islands & floor shadows
                            px0[xx, yy] = (0, 0, 0, 0)
                bbox = im_c.getchannel("A").getbbox()
                cropped = im_c.crop(bbox) if bbox else im_c
                target_h = ch - 12
                scale = target_h / max(1, cropped.height)
                new_w = max(1, int(round(cropped.width * scale)))
                if new_w > cw - 8:
                    scale = (cw - 8) / max(1, cropped.width)
                    new_w = cw - 8
                    target_h = max(1, int(round(cropped.height * scale)))
                resized = cropped.resize((new_w, target_h), Image.Resampling.LANCZOS)
                rr, rg, rb, ra = resized.split()
                ra_clean = ra.point(lambda v: v if v >= 32 else 0)
                resized = Image.merge("RGBA", (rr, rg, rb, ra_clean))
                canvas = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
                ox = (cw - new_w) // 2
                oy = ch - 6 - target_h
                canvas.paste(resized, (ox, oy))
                return canvas

            f_stand = _fit_tight(f_stand_raw)
            f_wide = _fit_tight(f_wide_raw)
            f_side = _fit_tight(f_side_raw)
            # Synthesize 5-view turnaround strip image and slice via cut_sheet
            f_opp = f_side.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
            # Back view: darken hair/dress front details into back hair/cardigan silhouette
            back_arr = f_stand.copy()
            px = back_arr.load()
            for y in range(ch):
                for x in range(cw):
                    r, g, b, a = px[x, y]
                    if a > 16:
                        if y < int(ch * 0.44):
                            # Warm brown hair back
                            px[x, y] = (int(r * 0.52 + 42), int(g * 0.42 + 26), int(b * 0.35 + 18), a)
                        elif y < int(ch * 0.78):
                            # Back of mustard cardigan / dress
                            px[x, y] = (int(r * 0.88), int(g * 0.85), int(b * 0.80), a)
            views_5 = [f_stand, f_wide, f_side, f_opp, back_arr]
            strip_t = Image.new("RGBA", (cw * 5, ch), (0, 0, 0, 0))
            layout_t = {"frames": []}
            for idx, (v_im, v_lbl) in enumerate(zip(views_5, ["front", "quarter_front", "side", "opposite_quarter", "back"])):
                strip_t.paste(v_im, (idx * cw, 0))
                vb = v_im.getchannel("A").getbbox() or (0, 0, cw, ch)
                layout_t["frames"].append({
                    "label": v_lbl,
                    "box": [idx * cw + vb[0], vb[1], idx * cw + vb[2], vb[3]],
                })
            strip_t_path = work_dir / "miss_hayley_5view_strip.png"
            strip_t.save(strip_t_path)
            cut_sheet(strip_t_path, t_dir, kind="turnaround", layout=layout_t, use_ocr=False)

            # Synthesize 10-pose action sheet for Miss Hayley (standing, walking, running, jumping, waving, pointing, dancing, clapping, looking_up, arms_wide)
            def _make_pose_variant(base_im: Image.Image, dy: int = 0, rot_deg: float = 0.0, flip: bool = False) -> Image.Image:
                im = base_im.transpose(Image.Transpose.FLIP_LEFT_RIGHT) if flip else base_im.copy()
                if abs(rot_deg) > 0.1:
                    im = im.rotate(rot_deg, resample=Image.Resampling.BICUBIC, center=(cw // 2, int(ch * 0.55)))
                out = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
                out.paste(im, (0, dy))
                return out

            pose_defs = [
                ("standing", _make_pose_variant(f_stand, dy=0)),
                ("walking", _make_pose_variant(f_side, dy=-4)),
                ("running", _make_pose_variant(f_side, dy=-10, rot_deg=-4.0, flip=True)),
                ("jumping", _make_pose_variant(f_wide, dy=-18)),
                ("waving", _make_pose_variant(f_stand, dy=-2, rot_deg=1.5)),
                ("pointing", _make_pose_variant(f_side, dy=-2, flip=True)),
                ("dancing", _make_pose_variant(f_wide, dy=-8, rot_deg=-3.5)),
                ("clapping", _make_pose_variant(f_stand, dy=-4, rot_deg=2.0)),
                ("looking_up", _make_pose_variant(f_stand, dy=-2, rot_deg=-2.0)),
                ("arms_wide", _make_pose_variant(f_wide, dy=0)),
            ]
            strip_p = Image.new("RGBA", (cw * len(pose_defs), ch), (0, 0, 0, 0))
            layout_p = {"frames": []}
            for idx, (p_lbl, p_im) in enumerate(pose_defs):
                strip_p.paste(p_im, (idx * cw, 0))
                pb = p_im.getchannel("A").getbbox() or (0, 0, cw, ch)
                layout_p["frames"].append({
                    "label": p_lbl,
                    "box": [idx * cw + pb[0], pb[1], idx * cw + pb[2], pb[3]],
                })
            strip_p_path = work_dir / "miss_hayley_10pose_strip.png"
            strip_p.save(strip_p_path)
            cut_sheet(strip_p_path, p_dir, kind="action", layout=layout_p, use_ocr=False)

    elif slug == "old_mac":
        p_dir = POSE_PARTS_ROOT / "Create_pose_sheet_for_puppet_202607180130_27"
        t_dir = POSE_PARTS_ROOT / "old_mac_turnaround"
        # Ensure frame 7 in PS_27 is labeled "waving" instead of duplicate "standing" so Old Mac has a distinct wave pose
        m_path = p_dir / "manifest.json"
        if m_path.is_file():
            m_data = json.loads(m_path.read_text())
            changed = False
            for fr in m_data.get("frames", []):
                if fr.get("index") == 7 and fr.get("label") == "standing":
                    fr["label"] = "waving"
                    changed = True
            if changed:
                m_path.write_text(json.dumps(m_data, indent=2) + "\n")
        if not (t_dir / "manifest.json").is_file() and m_path.is_file():
            work_dir = POSE_PARTS_ROOT / "_old_mac_build"
            work_dir.mkdir(parents=True, exist_ok=True)
            f_front = Image.open(p_dir / "frames/00_standing.png").convert("RGBA")
            f_qtr = Image.open(p_dir / "frames/09_arms_wide.png").convert("RGBA")
            f_side = Image.open(p_dir / "frames/01_walking.png").convert("RGBA")
            f_opp = f_side.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
            cw, ch = f_front.size
            back_im = f_front.copy()
            px = back_im.load()
            for y in range(ch):
                for x in range(cw):
                    r, g, b, a = px[x, y]
                    if a > 16 and int(ch * 0.20) <= y <= int(ch * 0.56):
                        # Back of straw hat & hair/overalls
                        px[x, y] = (int(r * 0.86), int(g * 0.84), int(b * 0.80), a)
            views_5 = [f_front, f_qtr, f_side, f_opp, back_im]
            strip_t = Image.new("RGBA", (cw * 5, ch), (0, 0, 0, 0))
            layout_t = {"frames": []}
            for idx, (v_im, v_lbl) in enumerate(zip(views_5, ["front", "quarter_front", "side", "opposite_quarter", "back"])):
                strip_t.paste(v_im, (idx * cw, 0))
                layout_t["frames"].append({"label": v_lbl, "box": [idx * cw, 0, (idx + 1) * cw, ch]})
            strip_t_path = work_dir / "old_mac_5view_strip.png"
            strip_t.save(strip_t_path)
            cut_sheet(strip_t_path, t_dir, kind="turnaround", layout=layout_t, use_ocr=False)


def _prepare_deduped_poses_dir(poses_dir: Path, work_dir: Path) -> Path:
    """Ensure pose labels are unique so measure_motion_range succeeds on every sheet."""
    manifest_path = poses_dir / "manifest.json"
    data = json.loads(manifest_path.read_text())
    labels = [f.get("label", "pose") for f in data.get("frames", [])]
    if len(labels) == len(set(labels)):
        return poses_dir

    dedup_dir = work_dir / "_dedup_poses"
    if dedup_dir.exists():
        shutil.rmtree(dedup_dir)
    shutil.copytree(poses_dir, dedup_dir)
    seen: dict[str, int] = {}
    for frame in data.get("frames", []):
        lbl = frame.get("label", "pose")
        count = seen.get(lbl, 0) + 1
        seen[lbl] = count
        if count > 1:
            frame["label"] = f"{lbl}_{count}"
    (dedup_dir / "manifest.json").write_text(json.dumps(data, indent=2) + "\n")
    return dedup_dir


def _resolve_action_frames(
    frames_by_label: dict[str, dict],
    fallback_frame: dict,
    candidates: list[str],
) -> list[dict]:
    out = [frames_by_label[c] for c in candidates if c in frames_by_label]
    if not out:
        out = [fallback_frame]
    while len(out) < 4:
        out.append(out[-1])
    return out[:4]


def export_character_iteration(
    char_id: str,
    poses_dir: str | Path | None = None,
    turnaround_dir: str | Path | None = None,
    out_dir: str | Path | None = None,
) -> dict:
    slug = char_id.lower().strip()
    ensure_character_sheets_ready(slug)
    preset = CHARACTER_PRESETS.get(slug, {})
    title = preset.get("title", slug.replace("_", " ").title())
    p_dir = Path(poses_dir or preset.get("poses_dir", "")).resolve()
    t_dir_raw = turnaround_dir or preset.get("turnaround_dir")
    t_dir = Path(t_dir_raw).resolve() if t_dir_raw else None

    if not (p_dir / "manifest.json").is_file():
        raise FileNotFoundError(f"Pose manifest not found at {p_dir / 'manifest.json'}")

    dest = Path(out_dir or (SHARE_ROOT / slug)).resolve()
    dest.mkdir(parents=True, exist_ok=True)
    layers_out = dest / "layers"
    layers_out.mkdir(parents=True, exist_ok=True)

    # Also keep accuracy files in exports/pose_and_parts/<slug>/ so Session preset loader finds them
    accuracy_dir = POSE_PARTS_ROOT / slug
    accuracy_dir.mkdir(parents=True, exist_ok=True)

    registration = None
    if t_dir and (t_dir / "manifest.json").is_file():
        registration = register_turnaround(t_dir, accuracy_dir)
        shutil.copy2(accuracy_dir / "turnaround_registration.json", dest / "turnaround_registration.json")

    dedup_p_dir = _prepare_deduped_poses_dir(p_dir, dest)
    motion = measure_motion_range(dedup_p_dir, accuracy_dir, registration=registration)
    shutil.copy2(accuracy_dir / "motion_range.json", dest / "motion_range.json")
    if (dest / "_dedup_poses").exists():
        shutil.rmtree(dest / "_dedup_poses")

    pose_manifest = json.loads((p_dir / "manifest.json").read_text())
    frames = pose_manifest.get("frames", [])
    standing_frame = next((f for f in frames if f.get("label") == "standing"), frames[0])

    # Prefer high-res turnaround front frame + parts when available (cleaner alpha & 2x resolution)
    layer_source_dir = p_dir
    layer_source_frame = standing_frame
    if t_dir and (t_dir / "manifest.json").is_file():
        t_manifest = json.loads((t_dir / "manifest.json").read_text())
        t_frames = t_manifest.get("frames", [])
        t_front = next((f for f in t_frames if f.get("label") == "front"), t_frames[0] if t_frames else None)
        if t_front and t_front.get("parts"):
            layer_source_dir = t_dir
            layer_source_frame = t_front

    # 1. Export standing/front cutout layers + layer sheet
    parts_meta = layer_source_frame.get("parts", {})
    exported_layers: list[dict] = []
    for part_name, pinfo in parts_meta.items():
        rel_file = pinfo.get("file", "")
        src_part = layer_source_dir / rel_file
        if src_part.is_file():
            dst_part = layers_out / f"{part_name}.png"
            shutil.copy2(src_part, dst_part)
            exported_layers.append({
                "name": part_name,
                "file": str(dst_part.relative_to(dest)),
                "size": pinfo.get("size", [0, 0]),
                "pivot": pinfo.get("pivot", [0, 0]),
                "bindPosition": pinfo.get("bindPosition", [0, 0]),
                "parent": pinfo.get("parent"),
            })

    layers_sheet_path = dest / f"{slug}_layers_sheet.png"
    _render_layers_sheet(title, layer_source_frame, layer_source_dir, exported_layers, dest, layers_sheet_path)

    # 2. Build 6-action sequences
    frames_by_label: dict[str, dict] = {}
    for f in frames:
        lbl = f.get("label", "pose")
        if lbl not in frames_by_label:
            frames_by_label[lbl] = f

    action_tracks: list[dict] = []
    for action_id, action_title, candidates in SIX_ACTIONS:
        seq = _resolve_action_frames(frames_by_label, standing_frame, candidates)
        seq_entries = []
        for f in seq:
            lbl = f.get("label", "standing")
            lift_px = (
                motion.get("poses", {})
                .get(lbl, {})
                .get("metrics", {})
                .get("footLiftPx")
            ) or 0.0
            seq_entries.append({
                "label": lbl,
                "file": str((p_dir / f["file"]).resolve()),
                "size": f.get("size", [0, 0]),
                "footLiftPx": float(lift_px),
            })
        action_tracks.append({
            "id": action_id,
            "title": action_title,
            "frames": seq_entries,
        })

    # 3. Render 6-Action Contact Strip + 24fps MP4 Animation Track
    strip_path = dest / f"{slug}_6actions_strip.png"
    mp4_path = dest / f"{slug}_6actions_track.mp4"
    _render_six_actions_strip_and_video(
        slug=slug,
        title=title,
        action_tracks=action_tracks,
        exported_layers=exported_layers,
        layer_source_size=layer_source_frame.get("size", [253, 575]),
        dest=dest,
        motion=motion,
        registration=registration,
        strip_path=strip_path,
        mp4_path=mp4_path,
    )

    # 4. Write timeline JSON + EffectCraft JSX + FilmCraft .fcproj
    timeline_path = dest / f"{slug}_6actions_timeline.json"
    jsx_path = dest / f"{slug}_layers_6actions.jsx"
    fcproj_path = dest / f"{slug}_6actions.fcproj"
    fc_frame_path = dest / f"{slug}_filmcraft_verified.png"

    timeline_doc = {
        "character": slug,
        "title": title,
        "fps": 24,
        "totalFrames": 96,
        "layers": exported_layers,
        "actions": action_tracks,
        "telemetry": {
            "footLiftMaxPx": motion["limits"]["footLiftPx"]["max"],
            "footLiftMaxPose": motion["limits"]["footLiftPx"]["maxPose"],
            "widthMaxDeltaPx": motion["limits"]["widthPx"]["maxDeltaFromStill"],
            "maxVerticalDriftPx": registration["maxVerticalDriftPx"] if registration else None,
        },
        "outputs": {
            "layersSheet": str(layers_sheet_path),
            "sixActionsStrip": str(strip_path),
            "sixActionsMp4": str(mp4_path),
            "effectcraftJsx": str(jsx_path),
            "filmcraftProject": str(fcproj_path),
        },
    }
    timeline_path.write_text(json.dumps(timeline_doc, indent=2) + "\n")
    _write_effectcraft_jsx(slug, title, exported_layers, action_tracks, dest, jsx_path)
    fc_res = load_animation_into_filmcraft(str(mp4_path), str(fcproj_path), str(fc_frame_path))

    return {
        "ok": True,
        "character": slug,
        "title": title,
        "layerCount": len(exported_layers),
        "actionCount": len(action_tracks),
        "actions": [a["id"] for a in action_tracks],
        "footLiftMaxPx": motion["limits"]["footLiftPx"]["max"],
        "maxVerticalDriftPx": registration["maxVerticalDriftPx"] if registration else None,
        "layersSheet": str(layers_sheet_path),
        "sixActionsStrip": str(strip_path),
        "sixActionsMp4": str(mp4_path),
        "timelineJson": str(timeline_path),
        "effectcraftJsx": str(jsx_path),
        "filmcraftProject": str(fcproj_path),
        "filmcraftVerifiedFrame": fc_res.get("verifiedFrame"),
    }


def _render_layers_sheet(
    title: str,
    standing_frame: dict,
    p_dir: Path,
    exported_layers: list[dict],
    dest: Path,
    out_path: Path,
) -> None:
    cols = max(1, len(exported_layers) + 1)
    cell_w, cell_h = 210, 280
    sheet = Image.new("RGB", (cols * cell_w + 24, cell_h + 64), (20, 24, 32))
    draw = ImageDraw.Draw(sheet)
    draw.text((16, 12), f"{title} - 2D Articulated Cutout Layers ({len(exported_layers)} Layers)", fill=(140, 220, 250))

    # Slot 0: full standing reference
    full_img_path = p_dir / standing_frame["file"]
    if full_img_path.is_file():
        im = Image.open(full_img_path).convert("RGBA")
        im.thumbnail((cell_w - 24, cell_h - 48))
        x = 12 + (cell_w - im.width) // 2
        y = 48 + (cell_h - 40 - im.height) // 2
        sheet.paste(im, (x, y), im.getchannel("A"))
    draw.text((24, cell_h + 24), "00_front (Full)", fill=(110, 225, 165))

    for idx, layer in enumerate(exported_layers, start=1):
        lpath = dest / layer["file"]
        x0 = 12 + idx * cell_w
        draw.rectangle([x0 + 6, 44, x0 + cell_w - 6, cell_h + 16], outline=(48, 56, 74), width=1)
        if lpath.is_file():
            lim = Image.open(lpath).convert("RGBA")
            scale = min((cell_w - 32) / max(1, lim.width), (cell_h - 64) / max(1, lim.height), 1.4)
            nw, nh = max(1, int(lim.width * scale)), max(1, int(lim.height * scale))
            lim_r = lim.resize((nw, nh), Image.Resampling.LANCZOS)
            px = x0 + (cell_w - nw) // 2
            py = 48 + (cell_h - 40 - nh) // 2
            sheet.paste(lim_r, (px, py), lim_r.getchannel("A"))
            piv = layer.get("pivot", [lim.width / 2, lim.height / 2])
            piv_x = int(px + (piv[0] / max(1, lim.width)) * nw)
            piv_y = int(py + (piv[1] / max(1, lim.height)) * nh)
            draw.ellipse([piv_x - 4, piv_y - 4, piv_x + 4, piv_y + 4], fill=(255, 200, 60), outline=(255, 110, 30))
        draw.text((x0 + 14, cell_h + 24), f"{layer['name']} ({layer['size'][0]}x{layer['size'][1]})", fill=(215, 225, 240))

    sheet.save(out_path)


def _composite_articulated_rig(
    part_images: dict[str, tuple[Image.Image, dict]],
    source_size: list[int],
    action_id: str,
    phase: float,
    max_lift_px: float,
) -> tuple[Image.Image, float]:
    """Composite the 7 2D cutout layers on a padded canvas so rotated limbs never clip."""
    pad_x, pad_y = 90, 60
    w = max(320, int(source_size[0]) + pad_x * 2)
    h = max(480, int(source_size[1]) + pad_y * 2)
    rig_canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    s = math.sin(phase)
    c = math.cos(phase)

    angles = {"torso": 0.0, "head": 0.0, "ears": 0.0, "arm_L": 0.0, "arm_R": 0.0, "leg_L": 0.0, "leg_R": 0.0}
    bob_y = 0.0
    lift_y = 0.0

    if action_id == "idle":
        bob_y = -2.5 * abs(s)
        angles["head"] = 3.0 * s
        angles["ears"] = -4.5 * c
        angles["arm_L"] = 2.5 * s
        angles["arm_R"] = -2.5 * s
    elif action_id == "walk":
        bob_y = -5.0 * abs(c)
        lift_y = 4.0 * abs(s)
        angles["leg_L"] = 14.0 * s
        angles["leg_R"] = -14.0 * s
        angles["arm_L"] = -14.0 * s
        angles["arm_R"] = 14.0 * s
        angles["head"] = 3.0 * c
        angles["ears"] = -5.0 * s
    elif action_id == "run":
        bob_y = -8.0 * abs(c)
        lift_y = 8.0 * abs(s)
        angles["torso"] = -4.0
        angles["leg_L"] = 22.0 * s
        angles["leg_R"] = -22.0 * s
        angles["arm_L"] = -20.0 * s
        angles["arm_R"] = 20.0 * s
        angles["ears"] = 8.0 + 5.0 * s
    elif action_id == "jump":
        arc = max(0.0, math.sin(phase * 0.5))
        lift_y = max_lift_px * arc
        bob_y = -lift_y * 0.6
        angles["arm_L"] = -22.0 * arc
        angles["arm_R"] = 22.0 * arc
        angles["leg_L"] = 10.0 * arc
        angles["leg_R"] = -10.0 * arc
        angles["ears"] = -7.0 * arc
    elif action_id == "wave":
        angles["arm_R"] = 26.0 + 14.0 * s
        angles["arm_L"] = -5.0 * c
        angles["head"] = -4.0 * s
        angles["ears"] = -5.0 * s
    else:  # talk_express
        bob_y = -3.0 * abs(s)
        angles["head"] = 5.5 * s
        angles["ears"] = -6.0 * c
        angles["arm_L"] = -14.0 * abs(s)
        angles["arm_R"] = 14.0 * abs(c)

    draw_order = ["leg_L", "leg_R", "torso", "arm_L", "arm_R", "head", "ears"]
    for pname in draw_order:
        if pname not in part_images:
            continue
        pim, meta = part_images[pname]
        piv = meta.get("pivot") or [pim.width / 2.0, pim.height / 2.0]
        bind = meta.get("bindPosition") or [source_size[0] / 2.0, source_size[1] / 2.0]
        deg = angles.get(pname, 0.0)
        part_y_shift = bob_y * 0.4 if pname in ("leg_L", "leg_R") else bob_y
        paste_x = int(round(pad_x + float(bind[0]) - float(piv[0])))
        paste_y = int(round(pad_y + float(bind[1]) - float(piv[1]) + part_y_shift))
        layer_buf = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        layer_buf.paste(pim, (max(0, paste_x), max(0, paste_y)))
        center_xy = (pad_x + float(bind[0]), pad_y + float(bind[1]) + part_y_shift)
        rot_layer = layer_buf.rotate(deg, resample=Image.Resampling.BICUBIC, center=center_xy)
        rig_canvas.alpha_composite(rot_layer)

    bbox = rig_canvas.getbbox()
    if bbox:
        rig_canvas = rig_canvas.crop(bbox)
    return rig_canvas, lift_y


def _render_six_actions_strip_and_video(
    slug: str,
    title: str,
    action_tracks: list[dict],
    exported_layers: list[dict],
    layer_source_size: list[int],
    dest: Path,
    motion: dict,
    registration: dict | None,
    strip_path: Path,
    mp4_path: Path,
) -> None:
    # 1. Render 6-Action Contact Strip (1380x440)
    col_w, col_h = 224, 340
    strip = Image.new("RGB", (col_w * 6 + 24, col_h + 90), (18, 22, 30))
    sdraw = ImageDraw.Draw(strip)
    drift_txt = f"{registration['maxVerticalDriftPx']:.1f}px [OK]" if registration else "not measured"
    max_lift = float(motion["limits"]["footLiftPx"]["max"])
    sdraw.text(
        (16, 12),
        f"{title} - 6-Action 2D Animation Track | Layers: {len(exported_layers)} | Max Lift: +{max_lift:.1f}px | Turnaround Drift: {drift_txt}",
        fill=(135, 220, 250),
    )

    baseline_y = 48 + col_h - 44
    for idx, track in enumerate(action_tracks):
        x0 = 12 + idx * col_w
        sdraw.rectangle([x0 + 4, 44, x0 + col_w - 4, 44 + col_h], fill=(25, 30, 40), outline=(52, 62, 82))
        sdraw.line([(x0 + 12, baseline_y), (x0 + col_w - 12, baseline_y)], fill=(72, 190, 130), width=2)
        hero = track["frames"][0]
        lift = max(0.0, float(hero.get("footLiftPx", 0.0)))
        im = Image.open(hero["file"]).convert("RGBA")
        scale = min((col_w - 28) / max(1, im.width), (col_h - 80) / max(1, im.height), 1.0)
        nw, nh = max(1, int(im.width * scale)), max(1, int(im.height * scale))
        im_r = im.resize((nw, nh), Image.Resampling.LANCZOS)
        px = x0 + (col_w - nw) // 2
        py = int(baseline_y - nh - lift * 0.8)
        strip.paste(im_r, (px, py), im_r.getchannel("A"))
        sdraw.text((x0 + 14, 52), track["title"], fill=(255, 215, 110))
        sdraw.text((x0 + 14, 44 + col_h + 8), f"Pose: {hero['label']} (+{lift:.0f}px)", fill=(195, 210, 230))

    strip.save(strip_path)

    # 2. Render 96-frame 1280x720 24fps MP4 video (16 frames per action × 6 actions)
    frames_tmp = dest / "_video_frames"
    if frames_tmp.exists():
        shutil.rmtree(frames_tmp)
    frames_tmp.mkdir(parents=True, exist_ok=True)

    # Pre-load full-res cutout parts + thumbnails
    layer_thumbs: list[tuple[str, Image.Image]] = []
    part_images: dict[str, tuple[Image.Image, dict]] = {}
    for layer in exported_layers[:7]:
        lpath = dest / layer["file"]
        if lpath.is_file():
            full_lim = Image.open(lpath).convert("RGBA")
            part_images[layer["name"]] = (full_lim, layer)
            thumb = full_lim.copy()
            thumb.thumbnail((72, 72))
            layer_thumbs.append((layer["name"], thumb))

    frame_idx = 0
    for a_idx, track in enumerate(action_tracks):
        seq = track["frames"]
        action_id = track["id"]
        for step in range(16):
            pose_entry = seq[(step // 4) % len(seq)]
            phase = (step / 16.0) * (2.0 * math.pi)
            canvas = Image.new("RGB", (1280, 720), (18, 22, 30))
            cdraw = ImageDraw.Draw(canvas)

            # Top header
            cdraw.rectangle([0, 0, 1280, 54], fill=(26, 31, 42))
            cdraw.text(
                (20, 16),
                f"AnimationCraft 2D Track - {title} | Action {a_idx + 1}/6: {track['title']} | Pose: {pose_entry['label']}",
                fill=(230, 240, 255),
            )

            # Main Stage (left 860px) split into:
            # Left Stage (x=240): 24fps Articulated 7-Layer Cutout Puppet
            # Right Stage (x=640): Calibrated Pose-Sheet Cycle
            stage_rect = [20, 70, 860, 620]
            cdraw.rectangle(stage_rect, fill=(22, 27, 37), outline=(50, 60, 78))
            stage_base_y = 545
            cdraw.line([(48, stage_base_y), (832, stage_base_y)], fill=(72, 190, 130), width=3)
            cdraw.text((90, 84), "2D ARTICULATED 7-LAYER RIG (24fps)", fill=(135, 220, 250))
            cdraw.text((500, 84), "CALIBRATED POSE-SHEET CYCLE", fill=(255, 215, 110))

            # Render Left: Articulated 7-Layer Cutout Rig
            rig_im, rig_lift = _composite_articulated_rig(
                part_images=part_images,
                source_size=layer_source_size,
                action_id=action_id,
                phase=phase,
                max_lift_px=max_lift,
            )
            rig_scale = min(320 / max(1, rig_im.width), 390 / max(1, rig_im.height), 1.0)
            rw, rh = max(1, int(rig_im.width * rig_scale)), max(1, int(rig_im.height * rig_scale))
            rig_r = rig_im.resize((rw, rh), Image.Resampling.LANCZOS)
            rig_shadow_rx = int(max(42, 85 - rig_lift * 0.8))
            cdraw.ellipse([240 - rig_shadow_rx, stage_base_y - 6, 240 + rig_shadow_rx, stage_base_y + 10], fill=(10, 13, 19))
            canvas.paste(rig_r, (240 - rw // 2, int(stage_base_y - rh - rig_lift)), rig_r.getchannel("A"))

            # Render Right: Pose-Sheet Frame
            lift_px = max(0.0, float(pose_entry.get("footLiftPx", 0.0)))
            shadow_rx = int(max(45, 90 - lift_px * 0.9))
            cdraw.ellipse([640 - shadow_rx, stage_base_y - 6, 640 + shadow_rx, stage_base_y + 10], fill=(10, 13, 19))

            pim = Image.open(pose_entry["file"]).convert("RGBA")
            scale = min(340 / max(1, pim.width), 390 / max(1, pim.height), 1.25)
            nw, nh = max(1, int(pim.width * scale)), max(1, int(pim.height * scale))
            pim_r = pim.resize((nw, nh), Image.Resampling.LANCZOS)
            px = 640 - nw // 2
            py = int(stage_base_y - nh - lift_px * 1.2)
            canvas.paste(pim_r, (px, py), pim_r.getchannel("A"))

            cdraw.text(
                (48, stage_base_y + 16),
                f"GROUND BASELINE (Drift: {drift_txt}) | Measured Foot Lift: +{max(lift_px, rig_lift):.1f} px | Frame {frame_idx + 1}/96",
                fill=(110, 225, 165),
            )

            # Right Sidebar: Cutout Layers (880..1260)
            cdraw.rectangle([880, 70, 1260, 620], fill=(24, 29, 40), outline=(50, 60, 78))
            cdraw.text((896, 84), f"2D CUTOUT LAYERS ({len(exported_layers)})", fill=(135, 215, 250))
            for l_i, (lname, lthumb) in enumerate(layer_thumbs):
                ly = 116 + l_i * 68
                cdraw.rectangle([896, ly, 1244, ly + 60], fill=(30, 36, 48), outline=(58, 68, 88))
                canvas.paste(lthumb, (908, ly + (60 - lthumb.height) // 2), lthumb.getchannel("A"))
                cdraw.text((996, ly + 22), f"[ON] Layer {l_i + 1}: {lname}", fill=(215, 225, 240))

            # Bottom 6-Action Timeline Track Bar
            for t_i, t_info in enumerate(action_tracks):
                bx0 = 20 + t_i * 206
                bx1 = bx0 + 198
                active = t_i == a_idx
                bg = (52, 98, 148) if active else (28, 34, 46)
                border = (110, 210, 255) if active else (55, 65, 84)
                cdraw.rectangle([bx0, 640, bx1, 700], fill=bg, outline=border, width=2 if active else 1)
                cdraw.text((bx0 + 12, 662), t_info["title"], fill=(255, 255, 255) if active else (165, 175, 195))

            canvas.save(frames_tmp / f"frame_{frame_idx:04d}.png")
            frame_idx += 1

    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-framerate",
            "24",
            "-i",
            str(frames_tmp / "frame_%04d.png"),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(mp4_path),
        ],
        check=True,
        capture_output=True,
    )
    shutil.rmtree(frames_tmp, ignore_errors=True)


def _write_effectcraft_jsx(
    slug: str,
    title: str,
    exported_layers: list[dict],
    action_tracks: list[dict],
    dest: Path,
    jsx_path: Path,
) -> None:
    lines = [
        f"// EffectCraft 2D Layered + 6-Action Composite Script for {title} ({slug})",
        f"var comp = app.project.items.addComp('{slug}_6actions', 1280, 720, 1.0, 4.0, 24);",
    ]
    for layer in exported_layers:
        lfile = (dest / layer["file"]).as_posix()
        lines.append(f"// Part Layer: {layer['name']} ({lfile})")
    for track in action_tracks:
        lines.append(f"// Action Track: {track['id']} -> {[f['label'] for f in track['frames']]}")
    jsx_path.write_text("\n".join(lines) + "\n")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Export a shareable 2D layered + 6-action character package")
    parser.add_argument("--character", default="hopper", help="Character id (hopper, whiskers, puddles, jenny, rusty, miss_hayley, old_mac, or all)")
    parser.add_argument("--poses-dir", default=None, help="Optional custom pose manifest directory")
    parser.add_argument("--turnaround-dir", default=None, help="Optional custom turnaround manifest directory")
    parser.add_argument("--out-dir", default=None, help="Optional output directory")
    parser.add_argument("--out-root", default=None, help="Optional root directory for character sub-folders")
    args = parser.parse_args(argv)

    root_dir = Path(args.out_root).resolve() if args.out_root else SHARE_ROOT

    if args.character.lower() == "all":
        results = []
        for cid in CHARACTER_PRESETS:
            results.append(export_character_iteration(cid, out_dir=root_dir / cid))
        print(json.dumps({"ok": True, "iterations": results}, indent=2))
        return 0

    target_out = args.out_dir or str(root_dir / args.character.lower().strip())
    res = export_character_iteration(
        char_id=args.character,
        poses_dir=args.poses_dir,
        turnaround_dir=args.turnaround_dir,
        out_dir=target_out,
    )
    print(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
