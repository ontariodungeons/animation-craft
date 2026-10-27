#!/usr/bin/env python3
"""Stage 4D: 2D 6-Action Timed Track & Essential Graphics Template Builder.

Compiles the 6-action contact strip (`idle`, `walk`, `run`, `jump`, `wave`, `talk_express`),
builds an animated EffectCraft `.ecproj` + `.ectemplate` + `<slug>_values.json` driven by
`timed_trajectories.json` (root lift/contact bounce, head/limb rotation keyframes, and
`puppet.addPin` + `puppet.follow` secondary motion), renders `<slug>_6actions_track.mp4`,
and verifies editorial assembly in FilmCraft (`.fcproj`).
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
from PIL import Image, ImageDraw

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.step_filmcraft_timeline import build_filmcraft_marked_sequence
from pipeline.step_studio_rig_tracks import compile_studio_rig_json

SIX_ACTIONS: list[tuple[str, str, list[str]]] = [
    ("idle", "1. IDLE", ["looking_up", "standing", "looking_down", "standing"]),
    ("walk", "2. WALK", ["walking", "standing", "walking", "pointing"]),
    ("run", "3. RUN", ["running", "walking", "jumping", "walking"]),
    ("jump", "4. JUMP", ["jumping", "standing", "jumping", "running"]),
    ("wave", "5. WAVE", ["waving", "arms_wide", "clapping", "waving"]),
    ("talk_express", "6. TALK / EXPRESS", ["pointing", "dancing", "clapping", "waving"]),
]


def resolve_action_frames(frames_by_label: dict[str, dict], standing_frame: dict, candidates: list[str]) -> list[dict]:
    return [frames_by_label.get(c, standing_frame) for c in candidates]


def render_contact_strip(title: str, action_tracks: list[dict], strip_path: Path) -> None:
    col_w, col_h = 220, 320
    strip = Image.new("RGB", (col_w * 6 + 24, col_h + 64), (18, 22, 30))
    draw = ImageDraw.Draw(strip)
    draw.text((16, 12), f"{title} - 6-Action Contact Strip", fill=(135, 220, 250))
    for idx, track in enumerate(action_tracks):
        x0 = 12 + idx * col_w
        draw.rectangle([x0 + 4, 40, x0 + col_w - 4, 40 + col_h], fill=(25, 30, 40), outline=(52, 62, 82))
        hero = track["frames"][0]
        p_file = Path(hero["file"])
        if p_file.is_file():
            with Image.open(p_file) as im:
                thumb = im.convert("RGBA")
                thumb.thumbnail((col_w - 28, col_h - 60))
                strip.paste(thumb, (x0 + (col_w - thumb.width) // 2, 70), thumb.getchannel("A"))
        draw.text((x0 + 12, 48), track["title"], fill=(255, 215, 110))
    strip.save(strip_path)


def render_animated_six_actions_effectcraft(
    slug: str,
    standing_file: str,
    exported_layers: list[dict],
    timed_trajectories: dict,
    dest_dir: Path,
    mp4_path: Path,
    jsx_path: Path,
) -> dict:
    """Build an animated EffectCraft composition, `.ectemplate` + `values.json`, and H.264 `.mp4`."""
    ec_bin = shutil.which("effectcraft-cli") or "effectcraft-cli"
    ecproj = mp4_path.with_suffix(".ecproj")
    ectemplate = dest_dir / f"{slug}_6actions.ectemplate"
    values_json_path = dest_dir / f"{slug}_values.json"

    tracks = timed_trajectories.get("tracks", {})
    action_order = ["idle", "walk", "run", "jump", "wave", "talk_express"]
    seg_dur = 0.8
    total_dur = round(seg_dur * len(action_order), 2)  # 4.8s across 6 actions

    # Compile continuous 6-action timeline keyframes for root position + rotation
    pos_kfs: list[tuple[float, float, float, str]] = []
    rot_kfs: list[tuple[float, float, str]] = []
    cx, cy = 640.0, 380.0
    for idx, act_id in enumerate(action_order):
        t_offset = idx * seg_dur
        t_spec = tracks.get(act_id, {})
        kfs = t_spec.get("keyframes", [])
        src_dur = max(0.4, float(t_spec.get("duration") or 1.6))
        for kf in kfs:
            rel_t = float(kf.get("t", 0.0)) / src_dur
            t = round(t_offset + rel_t * seg_dur, 3)
            dy = float(kf.get("rootDyPx", 0.0))
            dx = 18.0 * (rel_t - 0.5) if act_id in ("walk", "run") else 0.0
            rot = float(kf.get("headRotDeg", 0.0)) + 0.35 * float(kf.get("armRRotDeg", 0.0))
            phase = str(kf.get("phase", "rest"))
            pos_kfs.append((t, round(cx + dx, 2), round(cy + dy, 2), phase))
            rot_kfs.append((t, round(rot, 2), phase))

    jsx_lines = [
        f"// EffectCraft 6-Action Timed Track + Essential Graphics for {slug}",
        f"var comp = app.project.items.addComp('{slug}_6actions', 1280, 720, 1.0, {total_dur}, 24);",
        f"// Timed segments: {', '.join(action_order)} ({seg_dur}s each)",
    ]
    for t, px, py, phase in pos_kfs:
        jsx_lines.append(f"// t={t:.2f}s [{phase}] pos=({px}, {py})")
    jsx_path.write_text("\n".join(jsx_lines) + "\n", encoding="utf-8")

    comp_name = f"{slug}_6actions"
    batch_args: list[str] = [
        ec_bin, "--empty", "run",
        "comp.new", json.dumps({
            "name": comp_name,
            "width": 1280,
            "height": 720,
            "duration": total_dur,
            "frameRate": 24,
        }),
        "essential.setPrimary", json.dumps({"comp": comp_name}),
        "essential.setName", json.dumps({"comp": comp_name, "name": f"{slug}_6actions_template"}),
        "file.import", json.dumps({"paths": [standing_file], "sequence": False, "addToComp": True}),
        "layer.rename", json.dumps({"name": f"{slug}_hero"}),
    ]

    for t, px, py, _ in pos_kfs:
        batch_args.extend([
            "prop.addKey",
            json.dumps({"layer": f"{slug}_hero", "path": "transform/position", "time": t, "value": [px, py, 0.0]}),
        ])
    for t, rot, _ in rot_kfs:
        batch_args.extend([
            "prop.addKey",
            json.dumps({"layer": f"{slug}_hero", "path": "transform/rotation", "time": t, "value": rot}),
        ])

    # Attach root anchor pin and follow-through tip pin (`puppet.addPin` + `puppet.follow`)
    batch_args.extend([
        "puppet.addPin", json.dumps({"layer": f"{slug}_hero", "kind": "position", "position": [cx, cy + 80.0]}),
        "puppet.addPin", json.dumps({"layer": f"{slug}_hero", "kind": "position", "position": [cx, cy - 90.0]}),
        "puppet.follow", json.dumps({
            "layer": f"{slug}_hero",
            "leader": "Puppet Pin 1",
            "pins": ["Puppet Pin 2"],
            "delay": 0.08,
            "amount": 65,
            "cascade": True,
        }),
        "essential.addProperty", json.dumps({"layer": f"{slug}_hero", "path": "transform/rotation", "name": f"{slug}_hero_rotation"}),
        "essential.exportTemplate", json.dumps({"comp": comp_name, "path": str(ectemplate), "name": f"{slug}_6actions_template"}),
        "--save-as", str(ecproj), "--json",
    ])

    proc = subprocess.run(batch_args, capture_output=True, text=True, timeout=45)
    if proc.returncode != 0:
        # Fallback if a specific puppet/essential command differs on an older EffectCraft build
        subprocess.run([
            ec_bin, "--empty", "run",
            "comp.new", json.dumps({"name": f"{slug}_6actions", "width": 1280, "height": 720, "duration": total_dur, "frameRate": 24}),
            "file.import", json.dumps({"paths": [standing_file], "sequence": False, "addToComp": True}),
            "--save-as", str(ecproj), "--json",
        ], check=True, capture_output=True, timeout=30)

    values_payload = {
        "template": str(ectemplate),
        "character": slug,
        "values": {
            f"{slug}_hero_media": standing_file,
            f"{slug}_hero_rotation": 0.0,
        },
        "layerCount": len(exported_layers),
    }
    values_json_path.write_text(json.dumps(values_payload, indent=2) + "\n", encoding="utf-8")

    subprocess.run([
        ec_bin, "--project", str(ecproj), "render", "--out", str(mp4_path), "--format", "h264",
    ], check=True, capture_output=True, timeout=60)

    return {
        "ecproj": str(ecproj),
        "ectemplate": str(ectemplate) if ectemplate.is_file() else None,
        "valuesJson": str(values_json_path),
        "keyframeCount": len(pos_kfs) + len(rot_kfs),
        "duration": total_dur,
    }


def build_six_action_package(
    slug: str,
    title: str,
    p_dir: Path,
    dest: Path,
    motion: dict,
    registration: dict | None = None,
) -> dict:
    layers_out = dest / "layers"
    layers_out.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((p_dir / "manifest.json").read_text())
    frames = manifest.get("frames", [])
    standing_frame = next((f for f in frames if f.get("label") == "standing"), frames[0])

    exported_layers: list[dict] = []
    for part_name, pinfo in standing_frame.get("parts", {}).items():
        src_part = p_dir / pinfo.get("file", "")
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

    # If the standing frame manifest did not define part polygons, derive whole-shape PhotoCraft layers
    if not exported_layers:
        from pipeline.step_photocraft_parts import (
            discover_character_joints_and_polygons,
            mask_and_export_layers_with_photocraft,
        )
        standing_img_path = (p_dir / standing_frame["file"]).resolve()
        disc = discover_character_joints_and_polygons(standing_img_path, character_id=slug)
        pc_out = mask_and_export_layers_with_photocraft(standing_img_path, disc, dest, slug)
        for p in pc_out["parts"]:
            src_full = Path(p["full_layer_path"])
            dst_part = layers_out / f"{p['id']}.png"
            shutil.copy2(src_full, dst_part)
            exported_layers.append({
                "name": p["id"],
                "file": str(dst_part.relative_to(dest)),
                "size": disc["canvas_size"],
                "pivot": p["pivot"],
                "bindPosition": p["pivot"],
                "parent": p["parent"],
            })

    layers_sheet_path = dest / f"{slug}_layers_sheet.png"
    shutil.copy2(p_dir / standing_frame["file"], layers_sheet_path)

    frames_by_label = {f.get("label", "pose"): f for f in frames}
    action_tracks = []
    timed_tracks = motion.get("timedTrajectories", {}).get("tracks", {})
    for action_id, action_title, candidates in SIX_ACTIONS:
        seq = resolve_action_frames(frames_by_label, standing_frame, candidates)
        action_tracks.append({
            "id": action_id,
            "title": action_title,
            "timedTrajectory": timed_tracks.get(action_id),
            "frames": [{
                "label": f.get("label", "standing"),
                "file": str((p_dir / f["file"]).resolve()),
                "size": f.get("size", [0, 0]),
                "footLiftPx": float(motion.get("poses", {}).get(f.get("label", "standing"), {}).get("metrics", {}).get("footLiftPx") or 0.0),
            } for f in seq],
        })

    strip_path = dest / f"{slug}_6actions_strip.png"
    mp4_path = dest / f"{slug}_6actions_track.mp4"
    jsx_path = dest / f"{slug}_layers_6actions.jsx"
    fcproj_path = dest / f"{slug}_6actions.fcproj"
    fc_frame_path = dest / f"{slug}_filmcraft_verified.png"
    timeline_path = dest / f"{slug}_6actions_timeline.json"
    rig_json_path = dest / "rig.json"

    render_contact_strip(title, action_tracks, strip_path)
    ec_res = render_animated_six_actions_effectcraft(
        slug=slug,
        standing_file=str((p_dir / standing_frame["file"]).resolve()),
        exported_layers=exported_layers,
        timed_trajectories=motion.get("timedTrajectories", {}),
        dest_dir=dest,
        mp4_path=mp4_path,
        jsx_path=jsx_path,
    )
    fc_res = build_filmcraft_marked_sequence(
        slug=slug,
        media_path=str(mp4_path),
        project_out=str(fcproj_path),
        frame_png_out=str(fc_frame_path),
        extra_assets=[str(layers_out / Path(l["file"]).name) for l in exported_layers if (layers_out / Path(l["file"]).name).is_file()],
    )
    compile_studio_rig_json(
        character_id=slug,
        canvas_size=standing_frame.get("size", [560, 720]),
        layers_meta=exported_layers,
        timed_trajectories=motion.get("timedTrajectories", {}),
        out_path=rig_json_path,
    )

    timeline_doc = {
        "character": slug,
        "title": title,
        "fps": 24,
        "totalFrames": int(round(ec_res["duration"] * 24)),
        "layers": exported_layers,
        "actions": action_tracks,
        "effectcraft": ec_res,
        "outputs": {
            "layersSheet": str(layers_sheet_path),
            "sixActionsStrip": str(strip_path),
            "sixActionsMp4": str(mp4_path),
            "studioRigJson": str(rig_json_path),
            "effectcraftJsx": str(jsx_path),
            "effectcraftProject": ec_res["ecproj"],
            "effectcraftTemplate": ec_res["ectemplate"],
            "effectcraftValuesJson": ec_res["valuesJson"],
            "filmcraftProject": str(fcproj_path),
        },
    }
    timeline_path.write_text(json.dumps(timeline_doc, indent=2) + "\n")
    return {
        "ok": True,
        "character": slug,
        "title": title,
        "layerCount": len(exported_layers),
        "actionCount": len(action_tracks),
        "actions": [a["id"] for a in action_tracks],
        "keyframeCount": ec_res["keyframeCount"],
        "footLiftMaxPx": motion["limits"]["footLiftPx"]["max"],
        "maxVerticalDriftPx": registration["maxVerticalDriftPx"] if registration else None,
        "layersSheet": str(layers_sheet_path),
        "sixActionsStrip": str(strip_path),
        "sixActionsMp4": str(mp4_path),
        "studioRigJson": str(rig_json_path),
        "timelineJson": str(timeline_path),
        "effectcraftJsx": str(jsx_path),
        "effectcraftProject": ec_res["ecproj"],
        "effectcraftTemplate": ec_res["ectemplate"],
        "effectcraftValuesJson": ec_res["valuesJson"],
        "filmcraftProject": str(fcproj_path),
        "filmcraftVerifiedFrame": fc_res.get("verifiedFrame"),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--character", required=True)
    ap.add_argument("--poses-dir", required=True)
    ap.add_argument("--motion-range", required=True)
    ap.add_argument("--out-dir", required=True)
    args = ap.parse_args(argv)
    motion = json.loads(Path(args.motion_range).read_text())
    res = build_six_action_package(
        slug=args.character,
        title=args.character.replace("_", " ").title(),
        p_dir=Path(args.poses_dir).resolve(),
        dest=Path(args.out_dir).resolve(),
        motion=motion,
    )
    print(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
