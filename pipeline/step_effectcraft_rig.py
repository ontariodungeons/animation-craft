"""Stage 3C: Hierarchical cutout animation, Essential Graphics .ectemplate & secondary puppet motion via effectcraft-cli."""
from __future__ import annotations

import json
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

SECONDARY_APPENDAGE_PREFIXES = ("ear_", "tail", "hair", "prop_", "cape", "scarf", "wing_", "feather_")


def _is_secondary_appendage(part: Dict[str, Any]) -> bool:
    pid = str(part.get("id", "")).lower()
    return part.get("kind") == "appendage" or pid.startswith(SECONDARY_APPENDAGE_PREFIXES)


def build_pose_library(character_id: str) -> Dict[str, Any]:
    """Pose-library-design timing & joint rotations."""
    return {
        "character_id": character_id,
        "acting_pattern": "anticipation -> action -> hold -> settle",
        "poses": {
            "idle": {"pose": "idle", "category": "Neutral", "hold_frames": 24, "transition": "sine.inOut",
                     "parts": {"torso": {"rotation": 0.0, "y": 0.0}, "head": {"rotation": 1.5},
                               "ear_left": {"rotation": -4.0}, "ear_right": {"rotation": 4.0},
                               "arm_left": {"rotation": 4.0}, "arm_right": {"rotation": -2.0},
                               "leg_left": {"rotation": 2.0}, "foot_left": {"rotation": -8.0},
                               "leg_right": {"rotation": -2.0}, "foot_right": {"rotation": 6.0}}},
            "idle_breathe": {"pose": "idle_breathe", "category": "Neutral", "hold_frames": 12, "transition": "sine.inOut",
                             "parts": {"torso": {"rotation": -1.5, "y": -3.0}, "head": {"rotation": -2.0},
                                       "ear_left": {"rotation": 5.0}, "ear_right": {"rotation": -5.0},
                                       "arm_left": {"rotation": 10.0}, "arm_right": {"rotation": 3.0},
                                       "leg_left": {"rotation": -3.0}, "foot_left": {"rotation": 12.0},
                                       "leg_right": {"rotation": 2.0}, "foot_right": {"rotation": -10.0}}},
            "walk_contact": {"pose": "walk_contact", "category": "Action", "hold_frames": 6, "transition": "power2.inOut",
                             "parts": {"torso": {"rotation": 3.0, "y": -5.0}, "head": {"rotation": -2.5},
                                       "ear_left": {"rotation": -10.0}, "ear_right": {"rotation": -6.0},
                                       "arm_left": {"rotation": 18.0}, "arm_right": {"rotation": -4.0},
                                       "leg_left": {"rotation": -12.0}, "foot_left": {"rotation": -16.0},
                                       "leg_right": {"rotation": 12.0}, "foot_right": {"rotation": 16.0}}},
            "walk_passing": {"pose": "walk_passing", "category": "Action", "hold_frames": 6, "transition": "power2.inOut",
                             "parts": {"torso": {"rotation": -2.5, "y": 4.0}, "head": {"rotation": 2.5},
                                       "ear_left": {"rotation": 8.0}, "ear_right": {"rotation": 10.0},
                                       "arm_left": {"rotation": -14.0}, "arm_right": {"rotation": 4.0},
                                       "leg_left": {"rotation": 12.0}, "foot_left": {"rotation": 16.0},
                                       "leg_right": {"rotation": -12.0}, "foot_right": {"rotation": -16.0}}},
            "run_contact": {"pose": "run_contact", "category": "Action", "hold_frames": 4, "transition": "power2.inOut",
                            "parts": {"torso": {"rotation": 5.0, "y": -10.0}, "head": {"rotation": -4.0},
                                      "ear_left": {"rotation": -16.0}, "ear_right": {"rotation": -14.0},
                                      "arm_left": {"rotation": 28.0}, "arm_right": {"rotation": -6.0},
                                      "leg_left": {"rotation": -18.0}, "foot_left": {"rotation": -20.0},
                                      "leg_right": {"rotation": 18.0}, "foot_right": {"rotation": 20.0}}},
            "run_passing": {"pose": "run_passing", "category": "Action", "hold_frames": 4, "transition": "power2.inOut",
                            "parts": {"torso": {"rotation": -4.0, "y": 6.0}, "head": {"rotation": 4.0},
                                      "ear_left": {"rotation": 14.0}, "ear_right": {"rotation": 16.0},
                                      "arm_left": {"rotation": -22.0}, "arm_right": {"rotation": 6.0},
                                      "leg_left": {"rotation": 18.0}, "foot_left": {"rotation": 20.0},
                                      "leg_right": {"rotation": -18.0}, "foot_right": {"rotation": -20.0}}},
            "jump_anticipation": {"pose": "jump_anticipation", "category": "Action", "hold_frames": 8, "transition": "power2.in",
                                  "parts": {"torso": {"rotation": 5.0, "y": 14.0}, "head": {"rotation": -6.0},
                                            "ear_left": {"rotation": -12.0}, "ear_right": {"rotation": -10.0},
                                            "arm_left": {"rotation": -18.0}, "arm_right": {"rotation": 5.0},
                                            "leg_left": {"rotation": 14.0}, "foot_left": {"rotation": 14.0},
                                            "leg_right": {"rotation": 12.0}, "foot_right": {"rotation": 14.0}}},
            "jump_apex": {"pose": "jump_apex", "category": "Action", "hold_frames": 14, "transition": "back.out",
                          "parts": {"torso": {"rotation": -4.0, "y": -32.0}, "head": {"rotation": 6.0},
                                    "ear_left": {"rotation": 14.0}, "ear_right": {"rotation": 16.0},
                                    "arm_left": {"rotation": 28.0}, "arm_right": {"rotation": -6.0},
                                    "leg_left": {"rotation": -16.0}, "foot_left": {"rotation": -18.0},
                                    "leg_right": {"rotation": -14.0}, "foot_right": {"rotation": -16.0}}},
            "wave_greeting": {"pose": "wave_greeting", "category": "Emotion", "hold_frames": 18, "transition": "back.out",
                              "parts": {"torso": {"rotation": -2.0, "y": 0.0}, "head": {"rotation": -5.0},
                                        "ear_left": {"rotation": -10.0}, "ear_right": {"rotation": 8.0},
                                        "arm_left": {"rotation": 32.0}, "arm_right": {"rotation": 4.0},
                                        "leg_left": {"rotation": -8.0}, "foot_left": {"rotation": -14.0},
                                        "leg_right": {"rotation": 6.0}, "foot_right": {"rotation": 12.0}}},
            "wave_greeting_alt": {"pose": "wave_greeting_alt", "category": "Emotion", "hold_frames": 12, "transition": "sine.inOut",
                                  "parts": {"torso": {"rotation": 2.0, "y": 0.0}, "head": {"rotation": 4.0},
                                            "ear_left": {"rotation": 8.0}, "ear_right": {"rotation": -8.0},
                                            "arm_left": {"rotation": -12.0}, "arm_right": {"rotation": -3.0},
                                            "leg_left": {"rotation": 6.0}, "foot_left": {"rotation": 12.0},
                                            "leg_right": {"rotation": -6.0}, "foot_right": {"rotation": -12.0}}},
        },
        "sequences": {
            "idle": ["idle", "idle_breathe", "idle"],
            "walk": ["walk_contact", "walk_passing", "walk_contact", "walk_passing"],
            "run": ["run_contact", "run_passing", "run_contact", "run_passing"],
            "jump": ["idle", "jump_anticipation", "jump_apex", "idle"],
            "wave": ["idle", "wave_greeting", "wave_greeting_alt", "wave_greeting", "idle"],
            "static": ["idle", "idle"],
        },
    }


