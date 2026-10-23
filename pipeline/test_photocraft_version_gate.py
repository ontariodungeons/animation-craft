"""Exercise the real discovery function without importing optional image packages."""
import ast
from pathlib import Path
import re
import subprocess
import unittest
from unittest.mock import patch

class PhotoCraftVersionGate(unittest.TestCase):
    def accepted(self, banner, code=0):
        source = Path('pipeline/pose_and_part_cutter.py').read_text()
        node = next(n for n in ast.parse(source).body if isinstance(n, ast.FunctionDef) and n.name == '_find_photocraft_cli')
        namespace = {'find_craft_cli': lambda _: '/configured/photocraft-cli', 'subprocess': subprocess, 're': re}
        exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual PhotoCraft discovery>', 'exec'), namespace)
        with patch.object(subprocess, 'run', return_value=subprocess.CompletedProcess([], code, banner, '')):
            return namespace['_find_photocraft_cli']()

    def test_supported_newer_releases(self):
        for version in ['0.10.0', '0.11.4', '0.12.0', '0.13.0', '1.0.0']:
            with self.subTest(version=version):
                self.assertEqual(self.accepted(f'photocraft-cli {version} (abcdef, 2026-10-21)'), '/configured/photocraft-cli')

    def test_unrelated_build_text_and_failed_probe_are_not_versions(self):
        for banner in ['photocraft-cli 0.9.0 (build 1.99.0)', 'unexpected 1.0.0', 'photocraft-cli unknown']:
            with self.subTest(banner=banner):
                with self.assertRaisesRegex(RuntimeError, 'PHOTOCRAFT_CLI'):
                    self.accepted(banner)
        with self.assertRaisesRegex(RuntimeError, 'PHOTOCRAFT_CLI'):
            self.accepted('photocraft-cli 0.12.0', 1)

if __name__ == '__main__':
    unittest.main()
