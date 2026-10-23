from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from pipeline.production import spec as prod_spec

SCRIPT = """# SHOW — Episode 09: TEST

## 01 · Opening — 00:00–00:20

Some direction.

A: “Hello.”
B, off: “Hi there.”

**Show/teach:** stuff. Tiny chapter label: **Cut on action.** More notes.

## 02 · Song — 00:20–00:35

A, singing, original tune:
“Line one!
Line two!”

B: “Nice.”

Label: **Make space for dialogue.**
"""

LEDGER_TARGET = """| Chapter | Distinct planned contributions | Creative tools |
|---|---|---|
| 1. Opening | **1** first thing; **2** second thing | Blender |
| 2. Song | **3** third thing | Voicebox, SoundCraft |
"""


class TestProductionSpec(unittest.TestCase):
    def test_import_script_chapters_lines_lessons(self) -> None:
        chapters = prod_spec.import_script(SCRIPT)
        self.assertEqual([c.id for c in chapters], ["01", "02"])
        self.assertEqual(chapters[0].title, "Opening")
        self.assertEqual(chapters[0].lesson, "Cut on action.")
        self.assertEqual((chapters[0].start_s, chapters[0].end_s), (0.0, 20.0))
        lines = chapters[0].shots[0].lines
        self.assertEqual([(l.speaker, l.text, l.direction) for l in lines],
                         [("A", "Hello.", ""), ("B", "Hi there.", "off")])
        self.assertEqual([l.id for l in lines], ["01-01", "01-02"])

    def test_import_script_multiline_song_is_one_line(self) -> None:
        song = prod_spec.import_script(SCRIPT)[1].shots[0].lines[0]
        self.assertEqual(song.speaker, "A")
        self.assertEqual(song.direction, "singing, original tune")
        self.assertEqual(song.text, "Line one!\nLine two!")

    def test_assign_techniques_from_ledger_table(self) -> None:
        chapters = prod_spec.import_script(SCRIPT)
        prod_spec.assign_techniques(chapters, LEDGER_TARGET)
        self.assertEqual(chapters[0].shots[0].techniques, ["T01", "T02"])
        self.assertEqual(chapters[1].shots[0].techniques, ["T03"])
        self.assertEqual(chapters[1].tools, ["Voicebox", "SoundCraft"])

    def test_roundtrip_and_total_duration(self) -> None:
        chapters = prod_spec.import_script(SCRIPT)
        spec = prod_spec.Spec(title="T", fps=30, width=1920, height=1080, chapters=chapters)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "production.json"
            prod_spec.save_spec(spec, path)
            loaded = prod_spec.load_spec(path)
        self.assertEqual(loaded, spec)
        self.assertAlmostEqual(prod_spec.total_duration(loaded), 35.0)


if __name__ == "__main__":
    unittest.main()
