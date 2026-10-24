"""Overlapping actors and alternate drawings have different visibility rules."""
import tempfile
from pathlib import Path
import unittest

from PIL import Image
from layered_motion import layer_checks


class AlternateTests(unittest.TestCase):
    def test_explicit_pairs_still_fail_but_distinct_actors_can_overlap(self):
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / 'layer.png'
            Image.new('RGBA', (8, 8), (10, 20, 30, 255)).save(image)
            layer = {'path': image, 'crop_origin': [0, 0]}
            spec = {'canvas': [8, 8], 'fps': 30, 'layers': {'a': layer, 'b': layer},
                    'tracks': [{'id': 'scene', 'duration': 1}]}
            self.assertTrue(layer_checks(spec)['issues'])
            spec['alternates'] = []
            self.assertFalse(layer_checks(spec)['issues'])
            spec['alternates'] = [['a', 'b']]
            self.assertEqual(layer_checks(spec)['issues'][0]['level'], 'error')
            spec['alternates'] = [['a', 'missing']]
            with self.assertRaisesRegex(ValueError, 'existing layer ids'):
                layer_checks(spec)


if __name__ == '__main__':
    unittest.main()
