"""Receipt checks use a fake mc; they never render or contact a live Studio."""
import argparse
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from render_motion_recipe import inputs_for, render, reusable, run


class RenderReceiptTests(unittest.TestCase):
    def test_changed_images_and_outputs_cannot_reuse_a_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            image = root / 'part.png'
            image.write_bytes(b'original')
            spec = root / 'scene.json'
            spec.write_text(json.dumps({'layers': [{'file': 'part.png'}]}))
            command = {'id': 'export.layered_motion', 'params': {'spec': str(spec), 'out_dir': str(root)}}
            fingerprint, _ = inputs_for(command)
            receipt = {'inputs': fingerprint, 'outputs': {str(image): inputs_for(command)[1][str(image)]}}
            self.assertTrue(reusable(receipt, fingerprint))
            image.write_bytes(b'changed')
            self.assertNotEqual(inputs_for(command)[0], fingerprint)
            self.assertFalse(reusable(receipt, fingerprint))

    def test_failed_motion_gate_keeps_receipt_and_does_not_replace_current(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            spec = root / 'scene.json'
            spec.write_text('{"layers": []}')
            recipe = root / 'recipe.jsonl'
            recipe.write_text(json.dumps({'id': 'export.layered_motion',
                                         'params': {'spec': str(spec), 'out_dir': str(root / 'clips')}}))
            receipts = root / 'receipts'
            receipts.mkdir()
            current = receipts / 'current.json'
            current.write_text('{"previous": {"receipt": "keep"}}')
            response = argparse.Namespace(returncode=0, stderr='', stdout=json.dumps(
                {'ok': True, 'result': {'ok': True, 'validation_ok': False}}))
            with patch('render_motion_recipe.subprocess.run', return_value=response):
                self.assertTrue(run(argparse.Namespace(recipe=str(recipe), scene=None, force=False, mc='fake')))
            self.assertEqual(json.loads(current.read_text()), {'previous': {'receipt': 'keep'}})
            attempt = next(receipts.glob('*/receipt.json'))
            self.assertEqual(json.loads(attempt.read_text())['status'], 'failed')


if __name__ == '__main__':
    unittest.main()
