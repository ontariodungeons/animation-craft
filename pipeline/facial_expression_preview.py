"""Render raster facial previews using installed PhotoCraft detection and Liquify.

This deforms existing artwork; it does not synthesize teeth, infer phonemes from
sound, or establish that a detected face is correct. Source files are read-only.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import subprocess

from PIL import Image


def tool_call(binary, source, commands, output=None):
    args = [binary, 'run', str(source)]
    for command, params in commands:
        args += ['--cmd', command, '--params', json.dumps(params)]
    if output:
        args += ['--out', str(output)]
    result = subprocess.run(args, capture_output=True, text=True, timeout=45)
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip())
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
    if any(row.get('error') or row.get('result', {}) == {'ok': False} for row in rows):
        raise RuntimeError('PhotoCraft reported a failed operation')
    return rows


def render(source, output_root, expression, opening, blink, gaze):
    source = Path(source).resolve(strict=True)
    if expression not in {'smile', 'laugh', 'ooh', 'mbp', 'talk'}:
        raise ValueError('Unknown expression')
    if not all(math.isfinite(v) for v in (opening, blink, gaze)):
        raise ValueError('Face controls must be finite')
    opening, blink, gaze = max(0, min(opening, 100)), max(0, min(blink, 100)), max(-45, min(gaze, 45))
    override = os.environ.get('PHOTOCRAFT_CLI')
    directory = os.environ.get('CRAFT_CLI_DIR')
    binary = override or (str(Path(directory) / 'photocraft-cli') if directory else shutil.which('photocraft-cli'))
    if not binary:
        raise RuntimeError('PhotoCraft CLI is required for facial preview')
    identity = hashlib.sha256(source.read_bytes() + json.dumps([expression, opening, blink, gaze]).encode()).hexdigest()[:24]
    out = Path(output_root) / identity
    out.mkdir(parents=True, exist_ok=True)
    frames = 6 if expression == 'talk' else 1
    paths = [out / f'frame-{i}.png' for i in range(frames)]
    manifest = out / 'preview.json'
    if manifest.is_file() and all(p.is_file() for p in paths):
        return json.loads(manifest.read_text())
    with Image.open(source) as image:
        width, height = image.size
    detected = tool_call(binary, source, [('image.detectFaces', {'minScore': 0.5})])
    faces = next((row['result'].get('faces', []) for row in detected if row.get('command') == 'image.detectFaces'), [])
    if len(faces) != 1:
        raise ValueError(f'Facial preview needs one detected face; found {len(faces)}. Source artwork is unchanged.')
    face = faces[0]
    marks = face['landmarks']
    if any(not (0 <= x < width and 0 <= y < height) for x, y in marks.values()):
        raise ValueError('Detected landmarks are outside the image')
    _, _, fw, fh = face['rect']
    left, right = sorted([marks['mouthLeft'], marks['mouthRight']])
    mx, my = (left[0] + right[0]) / 2, (left[1] + right[1]) / 2 + fh * .08
    mw, mh = max(right[0] - left[0], fw * .24) * 1.5, fh * .22
    configs = {'smile': (1, 1), 'laugh': (1.12, 1.38), 'ooh': (.62, 1.18), 'mbp': (1.08, .22), 'talk': (.95, 1.15)}
    sx, sy = configs[expression]
    for i, path in enumerate(paths):
        strokes, areas = [], []
        def move(start, end, size):
            if math.dist(start, end) > .05:
                strokes.append({'tool': 'forwardWarp', 'size': size, 'density': 65, 'pressure': 100, 'points': [start, end]})
        # Original artwork is the 20% opening baseline used by the existing UI.
        scale_y = sy * (0.2 + opening * .04)
        if expression == 'talk':
            scale_y *= .55 + .65 * abs(math.sin(i * math.pi / 5))
        for dx, dy in [(-mw / 2, 0), (mw / 2, 0), (0, -mh / 2), (0, mh / 2)]:
            move([mx + dx, my + dy], [mx + dx * sx, my + dy * scale_y], mw * .85)
        areas.append([[mx-mw, my-mh], [mx+mw, my-mh], [mx+mw, my+mh], [mx-mw, my+mh]])
        for key in ['leftEye', 'rightEye']:
            x, y = marks[key]
            ey = fh * .04
            for dy in [-ey, ey]:
                move([x, y+dy], [x + gaze/45*fw*.035, y+dy*(1-blink/100)], fw*.22)
            areas.append([[x-fw*.15,y-fh*.09],[x+fw*.15,y-fh*.09],[x+fw*.15,y+fh*.09],[x-fw*.15,y+fh*.09]])
        commands = []
        for index, points in enumerate(areas):
            points = [[max(0,min(width-1,x)),max(0,min(height-1,y))] for x,y in points]
            commands.append(('select.lasso', {'points':points, 'mode':'replace' if index==0 else 'add', 'feather':2, 'antiAlias':True}))
        if strokes:
            commands.append(('filter.liquify', {'strokes':strokes}))
        tool_call(binary, source, commands, path)
        with Image.open(path) as image:
            if image.size != (width,height):
                raise ValueError('PhotoCraft changed the preview dimensions')
    result = {'source':str(source),'frames':[str(p.resolve()) for p in paths], 'face':face, 'method':'PhotoCraft detected landmarks + Liquify', 'review_required':True, 'audio_driven':False}
    manifest.write_text(json.dumps(result,indent=2))
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source',required=True)
    parser.add_argument('--out-dir',required=True)
    parser.add_argument('--expression',choices=['smile','laugh','ooh','mbp','talk'],default='smile')
    parser.add_argument('--opening',type=float,default=20)
    parser.add_argument('--blink',type=float,default=0)
    parser.add_argument('--gaze',type=float,default=0)
    args=parser.parse_args()
    try:
        print(json.dumps(render(args.source,args.out_dir,args.expression,args.opening,args.blink,args.gaze)))
    except Exception as exc:
        parser.exit(1,f'{exc}\n')


if __name__=='__main__':
    main()
