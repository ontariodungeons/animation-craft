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
 def test_overview_fields_fill_the_existing_card(self):
  import deckcraft_template as d
  shapes=[dict(altText='user.story',id=1,box=[78,276,924,25.2],text='A story.'),dict(altText='layout.card.story',box=[60,220,960,280])]
  fields=d.pdf_fields([dict(shapes=shapes)],[dict(kind='overview')])
  self.assertEqual(fields[0]['rect'],[78,276,924,206])

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
class ChorusImport(unittest.TestCase):
 def test_moves_only_a_cut_that_splits_the_next_source_line(self):
  import copy
  with tempfile.TemporaryDirectory() as folder:
   root=Path(folder);(root/'artifacts').mkdir()
   (root/'artifacts/scene_plan.json').write_text(json.dumps(dict(scenes=[dict(script_section_id='ch2-l1,ch2-l2,break'),dict(script_section_id='v3-l1')])) )
   (root/'artifacts/script.json').write_text(json.dumps(dict(sections=[dict(id='ch2-l1',label='Chorus line 1',text='Hey ho fish, cry cry'),dict(id='ch2-l2',label='Chorus line 2',text='Hey ho fish, whale by by'),dict(id='v3-l1',label='Verse 3 line 1',text='You just swim around')])) )
   texts='Hey ho fish, cry cry Hey ho fish, whale by by You just swim around'.split()
   words=[dict(id=f'w{i}',text=t,start=i,end=i+.5) for i,t in enumerate(texts)];before=copy.deepcopy(words)
   packets=[dict(id='a',start_seconds=0,end_seconds=13),dict(id='b',start_seconds=13,end_seconds=15)]
   report=module.align_imported_sections(packets,words,root)
   self.assertEqual(packets[0]['end_seconds'],11);self.assertEqual(packets[1]['start_seconds'],11)
   self.assertEqual(packets[0]['kind'],'chorus');self.assertEqual(words,before)
   self.assertEqual(report[0]['convention'],'next source line onset; half-open intervals')
   spans=[[w['id'] for w in words if p['start_seconds']<=w['start']<p['end_seconds']] for p in packets]
   self.assertEqual(sum(spans,[]),[w['id'] for w in words])
 def test_four_panels_preserve_raw_words_and_timestamps(self):
  texts="Hey ho, little fish, don't cry, don't cry Hey ho, little fish, you'll be a whale by and by".split()
  words=[dict(id=f'w{i}',text=t,start=i,end=i+.5) for i,t in enumerate(texts)]
  panels=module.chorus_phrases(words)
  self.assertEqual(len(panels),4)
  self.assertEqual([w for p in panels for w in p['word_ids']],[w['id'] for w in words])
  self.assertEqual(panels[-1]['end'],18.5)
 def test_lyrics_keep_scene_font_even_when_long(self):
  import sys
  sys.path.insert(0,str(Path(__file__).parent));import deckcraft_template as d
  fields=d.pdf_fields([dict(shapes=[dict(altText='auto.scene.lyrics',id=9,box=[80,838,920,90],text='lyrics '*25)])],[dict(kind='scene')])
  self.assertEqual(fields[0]['font_size'],27)

class ExplicitDoctor(unittest.TestCase):
 def test_different_song_reports_spillover_and_never_changes_inputs(self):
  with tempfile.TemporaryDirectory() as folder:
   root=Path(folder);(root/'artifacts').mkdir()
   (root/'artifacts/scene_plan.json').write_text(json.dumps(dict(scenes=[dict(script_section_id='refrain'),dict(script_section_id='verse')])))
   (root/'artifacts/script.json').write_text(json.dumps(dict(sections=[dict(id='refrain',label='Chorus line 1',text='Repeat the refrain'),dict(id='verse',label='Verse line 1',text='Now we walk away')])))
   words=[dict(id=f'token-{i}',text=t,start=10+i,end=10.5+i) for i,t in enumerate('Repeat the refrain Now we walk away'.split())]
   transcript=root/'native.json';transcript.write_text(json.dumps(words))
   manifest=root/'board.json';manifest.write_text(json.dumps(dict(source_transcript=str(transcript),page_plan=[dict(group=dict(id='opening-refrain',span_start=0,span_end=15)),dict(group=dict(id='next-verse',span_start=15,span_end=20))])))
   before={p:p.read_bytes() for p in [manifest,transcript,*list((root/'artifacts').iterdir())]}
   r=module.doctor(manifest,root);self.assertFalse(r['healthy']);task=r['tasks'][0]
   self.assertEqual((task['scene_id'],task['page_id']),('opening-refrain',1));self.assertEqual(task['source_word_ids'],['token-3','token-4']);self.assertEqual(task['expected']['boundary'],13)
   self.assertTrue(all(p.read_bytes()==b for p,b in before.items()))
 def test_missing_section_evidence_is_unassessed_not_clear(self):
  with tempfile.TemporaryDirectory() as folder:
   root=Path(folder);manifest=root/'board.json';manifest.write_text('{}')
   r=module.doctor(manifest,root);self.assertIsNone(r['healthy']);self.assertEqual(r['status'],'insufficient_source_evidence');self.assertTrue(r['diagnostic_only'])

class SourceLineIds(unittest.TestCase):
 def test_keeps_supplied_ids_breaks_and_native_words(self):
  words=[dict(id='a',text='Hello,',start=1,end=2),dict(id='b',text='fish',start=2,end=3),dict(id='c',text='Keep',start=4,end=5),dict(id='d',text='swimming.',start=5,end=6)]
  lines=[dict(id='verse-A',text='Hello fish',source='linked-native.js'),dict(id='verse-B',text='Keep swimming',source='linked-native.js')]
  mapped=module.source_line_breaks(words,lines)
  self.assertEqual([l['id'] for l in mapped],['verse-A','verse-B']);self.assertEqual([l['text'] for l in mapped],['Hello, fish','Keep swimming.']);self.assertEqual([i for l in mapped for i in l['word_ids']],['a','b','c','d'])
  self.assertEqual(module.source_line_breaks(words+[dict(id='extra',text='unknown',start=7,end=8)],lines),[],"unmapped words must not be silently dropped")

if __name__=='__main__':unittest.main()
