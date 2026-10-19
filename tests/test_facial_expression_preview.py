import json
import stat
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import numpy as np
from PIL import Image, ImageDraw

from pipeline import facial_expression_preview as fep


def _make_face_png(path: Path, width: int = 240, height: int = 240) -> dict:
    """Create a synthetic character head PNG with transparent background and chin at y=195."""
    img = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Head oval ending at y=195 (leaving transparent space below chin)
    draw.ellipse([50, 35, 190, 195], fill=(240, 205, 175, 255), outline=(45, 35, 30, 255), width=3)
    # Eyes
    draw.ellipse([82, 92, 106, 112], fill=(30, 30, 35, 255))
    draw.ellipse([134, 92, 158, 112], fill=(30, 30, 35, 255))
    # Nose
    draw.polygon([(120, 118), (113, 138), (127, 138)], fill=(210, 165, 140, 255))
    # Mouth near lower face (y=164, close to chin at y=195)
    draw.arc([92, 152, 148, 174], start=10, end=170, fill=(150, 50, 55, 255), width=4)
    img.save(path)
    return {
        "rect": [50.0, 35.0, 140.0, 160.0],
        "landmarks": {
            "leftEye": [94.0, 102.0],
            "rightEye": [146.0, 102.0],
            "nose": [120.0, 132.0],
            "mouthLeft": [96.0, 164.0],
            "mouthRight": [144.0, 164.0],
        },
    }


class FacialExpressionPreviewTests(unittest.TestCase):
    def test_regions_inset_mouth_box_away_from_chin_silhouette(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            src = tmp_path / "head.png"
            face = _make_face_png(src)
            pixels = fep.rgba(src)
            alpha = pixels[:, :, 3]

            boxes, mouth = fep.regions(face, pixels.shape[1], pixels.shape[0], alpha)
            mouth_box = boxes[0]
            x0, y0, x1, y1 = mouth_box
            # Chin silhouette ends at y=195; mouth_box bottom y1 must stay well above y=195
            self.assertLessEqual(y1, 189.0, f"Mouth box bottom {y1} is too close to chin at 195")
            # Every pixel along the mouth box vertical and horizontal centerlines must be opaque
            mx, my, mw, mh = mouth
            self.assertGreater(alpha[int(round(my)), int(round(mx))], 0)
            self.assertLessEqual(my + mh, y1 + 1e-4)

            # Verify every stroke point lies inside one of the bounded region boxes
            for expr in fep.EXPRESSIONS:
                strokes, _ = fep.strokes_for(2, expr, 80, 40, 20, face, mouth)
                self.assertTrue(strokes)
                for stroke in strokes:
                    for px, py in stroke["points"]:
                        in_any = any(
                            bx0 <= px <= bx1 and by0 <= py <= by1
                            for (bx0, by0, bx1, by1) in boxes
                        )
                        self.assertTrue(in_any, f"Point ({px}, {py}) outside all region boxes {boxes}")

    def test_validate_face_rejects_inverted_geometry(self):
        bad_face = {
            "rect": [40, 40, 120, 120],
            "landmarks": {
                "leftEye": [70, 130],
                "rightEye": [130, 130],
                "nose": [100, 100],
                "mouthLeft": [75, 80],
                "mouthRight": [125, 80],
            },
        }
        with self.assertRaises(ValueError):
            fep.validate_face(bad_face, 200, 200)

    def test_render_rejects_no_op_and_preserves_chin_outline_with_fake_photocraft(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            src = tmp_path / "head.png"
            face = _make_face_png(src)
            face_json = tmp_path / "face.json"
            face_json.write_text(json.dumps(face))

            # Create a deterministic fake photocraft-cli supporting `run <source> --cmd ... --params ... [--out <out>]`
            fake_cli = tmp_path / "photocraft-cli"
            fake_cli.write_text(
                """#!/usr/bin/env python3
import json, sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw

if "--version" in sys.argv:
    print("photocraft-cli 0.10.0-test")
    sys.exit(0)

assert sys.argv[1] == "run"
src_path = Path(sys.argv[2])
img = Image.open(src_path).convert("RGBA")
arr = np.array(img)
mask = Image.new("L", img.size, 0)
draw = ImageDraw.Draw(mask)

has_liquify = False
out_path = None
i = 3
while i < len(sys.argv):
    arg = sys.argv[i]
    if arg == "--cmd":
        cmd = sys.argv[i + 1]
        params = json.loads(sys.argv[i + 3])
        i += 4
        if cmd == "select.lasso":
            pts = [tuple(p) for p in params["points"]]
            draw.polygon(pts, fill=255)
        elif cmd == "filter.liquify":
            has_liquify = True
        print(json.dumps({"command": cmd, "result": {"ok": True}}))
    elif arg == "--out":
        out_path = Path(sys.argv[i + 1])
        i += 2
    else:
        i += 1

if out_path is not None:
    if has_liquify:
        m = np.array(mask) > 0
        interior = m & (arr[:, :, 3] == 255)
        arr[interior, 0] = np.clip(arr[interior, 0].astype(int) - 18, 0, 255).astype(np.uint8)
    Image.fromarray(arr, "RGBA").save(out_path)
"""
            )
            fake_cli.chmod(fake_cli.stat().st_mode | stat.S_IEXEC)

            out_dir = tmp_path / "out"
            with mock.patch.object(fep, "require_craft_cli", return_value=str(fake_cli)):
                # No-op settings (smile baseline at 20% opening, 0 blink, 0 gaze) must raise ValueError
                with self.assertRaises(ValueError):
                    fep.render(src, out_dir, "smile", 20, 0, 0, face_json=face_json)

                res = fep.render(src, out_dir, "smile", 55, 35, 15, face_json=face_json)
                self.assertEqual(res["validation"]["status"], "changed_only_inside_regions")
                self.assertEqual(res["validation"]["outline_pixels_changed_per_frame"], [0])
                self.assertGreater(res["validation"]["changed_pixels_per_frame"][0], 0)
                self.assertEqual(res["warnings"], [])


if __name__ == "__main__":
    unittest.main()