def _mode_beat_times(mode: str, speed: float = 1.0) -> List[Tuple[float, str]]:
    m = (mode or "walk").lower().strip()
    inv_speed = 1.0 / max(0.1, float(speed))
    table = {
        "run": [(0.0, "idle"), (0.18, "run_contact"), (0.32, "run_contact"), (0.55, "run_passing"), (0.70, "run_passing"), (0.95, "idle")],
        "jump": [(0.0, "idle"), (0.25, "jump_anticipation"), (0.45, "jump_anticipation"), (0.75, "jump_apex"), (1.10, "jump_apex"), (1.50, "idle")],
        "wave": [(0.0, "idle"), (0.25, "wave_greeting"), (0.60, "wave_greeting_alt"), (0.95, "wave_greeting"), (1.25, "wave_greeting"), (1.50, "idle")],
        "idle": [(0.0, "idle"), (0.60, "idle_breathe"), (0.90, "idle_breathe"), (1.50, "idle")],
        "static": [(0.0, "idle"), (1.50, "idle")],
    }
    raw = table.get(m, [(0.0, "idle"), (0.25, "walk_contact"), (0.50, "walk_contact"), (0.85, "walk_passing"), (1.10, "walk_passing"), (1.50, "idle")])
    return [(round(t * inv_speed, 4), pose) for t, pose in raw]


