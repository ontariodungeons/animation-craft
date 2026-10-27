#!/usr/bin/env python3
"""
Unified CLI for Track B: Image -> 3-Question AI Intake -> Local Studio -> Blender 5.3 -> FilmCraft.
"""
import argparse
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

from pipeline.intake_router import run_three_question_intake
from pipeline.blender53_bridge import execute_archetype_in_blender53
from pipeline.filmcraft_bridge import load_animation_into_filmcraft

WORKDIR = os.path.dirname(os.path.abspath(__file__))
EXPORTS_DIR = os.path.join(WORKDIR, "exports")

Q1_TO_ARCHETYPE = {
    "walk_jump_talk": ("humanoid_biped", "move_traverse"),
    "swim_wave_fins": ("axial_creature", "move_traverse"),
    "rise_fall_pulse": ("radial_prop", "jump_rise_fall"),
    "traverse_left_right_turn": ("vehicle_hull", "move_traverse"),
}


def resolve_intake_overrides(args) -> tuple[str, dict]:
    image_hint = getattr(args, "image", None) or getattr(args, "name", None) or "hopper_character.png"
    overrides = {}
    q1 = getattr(args, "q1", None)
    if q1:
        if q1 in Q1_TO_ARCHETYPE:
            overrides["archetype"] = Q1_TO_ARCHETYPE[q1][0]
            overrides["primaryVerb"] = Q1_TO_ARCHETYPE[q1][1]
        elif q1 in ("humanoid_biped", "axial_creature", "radial_prop", "vehicle_hull"):
            overrides["archetype"] = q1
        overrides["q1MotionIntent"] = q1

    q2 = getattr(args, "q2", None)
    if q2:
        q2_str = str(q2).strip()
        if q2_str in (
            "base_mannequin_plus_costume_skin",
            "auto_slice_appendages",
            "single_deformable_sheet",
        ):
            overrides["slicingStrategy"] = q2_str
        else:
            parts = [p.strip() for p in q2_str.split(",") if p.strip()]
            overrides["selectedParts"] = parts
            overrides["slicingStrategy"] = (
                "single_deformable_sheet" if len(parts) <= 1 else "auto_slice_appendages"
            )

    q3 = getattr(args, "q3", None)
    if q3:
        q3_str = str(q3).strip()
        overrides["exportTarget"] = q3_str
        if q3_str in ("move_traverse", "jump_rise_fall", "turn_bank", "express_talk"):
            overrides["primaryVerb"] = q3_str

    if getattr(args, "archetype", None):
        overrides["archetype"] = args.archetype
    if getattr(args, "verb", None):
        overrides["primaryVerb"] = args.verb
    if getattr(args, "strip_costume", False):
        overrides["stripCostumeToBase"] = True
    return image_hint, overrides


def cmd_intake(args):
    image_hint, overrides = resolve_intake_overrides(args)
    res = run_three_question_intake(image_hint, overrides)
    print(json.dumps(res, indent=2))



def cmd_blender_check(args):
    os.makedirs(EXPORTS_DIR, exist_ok=True)
    char_slug = getattr(args, "character", None)
    image_arg = getattr(args, "image", None)
    if image_arg and not os.path.exists(image_arg):
        from pipeline.vault_characters import resolve_real_vault_image
        image_arg = resolve_real_vault_image(image_arg)
    elif not image_arg and char_slug:
        from pipeline.vault_characters import resolve_real_vault_image
        image_arg = resolve_real_vault_image(char_slug)
    elif not image_arg and not char_slug:
        raise FileNotFoundError("blender-check requires either --image <path> or --character <id>")

    out_mp4 = args.out_mp4 or os.path.join(EXPORTS_DIR, f"{args.archetype}_{args.verb}_blender53.mp4")
    out_png = args.out_png or os.path.join(EXPORTS_DIR, f"{args.archetype}_{args.verb}_blender53_preview.png")
    glb_path = args.glb or os.path.join(EXPORTS_DIR, "felt_bunny_explorer_blender.glb")
    known_cast = ("miss_hayley", "old_mac", "hopper", "whiskers", "puddles", "rusty", "jenny")
    use_cast_builder = (
        char_slug
        and char_slug.lower() in known_cast
        and (not getattr(args, "image", None) or char_slug.lower() in os.path.basename(image_arg).lower())
    )
    if use_cast_builder:
        from pipeline.farm_school_scene_builder import render_single_character_blender53
        res = render_single_character_blender53(
            slug=char_slug.lower(),
            out_mp4=out_mp4,
            out_png=out_png,
            out_glb=args.glb,
        )
    else:
        res = execute_archetype_in_blender53(
            archetype=args.archetype,
            verb=args.verb,
            glb_path=glb_path,
            out_mp4=out_mp4,
            out_preview_png=out_png,
            strip_costume=args.strip_costume,
            image_path=image_arg,
        )
    print(json.dumps(res, indent=2))


