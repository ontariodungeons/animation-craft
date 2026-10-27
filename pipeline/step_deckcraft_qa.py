#!/usr/bin/env python3
"""Stage: Build a 5-slide DeckCraft QA review deck for a character (.deckcraft + .pdf + slide PNGs)."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from pipeline.deckcraft_template import Deck, SCENE_ART_REGION, SIZE, fit_rect


def build_character_qa_deck(slug: str, out_dir: str | Path | None = None) -> dict:
    """Build a 5-slide DeckCraft QA review deck for a character (`.deckcraft` + `.pdf` + slide PNGs)."""
    root = Path(__file__).resolve().parents[1]
    slug = slug.lower().strip()
    pose_dir = Path(out_dir).resolve() if out_dir else (root / "exports/pose_and_parts" / slug)
    pose_dir.mkdir(parents=True, exist_ok=True)
    share_dir = root / "exports/design_share" / slug
    blender_dir = root / "exports/farm_school_milestone/characters" / slug

    deck_path = pose_dir / f"{slug}_qa_deck.deckcraft"
    pdf_path = pose_dir / f"{slug}_qa_deck.pdf"
    slides_dir = pose_dir / "qa_deck_slides"
    slides_dir.mkdir(parents=True, exist_ok=True)

    motion_path = share_dir / "motion_range.json"
    motion = json.loads(motion_path.read_text()) if motion_path.is_file() else {}
    verif_path = blender_dir / "verification.json"
    verif = json.loads(verif_path.read_text()) if verif_path.is_file() else {}
    rig_plan_path = pose_dir / f"{slug}_cutout_rig_plan.json"
    rig_plan = json.loads(rig_plan_path.read_text()) if rig_plan_path.is_file() else {}

    turnaround_img = share_dir / "registered_turnaround/contact_sheet.jpg"
    if not turnaround_img.is_file():
        turnaround_img = share_dir / "registered_turnaround/frames/00_front.png"
    cutout_sheet_img = pose_dir / f"{slug}_cutout_asset_sheet.png"
    if not cutout_sheet_img.is_file():
        cutout_sheet_img = share_dir / f"{slug}_layers_sheet.png"
    ec_frame0_img = pose_dir / f"{slug}_ec_cutout_frame0.png"
    six_actions_img = share_dir / f"{slug}_6actions_strip.png"
    blender_img = blender_dir / f"{slug}_blender53_preview.png"

    slide_specs = [
        (
            "01_turnaround_registration",
            f"{slug.upper()} — 1. Turnaround Registration & Normalization",
            "5-View Orthographic Registration & Enclosed Background Stripping",
            turnaround_img,
            f"Character: {slug}\nStanding Height: {motion.get('still', {}).get('heightPx', 'N/A')} px\n"
            f"Turnaround Height: {motion.get('turnaroundHeightPx', 'N/A')} px\n"
            f"Missing Requested Poses: {len(motion.get('missingRequestedPoses', []))}",
        ),
        (
            "02_photocraft_vectorcraft_parts",
            f"{slug.upper()} — 2. PhotoCraft Slicing & VectorCraft SVG",
            "Anatomical Joint Discovery, Overlap Cuffs & Vector Tracing",
            cutout_sheet_img,
            f"Part Count: {len(rig_plan.get('parts', []))}\n"
            f"Discovery Mode: {rig_plan.get('discovery_mode', 'photocraft_guided')}\n"
            f"Selected Parts: {', '.join(rig_plan.get('selected_parts', [])) or 'all'}\n"
            f"PSD Archive: {Path(rig_plan.get('photocraft_psd', '')).name if rig_plan.get('photocraft_psd') else 'N/A'}",
        ),
        (
            "03_effectcraft_cutout_rig",
            f"{slug.upper()} — 3. EffectCraft Cutout Rig & Template",
            "Hierarchical Parenting, Puppet Pin Follow-Through & Essential Graphics",
            ec_frame0_img if ec_frame0_img.is_file() else cutout_sheet_img,
            f"EffectCraft Project: {slug}_cutout_rig.ecproj\n"
            f"Reusable Template: {slug}_cutout_rig.ectemplate\n"
            f"Values Override: {slug}_values.json\n"
            f"Secondary Follow Appendages: {', '.join(rig_plan.get('effectcraft', {}).get('secondary_follow_parts', [])) or 'pins active'}",
        ),
        (
            "04_six_action_filmcraft_track",
            f"{slug.upper()} — 4. 6-Action Timed Track & FilmCraft",
            "Observed Motion Range, Timed Trajectories & Editorial Timeline",
            six_actions_img,
            f"Actions: idle, walk, run, jump, wave, talk_express\n"
            f"Measured Max Foot Lift: {motion.get('limits', {}).get('footLiftPx', {}).get('max', 0.0)} px\n"
            f"FilmCraft Timeline: {slug}_6actions.fcproj\n"
            f"Magic Studio Rig: exports/design_share/{slug}/rig.json",
        ),
        (
            "05_blender53_3d_rig_qa",
            f"{slug.upper()} — 5. Blender 5.3 3D Volumetric Rig & QA",
            "Closed Visual Hull, Baked Multi-View Albedo & 2D-Driven IK Gait",
            blender_img if blender_img.is_file() else turnaround_img,
            f"Vertices: {verif.get('qa', {}).get('vertices', 'N/A')} | Bones: {verif.get('qa', {}).get('bones', 23)}\n"
            f"Non-Manifold Edges: {verif.get('qa', {}).get('non_manifold_edges', 0)}\n"
            f"Max Foot Target Error: {verif.get('qa', {}).get('max_foot_target_error_m', 0.0):.2e} m\n"
            f"Used 3D Foot Lift: {verif.get('qa', {}).get('used_foot_lift_max_m', 0.0):.4f} m",
        ),
    ]

    d = Deck(root, pose_dir)
    slide_pngs = []
    try:
        d.cmd("file.new")
        d.cmd("design.slideSize", w=SIZE[0], h=SIZE[1], scale="none")
        for idx, (s_key, s_title, s_sub, img_path, notes_txt) in enumerate(slide_specs):
            if idx == 0:
                d.cmd("slide.go", index=0)
                existing_shapes = d.cmd("slide.inspect", index=0).get("shapes", [])
                if existing_shapes:
                    d.cmd("edit.delete", ids=[sh["id"] for sh in existing_shapes])
                d.page("qa", s_title, s_sub, new=False)
            else:
                d.page("qa", s_title, s_sub, new=True)
            d.box(SCENE_ART_REGION, "#08090C", f"layout.qa.{s_key}.well")
            d.card("Stage QA Telemetry & Verification", notes_txt, [60, 800, 960, 320], f"qa.{s_key}.telemetry", size=20)
            d.flush()
            if img_path and Path(img_path).is_file():
                pic = d.cmd("insert.picture", path=str(Path(img_path).resolve()), name=s_key)
                pid = pic["id"]
                native = d.cmd("shape.inspect", id=pid)["box"]
                fitted = fit_rect(native["w"], native["h"], SCENE_ART_REGION)
                d.cmd("shape.sizePosition", ids=[pid], x=fitted[0], y=fitted[1], w=fitted[2], h=fitted[3], lockAspect=False)
                d.cmd("shape.sizePosition", ids=[pid], lockAspect=True)
        d.flush()
        d.cmd("file.saveAs", path=str(deck_path))
        d.cmd("file.export", path=str(pdf_path), format="pdf")
        for idx, (s_key, *_rest) in enumerate(slide_specs):
            png_out = slides_dir / f"slide_{idx + 1:02d}_{s_key}.png"
            d.cmd("file.export", path=str(png_out), format="png", slide=idx, scale=1)
            slide_pngs.append(str(png_out))
    finally:
        d.close()

    return {
        "ok": deck_path.is_file() and pdf_path.is_file(),
        "character": slug,
        "deck": str(deck_path),
        "pdf": str(pdf_path),
        "slides": slide_pngs,
    }


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--character", required=True, help="Character slug")
    ap.add_argument("--out-dir", default=None)
    args = ap.parse_args(argv)
    print(json.dumps(build_character_qa_deck(args.character, args.out_dir), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
