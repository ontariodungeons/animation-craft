#!/usr/bin/env python3
"""Rig sheet -> placed parts for a layered rig.

A rig sheet (image.generate rig_parts=...) holds the assembled asset in cell 1 and one part per later
cell, all at one scale. This cuts every cell with the sheet cutter, finds where each part sits on the
assembled asset (masked template matching over position and a narrow scale range), and writes each part
as a canvas-sized PNG at that place. The joint of a part without a given pivot is the centre of where it
overlaps its parent. placement.json reports the match per part and how well the placed parts rebuild
the assembled asset; the engine writes rig.json from it.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import cv2
import numpy as np
from PIL import Image
from scipy import ndimage as ndi

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from pipeline.pose_and_part_cutter import cut_sheet

MARGIN = 0.15  # canvas room around the assembled asset so turning parts are not clipped


def trimmed(path: Path, cell=None) -> np.ndarray:
    """The image cropped to its drawing. With `cell` (the cut cell's [x0, y0, x1, y1] inside the cutter's
    padded frame), pieces touching the cell's edge are dropped unless they are the main one: a neighbouring
    cell's drawing can cross the cell line."""
    with Image.open(path) as im:
        rgba = np.array(im.convert('RGBA'))
    if cell is not None:
        n, labels, stats, _ = cv2.connectedComponentsWithStats((rgba[:, :, 3] > 16).astype(np.uint8), connectivity=8)
        if n > 2:
            main = 1 + int(stats[1:, cv2.CC_STAT_AREA].argmax())
            x0, y0, x1, y1 = cell
            for c in range(1, n):
                x, y, w, h = stats[c, :4]
                if c != main and (x <= x0 + 1 or y <= y0 + 1 or x + w >= x1 - 1 or y + h >= y1 - 1):
                    rgba[labels == c, 3] = 0
    ys, xs = np.nonzero(rgba[:, :, 3] > 16)
    if not len(xs):
        raise ValueError(f'{path.name} is empty')
    return rgba[ys.min():ys.max() + 1, xs.min():xs.max() + 1]