def export_effectcraft_jsx_script(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_jsx_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
) -> Path:
    cid = rig_plan["character_id"]
    w, h = rig_plan["canvas_size"]
    comp_w, comp_h = int(w) + (int(w) % 2), int(h) + (int(h) % 2)
    comp_dur = max(0.5, float(duration))
    parts_sorted = sorted(rig_plan["parts"], key=lambda p: p["layer"])
    beat_times = _mode_beat_times(mode, speed)
    poses = pose_library.get("poses", {})

    lines = [
        f"// EffectCraft Cutout Rig Script for '{cid}' ({mode})",
        f"var comp = app.project.items.addComp({json.dumps(f'{cid}_{mode}_cutout')}, {comp_w}, {comp_h}, 1.0, {comp_dur:.3f}, 24);",
        "var layerMap = {};",
    ]
    for part in parts_sorted:
        pid = part["id"]
        v_pid = re.sub(r"\W", "_", pid)
        px, py = float(part["pivot"][0]), float(part["pivot"][1])
        lines.extend([
            f"var item_{v_pid} = app.project.importFile(new ImportOptions(new File({json.dumps(part['full_layer_path'])})));",
            f"var lyr_{v_pid} = comp.layers.add(item_{v_pid}); lyr_{v_pid}.name = {json.dumps(pid)};",
            f"lyr_{v_pid}.property('Anchor Point').setValue([{px:.1f}, {py:.1f}, 0.0]);",
            f"lyr_{v_pid}.property('Position').setValue([{px:.1f}, {py:.1f}, 0.0]);",
            f"layerMap[{json.dumps(pid)}] = lyr_{v_pid};",
        ])
    for part in parts_sorted:
        if part.get("parent"):
            lines.append(f"if (layerMap[{json.dumps(part['parent'])}]) layerMap[{json.dumps(part['id'])}].parent = layerMap[{json.dumps(part['parent'])}];")
    for part in parts_sorted:
        pid = part["id"]
        v_pid = re.sub(r"\W", "_", pid)
        r_min, r_max = part["rotation"]
        lines.append(f"var rot_{v_pid} = layerMap[{json.dumps(pid)}].property('Rotation');")
        for t_sec, pose_name in beat_times:
            rot_val = 0.0 if mode == "static" else float(poses.get(pose_name, {}).get("parts", {}).get(pid, {}).get("rotation", 0.0))
            lines.append(f"rot_{v_pid}.setValueAtTime({t_sec:.4f}, {max(float(r_min), min(float(r_max), rot_val)):.2f});")
        lines.append(f"rot_{v_pid}.expression = 'loopOut(\"cycle\")';")

    out_jsx_path.parent.mkdir(parents=True, exist_ok=True)
    out_jsx_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return out_jsx_path


