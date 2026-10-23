"""Generic native-source boundary regressions; no production assets or renderer mocks."""
import importlib.util
from pathlib import Path
import tempfile
import sys
import importlib
sys.path.insert(0,str(Path(__file__).parent))
sys.modules["worker"]=importlib.import_module("blender_animation_worker")
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
            (bundle/'scenes').mkdir()
            source = bundle/'scenes/scene.sif'
            source.write_text('<canvas width="16" height="12" fps="24" begin-time="0f" end-time="23f"><layer type="import"><param name="filename"><string>../art.png</string></param></layer></canvas>')
            frozen = worker.snapshot_synfig({'source':str(source), 'bundle':str(bundle)}, root/'job')
            original = Path(frozen['path']).read_bytes()
            self.assertEqual(Path(frozen['bundle']), root/'job/source-bundle')
            source.write_text('<canvas/>')
            image.write_bytes(b'changed')
            self.assertEqual(Path(frozen['path']).read_bytes(), original)
            self.assertNotEqual((Path(frozen['bundle'])/'art.png').read_bytes(), b'changed')
            source.write_text('<canvas><layer><param name="filename"><string>../../outside.png</string></param></layer></canvas>')
            with self.assertRaisesRegex(ValueError, 'outside|escape'):
                worker.snapshot_synfig({'source':str(source), 'bundle':str(bundle)}, root/'bad')

    def test_native_xml_self_closing_tags_and_quoted_relative_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'scene.tnz'
            for xml in [
                '<project><folders><folder name="drawings" path="drawings"/></folders></project>',
                '<tnz><level id="1"/><level id="2" /></tnz>',
                '<tnz><path>"+drawings/a b.png"</path></tnz>',
                "<tnz><path>'drawings/a.png'</path></tnz>",
            ]:
                with self.subTest(xml=xml):
                    path.write_text(xml)
                    worker.validate_native_references(path)
                    self.assertEqual(path.read_text(),xml)

    def test_native_xml_rejects_absolute_and_escaping_resource_values(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'scene.tnz'
            for value in ['/outside.png', '../outside.png', 'drawings/../../outside.png',
                          'C:/outside.png', r'C:\outside.png', r'\\host\share\a.png']:
                for xml in [f'<tnz><path>"{value}"</path></tnz>',
                            f'<project><folder path="{value}"/></project>']:
                    with self.subTest(xml=xml):
                        path.write_text(xml)
                        with self.assertRaisesRegex(ValueError,'Absolute/escaping'):
                            worker.validate_native_references(path)

    def test_native_xml_rejects_entity_definitions(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'scene.tnz'
            path.write_text('<!DOCTYPE tnz [<!ENTITY path "../outside.png">]><tnz><path>&path;</path></tnz>')
            with self.assertRaisesRegex(ValueError,'externally defined'):
                worker.validate_native_references(path)

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

    def test_ordered_native_manifest_encodes_real_alpha_without_renderer(self):
        self.assertTrue(hasattr(worker, 'encode_frames'), 'Shared native encoding boundary is absent')
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); frames=root/'frames'; frames.mkdir()
            paths=[]
            for i, colour in enumerate([(255,0,0,255),(0,255,0,128)]):
                path=frames/f'frame.{i+1:04}.png'
                picture=Image.new('RGBA',(16,16),(0,0,0,0)); picture.paste(colour,(4,4,12,12));picture.save(path);paths.append(path)
            result=worker.encode_frames({'ffmpeg':'ffmpeg'},root,paths,12,16,16)
            self.assertEqual(result['alpha_evidence']['decoded_frames'],2)
            self.assertEqual(result['alpha_evidence']['clip_min'],0)
            self.assertTrue(Path(result['clip']).is_file())

if __name__ == '__main__': unittest.main()
