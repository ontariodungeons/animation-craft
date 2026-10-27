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

try:
    import numpy as np
    from PIL import Image
except ImportError as exc:
    sys.exit(f'Facial preview needs numpy and Pillow ({exc}); install them in the Python that runs this script')

# Bump when the warp recipe, regions or validation change so older cached frames are never reused.
RECIPE_VERSION = 6
EXPRESSIONS = {'smile': (1, 1), 'laugh': (1.12, 1.38), 'ooh': (.62, 1.18), 'mbp': (1.08, .22), 'talk': (.95, 1.15)}
LANDMARKS = ('leftEye', 'rightEye', 'nose', 'mouthLeft', 'mouthRight')
CALL_TIMEOUT = 45
MARGIN = 6          # px of slack around the regions when checking confinement
CHANGE_LEVEL = 4    # per-channel difference that counts as a changed pixel
MIN_CHANGED = 4     # fewer changed pixels than this is treated as a no-op
MAX_MOUTH_SCALE = 2.0   # keeps pull targets inside the mouth region away from the chin
# Region proportions relative to the detected face rect. These are unverified assumptions, not a
# property of any particular character; --face-json lets a reviewer set the landmarks explicitly.
MOUTH_DROP = .02    # slight offset from corner midpoint toward the lip parting line
SILHOUETTE_INSET_FRAC = .035  # inset selection boxes from outer alpha silhouette so chin outline stays intact


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
        result = row.get('result')
        if row.get('error') or (isinstance(result, dict) and result.get('ok') is False):
            detail = row.get('error') or (result.get('error') if isinstance(result, dict) else None) or row.get('command')
            raise RuntimeError(f'PhotoCraft reported a failed operation: {detail}')
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
    if max(marks['leftEye'][1], marks['rightEye'][1]) >= min(marks['mouthLeft'][1], marks['mouthRight'][1]):
        raise ValueError('Eye landmarks must be above mouth landmarks. Supply --face-json to set them explicitly.')
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


def _inset_box_to_silhouette(box, cx, cy, alpha, inset_px):
    """Clamp a region box so it stays inside the opaque character silhouette around (cx, cy)."""
    x0, y0, x1, y1 = box
    if alpha is None:
        return (x0, y0, x1, y1)
    h, w = alpha.shape[:2]
    ix = min(max(0, int(round(cx))), w - 1)
    iy = min(max(0, int(round(cy))), h - 1)
    if alpha[iy, ix] <= 16:
        return (x0, y0, x1, y1)
    # Scan downward/upward from cy at cx and horizontal span to find silhouette edge
    bx0 = max(0, int(math.floor(x0)))
    bx1 = min(w - 1, int(math.ceil(x1)))
    by0 = max(0, int(math.floor(y0)))
    by1 = min(h - 1, int(math.ceil(y1)))
    col_slice = alpha[by0:by1 + 1, bx0:bx1 + 1] > 16
    if not col_slice.any():
        return (x0, y0, x1, y1)
    # Require rows/cols in the box to be predominantly opaque so chin/cheek edges are not dragged
    row_opaque = col_slice.mean(axis=1) > 0.65
    col_opaque = col_slice.mean(axis=0) > 0.65
    if row_opaque.any():
        valid_rows = np.where(row_opaque)[0]
        ry0 = by0 + int(valid_rows[0]) + inset_px
        ry1 = by0 + int(valid_rows[-1]) - inset_px
        if ry1 > ry0 + 4:
            y0 = max(y0, float(ry0))
            y1 = min(y1, float(ry1))
    if col_opaque.any():
        valid_cols = np.where(col_opaque)[0]
        rx0 = bx0 + int(valid_cols[0]) + inset_px
        rx1 = bx0 + int(valid_cols[-1]) - inset_px
        if rx1 > rx0 + 4:
            x0 = max(x0, float(rx0))
            x1 = min(x1, float(rx1))
    return (x0, y0, x1, y1)


