"""Failure-path checks: source copies and invalid pin mappings cannot pass as deformations."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from PIL import Image
from pipeline.control_texture_preview import render
from pipeline.facial_expression_preview import render as render_face


class ControlTextureTests(unittest.TestCase):
    def test_tool_noop_is_rejected_and_source_preserved(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'source.png'
            Image.new('RGBA', (32, 32), (110, 170, 230, 255)).save(source)
            original = source.read_bytes()
            def noop(binary, src, commands, deadline, output):
                Path(output).write_bytes(Path(src).read_bytes())
                return []
            with patch('pipeline.control_texture_preview.require_craft_cli', return_value='unused'), patch('pipeline.control_texture_preview.tool_call', side_effect=noop):
                with self.assertRaises(ValueError):
                    render(source, root / 'out', {'pins': [{'src': [16, 16], 'dst': [20, 16]}]})
            self.assertEqual(source.read_bytes(), original)

    def test_off_canvas_pins_rejected_before_tool(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / 'source.png'
            Image.new('RGBA', (32, 32), (110, 170, 230, 255)).save(source)
            with patch('pipeline.control_texture_preview.require_craft_cli') as tool:
                with self.assertRaises(ValueError):
                    render(source, Path(folder) / 'out', {'pins': [{'src': [16, 16], 'dst': [40, 16]}]})
                tool.assert_not_called()

    def test_landmarks_alone_do_not_enable_fake_eye_motion(self):
        with self.assertRaises(ValueError):
            render_face('unused', 'unused', 'smile', 20, 100, 0)
        with self.assertRaises(ValueError):
            render_face('unused', 'unused', 'smile', 20, 0, 20)


if __name__ == '__main__':
    unittest.main()
