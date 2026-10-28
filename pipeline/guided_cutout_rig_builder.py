#!/usr/bin/env python3
"""Sequential orchestrator that loads and runs Stage 3A (PhotoCraft) -> Stage 3B (VectorCraft) -> Stage 3C (EffectCraft)."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.step_photocraft_parts import (
    discover_character_joints_and_polygons,
    mask_and_export_layers_with_photocraft,
    render_cutout_asset_sheet,
)
from pipeline.step_vector_trace import trace_part_layers_with_vectorcraft
from pipeline.step_effectcraft_rig import (
    build_effectcraft_cutout_animation,
    build_pose_library,
    export_blender_python_script,
    export_effectcraft_jsx_script,
    export_react_motion_component,
    export_threejs_bundle,
)


def build_guided_cutout_rig(
    rgba_path: str | Path,
    character_id: str = "hopper",
    out_dir: Optional[str | Path] = None,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    selected_parts: Optional[List[str]] = None,
    out_mp4: Optional[str | Path] = None,
    out_jsx: Optional[str | Path] = None,
) -> Dict[str, Any]:
    """Run Stage 3A -> Stage 3B -> Stage 3C sequentially."""
    src_path = Path(rgba_path).resolve()
    if not src_path.exists():
        raise FileNotFoundError(f"Input image not found: {src_path}")
    target_dir = Path(out_dir).resolve() if out_dir else src_path.parent
    target_dir.mkdir(parents=True, exist_ok=True)

    # 1. Stage 3A: PhotoCraft part slicing
    discovery = discover_character_joints_and_polygons(src_path, character_id=character_id, selected_parts=selected_parts)
    pc_res = mask_and_export_layers_with_photocraft(src_path, discovery, target_dir, character_id)
    pose_lib = build_pose_library(character_id)

    joints_map = {
        p["id"]: {
            "pivot": p["pivot"], "pivot_uv": p["pivot_uv"],
            "rotation": p["rotation"], "joint_radius_px": p["joint_radius_px"],
        }
        for p in pc_res["parts"]
    }
    rig_plan: Dict[str, Any] = {
        "character_id": character_id,
        "rig_type": "cutout_rig",
        "discovery_mode": discovery.get("discovery_mode", "photocraft_guided"),
        "selected_parts": discovery.get("selected_parts", []),
        "source_image": str(src_path),
        "canvas_size": discovery["canvas_size"],
        "character_bbox": discovery["character_bbox"],
        "photocraft_psd": pc_res["psd_path"],
        "parts": pc_res["parts"],
        "joints": joints_map,
        "pose_library": pose_lib,
        "qa_telemetry": pc_res.get("qa_telemetry", {}),
    }
    sheet_path = target_dir / f"{character_id}_cutout_asset_sheet.png"
    render_cutout_asset_sheet(src_path, rig_plan, sheet_path)
    rig_plan["cutout_asset_sheet"] = str(sheet_path)

    # 2. Stage 3C: EffectCraft rig composition & video render
    ec_res = build_effectcraft_cutout_animation(
        rig_plan, pose_lib, target_dir,
        mode=mode, speed=speed, duration=duration,
        out_mp4=Path(out_mp4) if out_mp4 else None,
        out_jsx=Path(out_jsx) if out_jsx else None,
    )
    rig_plan["effectcraft"] = ec_res

    rig_plan_path = target_dir / f"{character_id}_cutout_rig_plan.json"
    pose_lib_path = target_dir / f"{character_id}_pose_library.json"
    rig_plan_path.write_text(json.dumps(rig_plan, indent=2), encoding="utf-8")

    # 3. Stage 3B: VectorCraft SVG tracing
    try:
        vc_res = trace_part_layers_with_vectorcraft(rig_plan_path)
        if vc_res.get("ok"):
            rig_plan = json.loads(rig_plan_path.read_text(encoding="utf-8"))
            rig_plan["vectorcraft"] = vc_res
    except Exception as exc:
        rig_plan["vectorcraft"] = {"ok": False, "error": str(exc)}

    threejs_path = target_dir / f"{character_id}_threejs_puppet.js"
    react_motion_path = target_dir / f"{character_id}_react_motion.tsx"
    blender_py_path = target_dir / f"{character_id}_blender_rig.py"
    export_threejs_bundle(rig_plan, pose_lib, threejs_path, mode=mode, speed=speed, duration=duration)
    export_react_motion_component(rig_plan, pose_lib, react_motion_path, mode=mode, speed=speed, duration=duration)
    export_blender_python_script(rig_plan, pose_lib, blender_py_path, mode=mode, speed=speed, duration=duration)
    rig_plan["exporters"] = {
        "threejs_path": str(threejs_path),
        "react_motion_path": str(react_motion_path),
        "blender_py_path": str(blender_py_path),
        "effectcraft_jsx_path": ec_res.get("jsx_path"),
    }
    rig_plan_path.write_text(json.dumps(rig_plan, indent=2), encoding="utf-8")
    pose_lib_path.write_text(json.dumps(pose_lib, indent=2), encoding="utf-8")

    # Sync native Magic Studio `rig.json` when the source or character folder lives under `exports/design_share/<character>`
    design_share_dir = src_path.parent
    if design_share_dir.name == "frames" and design_share_dir.parent.name == "registered_turnaround":
        design_share_dir = design_share_dir.parent.parent
    elif not (design_share_dir / "rig.json").is_file():
        candidate_ds = Path(_REPO_ROOT) / "exports/design_share" / character_id
        if candidate_ds.is_dir():
            design_share_dir = candidate_ds
    if design_share_dir.is_dir() and ((design_share_dir / "rig.json").is_file() or design_share_dir.parent.name == "design_share"):
        import shutil as _shutil
        from pipeline.step_studio_rig_tracks import compile_studio_rig_json
        ds_layers = design_share_dir / "layers"
        ds_layers.mkdir(parents=True, exist_ok=True)
        layers_meta = []
        for p in pc_res["parts"]:
            src_full = Path(p["full_layer_path"])
            dst_part = ds_layers / f"{p['id']}.png"
            _shutil.copy2(src_full, dst_part)
            layers_meta.append({
                "name": p["id"],
                "file": f"layers/{p['id']}.png",
                "size": discovery["canvas_size"],
                "pivot": p["pivot"],
                "bindPosition": p["pivot"],
                "parent": p["parent"],
            })
        timed_traj = {}
        for mr_cand in [
            design_share_dir / "motion_range.json",
            design_share_dir / f"{character_id}_motion_range.json",
            design_share_dir / "accuracy/motion_range.json",
            Path(_REPO_ROOT) / f"exports/pose_and_parts/{character_id}/motion_range.json",
        ]:
            if mr_cand.is_file():
                try:
                    timed_traj = json.loads(mr_cand.read_text(encoding="utf-8")).get("timedTrajectories", {})
                    if timed_traj:
                        break
                except Exception:
                    pass
        compile_studio_rig_json(
            character_id=character_id,
            canvas_size=discovery["canvas_size"],
            layers_meta=layers_meta,
            timed_trajectories=timed_traj,
            out_path=design_share_dir / "rig.json",
        )

    return {
        "ok": True,
        "character_id": character_id,
        "discovery_mode": rig_plan["discovery_mode"],
        "selected_parts": rig_plan["selected_parts"],
        "rig_plan_path": str(rig_plan_path),
        "pose_library_path": str(pose_lib_path),
        "cutout_asset_sheet": str(sheet_path),
        "photocraft_psd": pc_res["psd_path"],
        "part_count": len(pc_res["parts"]),
        "qa_telemetry": pc_res.get("qa_telemetry", {}),
        "effectcraft": ec_res,
        "exporters": rig_plan["exporters"],
    }


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", nargs="?", default=None)
    parser.add_argument("--character", default="hopper")
    parser.add_argument("--out-dir", default=None)
    parser.add_argument("--mode", default="walk")
    parser.add_argument("--speed", type=float, default=1.0)
    parser.add_argument("--duration", type=float, default=3.0)
    parser.add_argument("--parts", default=None)
    parser.add_argument("--rig-plan", default=None)
    parser.add_argument("--render-only", action="store_true")
    parser.add_argument("--jsx-only", action="store_true")
    parser.add_argument("--threejs-only", action="store_true")
    parser.add_argument("--react-motion-only", action="store_true")
    parser.add_argument("--blender-py-only", action="store_true")
    parser.add_argument("--trace-svg-only", action="store_true")
    parser.add_argument("--trace-preset", default="16 Colors")
    parser.add_argument("--out-mp4", default=None)
    parser.add_argument("--out-jsx", default=None)
    parser.add_argument("--out-threejs", default=None)
    parser.add_argument("--out-react-motion", default=None)
    parser.add_argument("--out-blender-py", default=None)
    parser.add_argument("--glb-path", default=None)
    parser.add_argument("--blend-path", default=None)
    args = parser.parse_args(argv)

    selected_parts = [p.strip() for p in args.parts.split(",") if p.strip()] if args.parts else None
    export_only = any([
        args.render_only, args.jsx_only, args.threejs_only,
        args.react_motion_only, args.blender_py_only, args.trace_svg_only,
    ])
    if export_only:
        if not args.rig_plan:
            raise SystemExit("Export-only flags require --rig-plan <path>")
        plan_path = Path(args.rig_plan).resolve()
        if args.trace_svg_only:
            print(json.dumps(trace_part_layers_with_vectorcraft(plan_path, preset=args.trace_preset), indent=2))
            return 0
        rig_plan_data = json.loads(plan_path.read_text(encoding="utf-8"))
        cid_val = rig_plan_data.get("character_id", args.character)
        pose_lib_data = rig_plan_data.get("pose_library") or build_pose_library(cid_val)
        out_dir_path = Path(args.out_dir).resolve() if args.out_dir else plan_path.parent
        if args.jsx_only:
            target = Path(args.out_jsx).resolve() if args.out_jsx else (out_dir_path / f"{cid_val}_cutout_rig.jsx")
            export_effectcraft_jsx_script(rig_plan_data, pose_lib_data, target, mode=args.mode, speed=args.speed, duration=args.duration)
            print(json.dumps({"ok": True, "jsx_path": str(target), "mode": args.mode, "speed": args.speed, "duration": args.duration}, indent=2))
        elif args.threejs_only:
            target = Path(args.out_threejs).resolve() if args.out_threejs else (out_dir_path / f"{cid_val}_threejs_puppet.js")
            export_threejs_bundle(rig_plan_data, pose_lib_data, target, mode=args.mode, speed=args.speed, duration=args.duration, glb_path=args.glb_path)
            print(json.dumps({"ok": True, "threejs_path": str(target), "mode": args.mode, "speed": args.speed, "duration": args.duration}, indent=2))
        elif args.react_motion_only:
            target = Path(args.out_react_motion).resolve() if args.out_react_motion else (out_dir_path / f"{cid_val}_react_motion.tsx")
            export_react_motion_component(rig_plan_data, pose_lib_data, target, mode=args.mode, speed=args.speed, duration=args.duration, glb_path=args.glb_path)
            print(json.dumps({"ok": True, "react_motion_path": str(target), "mode": args.mode, "speed": args.speed, "duration": args.duration}, indent=2))
        elif args.blender_py_only:
            target = Path(args.out_blender_py).resolve() if args.out_blender_py else (out_dir_path / f"{cid_val}_blender_rig.py")
            export_blender_python_script(rig_plan_data, pose_lib_data, target, mode=args.mode, speed=args.speed, duration=args.duration, glb_path=args.glb_path, blend_path=args.blend_path)
            print(json.dumps({"ok": True, "blender_py_path": str(target), "mode": args.mode, "speed": args.speed, "duration": args.duration}, indent=2))
        else:
            ec_out = build_effectcraft_cutout_animation(
                rig_plan_data, pose_lib_data, out_dir_path,
                mode=args.mode, speed=args.speed, duration=args.duration,
                out_mp4=Path(args.out_mp4) if args.out_mp4 else None,
                out_jsx=Path(args.out_jsx) if args.out_jsx else None,
            )
            rig_plan_data["effectcraft"] = ec_out
            plan_path.write_text(json.dumps(rig_plan_data, indent=2), encoding="utf-8")
            print(json.dumps({"ok": True, "effectcraft": ec_out}, indent=2))
        return 0

    if not args.image:
        cand = Path(_REPO_ROOT) / "exports/design_share" / args.character / "registered_turnaround/frames/00_front.png"
        if cand.is_file():
            args.image = str(cand)
            if not args.out_dir:
                args.out_dir = str(Path(_REPO_ROOT) / "exports/pose_and_parts" / args.character)
        else:
            raise SystemExit("image argument is required unless an export-only flag is set")
    res = build_guided_cutout_rig(
        args.image, character_id=args.character, out_dir=args.out_dir,
        mode=args.mode, speed=args.speed, duration=args.duration,
        selected_parts=selected_parts, out_mp4=args.out_mp4, out_jsx=args.out_jsx,
    )
    print(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
