"""Render raster facial previews using installed PhotoCraft detection and Liquify.

This deforms existing artwork; it does not synthesize teeth, infer phonemes from
sound, or establish that a detected face is correct. Source files are read-only.
A result means "PhotoCraft changed pixels only inside the face regions"; it never
means the expression is right, so every result is marked review_required.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from pipeline.craft_cli import require_craft_cli  # noqa: E402

try:
    import numpy as np
    from PIL import Image
except ImportError as exc:
    sys.exit(f'Facial preview needs numpy and Pillow ({exc}); install them in the Python that runs this script')

# Bump when the warp recipe, regions or validation change so older cached frames are never reused.
RECIPE_VERSION = 3
EXPRESSIONS = {'smile': (1, 1), 'laugh': (1.12, 1.38), 'ooh': (.62, 1.18), 'mbp': (1.08, .22), 'talk': (.95, 1.15)}
LANDMARKS = ('leftEye', 'rightEye', 'nose', 'mouthLeft', 'mouthRight')
CALL_TIMEOUT = 45
MARGIN = 6          # px of slack around the regions when checking confinement
CHANGE_LEVEL = 4    # per-channel difference that counts as a changed pixel
MIN_CHANGED = 4     # fewer changed pixels than this is treated as a no-op
MAX_MOUTH_SCALE = 1.8  # keeps pull targets (at half the region height) inside the region
# Region proportions relative to the detected face rect. These are unverified assumptions, not a
# property of any particular character; --face-json lets a reviewer set the landmarks explicitly.
MOUTH_DROP = .08    # mouth centre sits this fraction of face height below the corner midpoint


class Deadline:
    def __init__(self, seconds):
        self.seconds = seconds
        self.end = time.monotonic() + seconds

    def left(self, cap=CALL_TIMEOUT):
        remaining = self.end - time.monotonic()
        if remaining <= 0:
            raise TimeoutError(f'Facial preview exceeded its {self.seconds:.0f}s limit; PhotoCraft may be hung. Adjust a control to retry.')
        return min(cap, remaining)


def last_line(text):
    lines = [line.strip() for line in (text or '').splitlines() if line.strip()]
    return lines[-1][:300] if lines else ''


def run_tool(args, deadline):
    try:
        result = subprocess.run(args, capture_output=True, text=True, timeout=deadline.left())
    except subprocess.TimeoutExpired:
        deadline.left(0)  # raises the overall-limit message once that limit is spent
        raise TimeoutError(f'`{Path(args[0]).name}` did not finish within {CALL_TIMEOUT}s') from None
    except OSError as exc:
        raise RuntimeError(f'Could not start `{args[0]}`: {exc}') from None
    if result.returncode:
        raise RuntimeError(last_line(result.stderr) or last_line(result.stdout) or f'`{Path(args[0]).name}` exited with {result.returncode}')
    return result.stdout


def tool_call(binary, source, commands, deadline, output=None):
    args = [binary, 'run', str(source)]
    for command, params in commands:
        args += ['--cmd', command, '--params', json.dumps(params)]
    if output:
        args += ['--out', str(output)]
    rows = []
    for line in run_tool(args, deadline).splitlines():
        if line.startswith('{'):
            try:
                rows.append(json.loads(line))
            except ValueError:
                continue
    for row in rows:
        if row.get('error') or row.get('result') == {'ok': False}:
            raise RuntimeError(f"PhotoCraft reported a failed operation: {row.get('error') or row.get('command')}")
    return rows


def sha(*parts):
    digest = hashlib.sha256()
    for part in parts:
        digest.update(part if isinstance(part, bytes) else str(part).encode())
        digest.update(b'\0')
    return digest.hexdigest()


def private_root(path):
    root = Path(path)
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    if hasattr(os, 'getuid'):
        if root.stat().st_uid != os.getuid():
            raise PermissionError(f'{root} belongs to another user; choose a different output folder')
        root.chmod(0o700)
    return root


def validate_face(face, width, height):
    """Accept only a complete, finite, in-bounds detection payload."""
    try:
        x, y, w, h = (float(v) for v in face['rect'])
        marks = {k: [float(face['landmarks'][k][0]), float(face['landmarks'][k][1])] for k in LANDMARKS}
    except (KeyError, TypeError, ValueError, IndexError) as exc:
        raise ValueError(f'Face data is missing or malformed ({exc!r}). Supply --face-json to set it explicitly.') from None
    values = [x, y, w, h] + [c for point in marks.values() for c in point]
    if not all(math.isfinite(v) for v in values) or w <= 0 or h <= 0:
        raise ValueError('Face data has non-finite or empty values. Supply --face-json to set it explicitly.')
    for name, (px, py) in marks.items():
        if not (0 <= px < width and 0 <= py < height):
            raise ValueError(f'Landmark {name} is outside the image. Supply --face-json to set it explicitly.')
    if abs(marks['mouthRight'][0] - marks['mouthLeft'][0]) < .02 * w or abs(marks['rightEye'][0] - marks['leftEye'][0]) < .02 * w:
        raise ValueError('Mouth or eye landmarks coincide, so the regions would be meaningless. Supply --face-json to set them explicitly.')
    return {'rect': [x, y, w, h], 'landmarks': marks}


def locate_face(binary, source, key, face_json, width, height, root, deadline):
    if face_json:
        try:
            payload = json.loads(Path(face_json).read_text())
        except (OSError, ValueError) as exc:
            raise ValueError(f'Cannot read --face-json: {exc}') from None
        return validate_face(payload, width, height), 'explicit landmarks'
    cached = root / f'detect-{key}.json'
    if cached.is_file():
        try:
            return validate_face(json.loads(cached.read_text()), width, height), 'detector (cached)'
        except ValueError:
            cached.unlink(missing_ok=True)
    rows = tool_call(binary, source, [('image.detectFaces', {'minScore': 0.5})], deadline)
    result = next((row.get('result') for row in rows if row.get('command') == 'image.detectFaces'), None)
    if not isinstance(result, dict) or not isinstance(result.get('faces'), list):
        raise RuntimeError('PhotoCraft returned no face-detection result')
    if len(result['faces']) != 1:
        raise ValueError(f"Facial preview needs exactly one detected face; found {len(result['faces'])}. "
                         'Source artwork is unchanged; supply --face-json to set the face explicitly.')
    face = validate_face(result['faces'][0], width, height)
    cached.write_text(json.dumps(face))
    return face, 'detector'


def regions(face, width, height):
    """Mouth and eye rectangles as (x0, y0, x1, y1), plus the mouth geometry the warp uses."""
    marks, (_, _, fw, fh) = face['landmarks'], face['rect']
    left, right = sorted([marks['mouthLeft'], marks['mouthRight']])
    mx, my = (left[0] + right[0]) / 2, (left[1] + right[1]) / 2 + fh * MOUTH_DROP
    mw, mh = max(right[0] - left[0], fw * .24) * 1.5, fh * .22
    boxes = [(mx - mw, my - mh, mx + mw, my + mh)]
    for key in ('leftEye', 'rightEye'):
        x, y = marks[key]
        boxes.append((x - fw * .15, y - fh * .09, x + fw * .15, y + fh * .09))
    clipped = [(max(0, x0), max(0, y0), min(width - 1, x1), min(height - 1, y1)) for x0, y0, x1, y1 in boxes]
    return clipped, (mx, my, mw, mh)


def strokes_for(frame, expression, opening, blink, gaze, face, mouth):
    marks, (_, _, fw, fh) = face['landmarks'], face['rect']
    mx, my, mw, mh = mouth
    sx, sy = EXPRESSIONS[expression]
    strokes = []

    def move(start, end, size):
        if math.dist(start, end) > .05:
            strokes.append({'tool': 'forwardWarp', 'size': max(2, size), 'density': 65, 'pressure': 100, 'points': [start, end]})

    # Original artwork is the 20% opening baseline used by the existing UI.
    scale_y = sy * (.2 + opening * .04)
    if expression == 'talk':
        scale_y *= .55 + .65 * abs(math.sin(frame * math.pi / 5))
    clamped = scale_y > MAX_MOUTH_SCALE
    scale_y = min(scale_y, MAX_MOUTH_SCALE)
    for dx, dy in [(-mw / 2, 0), (mw / 2, 0), (0, -mh / 2), (0, mh / 2)]:
        move([mx + dx, my + dy], [mx + dx * sx, my + dy * scale_y], mw * .85)
    for key in ('leftEye', 'rightEye'):
        x, y = marks[key]
        ey = fh * .04
        for dy in (-ey, ey):
            move([x, y + dy], [x + gaze / 45 * fw * .035, y + dy * (1 - blink / 100)], fw * .22)
    return strokes, clamped


def selection(boxes):
    return [('select.lasso', {'points': [[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
                              'mode': 'replace' if i == 0 else 'add', 'feather': 2, 'antiAlias': True})
            for i, (x0, y0, x1, y1) in enumerate(boxes)]


def rgba(path):
    with Image.open(path) as image:
        return np.asarray(image.convert('RGBA'), dtype=np.int16)


def region_mask(boxes, width, height):
    mask = np.zeros((height, width), bool)
    for x0, y0, x1, y1 in boxes:
        mask[max(0, int(y0) - MARGIN):int(y1) + MARGIN + 1, max(0, int(x0) - MARGIN):int(x1) + MARGIN + 1] = True
    return mask


def check_against_baseline(output, baseline, mask):
    """Output must differ from the tool's own no-op round trip, and only inside the regions.

    This is evidence of change and confinement only; it says nothing about whether the
    expression looks right.
    """
    out, base = rgba(output), rgba(baseline)
    if out.shape != base.shape:
        raise ValueError('PhotoCraft changed the preview dimensions')
    changed = np.abs(out - base).max(axis=2) > CHANGE_LEVEL
    inside, outside = int((changed & mask).sum()), int((changed & ~mask).sum())
    outline = int((np.abs(out[:, :, 3] - base[:, :, 3]) > CHANGE_LEVEL).sum())
    if inside < MIN_CHANGED:
        raise ValueError('PhotoCraft returned the image unchanged for these settings, so no preview was produced')
    if outside:
        raise ValueError(f'PhotoCraft changed {outside} pixels outside the face regions; preview rejected')
    return inside, outline


def render(source, output_root, expression, opening, blink, gaze, face_json=None, deadline_s=90):
    deadline = Deadline(deadline_s)
    source = Path(source).resolve(strict=True)
    if expression not in EXPRESSIONS:
        raise ValueError('Unknown expression')
    if not all(math.isfinite(v) for v in (opening, blink, gaze)):
        raise ValueError('Face controls must be finite')
    opening, blink, gaze = max(0, min(opening, 100)), max(0, min(blink, 100)), max(-45, min(gaze, 45))
    binary = require_craft_cli('photocraft-cli')
    version = run_tool([binary, '--version'], deadline).strip()
    root = private_root(output_root)
    source_hash = sha(source.read_bytes())
    explicit = Path(face_json).read_bytes() if face_json else b''
    with Image.open(source) as image:
        width, height = image.size

    identity = sha(source_hash, RECIPE_VERSION, version, expression, opening, blink, gaze, explicit)[:24]
    final = root / identity
    manifest = final / 'preview.json'
    if manifest.is_file():
        cached = json.loads(manifest.read_text())
        if all(Path(p).is_file() for p in cached['frames']):
            return cached

    face, face_source = locate_face(binary, source, sha(source_hash, version, 'detect')[:24], face_json, width, height, root, deadline)
    boxes, mouth = regions(face, width, height)
    frames = 6 if expression == 'talk' else 1
    plans = [strokes_for(i, expression, opening, blink, gaze, face, mouth) for i in range(frames)]
    if not any(strokes for strokes, _ in plans):
        raise ValueError('These settings do not move any pixels; nothing to preview')
    select = selection(boxes)
    mask = region_mask(boxes, width, height)

    work = Path(tempfile.mkdtemp(dir=root, prefix='.work-'))
    try:
        baseline = work / 'baseline.png'
        tool_call(binary, source, select, deadline, baseline)
        source_pixels, baseline_pixels = rgba(source), rgba(baseline)
        roundtrip = int(np.abs(source_pixels - baseline_pixels).max()) if source_pixels.shape == baseline_pixels.shape else None
        changed_pixels, outline_pixels = [], []
        for i, (strokes, _) in enumerate(plans):
            path = work / f'frame-{i}.png'
            tool_call(binary, source, select + ([('filter.liquify', {'strokes': strokes})] if strokes else []), deadline, path)
            inside, outline = check_against_baseline(path, baseline, mask)
            changed_pixels.append(inside)
            outline_pixels.append(outline)
        baseline.unlink()
        if final.exists():
            shutil.rmtree(final)
        os.replace(work, final)
    except BaseException:
        shutil.rmtree(work, ignore_errors=True)
        raise
    warnings = []
    if any(outline_pixels):
        warnings.append('the warp changed the character outline (transparency); check that nothing bulges past the silhouette')
    if any(clamped for _, clamped in plans):
        warnings.append('mouth opening was limited to keep the deformation inside its region')
    result = {
        'source': str(source),
        'frames': [str((final / f'frame-{i}.png').resolve()) for i in range(frames)],
        'face': face,
        'face_source': face_source,
        'method': 'PhotoCraft landmarks + Liquify inside bounded mouth/eye regions',
        'recipe_version': RECIPE_VERSION,
        'tool_version': version,
        'warnings': warnings,
        'validation': {'status': 'changed_only_inside_regions', 'changed_pixels_per_frame': changed_pixels, 'outline_pixels_changed_per_frame': outline_pixels,
                       'tool_roundtrip_max_difference': roundtrip},
        'review_required': True,
        'verified': False,
        'audio_driven': False,
    }
    manifest.write_text(json.dumps(result, indent=2))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True)
    parser.add_argument('--out-dir', required=True)
    parser.add_argument('--expression', choices=sorted(EXPRESSIONS), default='smile')
    parser.add_argument('--opening', type=float, default=20)
    parser.add_argument('--blink', type=float, default=0)
    parser.add_argument('--gaze', type=float, default=0)
    parser.add_argument('--face-json', help='JSON with "rect" [x,y,w,h] and "landmarks" (leftEye, rightEye, nose, mouthLeft, mouthRight) to use instead of detection')
    parser.add_argument('--deadline', type=float, default=90, help='total seconds allowed, including every PhotoCraft call')
    args = parser.parse_args()
    try:
        print(json.dumps(render(args.source, args.out_dir, args.expression, args.opening, args.blink, args.gaze, args.face_json, args.deadline)))
    except Exception as exc:
        parser.exit(1, f'{last_line(str(exc)) or type(exc).__name__}\n')


if __name__ == '__main__':
    main()
