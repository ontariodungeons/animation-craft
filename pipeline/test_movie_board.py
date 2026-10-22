import importlib.util,json,tempfile,unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('movie_board',Path(__file__).with_name('movie_board.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class CanonicalSceneMap(unittest.TestCase):
 def test_preserves_occurrences_corrections_and_instrumental_intervals(self):
  with tempfile.TemporaryDirectory() as folder:
   file=Path(folder)/'map.json'
   packets=[dict(target='scene-a',start=.125,end=1.75,lines=[dict(start=.125,lyric='repeat',heard='raw')]),dict(target='scene-b',start=1.75,end=2.25,lines=[dict(start=1.75,lyric='repeat changed')]),dict(target='tail',start=2.25,end=4.875,lines=[])]
   file.write_text(json.dumps(dict(scenes=packets)));result=module.canonical_scenes(file)
   self.assertEqual([s['id'] for s in result],[p['target'] for p in packets]);self.assertEqual([s['end_seconds'] for s in result],[p['end'] for p in packets])
   self.assertEqual([s['lyric_text'] for s in result],['repeat','repeat changed','']);self.assertEqual(result[0]['lines'][0]['heard'],'raw')
 def test_rejects_overlapping_or_duplicate_targets(self):
  with tempfile.TemporaryDirectory() as folder:
   file=Path(folder)/'map.json'
   for packets in [[dict(target='a',start=0,end=2),dict(target='b',start=1,end=3)],[dict(target='a',start=0,end=1),dict(target='a',start=1,end=2)]]:
    file.write_text(json.dumps(dict(scenes=packets)))
    with self.assertRaises(ValueError):module.canonical_scenes(file)
if __name__=='__main__':unittest.main()
