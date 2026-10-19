"""Measure a rendered transparent clip without looking at it, and check it against a spec or a reference image.

Everything is computed from decoded frames (alpha and colour), so it works for any subject:
frame count and size, whether alpha survived, empty or clipped frames, how smoothly the visible
shape moves (teleports and flicker), loop closure, and, given a reference image of the expected
look, how far the colours and silhouette drift from it. A pass means "no measurable problem";
it is not a judgement that the result looks good, so pair it with a visual review against the same reference.
"""
import argparse
import json
import math
import subprocess
import sys
from pathlib import Path

try:
    import numpy as np
    from PIL import Image
except ImportError as exc:
    sys.exit(f'motion_validate needs numpy and Pillow ({exc})')

ANALYSIS_WIDTH = 480
ALPHA_ON = 16
LIMITS = {
    'min_visible_px': 50,         # in analysis pixels; below this a frame counts as empty
    'teleport_factor': 5.0,       # a step this many times the median step (and > teleport_min_px) is a jump
    'teleport_min_px': 25.0,      # full-resolution pixels
    'flicker_ratio': 0.35,        # frame-to-frame relative change in visible area
    'color_drift': 30.0,          # RGB distance of mean colour from the reference
    'area_ratio': (0.5, 2.0),     # median visible area relative to the reference
    'loop_iou': 0.9,
}


def probe(path):
    out = subprocess.run(['ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries',
                          'stream=width,height,r_frame_rate:stream_tags=alpha_mode', '-of', 'json', str(path)],
                         capture_output=True, text=True, timeout=60)
    if out.returncode:
        raise RuntimeError(out.stderr.strip().splitlines()[-1] if out.stderr.strip() else 'ffprobe failed')
    streams = json.loads(out.stdout).get('streams') or []
    if not streams:
        raise RuntimeError(f'no video stream in {path}')
    stream = streams[0]
    num, den = (int(v) for v in stream['r_frame_rate'].split('/'))
    return int(stream['width']), int(stream['height']), num / (den or 1), stream.get('tags', {}).get('alpha_mode') == '1'


def decode(path, width, height):
    """Yield (alpha 0..255, rgb) arrays per frame at analysis size, keeping the alpha channel."""
    aw = ANALYSIS_WIDTH
    ah = max(1, int(round(aw * height / width)))
    cmd = ['ffmpeg', '-nostdin', '-v', 'error', '-c:v', 'libvpx-vp9', '-i', str(path),
           '-vf', f'scale={aw}:{ah}:flags=area', '-pix_fmt', 'rgba', '-f', 'rawvideo', '-']
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    size = aw * ah * 4
    try:
        while True:
            raw = proc.stdout.read(size)
            if len(raw) < size:
                break
            frame = np.frombuffer(raw, np.uint8).reshape(ah, aw, 4)
            yield frame[:, :, 3], frame[:, :, :3]
    finally:
        if proc.stdout:
            proc.stdout.close()
        rc = proc.wait()
        stderr_txt = proc.stderr.read().decode(errors='replace').strip() if proc.stderr else ''
        if proc.stderr:
            proc.stderr.close()
    if rc:
        raise RuntimeError((stderr_txt.splitlines() or ['ffmpeg failed'])[-1])


def reference_stats(path):
    """Alpha-weighted mean colour and visible area (native pixels) of the reference; its placement does not matter."""
    with Image.open(path) as image:
        rgba = np.asarray(image.convert('RGBA'))
    mask = rgba[:, :, 3] > ALPHA_ON
    if not mask.any():
        raise ValueError('reference image has no visible pixels')
    weights = rgba[:, :, 3][mask].astype(float)
    weighted_rgb = (rgba[:, :, :3][mask].astype(float) * weights[:, None]).sum(axis=0) / weights.sum()
    return weighted_rgb, int(mask.sum())


def check(name, ok, value, limit, note=''):
    return {'name': name, 'ok': bool(ok), 'value': value, 'limit': limit, 'note': note}


