import json
import struct
import tempfile
import unittest
from pathlib import Path

from PIL import Image, ImageDraw

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
        from pipeline.guided_cutout_rig_builder import (
            discover_character_joints_and_polygons,
            build_pose_library,
            export_effectcraft_jsx_script,
        )

        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            source = tmp / "custom_hero.png"
            im = Image.new("RGBA", (160, 240), (0, 0, 0, 0))
            draw = ImageDraw.Draw(im)
            draw.ellipse((50, 20, 110, 90), fill=(220, 170, 120, 255))
            draw.rounded_rectangle(
                (55, 85, 105, 190), radius=18, fill=(80, 130, 200, 255)
            )
            draw.rectangle((58, 185, 72, 230), fill=(60, 90, 160, 255))
            draw.rectangle((88, 185, 102, 230), fill=(60, 90, 160, 255))
            im.save(source)

            discovery = discover_character_joints_and_polygons(
                source,
                character_id="custom_hero",
                selected_parts=["head", "torso", "leg_left", "leg_right"],
            )
            self.assertEqual(discovery["discovery_mode"], "silhouette_contour_adaptive")
            self.assertEqual(
                discovery["selected_parts"],
                ["head", "torso", "leg_left", "leg_right"],
            )
            self.assertGreater(len(discovery["parts_spec"]), 0)

            jsx_out = tmp / "custom_run.jsx"
            rig_plan = {
                "character_id": "custom_hero",
                "canvas_size": discovery["canvas_size"],
                "parts": [
                    {
                        "id": p["id"],
                        "layer": p["layer"],
                        "parent": p["parent"],
                        "pivot": p["pivot"],
                        "rotation": p["rotation"],
                        "full_layer_path": str(source),
                    }
                    for p in discovery["parts_spec"]
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
        with tempfile.TemporaryDirectory() as tmp:
            glb_path = Path(tmp) / "fixture.glb"
            triangle_count = 5
            gltf = {
                "asset": {"version": "2.0"},
                "meshes": [
                    {
                        "name": "Fixture",
                        "primitives": [
                            {"attributes": {"POSITION": 0}, "indices": 1}
                        ],
                    }
                ],
                "accessors": [
                    {"count": triangle_count * 3, "type": "VEC3", "componentType": 5126},
                    {"count": triangle_count * 3, "type": "SCALAR", "componentType": 5125},
                ],
            }
            payload = json.dumps(gltf).encode("utf-8")
            payload += b" " * ((4 - len(payload) % 4) % 4)
            with glb_path.open("wb") as f:
                f.write(struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(payload)))
                f.write(struct.pack("<II", len(payload), 0x4E4F534A))
                f.write(payload)

            topo = parse_glb_topology(str(glb_path))
            self.assertEqual(topo["mesh_count"], 1)
            self.assertEqual(topo["total_tris"], triangle_count)
            self.assertEqual(topo["expected_loops"], triangle_count * 3)


if __name__ == "__main__":
    unittest.main()