def cmd_filmcraft_load(args):
    os.makedirs(EXPORTS_DIR, exist_ok=True)
    proj_out = args.project or os.path.join(EXPORTS_DIR, "track_b_animation.fcproj")
    frame_out = args.frame_out or os.path.join(EXPORTS_DIR, "track_b_filmcraft_frame.png")
    res = load_animation_into_filmcraft(args.media, proj_out, frame_out)
    print(json.dumps(res, indent=2))


def cmd_run_e2e(args):
    out_dir = getattr(args, "out_dir", None) or EXPORTS_DIR
    os.makedirs(out_dir, exist_ok=True)
    image_hint, overrides = resolve_intake_overrides(args)
    intake = run_three_question_intake(image_hint, overrides)
    arch = intake["recommended"]["archetype"]
    verb = intake["recommended"]["primaryVerb"]

    raw_stem = getattr(args, "name", None) or os.path.splitext(os.path.basename(image_hint))[0]
    base_stem = raw_stem.replace(" ", "_")

    out_mp4 = os.path.join(out_dir, f"{base_stem}_{arch}_{verb}_blender53.mp4")
    out_png = os.path.join(out_dir, f"{base_stem}_{arch}_{verb}_blender53.png")
    fcproj_out = os.path.join(out_dir, f"{base_stem}_{arch}_{verb}.fcproj")
    fc_frame = os.path.join(out_dir, f"{base_stem}_{arch}_{verb}_filmcraft_frame.png")
    glb_path = os.path.join(EXPORTS_DIR, "felt_bunny_explorer_blender.glb")

    blender_res = execute_archetype_in_blender53(
        archetype=arch,
        verb=verb,
        glb_path=glb_path,
        out_mp4=out_mp4,
        out_preview_png=out_png,
        strip_costume=getattr(args, "strip_costume", False),
        image_path=image_hint,
    )
    fc_res = load_animation_into_filmcraft(out_mp4, fcproj_out, fc_frame)

    summary = {
        "step1_intake": intake,
        "step2_blender53": blender_res,
        "step3_filmcraft": fc_res,
    }
    print(json.dumps(summary, indent=2))


def cmd_serve_agent_bridge(args):
    class Handler(BaseHTTPRequestHandler):
        def do_OPTIONS(self):
            self.send_response(200)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
            self.send_header("Access-Control-Allow-Headers", "Content-Type")
            self.end_headers()

        def do_POST(self):
            length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(length).decode("utf-8") or "{}")
            image_hint = body.get("image") or body.get("subjectHint") or "uploaded_character.png"
            res = run_three_question_intake(image_hint, body.get("overrides"))
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(json.dumps(res).encode("utf-8"))

    server = HTTPServer(("127.0.0.1", args.port), Handler)
    print(f"Omarchy Agent Intake Bridge listening on http://127.0.0.1:{args.port}")
    server.serve_forever()


def add_shared_intake_flags(parser):
    parser.add_argument("image", nargs="?", default=None, help="Path or filename of source image")
    parser.add_argument("--name", default=None, help="Asset slug name")
    parser.add_argument("--image", dest="image_flag", default=None, help="Optional path to source image")
    parser.add_argument("--q1", default=None, help="Q1 motion intent (walk_jump_talk, swim_wave_fins, rise_fall_pulse, traverse_left_right_turn)")
    parser.add_argument("--q2", default=None, help="Q2 comma-separated slice/bend parts")
    parser.add_argument("--q3", default="filmcraft_mp4", help="Q3 export target")
    parser.add_argument("--archetype", choices=["humanoid_biped", "axial_creature", "radial_prop", "vehicle_hull"], default=None)
    parser.add_argument("--verb", choices=["move_traverse", "jump_rise_fall", "turn_bank", "express_talk"], default=None)
    parser.add_argument("--strip-costume", action="store_true")