def analyze(path, expected_frames=None, expected_fps=None, loop=False, reference=None, limits=None):
    lim = {**LIMITS, **(limits or {})}
    width, height, fps, alpha_flag = probe(path)
    scale = width / ANALYSIS_WIDTH
    areas, centroids, touches, colors, first_mask, last_mask = [], [], [], [], None, None
    last_alpha_size = max(1, ANALYSIS_WIDTH * max(1, int(round(ANALYSIS_WIDTH * height / width))))
    for alpha, rgb in decode(path, width, height):
        last_alpha_size = alpha.size
        mask = alpha > ALPHA_ON
        count = int(mask.sum())
        areas.append(count)
        touches.append(bool(mask[0].any() or mask[-1].any() or mask[:, 0].any() or mask[:, -1].any()))
        if count:
            ys, xs = np.nonzero(mask)
            weights = alpha[mask].astype(float)
            w_sum = weights.sum()
            centroids.append((float((xs * weights).sum() / w_sum) * scale, float((ys * weights).sum() / w_sum) * scale))
            colors.append((rgb[mask].astype(float) * weights[:, None]).sum(axis=0) / w_sum)
        else:
            centroids.append(None)
            colors.append(None)
        first_mask = mask if first_mask is None else first_mask
        last_mask = mask
    frames = len(areas)
    if not frames:
        raise RuntimeError('no frames decoded')
    areas_np = np.array(areas, float)
    checks = [check('alpha_channel', alpha_flag and int(areas_np.min()) < last_alpha_size, alpha_flag, True, 'file declares alpha and has transparent pixels')]
    if expected_frames is not None:
        checks.append(check('frame_count', frames == expected_frames, frames, expected_frames))
    if expected_fps is not None:
        checks.append(check('frame_rate', abs(fps - expected_fps) < 1e-3, fps, expected_fps))
    empty = int((areas_np < lim['min_visible_px']).sum())
    checks.append(check('no_empty_frames', empty == 0, empty, 0, 'frames with almost nothing visible'))
    clipped = int(sum(touches))
    checks.append(check('not_clipped_by_canvas', clipped == 0, clipped, 0, 'frames whose visible pixels touch the canvas edge'))
    pts = [c for c in centroids if c]
    steps = np.array([math.dist(a, b) for a, b in zip(pts, pts[1:])]) if len(pts) > 1 else np.zeros(1)
    median = float(np.median(steps))
    cutoff = max(lim['teleport_factor'] * median, lim['teleport_min_px'])
    jumps = int((steps > cutoff).sum())
    checks.append(check('no_teleports', jumps == 0, jumps, 0, f'steps above {cutoff:.1f}px (median step {median:.2f}px)'))
    rel = np.abs(np.diff(areas_np)) / np.maximum(areas_np[:-1], 1) if len(areas_np) > 1 else np.zeros(1)
    flicker = int((rel > lim['flicker_ratio']).sum())
    checks.append(check('no_flicker', flicker == 0, flicker, 0, 'frame-to-frame visible-area changes above the limit'))
    loop_iou = None
    if loop:
        inter, union = int((first_mask & last_mask).sum()), int((first_mask | last_mask).sum())
        loop_iou = inter / union if union else 1.0
        checks.append(check('loop_closes', loop_iou >= lim['loop_iou'], round(loop_iou, 3), lim['loop_iou'], 'silhouette overlap of first and last frame'))
    reference_report = None
    if reference:
        ref_color, ref_native_area = reference_stats(reference)
        ref_area = ref_native_area * (ANALYSIS_WIDTH / width) ** 2
        drift = [float(np.linalg.norm(c - ref_color)) for c in colors if c is not None]
        worst = max(drift) if drift else float('inf')
        ratio = float(np.median(areas_np[areas_np >= lim['min_visible_px']]) / ref_area) if (areas_np >= lim['min_visible_px']).any() else 0.0
        low, high = lim['area_ratio']
        reference_report = {'worst_color_distance': round(worst, 2), 'median_area_ratio': round(ratio, 3)}
        checks.append(check('colors_match_reference', worst <= lim['color_drift'], round(worst, 2), lim['color_drift'], 'alpha-weighted mean colour vs the reference'))
        checks.append(check('size_matches_reference', low <= ratio <= high, round(ratio, 3), [low, high], 'median visible area vs the reference'))
    return {
        'file': str(path), 'width': width, 'height': height, 'fps': fps, 'frames': frames,
        'metrics': {'median_step_px': round(median, 3), 'max_step_px': round(float(steps.max()), 3),
                    'visible_area_min': int(areas_np.min()), 'visible_area_max': int(areas_np.max()),
                    'loop_iou': None if loop_iou is None else round(loop_iou, 4), 'reference': reference_report},
        'checks': checks,
        # A check that could not run is not a pass: name it so a reader does not assume it was verified.
        'not_measured': [] if reference else ['colors_match_reference (no reference image)',
                                              'size_matches_reference (no reference image)'],
        'ok': all(c['ok'] for c in checks),
    }


