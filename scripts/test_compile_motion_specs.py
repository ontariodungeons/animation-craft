import unittest

from compile_motion_specs import compose


class CompositionTests(unittest.TestCase):
    def test_scene_order_metadata_and_overrides_survive_without_changing_shared_layers(self):
        library = {'variants': {'rig': {'a': {'file': '../layers/a.png', 'pivot': [2, 3], 'parent': 'b'},
                                        'b': {'file': '../layers/b.png', 'pivot': [4, 5]}}}}
        scene = {'layers': ['rig/b', {'use': 'rig/a', 'override': {'parent': None, 'pivot': [6, 7]}}],
                 'tracks': [{'motion': {'a': {'rotation': [{'kind': 'oscillate', 'hz': 2}]}}}],
                 'alternates': [], 'canvas': [100, 100]}
        result = compose(scene, library)
        self.assertEqual([layer['id'] for layer in result['layers']], ['b', 'a'])
        self.assertEqual(result['layers'][1]['pivot'], [6, 7])
        self.assertNotIn('parent', result['layers'][1])
        self.assertEqual(result['alternates'], [])
        self.assertEqual(result['tracks'], scene['tracks'])
        self.assertEqual(library['variants']['rig']['a']['pivot'], [2, 3])
        with self.assertRaises(KeyError):
            compose({'layers': ['missing/a']}, library)


if __name__ == '__main__':
    unittest.main()