def main():
    parser = argparse.ArgumentParser(description="Image-to-Animation Studio -> Blender 5.3 -> FilmCraft CLI")
    sub = parser.add_subparsers(dest="subcommand", required=True)
    sub.add_parser("cut-sheet", help="Export pose/view sprites, articulated masks and exploded prompt (cut-sheet --help)", add_help=False)
    if len(sys.argv) > 1 and sys.argv[1] == "cut-sheet":
        from pipeline.pose_and_part_cutter import main as cut_sheet_main
        raise SystemExit(cut_sheet_main(sys.argv[2:]))

    sub.add_parser("optimize-accuracy", help="Register 4+ turnaround directions (0px crown/foot drift) and measure still-to-limit pose motion range", add_help=False)
    if len(sys.argv) > 1 and sys.argv[1] == "optimize-accuracy":
        from pipeline.codex_accuracy_optimizer import main as optimize_main
        raise SystemExit(optimize_main(sys.argv[2:]))

    sub.add_parser("iterate-character", help="Export shareable 2D cutout layers + 6-action animation track package for one or all characters", add_help=False)
    if len(sys.argv) > 1 and sys.argv[1] == "iterate-character":
        from pipeline.character_iteration_exporter import main as iterate_main
        raise SystemExit(iterate_main(sys.argv[2:]))

    sub.add_parser("farm-school-scene", help="Build 6-character Blender 5.3 animations + combined Farm School Lineup scene", add_help=False)
    if len(sys.argv) > 1 and sys.argv[1] == "farm-school-scene":
        from pipeline.farm_school_scene_builder import main as farm_school_main
        raise SystemExit(farm_school_main(sys.argv[2:]))

    p_val = sub.add_parser("validate", help="Validate any imported image against the 360° turnaround + pose-sheet contract")
    p_val.add_argument("image", help="Path to any image or sheet to validate")
    p_val.set_defaults(func=lambda a: print(json.dumps(__import__("pipeline.import_validator", fromlist=["validate_imported_asset"]).validate_imported_asset(a.image), indent=2)))

    p_cmp = sub.add_parser("compare-reference", help="Compare raw reference image vs application imported RGBA image + OCR + TypeSafe Jev")
    p_cmp.add_argument("reference", help="Path to raw reference image")
    p_cmp.add_argument("imported", help="Path to application imported transparent RGBA image")
    p_cmp.add_argument("--manifest", default=None, help="Optional path to sliced manifest.json")
    p_cmp.add_argument("--yaw", type=float, default=0.0, help="Yaw rotation angle (0, 45, 90, 135, 180) for profile distortion check")
    p_cmp.set_defaults(
        func=lambda a: print(
            json.dumps(
                __import__(
                    "pipeline.jev_qa_verifier",
                    fromlist=["compare_reference_and_import_with_jev"],
                ).compare_reference_and_import_with_jev(a.reference, a.imported, a.manifest, a.yaw),
                indent=2,
            )
        )
    )

    p_in = sub.add_parser("intake", help="Run 3-Question AI Intake on an image")
    add_shared_intake_flags(p_in)
    p_in.add_argument("--json", action="store_true")
    p_in.set_defaults(func=cmd_intake)

    p_bl = sub.add_parser("blender-check", help="Build/rig archetype in live Blender 5.3 and render MP4 + preview PNG")
    p_bl.add_argument("--archetype", required=True, choices=["humanoid_biped", "axial_creature", "radial_prop", "vehicle_hull"])
    p_bl.add_argument("--verb", default="move_traverse", choices=["move_traverse", "jump_rise_fall", "turn_bank", "express_talk"])
    p_bl.add_argument("--character", default=None, help="Optional character slug (miss_hayley, old_mac, hopper, whiskers, puddles, rusty)")
    p_bl.add_argument("--image", default=None, help="Explicit path to imported source image")
    p_bl.add_argument("--glb", default=None)
    p_bl.add_argument("--out-mp4", default=None)
    p_bl.add_argument("--out-png", default=None)
    p_bl.add_argument("--strip-costume", action="store_true")
    p_bl.set_defaults(func=cmd_blender_check)

    p_fc = sub.add_parser("filmcraft-load", help="Import rendered MP4 into FilmCraft (.fcproj) and verify")
    p_fc.add_argument("media", help="Path to rendered .mp4 animation")
    p_fc.add_argument("--project", default=None)
    p_fc.add_argument("--frame-out", default=None)
    p_fc.set_defaults(func=cmd_filmcraft_load)

    for cmd_name in ("run-e2e", "run"):
        p_e2e = sub.add_parser(cmd_name, help="Run full Track B: 3-Question Intake -> Blender 5.3 Render -> FilmCraft Load")
        add_shared_intake_flags(p_e2e)
        p_e2e.add_argument("--frames", type=int, default=24)
        p_e2e.add_argument("--out-dir", default=None)
        p_e2e.set_defaults(func=cmd_run_e2e)

    p_srv = sub.add_parser("serve-agent-bridge", help="Start local HTTP bridge for the 'Ask Omarchy Agent' button")
    p_srv.add_argument("--port", type=int, default=8766)
    p_srv.set_defaults(func=cmd_serve_agent_bridge)

    args = parser.parse_args()
    if getattr(args, "image_flag", None) and not getattr(args, "image", None):
        args.image = args.image_flag
    args.func(args)


if __name__ == "__main__":
    main()
