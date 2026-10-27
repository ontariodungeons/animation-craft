"""Bake explicitly bound pins and mouth regions through PhotoCraft, preserving source art."""
import argparse
import json
import math
from pathlib import Path
import shutil
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from pipeline.facial_expression_preview import Deadline, private_root, tool_call, rgba, render as render_face
import numpy as np


def render(source, out_dir, recipe, expression='smile', opening=20, deadline_s=90):
    source = Path(source).resolve(strict=True)
    root = private_root(out_dir)
    deadline = Deadline(deadline_s)
    pins = recipe.get('pins', [])
    for pin in pins:
        for key in ('src', 'dst'):
            point = pin.get(key)
            if not isinstance(point, list) or len(point) != 2 or not all(isinstance(v, (int, float)) and math.isfinite(v) for v in point):
                raise ValueError('Each puppet pin needs finite src/dst coordinates in cropped image pixels')
    if pins:
        original = rgba(source)
        height, width = original.shape[:2]
        for pin in pins:
            x, y = pin['src']
            if not (0 <= x < width and 0 <= y < height) or original[int(y), int(x), 3] <= 16:
                raise ValueError('Pin source must lie on visible pixels of the cropped layer')
            x, y = pin['dst']
            if not (0 <= x < width and 0 <= y < height):
                raise ValueError('Pin target leaves the cropped layer; enlarge its canvas before deforming')
    changed = any(p['src'] != p['dst'] for p in pins)
    # Landmarks are authored in source coordinates. Bake the mouth first, then
    # deform each resulting frame with pins; never detect against moved coordinates.
    face = recipe.get('face')
    if face is not None:
        result = render_face(source, root, expression, opening, 0, 0, face.get('landmarks'), deadline.left(90))
    else:
        result = {'frames': [str(source)], 'warnings': [], 'review_required': changed}
    if not changed:
        return result
    binary = shutil.which('photocraft-cli') or 'photocraft-cli'
    work = Path(tempfile.mkdtemp(prefix='pins-', dir=root))
    frames = []
    for i, frame in enumerate(result['frames']):
        baseline, warped = work / f'baseline-{i}.png', work / f'warped-{i}.png'
        tool_call(binary, frame, [], deadline, baseline)
        tool_call(binary, frame, [('edit.puppetWarp', {'pins': pins, 'mode': 'normal', 'density': 'more', 'expansion': 0})], deadline, warped)
        before, after = rgba(baseline), rgba(warped)
        if before.shape != after.shape:
            raise ValueError('Puppet warp changed the canvas dimensions; preview rejected')
        visible_change = np.any(np.abs(before - after) > 4, axis=2) & ((before[:, :, 3] > 16) | (after[:, :, 3] > 16))
        if int(visible_change.sum()) < 4:
            raise ValueError('Puppet warp did not visibly change the artwork; check pin positions')
        frames.append(str(warped))
    result['frames'] = frames
    result['warnings'] = result.get('warnings', []) + ['Review the mapped pin deformation']
    result['review_required'] = True
    return result


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--source', required=True)
    p.add_argument('--out-dir', required=True)
    p.add_argument('--recipe', required=True)
    p.add_argument('--expression', default='smile')
    p.add_argument('--opening', type=float, default=20)
    p.add_argument('--blink', type=float, default=0)
    p.add_argument('--gaze', type=float, default=0)
    p.add_argument('--deadline', type=float, default=90)
    a = p.parse_args()
    try:
        if a.blink or a.gaze:
            raise ValueError('Blink and gaze require explicit part or pin bindings')
        print(json.dumps(render(a.source, a.out_dir, json.loads(a.recipe), a.expression, a.opening, a.deadline)))
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        sys.exit(1)
