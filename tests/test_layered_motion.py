from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image

from pipeline import layered_motion


def _make_sparse_layer(
    path: Path,
    canvas: tuple[int, int],
    box: tuple[int, int, int, int],
    rgba=(255, 180, 40, 255),
) -> None:
    img = Image.new("RGBA", canvas, (0, 0, 0, 0))
    patch = Image.new("RGBA", (box[2] - box[0], box[3] - box[1]), rgba)
    img.paste(patch, (box[0], box[1]))
    img.save(path)


class TestLayeredMotion(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.tmp_path = Path(self.tmp.name)
        self.work = self.tmp_path / "work"
        self.work.mkdir(parents=True, exist_ok=True)

    def test_crop_layer_to_alpha_and_parented_coordinates(self) -> None:
        body_png = self.tmp_path / "body.png"
        wing_png = self.tmp_path / "wing-l.png"
        _make_sparse_layer(body_png, (400, 300), (150, 100, 250, 220))
        _make_sparse_layer(wing_png, (400, 300), (80, 90, 160, 150))

        spec_path = self.tmp_path / "spec.json"
        spec_data = {
            "canvas": [400, 300],
            "fps": 24,
            "layers": [
                {"id": "wing-l", "file": "wing-l.png", "pivot": [155, 120], "parent": "body"},
                {"id": "body", "file": "body.png", "pivot": [200, 160]},
            ],
            "tracks": [
                {
                    "id": "hover",
                    "duration": 1.0,
                    "motion": {
                        "body": {
                            "position": [
                                {"kind": "oscillate", "axis": 0, "amplitude": 30.0, "hz": 1.0}
                            ]
                        },
                        "wing-l": {
                            "position": [
                                {
                                    "kind": "keys",
                                    "keys": [[0.0, [0, 0]], [1.0, [-4, -2]]],
                                }
                            ],
                            "rotation": [
                                {"kind": "oscillate", "amplitude": 25.0, "hz": 8.0},
                                {"kind": "follow", "layer": "body", "gain": -0.05, "limit": 12.0},
                            ],
                        },
                    },
                }
            ],
        }
        spec_path.write_text(json.dumps(spec_data), encoding="utf-8")

        loaded = layered_motion.load_spec(spec_path, self.work)
        layers_by_id = loaded["layers"]

        # Tight bbox [150, 100, 250, 220] with pad=2 -> [148, 98, 252, 222]
        self.assertEqual(layers_by_id["body"]["crop_origin"], [148.0, 98.0])
        with Image.open(layers_by_id["body"]["path"]) as body_crop:
            self.assertEqual(list(body_crop.size), [104, 124])
        self.assertEqual(layers_by_id["wing-l"]["crop_origin"], [78.0, 88.0])
        with Image.open(layers_by_id["wing-l"]["path"]) as wing_crop:
            self.assertEqual(list(wing_crop.size), [84, 64])

        jsx = layered_motion.build_jsx(loaded, self.work / "out.ecproj", self.work, None)
        # Body anchor is pivot - crop_origin = [200 - 148, 160 - 98, 0] = [52, 62, 0]
        self.assertIn('L_hover_body.property("Anchor Point").setValue([52.0000,62.0000,0]);', jsx)
        self.assertIn('L_hover_body.property("Position").setValue([200.0000,160.0000,0]);', jsx)
        # Wing anchor is [155 - 78, 120 - 88, 0] = [77, 32, 0]
        self.assertIn('L_hover_wing_l.property("Anchor Point").setValue([77.0000,32.0000,0]);', jsx)
        # Parenting occurs before keyframing, and child local base_pos is child_pivot - parent_crop_origin = [155 - 148, 120 - 98] = [7, 22]
        parent_idx = jsx.index("L_hover_wing_l.parent = L_hover_body;")
        key_idx = jsx.index('L_hover_wing_l.property("Position").setValueAtTime(1.00000, [3.0000,20.0000,0]);')
        self.assertLess(parent_idx, key_idx)

    def test_velocity_x_includes_horizontal_oscillate_for_follow(self) -> None:
        terms = [
            {
                "kind": "oscillate",
                "axis": 0,
                "amplitude": 50.0,
                "hz": 1.0,
                "phase": 0.0,
                "envelope": [[0.0, 0.0], [0.5, 1.0], [2.0, 1.0]],
            }
        ]
        velocities = layered_motion.velocity_x(terms, 24.0, 2.0)
        # At t = 1.0s (frame 24), envelope is 1.0 and cos(2*pi*1.0) = 1.0 -> positive velocity > 250 px/s
        self.assertGreater(velocities[24], 250.0)

    def test_js_envelope_sorts_and_guards_duplicate_timestamps(self) -> None:
        expr = layered_motion.js_envelope({"envelope": [[1.0, 1.0], [0.0, 0.0], [1.0, 0.5]]})
        self.assertIn("var dt=K[i][0]-K[i-1][0];", expr)
        self.assertIn("dt>0?", expr)
        self.assertLess(expr.index("[0.0, 0.0]"), expr.index("[1.0, 1.0]"))

    def test_spec_validation_rejects_invalid_inputs_and_clamps_opacity(self) -> None:
        layer_png = self.tmp_path / "part.png"
        _make_sparse_layer(layer_png, (64, 64), (10, 10, 30, 30))

        bad_follow = self.tmp_path / "bad_follow.json"
        bad_follow.write_text(
            json.dumps(
                {
                    "canvas": [64, 64],
                    "fps": 24,
                    "layers": [{"id": "part", "file": "part.png", "pivot": [20, 20]}],
                    "tracks": [
                        {
                            "id": "t1",
                            "duration": 1.0,
                            "motion": {
                                "part": {
                                    "rotation": [
                                        {
                                            "kind": "follow",
                                            "layer": "missing_parent",
                                            "gain": 0.1,
                                            "limit": 10.0,
                                        }
                                    ]
                                }
                            },
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        loaded_bad_follow = layered_motion.load_spec(bad_follow, self.work)
        with self.assertRaisesRegex(ValueError, "unknown layer 'missing_parent'"):
            layered_motion.build_jsx(loaded_bad_follow, self.work / "out.ecproj", self.work, None)

        bad_track_layer = self.tmp_path / "bad_track_layer.json"
        bad_track_layer.write_text(
            json.dumps(
                {
                    "canvas": [64, 64],
                    "fps": 24,
                    "layers": [{"id": "part", "file": "part.png", "pivot": [20, 20]}],
                    "tracks": [
                        {
                            "id": "t1",
                            "duration": 1.0,
                            "motion": {"ghost": {"rotation": []}},
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "motion references unknown layer 'ghost'"):
            layered_motion.load_spec(bad_track_layer, self.work)

        clamp_spec = self.tmp_path / "clamp.json"
        clamp_spec.write_text(
            json.dumps(
                {
                    "canvas": [64, 64],
                    "fps": 24,
                    "layers": [{"id": "part", "file": "part.png", "pivot": [20, 20]}],
                    "tracks": [
                        {
                            "id": "t1",
                            "duration": 1.0,
                            "motion": {
                                "part": {
                                    "opacity": [
                                        {
                                            "kind": "keys",
                                            "keys": [[0.0, -120.0], [1.0, 50.0]],
                                        }
                                    ]
                                }
                            },
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        loaded = layered_motion.load_spec(clamp_spec, self.work)
        jsx = layered_motion.build_jsx(loaded, self.work / "out.ecproj", self.work, None)
        self.assertIn("setValueAtTime(0.00000, 0.0000);", jsx)
        self.assertIn("setValueAtTime(1.00000, 100.0000);", jsx)

    def test_extract_component_orders_left_to_right_and_guards_empty(self) -> None:
        pair_png = self.tmp_path / "pair.png"
        img = Image.new("RGBA", (100, 50), (0, 0, 0, 0))
        img.paste(Image.new("RGBA", (10, 10), (255, 0, 0, 255)), (10, 20))
        img.paste(Image.new("RGBA", (10, 10), (0, 255, 0, 255)), (70, 20))
        img.save(pair_png)

        left_out = layered_motion.extract_component(pair_png, 0, self.tmp_path / "left.png")
        right_out = layered_motion.extract_component(pair_png, 1, self.tmp_path / "right.png")
        self.assertEqual(layered_motion.alpha_bounds(left_out), (10, 20, 20, 30))
        self.assertEqual(layered_motion.alpha_bounds(right_out), (70, 20, 80, 30))

        empty_png = self.tmp_path / "empty.png"
        Image.new("RGBA", (32, 32), (0, 0, 0, 0)).save(empty_png)
        with self.assertRaisesRegex(ValueError, "has 0 separate piece"):
            layered_motion.extract_component(empty_png, 0, self.tmp_path / "none.png")

    def test_margin_shifts_comp_canvas_and_root_positions_while_preserving_parent_local_coords(self) -> None:
        body_png = self.tmp_path / "body_m.png"
        wing_png = self.tmp_path / "wing_m.png"
        _make_sparse_layer(body_png, (200, 200), (50, 50, 150, 150))
        _make_sparse_layer(wing_png, (200, 200), (40, 40, 80, 80))

        spec_path = self.tmp_path / "margin_spec.json"
        spec_path.write_text(
            json.dumps(
                {
                    "canvas": [200, 200],
                    "margin": [30, 40, 50, 60],
                    "fps": 30,
                    "layers": [
                        {"id": "wing", "file": "wing_m.png", "pivot": [60, 60], "parent": "body"},
                        {"id": "body", "file": "body_m.png", "pivot": [100, 100]},
                    ],
                    "tracks": [
                        {
                            "id": "fly",
                            "duration": 1.0,
                            "motion": {
                                "body": {"position": [{"kind": "keys", "keys": [[0.0, [0, 0]], [1.0, [10, -5]]]}]},
                                "wing": {"position": [{"kind": "keys", "keys": [[0.0, [0, 0]], [1.0, [-2, 3]]]}]},
                            },
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        loaded = layered_motion.load_spec(spec_path, self.work)
        self.assertEqual(loaded["canvas"], [200, 200])
        self.assertEqual(loaded["margin"], [30, 40, 50, 60])
        jsx = layered_motion.build_jsx(loaded, self.work / "margin.ecproj", self.work, None)
        # Comp size includes margin [200+30+50, 200+40+60] = [280, 300]
        self.assertIn('addComp("fly", 280, 300, 1.0, 1.0000, 30.0)', jsx)
        # Initial world Position adds margin [left=30, top=40]
        self.assertIn('L_fly_body.property("Position").setValue([130.0000,140.0000,0]);', jsx)
        self.assertIn('L_fly_wing.property("Position").setValue([90.0000,100.0000,0]);', jsx)
        # Root keyframe at t=1.0 is [100+30+10, 100+40-5] = [140, 135]
        self.assertIn('L_fly_body.property("Position").setValueAtTime(1.00000, [140.0000,135.0000,0]);', jsx)
        # Child keyframe at t=1.0 is relative to parent crop_origin [48, 48]: [60-48-2, 60-48+3] = [10, 15]
        self.assertIn('L_fly_wing.property("Position").setValueAtTime(1.00000, [10.0000,15.0000,0]);', jsx)

    def test_motion_validate_spec_lint_and_alpha_weighted_reference_stats(self) -> None:
        from pipeline import motion_validate

        spec_path = self.tmp_path / "lint_spec.json"
        spec_path.write_text(
            json.dumps(
                {
                    "fps": 24,
                    "tracks": [
                        {
                            "id": "fast_buzz",
                            "motion": {
                                "wing": {
                                    "rotation": [{"kind": "oscillate", "hz": 12.0, "amplitude": 20.0}]
                                }
                            },
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        lint = motion_validate.lint_spec(spec_path)
        self.assertEqual(len(lint), 1)
        self.assertEqual(lint[0]["track"], "fast_buzz")
        self.assertEqual(lint[0]["level"], "warning")

        # Alpha-weighted mean RGB should weight opaque core pixels (200, 100, 50, a=255)
        # far higher than dark low-alpha fringe pixels (0, 0, 0, a=20).
        ref_png = self.tmp_path / "ref.png"
        img = Image.new("RGBA", (20, 20), (0, 0, 0, 0))
        img.paste(Image.new("RGBA", (10, 10), (0, 0, 0, 20)), (2, 2))
        img.paste(Image.new("RGBA", (6, 6), (200, 100, 50, 255)), (4, 4))
        img.save(ref_png)
        rgb, _size = motion_validate.reference_stats(ref_png)
        self.assertGreater(rgb[0], 170.0)


if __name__ == "__main__":
    unittest.main()

