import unittest
from deckcraft_template import build_plan,toc_entries,SIZE,SCENE_ART_REGION,validate_groups

class PortraitTemplateTests(unittest.TestCase):
    def setUp(self):
        self.groups=[dict(id=f'scene-{i}',kind='scene',word_ids=[f'w{i}'],span_start=i,span_end=i+1) for i in range(12)]
        self.pages=build_plan({},[dict(text='existing caption')]*12,self.groups)
    def test_practical_document_without_repeated_lyric_or_admin_pages(self):
        self.assertEqual(self.pages[0]['kind'],'overview')
        self.assertEqual(len(self.pages),16)
        self.assertEqual(sum(p['kind']=='toc' for p in self.pages),1)
        self.assertFalse({'lyrics','qc'} & {p['kind'] for p in self.pages})
        self.assertEqual([p['group'] for p in self.pages if p['kind']=='scene'],self.groups)
    def test_contents_excludes_itself_and_targets_actual_document_indices(self):
        entries=toc_entries(self.pages)
        self.assertEqual([i for i,_ in entries],[0]+list(range(2,16)))
        self.assertTrue(all(p['kind']!='toc' for _,p in entries))
        self.assertEqual(entries[3][1]['title'],'Scene 01')
        self.assertEqual(entries[3][0],4)
    def test_portrait_page_and_true_widescreen_picturewell(self):
        self.assertGreater(SIZE[1],SIZE[0]);x,y,w,h=SCENE_ART_REGION
        self.assertAlmostEqual(w/h,16/9);self.assertGreaterEqual(w,900)
        self.assertLessEqual(x+w,SIZE[0]);self.assertLessEqual(y+h,SIZE[1])
    def test_full_audio_tail_and_existing_word_order_remain_required(self):
        words=[dict(id=f'w{i}',text='word',start=i,end=i+1) for i in range(12)]
        validate_groups(self.groups,words,12)
        with self.assertRaises(ValueError):validate_groups(self.groups[:-1],words,12)

if __name__=='__main__':unittest.main()