def build_effectcraft_cutout_animation(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_dir: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    out_mp4: Optional[Path] = None,
    out_jsx: Optional[Path] = None,
) -> Dict[str, Any]:
    cid = rig_plan["character_id"]
    w, h = rig_plan["canvas_size"]
    comp_w, comp_h = int(w) + (int(w) % 2), int(h) + (int(h) % 2)
    comp_dur = max(0.5, float(duration))
    ecproj_path = out_dir / f"{cid}_cutout_rig.ecproj"
    ectemplate_path = out_dir / f"{cid}_cutout_rig.ectemplate"
    values_json_path = out_dir / f"{cid}_values.json"
    mp4_path = Path(out_mp4).resolve() if out_mp4 else (out_dir / f"{cid}_effectcraft_cutout.mp4")
    jsx_path = Path(out_jsx).resolve() if out_jsx else (out_dir / f"{cid}_cutout_rig.jsx")
    frame0_path = out_dir / f"{cid}_ec_cutout_frame0.png"
    frame1_path = out_dir / f"{cid}_ec_cutout_frame1.png"
    webm_alpha_path = out_dir / f"{cid}_effectcraft_cutout_alpha.webm"

    export_effectcraft_jsx_script(rig_plan, pose_library, jsx_path, mode=mode, speed=speed, duration=comp_dur)
    parts_sorted = sorted(rig_plan["parts"], key=lambda p: p["layer"])
    ec_bin = shutil.which("effectcraft-cli") or "effectcraft-cli"
    comp_name = f"{cid}_cutout_rig"
    run_args: List[str] = [
        ec_bin, "--empty", "run",
        "comp.new", json.dumps({"name": comp_name, "width": comp_w, "height": comp_h, "duration": comp_dur, "frameRate": 24}),
        "essential.setPrimary", json.dumps({"comp": comp_name}),
        "essential.setName", json.dumps({"comp": comp_name, "name": f"{cid} Reusable Cutout Template"}),
    ]
    template_values: Dict[str, Any] = {}
    secondary_follow_parts: List[str] = []

    for part in parts_sorted:
        pid = part["id"]
        px, py = float(part["pivot"][0]), float(part["pivot"][1])
        media_ctrl = f"{pid}_media"
        rot_ctrl = f"{pid}_rotation"
        run_args.extend([
            "file.import", json.dumps({"paths": [part["full_layer_path"]], "sequence": False, "addToComp": True}),
            "layer.rename", json.dumps({"name": pid}),
            "prop.set", json.dumps({"layer": pid, "path": "transform/anchor", "value": [px, py, 0.0]}),
            "prop.set", json.dumps({"layer": pid, "path": "transform/position", "value": [px, py, 0.0]}),
            "essential.addProperty", json.dumps({"layer": pid, "path": "transform/rotation", "name": rot_ctrl}),
        ])
        template_values[media_ctrl] = part["full_layer_path"]

    for part in parts_sorted:
        if part.get("parent"):
            run_args.extend(["layer.setParent", json.dumps({"layers": [part["id"]], "parent": part["parent"], "compensate": True})])

    beat_times = _mode_beat_times(mode, speed)
    poses = pose_library["poses"]
    for part in parts_sorted:
        pid = part["id"]
        r_min, r_max = part["rotation"]
        for t_sec, pose_name in beat_times:
            rot_val = 0.0 if mode == "static" else float(poses.get(pose_name, {}).get("parts", {}).get(pid, {}).get("rotation", 0.0))
            run_args.extend(["prop.addKey", json.dumps({"layer": pid, "path": "transform/rotation", "time": t_sec, "value": max(float(r_min), min(float(r_max), rot_val))})])
        run_args.extend(["prop.setExpression", json.dumps({"layer": pid, "path": "transform/rotation", "expression": 'loopOut("cycle")'})])
        if _is_secondary_appendage(part):
            secondary_follow_parts.append(pid)

    run_args.extend([
        "essential.exportTemplate", json.dumps({"comp": comp_name, "path": str(ectemplate_path), "name": f"{cid}_cutout_rig"}),
        "--save-as", str(ecproj_path), "--json",
    ])
    proc = subprocess.run(run_args, capture_output=True, text=True, timeout=60)
    if proc.returncode != 0:
        raise RuntimeError(f"effectcraft-cli rig build failed: {proc.stderr or proc.stdout}")

    values_json_path.write_text(json.dumps(template_values, indent=2) + "\n", encoding="utf-8")
    mp4_path.parent.mkdir(parents=True, exist_ok=True)
    peak_time = f"{min(comp_dur * 0.5, beat_times[1][0] if len(beat_times) > 1 else 0.35):.2f}"
    subprocess.run([ec_bin, "--project", str(ecproj_path), "render-frame", "--time", "0.0", "--transparent", "--out", str(frame0_path)], check=True, capture_output=True, timeout=30)
    subprocess.run([ec_bin, "--project", str(ecproj_path), "render-frame", "--time", peak_time, "--transparent", "--out", str(frame1_path)], check=True, capture_output=True, timeout=30)
    subprocess.run([ec_bin, "--project", str(ecproj_path), "render", "--out", str(webm_alpha_path), "--format", "webm", "--channels", "rgba"], check=False, capture_output=True, timeout=60)
    subprocess.run([ec_bin, "--project", str(ecproj_path), "render", "--out", str(mp4_path), "--format", "h264"], check=True, capture_output=True, timeout=60)

    return {
        "ecproj_path": str(ecproj_path),
        "ectemplate_path": str(ectemplate_path) if ectemplate_path.exists() else None,
        "values_json_path": str(values_json_path),
        "secondary_follow_parts": secondary_follow_parts,
        "jsx_path": str(jsx_path),
        "webm_alpha_path": str(webm_alpha_path) if webm_alpha_path.exists() else None,
        "mp4_path": str(mp4_path), "frame0_path": str(frame0_path), "frame1_path": str(frame1_path),
        "mode": mode, "speed": speed, "duration": comp_dur,
        "mp4_bytes": mp4_path.stat().st_size if mp4_path.exists() else 0,
    }


