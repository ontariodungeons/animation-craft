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
class SourceFieldMapping(unittest.TestCase):
 def test_maps_known_headings_without_rewriting_and_reports_leftovers(self):
  roles=['user.scene.action','user.scene.continuity','user.scene.image_prompt','user.scene.animation_prompt','user.scene.animated_text','user.scene.drawing']
  packets=[dict(id='scene-1',notes=['Scene plan: Keep both fish visible.','Imported status: draft'],sections={'Reference prompt':'Exact image words.\nKeep this line.','Layer stack':'- audio\n- fish','Animation prompt':'Turn, then settle.','Skills':'existing-skill','Mystery':'Do not drop this.'})]
  content,report=module.normalize_scene_content(packets,roles)
  self.assertEqual(content['scene-1']['sections']['action'],'Keep both fish visible.')
  self.assertEqual(content['scene-1']['sections']['reference_prompt'],'Exact image words.\nKeep this line.')
  self.assertEqual(content['scene-1']['source_sections']['Mystery'],'Do not drop this.')
  self.assertEqual(report['unmapped'][0]['text'],'Do not drop this.')
  self.assertEqual(report['expected_fields'],roles)
  self.assertEqual(content, module.normalize_scene_content(packets,roles)[0])
class RawTranscriptDraft(unittest.TestCase):
 def test_overrun_is_explicit_and_raw_timing_unchanged(self):
  import sys
  sys.path.insert(0,str(Path(__file__).parent))
  import deckcraft_template as d
  words=[dict(id='w0',text='by',start=118.92,end=119.04)]
  with self.assertRaises(ValueError):d.validate_words(words,118.933333)
  d.validate_words(words,118.933333,True)
  self.assertEqual(words,[dict(id='w0',text='by',start=118.92,end=119.04)])
  with self.assertRaises(ValueError):d.validate_words([dict(id='w0',text='wrong',start=120,end=121)],118.933333,True)
 def test_pdf_fields_use_template_boxes_and_exact_wording(self):
  import sys
  sys.path.insert(0,str(Path(__file__).parent))
  import deckcraft_template as d
  fields=d.pdf_fields([dict(shapes=[dict(altText='user.scene.action',id=9,box=[10,20,300,40],text='Unchanged wording.')])],[dict(kind='scene')])
  self.assertEqual(fields[0]['rect'],[10,20,300,40]);self.assertEqual(fields[0]['value'],'Unchanged wording.');self.assertNotIn('.',fields[0]['name'])
class AssetSheets(unittest.TestCase):
 def test_category_sheets_repeat_four_slots_without_losing_assets(self):
  import sys
  sys.path.insert(0,str(Path(__file__).parent))
  import deckcraft_template as d
  assets=[dict(id=str(i),category='Characters' if i<9 else 'Props') for i in range(11)]
  pages=d.sheet_pages('assets',assets,'Assets')
  self.assertEqual([len(p['sheet']) for p in pages],[4,4,1,2])
  self.assertEqual([a['id'] for p in pages for a in p['sheet']],[a['id'] for a in assets])
  self.assertEqual(len({p['key'] for p in pages}),4)
if __name__=='__main__':unittest.main()
