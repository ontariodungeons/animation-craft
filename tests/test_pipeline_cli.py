import os
import unittest
from pipeline.intake_router import run_three_question_intake
from pipeline.topology_parity import parse_glb_topology


class TestPipelineCLI(unittest.TestCase):
    def test_three_question_intake_archetype_detection(self):
        fish_res = run_three_question_intake("fish_with_fins.png")
        self.assertEqual(fish_res["recommended"]["archetype"], "axial_creature")
        self.assertEqual(len(fish_res["questions"]), 3)

        sun_res = run_three_question_intake("smiling_sun.png")
        self.assertEqual(sun_res["recommended"]["archetype"], "radial_prop")
        self.assertEqual(sun_res["recommended"]["primaryVerb"], "jump_rise_fall")

        boat_res = run_three_question_intake("wooden_boat.png")
        self.assertEqual(boat_res["recommended"]["archetype"], "vehicle_hull")
        self.assertEqual(boat_res["recommended"]["primaryVerb"], "turn_bank")

        jenny_res = run_three_question_intake("Jenny-no-background.png")
        self.assertEqual(jenny_res["recommended"]["archetype"], "humanoid_biped")

    def test_intake_overrides_consume_q2_and_q3(self):
        import types
        from studio_pipeline_cli import resolve_intake_overrides

        args = types.SimpleNamespace(
            image="custom_fish.png",
            name=None,
            q1="swim_wave_fins",
            q2="head,spine_wave,tail_fin",
            q3="turn_bank",
            archetype=None,
            verb=None,
            strip_costume=False,
        )
        img_hint, overrides = resolve_intake_overrides(args)
        self.assertEqual(img_hint, "custom_fish.png")
        self.assertEqual(overrides["archetype"], "axial_creature")
        self.assertEqual(overrides["selectedParts"], ["head", "spine_wave", "tail_fin"])
        self.assertEqual(overrides["slicingStrategy"], "auto_slice_appendages")
        self.assertEqual(overrides["exportTarget"], "turn_bank")
        self.assertEqual(overrides["primaryVerb"], "turn_bank")

    def test_resolve_real_vault_image_fails_for_unknown_character(self):
        from pipeline.vault_characters import resolve_real_vault_image

        with self.assertRaises(FileNotFoundError):
            resolve_real_vault_image("nonexistent_character_xyz_999")

    def test_adaptive_cutout_discovery_and_jsx_export(self):
        import tempfile
        from pathlib import Path
        from pipeline.guided_cutout_rig_builder import (
            discover_character_joints_and_polygons,
            build_pose_library,
            export_effectcraft_jsx_script,
        )

        hopper_png = Path("/home/jesse/Work/omhas-animation/exports/pose_and_parts/hopper/hopper_original_transparent.png")
        if hopper_png.is_file():
            h_disc = discover_character_joints_and_polygons(hopper_png, character_id="hopper")
            self.assertEqual(h_disc["discovery_mode"], "hopper_calibrated")
            c_disc = discover_character_joints_and_polygons(
                hopper_png,
                character_id="custom_hero",
                selected_parts=["head", "torso", "leg_left", "leg_right"],
            )
            self.assertEqual(c_disc["discovery_mode"], "silhouette_contour_adaptive")
            self.assertEqual(c_disc["selected_parts"], ["head", "torso", "leg_left", "leg_right"])

            with tempfile.TemporaryDirectory() as tmp:
                jsx_out = Path(tmp) / "custom_run.jsx"
                rig_plan = {
                    "character_id": "custom_hero",
                    "canvas_size": h_disc["canvas_size"],
                    "parts": [
                        {
                            "id": p["id"],
                            "layer": p["layer"],
                            "parent": p["parent"],
                            "pivot": p["pivot"],
                            "rotation": p["rotation"],
                            "full_layer_path": str(hopper_png),
                        }
                        for p in c_disc["parts_spec"]
                    ],
                }
                export_effectcraft_jsx_script(
                    rig_plan,
                    build_pose_library("custom_hero"),
                    jsx_out,
                    mode="run",
                    speed=1.5,
                    duration=2.0,
                )
                self.assertTrue(jsx_out.is_file())
                content = jsx_out.read_text(encoding="utf-8")
                self.assertIn("Mode: run | Speed: 1.50x | Duration: 2.00s", content)
                self.assertIn("custom_hero_run_cutout", content)

    def test_glb_topology_parser_matches_loops_formula(self):
        glb_path = "/home/jesse/Work/omhas-animation/exports/felt_bunny_explorer_blender.glb"
        if os.path.exists(glb_path):
            topo = parse_glb_topology(glb_path)
            self.assertEqual(topo["expected_loops"], topo["total_tris"] * 3)
            self.assertGreater(topo["mesh_count"], 0)


if __name__ == "__main__":
    unittest.main()