def regions(face, width, height, alpha=None):
    """Mouth and eye rectangles as (x0, y0, x1, y1), plus the mouth geometry the warp uses."""
    marks, (fx, fy, fw, fh) = face['landmarks'], face['rect']
    left, right = sorted([marks['mouthLeft'], marks['mouthRight']])
    corner_y = (left[1] + right[1]) / 2
    chin_y = min(height - 1.0, fy + fh)
    mx = (left[0] + right[0]) / 2
    my = min(corner_y + fh * MOUTH_DROP, corner_y + max(0.0, (chin_y - corner_y) * 0.20))
    mouth_span = max(right[0] - left[0], fw * .20)
    mw = min(mouth_span * .68, fw * .34)
    # Keep mouth half-height well above the chin boundary
    max_lower_reach = max(4.0, (chin_y - my) * .48)
    mh = min(fh * .11, max_lower_reach)
    inset_px = max(3, int(round(min(fw, fh) * SILHOUETTE_INSET_FRAC)))
    raw_mouth_box = (
        max(0.0, mx - mw),
        max(0.0, my - mh),
        min(width - 1.0, mx + mw),
        min(height - 1.0, my + mh),
    )
    mouth_box = _inset_box_to_silhouette(raw_mouth_box, mx, my, alpha, inset_px)
    # Update effective (mx, my, mw, mh) to fit within the clamped mouth_box
    mx = (mouth_box[0] + mouth_box[2]) / 2.0
    my = (mouth_box[1] + mouth_box[3]) / 2.0
    mw = max(2.0, (mouth_box[2] - mouth_box[0]) / 2.0)
    mh = max(2.0, (mouth_box[3] - mouth_box[1]) / 2.0)
    boxes = [mouth_box]
    for key in ('leftEye', 'rightEye'):
        x, y = marks[key]
        ebox = (
            max(0.0, x - fw * .14),
            max(0.0, y - fh * .085),
            min(width - 1.0, x + fw * .14),
            min(height - 1.0, y + fh * .085),
        )
        boxes.append(_inset_box_to_silhouette(ebox, x, y, alpha, max(2, inset_px // 2)))
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
    scale_y = sy * (.6 + opening * .02)
    if expression == 'talk':
        scale_y *= .55 + .65 * abs(math.sin(frame * math.pi / 5))
    clamped = scale_y > MAX_MOUTH_SCALE
    scale_y = min(scale_y, MAX_MOUTH_SCALE)
    brush_mouth = max(2.0, min(mw * .72, mh * 1.35))
    for dx, dy in [(-mw * .45, 0), (mw * .45, 0), (0, -mh * .42), (0, mh * .42)]:
        tx = max(mx - mw * .88, min(mx + mw * .88, mx + dx * sx))
        ty = max(my - mh * .88, min(my + mh * .88, my + dy * scale_y))
        move([mx + dx, my + dy], [tx, ty], brush_mouth)
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
    if blink or gaze:
        raise ValueError('Blink and gaze require explicit eyelid/pupil part or pin bindings; face landmarks alone are insufficient')
    source = Path(source).resolve(strict=True)
    if expression not in EXPRESSIONS:
        raise ValueError('Unknown expression')
    if not all(math.isfinite(v) for v in (opening, blink, gaze)):
        raise ValueError('Face controls must be finite')
    opening, blink, gaze = max(0, min(opening, 100)), max(0, min(blink, 100)), max(-45, min(gaze, 45))
    binary = os.environ.get('PHOTOCRAFT_CLI') or shutil.which('photocraft-cli') or 'photocraft-cli'
    version = run_tool([binary, '--version'], deadline).strip()
    root = private_root(output_root)
    source_hash = sha(source.read_bytes())
    explicit = Path(face_json).read_bytes() if face_json else b''
    with Image.open(source) as image:
        width, height = image.size

    identity = sha(source_hash, RECIPE_VERSION, version, expression, opening, blink, gaze, explicit)[:24]
    # Frame folders are unique per job and never modified or deleted once published, so another
    # app instance can keep using a result while this one renders. Only the small pointer file
    # (identity -> newest result) is replaced, atomically.
    pointer = root / f'{identity}.json'
    if pointer.is_file():
        try:
            cached = json.loads(pointer.read_text())
            if all(Path(p).is_file() for p in cached['frames']):
                return cached
        except (ValueError, KeyError, TypeError):
            pass

    source_pixels = rgba(source)
    face, face_source = locate_face(binary, source, sha(source_hash, version, 'detect')[:24], face_json, width, height, root, deadline)
    boxes, mouth = regions(face, width, height, source_pixels[:, :, 3])
    frames = 6 if expression == 'talk' else 1
    plans = [strokes_for(i, expression, opening, blink, gaze, face, mouth) for i in range(frames)]
    if not any(strokes for strokes, _ in plans):
        raise ValueError('These settings do not move any pixels; nothing to preview')
    select = selection(boxes)
    mask = region_mask(boxes, width, height)

    work = Path(tempfile.mkdtemp(dir=root, prefix=f'{identity}-'))
    try:
        baseline = work / 'baseline.png'
        tool_call(binary, source, select, deadline, baseline)
        baseline_pixels = rgba(baseline)
        roundtrip = int(np.abs(source_pixels - baseline_pixels).max()) if source_pixels.shape == baseline_pixels.shape else None
        changed_pixels, outline_pixels = [], []
        for i, (strokes, _) in enumerate(plans):
            path = work / f'frame-{i}.png'
            tool_call(binary, source, select + ([('filter.liquify', {'strokes': strokes})] if strokes else []), deadline, path)
            inside, outline = check_against_baseline(path, baseline, mask)
            changed_pixels.append(inside)
            outline_pixels.append(outline)
        baseline.unlink()
        final = work
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
    descriptor, staged = tempfile.mkstemp(dir=root, prefix='.pointer-')
    with os.fdopen(descriptor, 'w') as handle:
        handle.write(json.dumps(result, indent=2))
    os.replace(staged, pointer)
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
