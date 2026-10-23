import importlib.util
from pathlib import Path
import unittest
import xml.etree.ElementTree as ET

path=Path(__file__).with_name('mlt_animation_worker.py')
spec=importlib.util.spec_from_file_location('mlt_worker',path)
worker=importlib.util.module_from_spec(spec) if path.exists() else None
if worker: spec.loader.exec_module(worker)

class AssemblyTests(unittest.TestCase):
    def test_timeline_preserves_inclusive_cuts_and_explicit_audio_policy(self):
        self.assertIsNotNone(worker,'Reusable MLT timeline compiler absent')
        doc={'width':64,'height':48,'fps':[24,1],'clips':[{'path':'a.mp4','in':2,'out':13},{'path':'b.mp4','in':0,'out':11}],
             'audio':{'path':'tone.wav','in':0,'out':23,'start':0}}
        with self.assertRaisesRegex(ValueError,'audio_mode'):worker.validate_timeline(doc)
        doc['audio_mode']='replace'
        dimensions=worker.validate_timeline(doc);self.assertEqual(dimensions,(64,48,24,1,24))
        xml=ET.fromstring(worker.build_xml(doc,['a & b.mp4','second.mp4','tone.wav'],*dimensions))
        entries=xml.find("playlist[@id='picture']").findall('entry')
        self.assertEqual([(e.get('in'),e.get('out')) for e in entries],[('2','13'),('0','11')])
        self.assertEqual(xml.find("producer/property[@name='resource']").text,'a & b.mp4')
        self.assertEqual(xml.find("tractor").get('out'),'23')
        doc['audio']['start']=1
        with self.assertRaisesRegex(ValueError,'beyond'):worker.validate_timeline(doc)

if __name__=='__main__':unittest.main()