def contact_sheet(path, out, count=12, cols=4, background=(96, 138, 96), tile_width=480, reference=None, mode='path'):
    """Evenly spaced frames on a neutral background, with the reference (if any) as the first tile, for a visual review.

    mode "path" crops every tile to the area the subject covers over the whole clip (shows where it moves);
    mode "detail" crops each tile tightly to the subject in that frame (shows how it is drawn and posed).
    """
    count = max(1, int(count))
    cols = max(1, int(cols))
    width, height, _, _ = probe(path)
    work_w = tile_width * 2
    work_h = max(1, int(round(work_w * height / width)))
    tile_h = max(1, int(round(tile_width * height / width)))
    tiles = []
    frames = list(decode_rgba(path, width, height, work_w, work_h))
    if not frames:
        raise RuntimeError('no frames decoded')
    wanted = sorted({int(round(i * (len(frames) - 1) / max(1, count - 1))) for i in range(count)})
    boxes = [Image.fromarray(f[:, :, 3]).point(lambda a: 255 if a > ALPHA_ON else 0).getbbox() for f in frames]
    seen = [b for b in boxes if b]
    union = (min(b[0] for b in seen), min(b[1] for b in seen), max(b[2] for b in seen), max(b[3] for b in seen)) if seen else (0, 0, work_w, work_h)
    pad = 12
    if reference:
        with Image.open(reference) as ref:
            ref = ref.convert('RGBA')
            box = ref.getchannel('A').point(lambda a: 255 if a > ALPHA_ON else 0).getbbox() or (0, 0, ref.width, ref.height)
            ref = ref.crop(box)
            ref.thumbnail((tile_width, tile_h))
            tile = Image.new('RGBA', (tile_width, tile_h), background + (255,))
            tile.alpha_composite(ref, ((tile_width - ref.width) // 2, (tile_h - ref.height) // 2))
            tiles.append(tile)
    for index in wanted:
        box = union if mode == 'path' else (boxes[index] or union)
        crop = Image.fromarray(frames[index], 'RGBA').crop((max(0, box[0] - pad), max(0, box[1] - pad), min(work_w, box[2] + pad), min(work_h, box[3] + pad)))
        crop.thumbnail((tile_width, tile_h))
        tile = Image.new('RGBA', (tile_width, tile_h), background + (255,))
        tile.alpha_composite(crop, ((tile_width - crop.width) // 2, (tile_height_offset(tile_h, crop.height))))
        tiles.append(tile)
    rows = max(1, (len(tiles) + cols - 1) // cols)
    sheet = Image.new('RGBA', (cols * tile_width, rows * tile_h), background + (255,))
    for i, tile in enumerate(tiles):
        sheet.paste(tile, ((i % cols) * tile_width, (i // cols) * tile_h))
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    sheet.convert('RGB').save(out)
    return {'sheet': str(out), 'frames': wanted, 'mode': mode, 'reference_first': bool(reference)}


def tile_height_offset(tile_h, crop_h):
    return (tile_h - crop_h) // 2


def decode_rgba(path, width, height, out_w, out_h):
    cmd = ['ffmpeg', '-nostdin', '-v', 'error', '-c:v', 'libvpx-vp9', '-i', str(path),
           '-vf', f'scale={out_w}:{out_h}:flags=area', '-pix_fmt', 'rgba', '-f', 'rawvideo', '-']
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    size = out_w * out_h * 4
    try:
        while True:
            raw = proc.stdout.read(size)
            if len(raw) < size:
                break
            yield np.frombuffer(raw, np.uint8).reshape(out_h, out_w, 4).copy()
    finally:
        if proc.stdout:
            proc.stdout.close()
        rc = proc.wait()
        stderr_txt = proc.stderr.read().decode(errors='replace').strip() if proc.stderr else ''
        if proc.stderr:
            proc.stderr.close()
    if rc:
        raise RuntimeError((stderr_txt.splitlines() or ['ffmpeg failed'])[-1])


def lint_spec(spec):
    """Static checks on a layered motion spec: oscillations that cannot read as smooth motion at its frame rate."""
    if isinstance(spec, (Path, str)):
        spec = json.loads(Path(spec).read_text())
    fps = float(spec.get('fps', 30))
    blur = bool(spec.get('motionBlur'))
    issues = []
    for track in spec.get('tracks', []):
        for layer, props in (track.get('motion') or {}).items():
            if not isinstance(props, dict):
                continue
            for prop, terms in props.items():
                for term in (terms or []):
                    if not isinstance(term, dict) or term.get('kind') != 'oscillate':
                        continue
                    hz = float(term['hz'])
                    per_cycle = fps / hz if hz else math.inf
                    if hz > fps / 2:
                        issues.append({'track': track['id'], 'layer': layer, 'property': prop, 'hz': hz, 'level': 'error',
                                       'note': f'{hz} Hz is above the {fps / 2} Hz Nyquist limit of {fps} fps and will alias'})
                    elif per_cycle < 4 and not blur:
                        issues.append({'track': track['id'], 'layer': layer, 'property': prop, 'hz': hz, 'level': 'warning',
                                       'note': f'{per_cycle:.1f} frames per cycle without motion blur will strobe'})
    return issues


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--video', required=True, help='transparent WebM to measure')
    parser.add_argument('--frames', type=int, help='expected frame count')
    parser.add_argument('--fps', type=float, help='expected frame rate')
    parser.add_argument('--loop', action='store_true', help='require the last frame to match the first')
    parser.add_argument('--reference', help='image of the expected look (transparent PNG preferred)')
    parser.add_argument('--spec', help='also lint this layered motion spec')
    parser.add_argument('--sheet', help='also write a contact sheet PNG of evenly spaced frames (reference first when given)')
    args = parser.parse_args()
    try:
        report = analyze(args.video, args.frames, args.fps, args.loop, args.reference)
        if args.sheet:
            report['contact_sheet'] = contact_sheet(args.video, args.sheet, reference=args.reference)
        if args.spec:
            report['spec_lint'] = lint_spec(json.loads(Path(args.spec).read_text()))
            report['ok'] = report['ok'] and not any(i['level'] == 'error' for i in report['spec_lint'])
        print(json.dumps(report))
    except Exception as exc:
        text = str(exc).strip().splitlines()
        parser.exit(1, f'{text[-1] if text else type(exc).__name__}\n')


if __name__ == '__main__':
    main()