def render_ectemplate_with_values(
    template_path: Path,
    values_path: Path,
    out_path: Path,
    fmt: str = "h264",
    channels: str = "rgb",
) -> Dict[str, Any]:
    """Render a reusable .ectemplate with a values.json override file via effectcraft-cli."""
    ec_bin = shutil.which("effectcraft-cli") or "effectcraft-cli"
    out_path = Path(out_path).resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        ec_bin, "render",
        "--template", str(Path(template_path).resolve()),
        "--values", str(Path(values_path).resolve()),
        "--out", str(out_path),
        "--format", fmt,
        "--channels", channels,
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=90)
    if proc.returncode != 0:
        raise RuntimeError(f"effectcraft-cli template render failed: {proc.stderr or proc.stdout}")
    return {
        "ok": out_path.is_file(),
        "template": str(template_path),
        "values": str(values_path),
        "out": str(out_path),
        "bytes": out_path.stat().st_size if out_path.is_file() else 0,
    }


def _build_serializable_rig_spec(rig_plan: Dict[str, Any], pose_library: Dict[str, Any], mode: str = "walk", speed: float = 1.0, duration: float = 3.0, glb_path: Optional[str] = None) -> Dict[str, Any]:
    cid = rig_plan["character_id"]
    w, h = rig_plan["canvas_size"]
    return {
        "schemaVersion": "2.0", "characterId": cid, "canvasSize": [int(w), int(h)],
        "defaultMode": mode, "defaultSpeed": float(speed), "defaultDuration": float(duration),
        "glbPath": glb_path or f"exports/blender_ingest/{cid}/{cid}_4dir.glb",
        "parts": [{"id": p["id"], "layer": int(p["layer"]), "parent": p.get("parent"),
                   "pivot": [float(p["pivot"][0]), float(p["pivot"][1])],
                   "rotationLimitsDeg": [float(p["rotation"][0]), float(p["rotation"][1])],
                   "fullLayerPath": p["full_layer_path"], "croppedLayerPath": p.get("cropped_path", p["full_layer_path"])}
                  for p in sorted(rig_plan["parts"], key=lambda p: p["layer"])],
        "poses": pose_library.get("poses", {}),
        "modeBeats": {m: _mode_beat_times(m, 1.0) for m in ("idle", "walk", "run", "jump", "wave", "static")},
    }


def export_threejs_bundle(rig_plan: Dict[str, Any], pose_library: Dict[str, Any], out_js_path: Path, mode: str = "walk", speed: float = 1.0, duration: float = 3.0, glb_path: Optional[str] = None) -> Path:
    spec = _build_serializable_rig_spec(rig_plan, pose_library, mode, speed, duration, glb_path)
    out_js_path.parent.mkdir(parents=True, exist_ok=True)
    out_js_path.write_text(f"export const RIG_SPEC = {json.dumps(spec, indent=2)};\n", encoding="utf-8")
    return out_js_path


def export_react_motion_component(rig_plan: Dict[str, Any], pose_library: Dict[str, Any], out_tsx_path: Path, mode: str = "walk", speed: float = 1.0, duration: float = 3.0, glb_path: Optional[str] = None) -> Path:
    spec = _build_serializable_rig_spec(rig_plan, pose_library, mode, speed, duration, glb_path)
    out_tsx_path.parent.mkdir(parents=True, exist_ok=True)
    out_tsx_path.write_text(f"export const RIG_CONFIG = {json.dumps(spec, indent=2)} as const;\n", encoding="utf-8")
    return out_tsx_path


def export_blender_python_script(rig_plan: Dict[str, Any], pose_library: Dict[str, Any], out_py_path: Path, mode: str = "walk", speed: float = 1.0, duration: float = 3.0, glb_path: Optional[str] = None, blend_path: Optional[str] = None) -> Path:
    spec = _build_serializable_rig_spec(rig_plan, pose_library, mode, speed, duration, glb_path)
    out_py_path.parent.mkdir(parents=True, exist_ok=True)
    out_py_path.write_text(f"import bpy, json\nRIG_SPEC = json.loads({json.dumps(json.dumps(spec))})\n", encoding="utf-8")
    return out_py_path
