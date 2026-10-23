"""Generic native-source boundary regressions; no production assets or renderer mocks."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from PIL import Image

spec = importlib.util.spec_from_file_location('native_worker', Path(__file__).with_name('native_animation_worker.py'))
worker = importlib.util.module_from_spec(spec) if spec and Path(spec.origin).exists() else None
if worker:
    spec.loader.exec_module(worker)

class NativeWorkerTests(unittest.TestCase):
    def test_snapshot_freezes_dependencies_and_rejects_escape(self):
        self.assertIsNotNone(worker, 'Native snapshot boundary is not implemented')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); bundle = root/'bundle'; bundle.mkdir()
            image = bundle/'art.png'; Image.new('RGBA', (2, 2), (255,0,0,100)).save(image)
            source = bundle/'scene.sif'
            source.write_text('<canvas width="16" height="12" fps="24" begin-time="0f" end-time="23f"><layer type="import"><param name="filename"><string>art.png</string></param></layer></canvas>')
            frozen = worker.snapshot_synfig({'source':str(source), 'bundle':str(bundle)}, root/'job')
            original = Path(frozen['path']).read_bytes()
            source.write_text('<canvas/>')
            image.write_bytes(b'changed')
            self.assertEqual(Path(frozen['path']).read_bytes(), original)
            self.assertNotEqual((Path(frozen['path']).parent/'art.png').read_bytes(), b'changed')
            source.write_text('<canvas><layer><param name="filename"><string>../outside.png</string></param></layer></canvas>')
            with self.assertRaisesRegex(ValueError, 'outside|escape'):
                worker.snapshot_synfig({'source':str(source), 'bundle':str(bundle)}, root/'bad')

    def test_frame_collection_rejects_gap_and_preserves_alpha(self):
        self.assertIsNotNone(worker, 'Native frame validation is not implemented')
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); frames=root/'frames'; frames.mkdir()
            Image.new('RGBA',(16,12),(255,0,0,100)).save(frames/'frame.0000.png')
            Image.new('RGBA',(16,12),(0,0,255,0)).save(frames/'frame.0002.png')
            with self.assertRaisesRegex(ValueError, 'sequence|count|gap'):
                worker.collect_frames(frames, 3, 16, 12)
            Image.new('RGBA',(16,12),(0,255,0,255)).save(frames/'frame.0001.png')
            result=worker.collect_frames(frames,3,16,12)
            self.assertEqual(result['alpha_evidence']['min'],0)
            self.assertEqual(result['alpha_evidence']['max'],255)
            self.assertEqual(len(result['frames']),3)
            (frames/'frame.0001.png').write_bytes(b'not PNG')
            with self.assertRaises(Exception):
                worker.collect_frames(frames,3,16,12)

if __name__ == '__main__': unittest.main()
