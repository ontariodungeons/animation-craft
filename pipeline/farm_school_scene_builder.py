#!/usr/bin/env python3
"""
Blender 5.3 Multi-Character & Farm School Lineup Scene Orchestrator.

Sequentially loads the three refactored 3D+2D pipeline stages:
  1. `pipeline/step_volume_hull.py`     — 2D turnaround + `motion_range.json` (`footLiftPx`) -> `visual_hull.npz`
  2. `pipeline/step_blender_mesh_rig.py` — 3D remesh, arm-torso partition, UV projection bake & `ARM_<slug>` armature
  3. `pipeline/step_blender_2d_anim.py`  — 2D gait/pose cycle import -> 3D IK animation, manifold QA & `.blend`/`.glb`/`.mp4`
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.step_volume_hull import (
    ANATOMY,
    CHARACTER_STAGE_SPECS,
    CLASS_BG,
    PERFORMANCES,
    ROOT,
    SCHOOL_BG,
    SIX_CAST_ORDER,
    VIEW_ANGLES,
    prepare_volume,
    resolve_character_assets,
)
from pipeline.step_blender_mesh_rig import BLENDER_MESH_RIG_CODE
from pipeline.step_blender_2d_anim import (
    BLENDER_2D_ANIM_CODE,
    BLENDER_HELPERS,
    gait_frames,
    root_at,
    run_blender_stage as _run_blender,
    smooth01,
)


def _blender_rig_helper_code() -> str:
    return BLENDER_HELPERS


def render_single_character_blender53(
    slug: str,
    out_mp4: str,
    out_png: str,
    out_glb: str | None = None,
    out_blend: str | None = None,
) -> dict:
    out = Path(out_mp4).resolve().parent
    spec = prepare_volume(resolve_character_assets(slug), out / "construction")
    spec["gait"] = gait_frames(spec, 48, solo=True)
    paths = dict(
        mp4=str(Path(out_mp4).resolve()),
        png=str(Path(out_png).resolve()),
        glb=str(Path(out_glb).resolve()) if out_glb else str(Path(out_mp4).resolve().with_suffix(".glb")),
        blend=str(Path(out_blend).resolve()) if out_blend else str(Path(out_mp4).resolve().with_suffix(".blend")),
    )
    code = f"""
SPEC={spec!r}
PATHS={paths!r}
clear_scene()
configure_scene(48,960,540)
rig,obj=build_character_rig(SPEC,make_collection('Character'),make_collection('Rig'))
animate_character_rig(rig,SPEC)
qa=verify_character(rig,obj,SPEC)
cam=setup_camera('CharacterPreview',SPEC['height_m'])
build_environment(cam,{str(SCHOOL_BG)!r})
add_contact_shadows(SPEC)
render=render_frames_to_mp4(bpy.context.scene,PATHS['mp4'],PATHS['png'],1,48)
export_character(rig,obj,PATHS['blend'],PATHS['glb'])
print('FARM_SCHOOL_RESULT='+json.dumps(dict(ok=True,character=SPEC['slug'],qa=qa,render=render,**PATHS)))
"""
    report = _run_blender(code)
    for p in paths.values():
        if not Path(p).is_file() or Path(p).stat().st_size == 0:
            raise RuntimeError(f"Blender did not write {p}")
    (out / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def build_farm_school_milestone(out_dir: str | Path, only_character: str | None = None) -> dict:
    out = Path(out_dir).resolve()
    out.mkdir(parents=True, exist_ok=True)
    if only_character and only_character != "all":
        slug = only_character.lower().strip()
        folder = out / "characters" / slug
        return render_single_character_blender53(
            slug,
            str(folder / f"{slug}_blender53.mp4"),
            str(folder / f"{slug}_blender53_preview.png"),
            str(folder / f"{slug}_rig.glb"),
            str(folder / f"{slug}_rig.blend"),
        )
    reports = []
    specs = []
    sources = [resolve_character_assets(slug) for slug in SIX_CAST_ORDER]
    for source in sources:
        slug = source["slug"]
        folder = out / "characters" / slug
        reports.append(
            render_single_character_blender53(
                slug,
                str(folder / f"{slug}_blender53.mp4"),
                str(folder / f"{slug}_blender53_preview.png"),
                str(folder / f"{slug}_rig.glb"),
                str(folder / f"{slug}_rig.blend"),
            )
        )
        spec = json.loads((folder / "construction/asset_sources.json").read_text())
        spec["gait"] = gait_frames(spec, 48)
        specs.append(spec)
    paths = {ext: str(out / f"farm_school_lineup_scene.{ext}") for ext in ["blend", "glb", "mp4", "png"]}
    code = f"""
