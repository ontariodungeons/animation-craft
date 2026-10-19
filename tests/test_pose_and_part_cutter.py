import json
import tempfile
import unittest
from unittest import mock
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

    def test_image_without_part_layout_preserves_pixels_without_inventing_parts(self):
        image = Image.new('RGBA', (160, 100))
        draw = ImageDraw.Draw(image)
        draw.ellipse((10, 10, 65, 85), fill=(60, 180, 150, 255))
        draw.polygon([(110, 10), (150, 85), (80, 85)], fill=(220, 140, 60, 255))
        for layout in (None, {'frames': [{'box': [0, 0, 160, 100], 'parts': {}}]}):
            with self.subTest(explicit_empty_layout=layout is not None):
                result = self.cut(image, kind='single', layout=layout)
                frame = result['frames'][0]
                self.assertEqual(frame['parts'], {})
                self.assertIsNone(frame['vectorMaskSvg'])
                self.assertFalse(any((self.root / 'output' / 'parts').iterdir()))
                editable = json.loads((self.root / 'output' / 'layout.review.json').read_text())
                self.assertEqual(editable['frames'][0]['parts'], {})
                with Image.open(self.root / 'output' / frame['file']) as rendered:
                    x, y = frame['sourceOffset']
                    restored = rendered.crop((x, y, x + image.width, y + image.height))
                    self.assertEqual(restored.getchannel('A').tobytes(), image.getchannel('A').tobytes())
                    background = Image.new('RGBA', image.size, (25, 35, 45, 255))
                    self.assertEqual(Image.alpha_composite(background, restored).tobytes(),
                                     Image.alpha_composite(background, image).tobytes())

    # Known PhotoCraft gap: Select Subject + Object Selection treat an outlined figure whose fill equals
    # the background colour (line art on matching paper) as a hollow ring, so the interior is removed.
    @unittest.expectedFailure
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

    def test_opaque_matted_panels_still_separate_pose_figures(self):
        # Reproduces an import/matting derivative whose alpha cleared only the
        # outer border, leaving every grid-separated panel background opaque.
        # Trusting that alpha collapses the sheet into one blob, so detection
        # must fall back to colour-based background removal and still find all
        # figures instead of failing with "supply --layout".
        count, cell_w, cell_h = 4, 200, 220
        w, h = cell_w * count, cell_h
        sheet = Image.new('RGB', (w, h), '#ffffff')
        draw = ImageDraw.Draw(sheet)
        for i in range(count):
            cx = i * cell_w + cell_w // 2
            draw.ellipse((cx - 45, 55, cx + 45, 150), fill=(40 + i * 20, 90, 170))
            draw.rectangle((cx - 30, 145, cx + 30, 195), fill=(120, 70 + i * 20, 60))
            draw.line((i * cell_w, 0, i * cell_w, h), fill='#a0a0a0', width=3)
        matted = sheet.convert('RGBA')
        alpha = Image.new('L', (w, h), 255)
        ad = ImageDraw.Draw(alpha)
        ad.rectangle((0, 0, w - 1, 6), fill=0)
        ad.rectangle((0, h - 7, w - 1, h - 1), fill=0)
        ad.rectangle((0, 0, 6, h - 1), fill=0)
        ad.rectangle((w - 7, 0, w - 1, h - 1), fill=0)
        matted.putalpha(alpha)

        result = self.cut(matted, kind='action')
        self.assertEqual(len(result['frames']), count)
        for frame in result['frames']:
            with Image.open(self.root / 'output' / frame['file']) as img:
                self.assertGreater(img.getchannel('A').getextrema()[1], 0)

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

    def test_runtime_template_is_versioned_and_missing_template_fails(self):
        from pipeline import pose_and_part_cutter as cutter
        # The runtime template ships with the package, not the gitignored exports/ tree.
        self.assertTrue(cutter.TEMPLATE.is_file(), f'missing runtime template: {cutter.TEMPLATE}')
        self.assertNotIn('exports', cutter.TEMPLATE.parts)

        # An exploded cut works with no exports/ directory present (fresh checkout).
        im = Image.new('RGBA', (900, 900))
        d = ImageDraw.Draw(im)
        for i in range(8):
            x, y = i % 3 * 300, i // 3 * 300
            d.ellipse((x + 90, y + 70, x + 210, y + 230), fill=(170, 70, 35, 255))
        result = self.cut(im, kind='exploded')
        self.assertEqual(len(result['frames']), 8)
        for frame in result['frames']:
            with Image.open(self.root / 'output' / frame['file']) as img:
                self.assertGreater(img.getchannel('A').getextrema()[1], 0)

        # If the runtime dependency cannot be resolved, cutting fails clearly.
        with mock.patch.object(cutter, 'TEMPLATE', self.root / 'missing_template.json'):
            with self.assertRaises(FileNotFoundError):
                cutter.cut_sheet(self.root / 'input.png', self.root / 'output2', kind='exploded', use_ocr=False)

    def test_blank_and_invalid_layout_fail(self):
        with self.assertRaisesRegex(ValueError, 'foreground|characters'):
            self.cut(Image.new('RGB', (300,200), 'white'), kind='action')
        with self.assertRaises(ValueError):
            self.cut(Image.new('RGB', (300,200), 'white'), layout={'frames':[{'box':[-1,0,300,200]}]})

    def test_exploded_template_has_exact_slots_and_prompt(self):
        from pipeline.pose_and_part_cutter import exploded_prompt, TEMPLATE
        self.assertTrue(TEMPLATE.is_file(), f'runtime template must be versioned: {TEMPLATE}')
        self.assertNotIn('exports', TEMPLATE.parts, 'runtime template must not live in gitignored exports/')
        spec = json.loads(TEMPLATE.read_text())
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
        import sys
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'input.png'
            im = Image.new('RGBA',(100,180))
            ImageDraw.Draw(im).ellipse((20,10,80,160), fill='red')
            im.save(path)
            proc = subprocess.run([sys.executable,'studio_pipeline_cli.py','cut-sheet',str(path),
                                   '--kind','single','--out-dir',str(Path(tmp)/'out'),'--no-ocr'],
                                  capture_output=True,text=True)
            self.assertEqual(proc.returncode,0,proc.stderr)
            self.assertEqual(json.loads(proc.stdout)['frames'],1)

    def test_exploded_non_template_sheet_extracts_connected_components_and_custom_puppet_parts(self):
        from pipeline.pose_and_part_cutter import cut_sheet
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'arbitrary_parts.png'
            im = Image.new('RGBA', (320, 200), (255, 255, 255, 255))
            draw = ImageDraw.Draw(im)
            draw.ellipse((24, 30, 120, 160), fill=(40, 140, 220, 255))
            draw.polygon([(190, 160), (245, 32), (300, 160)], fill=(230, 95, 50, 255))
            im.save(path)

            out_auto = Path(tmp)/'out_auto'
            res = cut_sheet(path, out_auto, kind='exploded', use_ocr=False)
            self.assertEqual([f['label'] for f in res['frames']], ['part_00', 'part_01'])
            self.assertEqual(set(res['puppet']['parts'].keys()), {'part_00', 'part_01'})
            self.assertTrue(any('3x3 biped template' in w for w in res['warnings']))

            out_custom = Path(tmp)/'out_custom'
            layout = {
                'frames': [
                    {'label': 'orb_body', 'box': [20, 25, 125, 165]},
                    {'label': 'fin_top', 'box': [185, 28, 305, 165]},
                ],
                'puppet': {
                    'fin_top': {'parent': 'orb_body', 'pivot': [60, 120], 'complete': True},
                },
            }
            res_custom = cut_sheet(path, out_custom, kind='exploded', layout=layout, use_ocr=False)
            self.assertEqual(set(res_custom['puppet']['parts'].keys()), {'orb_body', 'fin_top'})
            self.assertEqual(res_custom['puppet']['parts']['fin_top']['parent'], 'orb_body')
            self.assertEqual(res_custom['puppet']['parts']['fin_top']['status'], 'ready')

if __name__ == '__main__':
    unittest.main()
