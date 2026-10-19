import json
import tempfile
import unittest
from pathlib import Path
from PIL import Image, ImageDraw


class TestPoseAndPartCutter(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def cut(self, image, **kwargs):
        from pipeline.pose_and_part_cutter import cut_sheet
        path = self.root / 'input.png'
        image.save(path)
        return cut_sheet(path, self.root / 'output', use_ocr=False, **kwargs)

    def test_two_rows_keep_all_ten_characters_and_pale_interiors(self):
        im = Image.new('RGB', (1000, 500), '#f5efdf')
        d = ImageDraw.Draw(im)
        for row in range(2):
            for col in range(5):
                x, y = 40 + col * 190, 40 + row * 230
                d.ellipse((x, y, x+80, y+155), fill='#f5efdf', outline='#453024', width=8)
                d.rectangle((x+25,y+85,x+55,y+130), fill='#e06020')
        result = self.cut(im, kind='action')
        self.assertEqual(len(result['frames']), 10)
        self.assertEqual(len({tuple(f['size']) for f in result['frames']}), 1)
        frame = Image.open(self.root / 'output' / result['frames'][0]['file'])
        self.assertEqual(frame.getpixel((0,0))[3], 0)
        self.assertGreater(frame.getchannel('A').getextrema()[1], 0)
        bbox = frame.getchannel('A').getbbox()
        self.assertEqual(frame.getpixel(((bbox[0]+bbox[2])//2, bbox[1]+35))[3], 255)
        self.assertEqual(result['partsStatus'], 'needs_review')
        self.assertFalse(result['animation']['continuousCycle'])

    def test_five_turnaround_views(self):
        im = Image.new('RGBA', (1000, 400))
        d = ImageDraw.Draw(im)
        for i in range(5):
            d.rectangle((30+i*195,40,130+i*195,360), fill=(190,80,40,255))
        result = self.cut(im, kind='turnaround')
        self.assertEqual(len(result['frames']), 5)
        self.assertEqual(result['frames'][2]['label'], 'side')
        self.assertEqual(result['frames'][0]['label'], 'front')

    def test_explicit_masks_and_pivots_reconstruct_visible_pixels(self):
        im = Image.new('RGBA', (200, 240))
        ImageDraw.Draw(im).rectangle((40,30,160,220), fill=(120,80,40,255))
        layout = {'frames': [{'label': 'standing', 'box': [20,20,180,230],
                   'parts': {'head': {'polygon': [[40,30],[161,30],[161,100],[40,100]],
                                     'pivot': [100,95], 'complete': True}}}]}
        result = self.cut(im, kind='action', layout=layout)
        part = result['frames'][0]['parts']['head']
        self.assertEqual(part['status'], 'ready')
        self.assertEqual(part['sourcePivot'], [100,95])
        self.assertTrue((self.root / 'output' / part['file']).exists())
        self.assertEqual(result['frames'][0]['label'], 'standing')

    def test_interior_panel_background_and_grid_do_not_merge_views(self):
        im = Image.new('RGB', (1000,400), '#71905a')
        d = ImageDraw.Draw(im)
        d.rectangle((8,8,992,392), fill='#eadac2')
        for x in range(20,990,40):
            d.line((x,10,x,390), fill='#bcb7a2', width=1)
        for y in range(20,390,40):
            d.line((10,y,990,y), fill='#bcb7a2', width=1)
        for i in range(5):
            d.ellipse((35+i*195,70,135+i*195,350), fill='#c56475')
        result = self.cut(im, kind='turnaround')
        self.assertEqual(len(result['frames']), 5)

    def test_explicit_exclusion_removes_caption_without_erasing_body(self):
        im = Image.new('RGBA',(200,240))
        d=ImageDraw.Draw(im)
        d.rectangle((40,30,160,200),fill='red')
        d.rectangle((40,210,160,230),fill='black')
        result=self.cut(im,kind='single',layout={'frames':[{'box':[0,0,200,240],
            'excludePolygons':[[[0,205],[200,205],[200,240],[0,240]]]}]})
        f=result['frames'][0]
        with Image.open(self.root/'output'/f['file']) as cut:
            ox,oy=f['sourceOffset']
            self.assertEqual(cut.getpixel((ox+80,oy+220))[3],0)
            self.assertEqual(cut.getpixel((ox+80,oy+100))[3],255)

    def test_existing_soft_alpha_and_tight_opaque_crop_are_preserved(self):
        im=Image.new('RGBA',(200,240))
        d=ImageDraw.Draw(im)
        d.rectangle((40,30,160,200),fill=(80,100,130,255))
        d.rectangle((35,30,39,200),fill=(80,100,130,8))
        result=self.cut(im,kind='single')
        f=result['frames'][0]
        with Image.open(self.root/'output'/f['file']) as cut:
            ox,oy=f['sourceOffset']
            self.assertEqual(cut.getpixel((ox+37,oy+100))[3],8)
        result=self.cut(im,kind='single',layout={'frames':[{'box':[50,40,150,190]}]})
        f=result['frames'][0]
        with Image.open(self.root/'output'/f['file']) as cut:
            ox,oy=f['sourceOffset']
            self.assertEqual(cut.getpixel((ox+50,oy+70)),(80,100,130,255))

    def test_blank_and_invalid_layout_fail(self):
        with self.assertRaisesRegex(ValueError, 'foreground|characters'):
            self.cut(Image.new('RGB', (300,200), 'white'), kind='action')
        with self.assertRaises(ValueError):
            self.cut(Image.new('RGB', (300,200), 'white'), layout={'frames':[{'box':[-1,0,300,200]}]})

    def test_exploded_template_has_exact_slots_and_prompt(self):
        from pipeline.pose_and_part_cutter import exploded_prompt
        spec = json.loads(Path('exports/templates/exploded_part_sheet_spec.json').read_text())
        self.assertEqual(len(spec['slots']), 8)
        prompt = exploded_prompt('Hopper', 'cream rabbit with orange vest')
        self.assertIn('Hopper', prompt)
        self.assertIn('arm_L', prompt)
        self.assertIn('hidden', prompt)

    def test_exploded_sheet_allows_absent_ears_and_prop(self):
        im = Image.new('RGBA', (900,900))
        d = ImageDraw.Draw(im)
        for i in [0,2,3,4,5,6]:
            x,y=i%3*300,i//3*300
            d.ellipse((x+90,y+70,x+210,y+230), fill=(170,70,35,255))
        result = self.cut(im, kind='exploded')
        self.assertEqual(len(result['frames']),6)
        self.assertEqual(set(result['missingParts']),{'ears','prop_backpack'})

    def test_exploded_sheet_produces_puppet_binding(self):
        im = Image.new('RGBA', (900,900))
        d = ImageDraw.Draw(im)
        for i in range(8):
            x,y=i%3*300,i//3*300
            d.ellipse((x+90,y+70,x+210,y+230), fill=(170,70,35,255))
        result = self.cut(im, kind='exploded')
        self.assertEqual(set(result['puppet']['parts']),
                         {'head','ears','torso','arm_L','arm_R','leg_L','leg_R','prop_backpack'})
        self.assertEqual(result['puppet']['parts']['head']['parent'], 'torso')
        self.assertEqual(result['puppet']['parts']['arm_L']['status'], 'needs_review')

    def test_exploded_sheet_uses_template_slot_labels(self):
        im = Image.new('RGBA', (900,900))
        d = ImageDraw.Draw(im)
        for i in range(8):
            x,y=i%3*300,i//3*300
            d.ellipse((x+90,y+70,x+210,y+230), fill=(170,70,35,255))
        result = self.cut(im, kind='exploded')
        self.assertEqual([f['label'] for f in result['frames']],
                         ['head','ears','torso','arm_L','arm_R','leg_L','leg_R','prop_backpack'])

class TestCutterCLI(unittest.TestCase):
    def test_unified_cut_sheet_command(self):
        import subprocess
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'input.png'
            im = Image.new('RGBA',(100,180))
            ImageDraw.Draw(im).ellipse((20,10,80,160), fill='red')
            im.save(path)
            proc = subprocess.run(['python3','studio_pipeline_cli.py','cut-sheet',str(path),
                                   '--kind','single','--out-dir',str(Path(tmp)/'out'),'--no-ocr'],
                                  capture_output=True,text=True)
            self.assertEqual(proc.returncode,0,proc.stderr)
            self.assertEqual(json.loads(proc.stdout)['frames'],1)

if __name__ == '__main__':
    unittest.main()