SPECS={specs!r}
PATHS={paths!r}
clear_scene();configure_scene(48,1280,720)
chars=make_collection('Characters');rigs=make_collection('Rigs');qa={{}}
for spec in SPECS:
    rig,obj=load_character_rig(spec,os.path.join({str(out)!r},'characters',spec['slug'],spec['slug']+'_rig.blend'),chars,rigs)
    animate_character_rig(rig,spec);qa[spec['slug']]=verify_character(rig,obj,spec)
cam=setup_camera('FarmSchoolLineupCamera');build_environment(cam,{str(SCHOOL_BG)!r})
for spec in SPECS:add_contact_shadows(spec)
render=render_frames_to_mp4(bpy.context.scene,PATHS['mp4'],PATHS['png'],1,48)
for frame,name in [(1,'enter'),(24,'walking'),(48,'lined_up')]:
    shutil.copy2(render['snapshots'][str(frame)],os.path.join({str(out)!r},'farm_school_lineup_scene_frame%02d_%s.png'%(frame,name)))
bpy.context.scene.frame_set(48)
bpy.ops.wm.save_as_mainfile(filepath=PATHS['blend'])
bpy.ops.export_scene.gltf(filepath=PATHS['glb'],export_format='GLB',export_animations=True,export_force_sampling=True,export_animation_mode='SCENE',export_anim_slide_to_zero=True,export_cameras=True,export_lights=True)
print('FARM_SCHOOL_RESULT='+json.dumps(dict(ok=True,qa=qa,render=render,**PATHS)))
"""
    combined = _run_blender(code)
    report = dict(ok=True, characters=reports, combined_scene=combined)
    (out / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def self_test() -> None:
    for slug in list(SIX_CAST_ORDER) + ["jenny"]:
        spec = dict(
            slug=slug,
            **CHARACTER_STAGE_SPECS[slug],
            anatomy=ANATOMY[slug],
            performance=PERFORMANCES[slug],
            foot_lift_max_m=0.03,
        )
        rows = gait_frames(spec, 48)
        for side in ["L", "R"]:
            for before, after in zip(rows, rows[1:]):
                assert after["feet"][side][2] >= 0
                assert after["feet"][side][2] <= spec["foot_lift_max_m"] + 1e-9
                if before["planted"][side] and after["planted"][side]:
                    assert before["feet"][side] == after["feet"][side], (slug, side, after["frame"])
        assert all(math.isfinite(v) for row in rows for v in row["root"])
    compile(BLENDER_MESH_RIG_CODE, "<blender mesh rig stage>", "exec")
    compile(BLENDER_2D_ANIM_CODE, "<blender 2d anim stage>", "exec")
    compile(BLENDER_HELPERS, "<blender helpers>", "exec")
    print("Gait contact, measured lift bounds, and sequential Blender stage syntax checks passed.")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build reference-derived volumetric characters and a verified 24 fps Farm School scene")
    parser.add_argument("--out-dir", default="exports/farm_school_milestone")
    parser.add_argument("--character", default=None)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if args.self_test:
        self_test()
        return 0
    print(json.dumps(build_farm_school_milestone(args.out_dir, args.character), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