def planes(rgba: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Colour premultiplied by alpha, with alpha as a fourth channel, and the binary mask."""
    a = rgba[:, :, 3:4].astype(np.float32) / 255.0
    img = np.concatenate([rgba[:, :, :3].astype(np.float32) * a, a * 255.0], axis=2)
    mask = (rgba[:, :, 3] > 16).astype(np.float32)
    return img, mask


def match(assembled: np.ndarray, part: np.ndarray, scales, step: float = 1.0, around=None, radius: int = 0):
    """Best (error, scale, x, y) of the part on the assembled asset; error is mean squared per masked pixel."""
    best = None
    a_img, _ = planes(assembled if step == 1 else cv2.resize(assembled, None, fx=1 / step, fy=1 / step, interpolation=cv2.INTER_AREA))
    for s in scales:
        f = s / step
        p = cv2.resize(part, None, fx=f, fy=f, interpolation=cv2.INTER_AREA if f < 1 else cv2.INTER_LINEAR)
        if p.shape[0] < 4 or p.shape[1] < 4 or p.shape[0] > a_img.shape[0] or p.shape[1] > a_img.shape[1]:
            continue
        p_img, p_mask = planes(p)
        if around is not None:
            x0 = max(0, int(around[0] / step) - radius); y0 = max(0, int(around[1] / step) - radius)
            x1 = min(a_img.shape[1], int(around[0] / step) + radius + p.shape[1]); y1 = min(a_img.shape[0], int(around[1] / step) + radius + p.shape[0])
            region = a_img[y0:y1, x0:x1]
            if region.shape[0] < p.shape[0] or region.shape[1] < p.shape[1]:
                continue
        else:
            x0 = y0 = 0
            region = a_img
        m4 = np.repeat(p_mask[:, :, None], 4, axis=2)
        res = cv2.matchTemplate(region, p_img, cv2.TM_SQDIFF, mask=m4) / max(1.0, float(p_mask.sum()))
        _, _, loc, _ = cv2.minMaxLoc(res)
        err = float(res[loc[1], loc[0]])
        if best is None or err < best[0]:
            best = (err, s, (x0 + loc[0]) * step, (y0 + loc[1]) * step)
    return best


def place(assembled: np.ndarray, part: np.ndarray, scales, fine=np.arange(0.95, 1.051, 0.01)):
    coarse = match(assembled, part, scales, step=4.0)
    if coarse is None:
        raise ValueError('part is larger than the assembled asset at every scale')
    _, s, x, y = coarse
    return match(assembled, part, s * np.asarray(fine), around=(x, y), radius=12)


def run(sheet: Path, receipt: Path | None = None, out_dir: Path | None = None, pivots: dict | None = None,
        places: dict | None = None, order: list | None = None, explicit_parts: list | None = None,
        explicit_ref: Path | None = None) -> dict:
    import math
    if out_dir is None:
        raise ValueError('out_dir is required')
    receipt_data = json.loads(receipt.read_text()) if (receipt and receipt.is_file()) else {}
    layout = receipt_data.get('rig_sheet')
    if explicit_parts:
        cells_count = len(explicit_parts) + 1
        cols = int(math.ceil(math.sqrt(cells_count)))
        rows = int(math.ceil(cells_count / cols))
        parts = explicit_parts
    elif layout:
        cols, rows, parts = layout['columns'], layout['rows'], layout['parts']
    else:
        raise ValueError('Provide either --parts (for an existing user sheet) or a valid rig_sheet --receipt')
    # Pivots given now ([u, v] in the part, by id) correct a joint without drawing a new sheet.
    parts = [{**p, 'pivot': (pivots or {}).get(p['id'], p.get('pivot'))} for p in parts]
    with Image.open(sheet) as im:
        w, h = im.size
    ids = ['assembled'] + [p['id'] for p in parts]
    frames = [{'label': pid, 'box': [i % cols * w // cols, i // cols * h // rows,
                                     (i % cols + 1) * w // cols, (i // cols + 1) * h // rows]}
              for i, pid in enumerate(ids)]
    cut_dir = out_dir / 'cut'
    manifest = cut_sheet(sheet, cut_dir, kind='single', layout={'frames': frames}, use_ocr=False)
    files = {f['label']: cut_dir / f['file'] for f in manifest['frames']}
    cells = {f['label']: [f['sourceOffset'][0], f['sourceOffset'][1],
                          f['sourceOffset'][0] + f['sourceBox'][2] - f['sourceBox'][0],
                          f['sourceOffset'][1] + f['sourceBox'][3] - f['sourceBox'][1]] for f in manifest['frames']}
    if order:  # back-to-front stacking given now; a part in front of its partner is backed by the partner's hidden art
        if sorted(order) != sorted(p['id'] for p in parts):
            raise ValueError(f'order must list every part once: {[p["id"] for p in parts]}')
        parts = sorted(parts, key=lambda p: order.index(p['id']))
    missing = [pid for pid in ids if pid not in files]
    if missing:
        raise ValueError(f'cells with nothing cut: {missing}')

    # What the rig shows at rest comes from the asset's own image (explicit_ref or the sheet's first reference)
    # when it has transparency; otherwise from the sheet's redrawn assembled cell.
    refs = [explicit_ref] if explicit_ref else [Path(r['path']) for r in receipt_data.get('references') or [] if r.get('path')]
    use_ref = bool(refs) and refs[0] is not None and refs[0].is_file() and Image.open(refs[0]).mode in ('RGBA', 'LA')
    assembled = trimmed(refs[0] if use_ref else files['assembled'])
    ah, aw = assembled.shape[:2]
    mx, my = int(aw * MARGIN), int(ah * MARGIN)
    canvas = (aw + 2 * mx, ah + 2 * my)
    placed, report = {}, []
    reference = np.zeros((canvas[1], canvas[0], 4), np.uint8)
    reference[my:my + ah, mx:mx + aw] = assembled
    ref_mask = reference[:, :, 3] > 16

    def put(spec, s, cx, cy, err, how):
        p = cv2.resize(trims[spec['id']], None, fx=s, fy=s, interpolation=cv2.INTER_AREA if s < 1 else cv2.INTER_LINEAR)
        layer = np.zeros((canvas[1], canvas[0], 4), np.uint8)
        cx, cy = int(round(cx)), int(round(cy))
        x0, y0 = max(0, cx), max(0, cy)
        x1, y1 = min(canvas[0], cx + p.shape[1]), min(canvas[1], cy + p.shape[0])
        layer[y0:y1, x0:x1] = p[y0 - cy:y1 - cy, x0 - cx:x1 - cx]
        placed[spec['id']] = layer
        report.append({'id': spec['id'], 'scale': round(float(s), 3), 'position': [cx, cy],
                       'size': [p.shape[1], p.shape[0]], 'match_error': round(err, 1), 'placed_by': how})

    # The sheet draws every part at one scale, though generators often enlarge them all to fill their cells.
    # Each part's own best scale over a wide range votes; the median is the sheet's scale, and every part is
    # then placed near it, so a part with much hidden artwork cannot settle at a wrong size.
    trims = {spec['id']: trimmed(files[spec['id']], cells[spec['id']]) for spec in parts}
    votes = [place(assembled, trims[spec['id']], np.geomspace(0.2, 4.0, 48))[1] for spec in parts]
    sheet_scale = float(np.median(votes))
    roots = [spec for spec in parts if not spec.get('parent')]
    for spec in parts:
        if spec in roots:
            continue
        if spec['id'] in (places or {}):  # a placement correction: the part's centre in canvas pixels
            cx, cy = places[spec['id']]
            h, w = trims[spec['id']].shape[:2]
            put(spec, sheet_scale, cx - w * sheet_scale / 2, cy - h * sheet_scale / 2, 0.0, 'given centre')
            continue
        err, s, x, y = place(assembled, trims[spec['id']], sheet_scale * np.linspace(0.9, 1.1, 9))
        put(spec, s, x + mx, y + my, err, 'colour')
    # A root carries the most hidden artwork, so a free scale search misjudges it. The sheet draws every
    # part at one scale, so the root takes the scale its children were placed at and only its position
    # is matched.
    child_scale = float(np.median([row['scale'] for row in report])) if report else sheet_scale
    for spec in roots:
        err, s, x, y = place(assembled, trims[spec['id']], [child_scale], fine=[1.0])
        put(spec, s, x + mx, y + my, err, 'colour at the children\'s scale')
    report.sort(key=lambda row: [p['id'] for p in parts].index(row['id']))

    def bounds(mask):
        ys, xs = np.nonzero(mask)
        return xs.min(), ys.min(), xs.max(), ys.max()

    by_id = {p['id']: p for p in parts}
    for row in report:
        spec = by_id[row['id']]
        mask = placed[row['id']][:, :, 3] > 16
        x0, y0, x1, y1 = bounds(mask)
        parent = spec.get('parent')
        if spec.get('pivot'):
            u, v = spec['pivot']
            pivot, source = [x0 + u * (x1 - x0), y0 + v * (y1 - y0)], 'given'
        elif parent:
            overlap = mask & cv2.dilate((placed[parent][:, :, 3] > 16).astype(np.uint8), np.ones((9, 9), np.uint8)).astype(bool)
            if overlap.any():
                ys, xs = np.nonzero(overlap)
                pivot, source = [float(xs.mean()), float(ys.mean())], 'overlap with parent'
            else:
                pivot, source = [(x0 + x1) / 2, (y0 + y1) / 2], 'centre (does not touch parent)'
        else:
            pivot, source = [(x0 + x1) / 2, (y0 + y1) / 2], 'centre'
        row.update(parent=parent, pivot=[round(pivot[0], 1), round(pivot[1], 1)], pivot_source=source)

    # Every visible pixel of the asset belongs to one part: of the parts whose placed sheet drawing covers it
    # and resembles it there, the one that matches best (smoothed, so regions are whole; nearer the front wins ties); a
    # pixel no part covers goes to the nearest owned pixel's part. A layer holds the asset's own pixels where
    # it owns them, and its sheet pixels only where they hide behind a joint partner in front (the painted
    # joint art a turning part uncovers). So the rig at rest is the asset itself.
    order = [p['id'] for p in parts]
    visible = reference[:, :, 3] > 0
    cost = np.full((len(order),) + visible.shape, np.inf, np.float32)
    loose = np.full_like(cost, np.inf)  # every covering part, for pixels no drawing resembles
    for k, pid in enumerate(order):
        cover = placed[pid][:, :, 3] > 16
        diff = np.abs(placed[pid][:, :, :3].astype(np.float32) - reference[:, :, :3].astype(np.float32)).mean(axis=2)
        smooth = cv2.GaussianBlur(np.where(cover, diff, 0).astype(np.float32), (0, 0), 6)
        weight = cv2.GaussianBlur(cover.astype(np.float32), (0, 0), 6)
        local = smooth / np.maximum(weight, 1e-3)
        claim = cover & (local < 45)  # a part claims only what its drawing resembles there
        cost[k][claim] = local[claim] - 3.0 * k
        loose[k][cover] = local[cover] - 3.0 * k
    owner = np.where(np.isfinite(cost).any(axis=0), cost.argmin(axis=0), -1)
    if (owner >= 0).any():  # a part's soft outline belongs to the part beside it
        dist, (iy, ix) = ndi.distance_transform_edt(owner < 0, return_indices=True)
        fringe = (owner < 0) & visible & (dist <= 6)
        owner[fringe] = owner[iy[fringe], ix[fringe]]
    reach = int(0.05 * max(canvas))
    for spec in roots:  # the root is the rest of the body: unresembled pixels near it are its own
        k = order.index(spec['id'])
        near = cv2.dilate((placed[spec['id']][:, :, 3] > 16).astype(np.uint8), np.ones((2 * reach + 1,) * 2, np.uint8)).astype(bool)
        owner[(owner < 0) & near] = k
    owner = np.where(owner >= 0, owner, np.where(np.isfinite(loose).any(axis=0), loose.argmin(axis=0), -1))
    owner[~visible] = -1
    for k in range(len(order)):  # specks of a part inside another part's region go to their surroundings
        n, labels, stats, _ = cv2.connectedComponentsWithStats((owner == k).astype(np.uint8), connectivity=8)
        if n > 1:
            keep = stats[1:, cv2.CC_STAT_AREA].max() * 0.05
            for c in range(1, n):
                if stats[c, cv2.CC_STAT_AREA] < keep:
                    owner[labels == c] = -1
    gaps = visible & (owner < 0)
    if gaps.any() and (owner >= 0).any():
        _, (iy, ix) = ndi.distance_transform_edt(owner < 0, return_indices=True)
        owner[gaps] = owner[iy[gaps], ix[gaps]]
    layers = {}
    for k, pid in enumerate(order):
        layer = np.zeros_like(reference)
        own = owner == k
        layer[own] = reference[own]
        # Hidden art is kept only behind the part's joint partners (its parent or children); behind an
        # unrelated part it would show as a stray fragment when that part moves.
        spec = parts[k]
        partners = [order.index(q['id']) for q in parts if q.get('parent') == pid] + ([order.index(spec['parent'])] if spec.get('parent') else [])
        behind_front = (placed[pid][:, :, 3] > 16) & (owner > k) & np.isin(owner, partners)
        layer[behind_front] = placed[pid][behind_front]
        layers[pid] = layer
        row = next(r for r in report if r['id'] == pid)
        row['owned_px'], row['hidden_fill_px'] = int(own.sum()), int(behind_front.sum())
    # Joint backing: near each joint, how much of the front part's region has the part behind it drawn
    # underneath, so turning the front part does not open a hole.
    radius = 0.06 * max(canvas)
    yy, xx = np.mgrid[:canvas[1], :canvas[0]]
    for row in report:
        if not row['parent']:
            continue
        a, b = sorted([row['id'], row['parent']], key=order.index)  # a behind, b in front
        near = (xx - row['pivot'][0]) ** 2 + (yy - row['pivot'][1]) ** 2 <= radius ** 2
        front = near & (owner == order.index(b))
        row['joint_backing'] = round(float((layers[a][:, :, 3][front] > 16).mean()) if front.any() else 0.0, 3)
    placed = layers

    out_dir.mkdir(parents=True, exist_ok=True)
    composite = Image.new('RGBA', canvas)
    for spec in parts:  # back to front
        Image.fromarray(placed[spec['id']]).save(out_dir / f"{spec['id']}.png")
        composite.alpha_composite(Image.fromarray(placed[spec['id']]))
    Image.fromarray(reference).save(out_dir / 'assembled.png')
    comp = np.array(composite)
    comp_mask = comp[:, :, 3] > 16
    iou = float((ref_mask & comp_mask).sum() / max(1, (ref_mask | comp_mask).sum()))
    both = ref_mask & comp_mask
    colour = float(np.abs(comp[both][:, :3].astype(int) - reference[both][:, :3].astype(int)).mean()) if both.any() else 255.0
    check = Image.new('RGBA', (canvas[0] * 2, canvas[1]), (40, 40, 40, 255))
    check.alpha_composite(Image.fromarray(reference), (0, 0))
    check.alpha_composite(composite, (canvas[0], 0))
    draw_pivots = np.array(check)
    for row in report:
        cv2.circle(draw_pivots, (int(row['pivot'][0]) + canvas[0], int(row['pivot'][1])), 7, (255, 0, 255, 255), 2)
    Image.fromarray(draw_pivots).save(out_dir / 'check.png')
    # Review sheet: each layer alone on grey, back to front, with its pivot marked.
    tiles = []
    for row in report:
        tile = Image.new('RGBA', canvas, (90, 90, 90, 255))
        tile.alpha_composite(Image.fromarray(placed[row['id']]))
        t = np.array(tile)
        cv2.circle(t, (int(row['pivot'][0]), int(row['pivot'][1])), 9, (255, 0, 255, 255), 3)
        cv2.putText(t, row['id'], (12, 40), cv2.FONT_HERSHEY_SIMPLEX, 1.2, (255, 255, 255, 255), 2)
        tiles.append(t)
    Image.fromarray(np.concatenate(tiles, axis=1)).save(out_dir / 'layers.png')
    result = {'ok': True, 'sheet': str(sheet), 'receipt': str(receipt) if receipt and receipt.is_file() else None,
              'provenance': 'generated_rig_sheet' if (receipt and receipt.is_file()) else 'existing_user_sheet',
              'canvas': list(canvas), 'sheet_scale': round(sheet_scale, 3),
              'rest_pose_from': str(refs[0]) if use_ref else 'sheet assembled cell',
              'order': [p['id'] for p in parts], 'parts': report,
              'rebuild': {'silhouette_iou': round(iou, 4), 'mean_colour_difference': round(colour, 1)},
              'check_image': str(out_dir / 'check.png'), 'layers_image': str(out_dir / 'layers.png'), 'assembled': str(out_dir / 'assembled.png'),
              'cut_warnings': manifest.get('warnings', [])}
    (out_dir / 'placement.json').write_text(json.dumps(result, indent=2))
    return result


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--sheet', required=True, type=Path)
    ap.add_argument('--receipt', default=None, type=Path, help="optional generation.json for generated sheets")
    ap.add_argument('--parts', type=json.loads, default=None, help='explicit JSON [{id, parent, pivot}] for existing user sheets without generation.json')
    ap.add_argument('--reference', default=None, type=Path, help='optional assembled reference PNG for existing user sheets')
    ap.add_argument('--out-dir', required=True, type=Path)
    ap.add_argument('--pivots', type=json.loads, default=None, help='JSON {part id: [u, v]} joint corrections')
    ap.add_argument('--places', type=json.loads, default=None, help='JSON {part id: [cx, cy]} placement corrections in canvas pixels')
    ap.add_argument('--order', type=json.loads, default=None, help='JSON [part ids back to front]')
    a = ap.parse_args(argv)
    print(json.dumps(run(
        a.sheet.resolve(),
        a.receipt.resolve() if a.receipt else None,
        a.out_dir.resolve(),
        a.pivots,
        a.places,
        a.order,
        a.parts,
        a.reference.resolve() if a.reference else None,
    )))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
