"""Offline pose/view sprites and editable articulated cutouts.

Run: python3 -m pipeline.pose_and_part_cutter --help
No network/model calls. Automatic masks contain visible pixels only; hidden joint
artwork must come from a reviewed exploded sheet. Coordinates are source pixels,
origin top-left; output paths are relative to the manifest directory.
"""
from __future__ import annotations

import argparse
import csv
import io
import json
import math
import hashlib
import re
import shutil
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageOps
from scipy import ndimage as ndi

# The exploded-sheet runtime template is versioned with the package (not the
# gitignored exports/ tree) so sheet cutting works from a clean checkout.
TEMPLATE = Path(__file__).resolve().parent / 'templates' / 'exploded_part_sheet_spec.json'
# Optional user/vault-specific layouts stay at the legacy export location and are
# not required; sheets without a preset layout fall back to auto-detection.
VAULT_LAYOUTS = Path(__file__).resolve().parents[1] / 'exports' / 'templates' / 'vault_sheet_layouts.json'
ACTIONS = ['standing', 'walking', 'running', 'jumping', 'dancing', 'waving',
           'clapping', 'looking_up', 'looking_down', 'arms_wide']
VIEWS = ['front', 'quarter_front', 'side', 'opposite_quarter', 'back']
PARTS = ['head', 'ears', 'torso', 'arm_L', 'arm_R', 'leg_L', 'leg_R', 'prop_backpack']
ALIASES = {
    'standing': ['standing', 'stand'], 'walking': ['walking', 'walk'],
    'running': ['running', 'run'], 'jumping': ['jumping', 'jump'],
    'dancing': ['dancing', 'dance'], 'waving': ['waving', 'wave'],
    'clapping': ['clapping', 'clap'], 'looking_up': ['looking upward', 'looking upwards', 'looking up'],
    'looking_down': ['looking downward', 'looking downwards', 'looking down'],
    'arms_wide': ['arms open wide', 'arms wide open', 'arms wide', 'arms out', 'arms spread'],
    'pointing': ['pointing', 'point'], 'sitting': ['sitting', 'sit'],
    'winking': ['winking', 'wink'],
}


def exploded_prompt(character: str, description: str = '') -> str:
    return json.loads(TEMPLATE.read_text())['promptTemplate'].format(
        character=character, description=description or 'use supplied reference')


import os
import tempfile

from pipeline.craft_cli import find_craft_cli

def _find_photocraft_cli() -> str | None:
    """Return path to photocraft-cli >= 0.10.0 if available locally."""
    cand = find_craft_cli('photocraft-cli')
    if not cand:
        return None
    try:
        proc = subprocess.run([cand, '--version'], capture_output=True, text=True, timeout=5)
    except OSError:
        return None
    if '0.10.' in proc.stdout or '0.11.' in proc.stdout or '1.' in proc.stdout:
        return cand
    return None


def _find_vectorcraft_cli() -> str | None:
    return find_craft_cli('vectorcraft-cli')


def _background_candidates(rgb, tolerance):
    h, w = rgb.shape[:2]
    smooth = ndi.gaussian_filter(rgb.astype(np.float32), (1.2, 1.2, 0))
    pixels = smooth[::max(1, h//200), ::max(1, w//200)].reshape(-1, 3)
    # Modal color rather than mean: cream fur and dark headings must not shift
    # the background estimate. Smooth first to handle linen/paper textures.
    quantized = (pixels/16).astype(int)
    values, counts = np.unique(quantized, axis=0, return_counts=True)
    modal = values[counts.argmax()]
    bg = np.median(pixels[np.all(quantized == modal, axis=1)], axis=0)
    return np.max(np.abs(smooth-bg), axis=2) <= tolerance


def _matte(image: Image.Image, tolerance: int = 25, use_photocraft: bool = False) -> tuple[Image.Image, np.ndarray]:
    """Step 1 background removal via PhotoCraft 0.10.0 4-corner contiguous magicEraser
    with interior fur/collar preservation and enclosed background hole cleanup.
    """
    arr = np.array(image.convert('RGBA'))
    if arr[:, :, 3].min() < 255:
        return Image.fromarray(arr), arr[:, :, 3] > 16
    rgb = arr[:, :, :3].astype(np.int16)
    h, w = rgb.shape[:2]

    # Estimate studio background from the outer border ring
    border_pixels = np.concatenate([rgb[0, :, :], rgb[-1, :, :], rgb[:, 0, :], rgb[:, -1, :]], axis=0)
    bg_med = np.median(border_pixels, axis=0)
    color_diff = np.max(np.abs(rgb.astype(np.float32) - bg_med), axis=2)
    chroma = np.max(rgb, axis=2) - np.min(rgb, axis=2)
    warmth = rgb[:, :, 0] - rgb[:, :, 2]

    # Studio backgrounds (white or gray gradient) are neutral (low chroma & warmth),
    # whereas character cream fur has warmth (R - B >= 12) or chroma >= 12.
    eff_tol = min(tolerance, 22)
    bg_cand = (color_diff <= eff_tol) & (chroma <= 10) & (warmth <= 10)

    seeds = np.zeros((h, w), bool)
    seeds[0, :] = bg_cand[0, :]
    seeds[-1, :] = bg_cand[-1, :]
    seeds[:, 0] = bg_cand[:, 0]
    seeds[:, -1] = bg_cand[:, -1]
    if not seeds.any():
        candidates = _background_candidates(rgb, eff_tol)
        labs, _ = ndi.label(candidates)
        areas = np.bincount(labs.ravel()); areas[0] = 0
        seeds = labs == areas.argmax()
        bg_cand = candidates

    exterior_bg = ndi.binary_propagation(seeds, mask=bg_cand)
    # Also clear any enclosed studio-background holes (e.g. between legs/arms) that are neutral studio gray/white
    strict_hole_bg = (color_diff <= eff_tol) & (chroma <= 8) & (warmth <= 8)
    # Protect small interior highlights (eyes, shirt collar) by only clearing enclosed holes in the lower 55% or > 80 px
    enclosed = strict_hole_bg & (~exterior_bg)
    enc_labs, n_enc = ndi.label(enclosed)
    for cid in range(1, n_enc + 1):
        pts_y, _ = np.where(enc_labs == cid)
        if len(pts_y) >= 55 and pts_y.mean() > h * 0.48:
            exterior_bg |= (enc_labs == cid)

    flood_mask = ~exterior_bg

    pc_bin = _find_photocraft_cli() if use_photocraft and min(h, w) >= 48 else None
    if pc_bin and flood_mask.any():
        try:
            with tempfile.TemporaryDirectory() as tmpd:
                in_p = Path(tmpd) / 'in.png'
                out_p = Path(tmpd) / 'out.png'
                image.save(in_p)
                pc_tol = 12 if np.mean(bg_med) > 244 else int(eff_tol)
                proc = subprocess.run(
                    [
                        pc_bin, 'run', str(in_p),
                        '--cmd', 'layer.new.layerFromBackground', '--params', '{"ifPresent":true}',
                        '--cmd', 'paint.magicEraser', '--params', json.dumps({'x': 1, 'y': 1, 'tolerance': pc_tol, 'antiAlias': True, 'contiguous': True}),
                        '--cmd', 'paint.magicEraser', '--params', json.dumps({'x': int(w - 2), 'y': 1, 'tolerance': pc_tol, 'antiAlias': True, 'contiguous': True}),
                        '--cmd', 'paint.magicEraser', '--params', json.dumps({'x': 1, 'y': int(h - 2), 'tolerance': pc_tol, 'antiAlias': True, 'contiguous': True}),
                        '--cmd', 'paint.magicEraser', '--params', json.dumps({'x': int(w - 2), 'y': int(h - 2), 'tolerance': pc_tol, 'antiAlias': True, 'contiguous': True}),
                        '--out', str(out_p),
                    ],
                    capture_output=True, text=True, timeout=20,
                )
                if proc.returncode == 0 and out_p.is_file():
                    pc_arr = np.array(Image.open(out_p).convert('RGBA'))
                    if pc_arr.shape[:2] == (h, w):
                        soft_alpha = np.where(flood_mask, 255, 0).astype(np.uint8)
                        aa_edge = (pc_arr[:, :, 3] > 0) & (pc_arr[:, :, 3] < 250) & ndi.binary_dilation(flood_mask, iterations=1) & (~ndi.binary_erosion(flood_mask, iterations=1))
                        soft_alpha[aa_edge] = pc_arr[aa_edge, 3]
                        arr[:, :, 3] = soft_alpha
                        return Image.fromarray(arr), soft_alpha > 16
        except (subprocess.SubprocessError, OSError):
            pass

    arr[:, :, 3] = flood_mask.astype(np.uint8) * 255
    return Image.fromarray(arr), flood_mask


def _components(mask: np.ndarray, kind: str) -> list[tuple[int, int, int, int]]:
    h, w = mask.shape
    labels, _ = ndi.label(mask)
    boxes = []
    areas = np.bincount(labels.ravel())
    for i, obj in enumerate(ndi.find_objects(labels), 1):
        if obj is None:
            continue
        ys, xs = obj
        bh, bw = ys.stop-ys.start, xs.stop-xs.start
        if (areas[i] < h*w*0.0012 or bh < h*(0.28 if kind == 'turnaround' else 0.105)
                or bw < w*0.018 or bw > w*0.55 or bh > h*0.94):
            continue
        boxes.append((xs.start, ys.start, xs.stop, ys.stop))
    # Detached appendages near a body can be merged, but never merge neighbors.
    boxes.sort(key=lambda b: (b[1]+b[3])/2)
    rows: list[list] = []
    for box in boxes:
        cy = (box[1]+box[3])/2
        row = next((r for r in rows if abs(cy-np.median([(b[1]+b[3])/2 for b in r])) < h*0.15), None)
        if row is None:
            rows.append([box])
        else:
            row.append(box)
    return [b for row in rows for b in sorted(row, key=lambda b: b[0])]


def _ocr_labels(source: Path) -> list[dict]:
    if not shutil.which('tesseract'):
        return []
    proc = subprocess.run(['tesseract', str(source), 'stdout', '--psm', '11', 'tsv'],
                          capture_output=True, text=True, timeout=45, check=True)
    lines: dict[tuple, list] = {}
    for word in csv.DictReader(io.StringIO(proc.stdout), delimiter='\t'):
        if not word.get('text', '').strip() or float(word.get('conf', '-1')) < 15:
            continue
        key = tuple(word[k] for k in ('page_num', 'block_num', 'par_num', 'line_num'))
        lines.setdefault(key, []).append(word)
    found = []
    for words in lines.values():
        text = ' '.join(w['text'] for w in words).lower()
        normalized = re.sub('[^a-z ]', ' ', text)
        for label, aliases in ALIASES.items():
            if any(re.search(r'\b'+re.escape(a)+r'\b', normalized) for a in aliases):
                left = min(int(w['left']) for w in words)
                right = max(int(w['left'])+int(w['width']) for w in words)
                top = min(int(w['top']) for w in words)
                found.append({'label': label, 'x': (left+right)/2, 'y': top,
                              'text': text, 'confidence': min(float(w['conf']) for w in words)/100,
                              'engine': 'tesseract'})
                break
    return found


def _vlm_ocr_label(crop_with_caption: Image.Image) -> dict | None:
    """Use local Ollama vision LLM (llava-llama3:latest at 127.0.0.1:11434) to read caption or classify pose."""
    import base64
    import urllib.request
    buf = io.BytesIO()
    thumb = crop_with_caption.convert('RGB')
    thumb.thumbnail((320, 380))
    thumb.save(buf, format='JPEG', quality=82)
    b64 = base64.b64encode(buf.getvalue()).decode('ascii')
    prompt = (
        "Read any printed label under this character or classify the pose into exactly ONE word from: "
        "standing, walking, running, jumping, dancing, waving, pointing, sitting, clapping, looking_up, looking_down, arms_wide."
    )
    payload = json.dumps({
        'model': os.environ.get('ANIMATIONCRAFT_OCR_VLM', 'llava-llama3:latest'),
        'prompt': prompt,
        'images': [b64],
        'stream': False,
    }).encode('utf-8')
    req = urllib.request.Request(
        os.environ.get('OLLAMA_URL', 'http://127.0.0.1:11434/api/generate'),
        data=payload,
        headers={'Content-Type': 'application/json'},
        method='POST',
    )
    try:
        with urllib.request.urlopen(req, timeout=4.5) as resp:
            data = json.loads(resp.read().decode('utf-8'))
            raw_text = re.sub(r'[^a-z_ ]', ' ', data.get('response', '').lower())
            for label, aliases in ALIASES.items():
                if label in raw_text or any(a in raw_text for a in aliases):
                    return {'label': label, 'text': raw_text.strip(), 'confidence': 0.85, 'engine': 'ollama-vlm'}
    except Exception:
        return None
    return None


def _validate_box(box, size):
    if not isinstance(box, (list, tuple)) or len(box) != 4:
        raise ValueError('box must be [left, top, right, bottom] in source pixels')
    if any(not isinstance(v, (int, float)) or not math.isfinite(v) for v in box):
        raise ValueError('box coordinates must be finite numbers')
    x0, y0, x1, y1 = map(int, box)
    if not (0 <= x0 < x1 <= size[0] and 0 <= y0 < y1 <= size[1]):
        raise ValueError(f'box outside source: {box}')
    return x0, y0, x1, y1


def _clean_crop(source: Image.Image, box, tolerance: int, use_photocraft: bool = False):
    # Alpha provenance belongs to the source, not the crop: an opaque torso
    # crop of an already-matted PNG must not be mistaken for a background.
    if source.getchannel('A').getextrema()[0] < 255:
        crop = source.crop(box)
        if crop.getchannel('A').getbbox() is None:
            raise ValueError(f'No foreground in frame {box}')
        return crop
    crop, mask = _matte(source.crop(box), tolerance, use_photocraft=use_photocraft)
    labels, count = ndi.label(mask)
    if not count:
        raise ValueError(f'No foreground in frame {box}')
    sizes = np.bincount(labels.ravel())
    sizes[0] = 0
    # Remove text and crumbs without throwing away detached ears/hands.
    main = int(sizes.argmax())
    main_y, main_x = np.where(labels == main)
    # Captions can be bigger than tiny facial details. Only retain detached
    # islands within the main character's extent, or substantial separate props.
    keep = sizes >= max(12, sizes.max()*0.10)
    for component, obj in enumerate(ndi.find_objects(labels), 1):
        if obj is None or sizes[component] < max(12, sizes.max()*.008):
            continue
        ys,xs = obj
        if (xs.start >= main_x.min() and xs.stop <= main_x.max()+1 and
                ys.start >= main_y.min() and ys.stop <= main_y.max()+1):
            keep[component] = True
    keep[0] = False
    mask = keep[labels]
    if not mask.any():
        raise ValueError('No foreground after cleanup')
    arr = np.array(crop)
    arr[~mask, 3] = 0
    # Edge decontamination without erosion of thin appendages.
    core = ndi.binary_erosion(mask, iterations=2)
    if core.any():
        _, near = ndi.distance_transform_edt(~core, return_indices=True)
        ring = mask & ~core
        arr[ring, :3] = arr[near[0][ring], near[1][ring], :3]
    if mask.any():
        dist, near = ndi.distance_transform_edt(~mask, return_indices=True)
        bleed = (~mask) & (dist <= 3)
        arr[bleed, :3] = arr[near[0][bleed], near[1][bleed], :3]
    return Image.fromarray(arr)


def _save_parts(image, source_box, frame_offset, definitions, output, index):
    parts = {}
    parents = {s['id']: s['parent'] for s in json.loads(TEMPLATE.read_text())['slots']}
    for name in PARTS:
        spec = definitions.get(name)
        if not spec:
            parts[name] = {'status': 'unavailable', 'reason': 'No isolated artwork or explicit mask'}
            continue
        polygon = spec.get('polygon', [])
        if len(polygon) < 3 or any(len(p) != 2 or any(not math.isfinite(v) for v in p) for p in polygon):
            raise ValueError(f'Invalid polygon for {name}')
        x0,y0,x1,y1 = source_box
        if any(not (x0 <= x <= x1 and y0 <= y <= y1) for x,y in polygon):
            raise ValueError(f'{name} polygon outside frame source box')
        pivot = spec.get('pivot')
        if not pivot or len(pivot) != 2 or any(not math.isfinite(v) for v in pivot):
            raise ValueError(f'Invalid pivot for {name}')
        mask_img = Image.new('L', image.size)
        ImageDraw.Draw(mask_img).polygon([(x-x0,y-y0) for x,y in polygon], fill=255)
        arr = np.array(image)
        arr[:,:,3] = np.minimum(arr[:,:,3], np.array(mask_img))
        part = Image.fromarray(arr)
        bounds = part.getchannel('A').getbbox()
        if not bounds:
            parts[name] = {'status': 'unavailable', 'reason': 'Mask contains no visible pixels'}
            continue
        px,py = bounds[:2]
        filename = f'parts/{index:02d}_{name}.png'
        part.crop(bounds).save(output / filename)
        ox,oy = frame_offset
        parts[name] = {'file': filename, 'size': [bounds[2]-px,bounds[3]-py],
            'status': 'ready' if spec.get('complete') and not spec.get('inferred') else 'needs_review',
            'parent': parents[name], 'sourcePivot': list(pivot),
            'pivot': [pivot[0]-x0-px,pivot[1]-y0-py],
            'bindPosition': [pivot[0]-x0+ox,pivot[1]-y0+oy],
            'sourcePolygon': polygon, 'visiblePixelsOnly': not bool(spec.get('complete')),
            'inferred': bool(spec.get('inferred'))}
    return parts


RUNTIME = '''// Pixel units. Serve this folder over HTTP. Three.js is supplied by caller.
export async function loadManifest(url) {
  const r = await fetch(url); if (!r.ok) throw Error(`Manifest HTTP ${r.status}`);
  const m = await r.json(); m.baseURL = new URL('.', new URL(url, location.href)); return m;
}
export function threeFrame(THREE, texture, manifest, index=0) {
  const f = manifest.frames[index], [w,h] = f.size;
  const map = texture.clone(); map.needsUpdate = true;
  map.repeat.set(f.uv[2], f.uv[3]); map.offset.set(f.uv[0], f.uv[1]);
  const sprite = new THREE.Sprite(new THREE.SpriteMaterial({map, transparent:true, depthWrite:false}));
  sprite.scale.set(w,h,1); sprite.center.set(f.anchor[0]/w, 1-f.anchor[1]/h); return sprite;
}
export function p5Frame(p, atlas, manifest, index, x, y) {
  const f=manifest.frames[index], [sx,sy,sw,sh]=f.atlasRect;
  p.push(); p.imageMode(p.CORNER);
  p.image(atlas, x-f.anchor[0], y-f.anchor[1], sw,sh, sx,sy,sw,sh); p.pop();
}
// angles in radians, clockwise in image space. Unavailable parts are skipped.
export function p5Puppet(p, images, frame, angles={}, x=0, y=0) {
  const parts=frame.parts;
  function transform(name) {
    const a=parts[name], b=a.bindPosition;
    if (a.parent && parts[a.parent]?.file) {
      transform(a.parent); const pb=parts[a.parent].bindPosition;
      p.translate(b[0]-pb[0], b[1]-pb[1]);
    } else p.translate(b[0], b[1]);
    p.rotate(angles[name] || 0);
  }
  p.push(); p.translate(x-frame.anchor[0], y-frame.anchor[1]); p.imageMode(p.CORNER);
  for (const name of ['prop_backpack','leg_R','leg_L','arm_R','torso','arm_L','head','ears']) {
    const a=parts[name]; if (!a?.file || !images[name]) continue;
    p.push(); transform(name); p.image(images[name],-a.pivot[0],-a.pivot[1]); p.pop();
  } p.pop();
}
export function threePuppet(THREE, textures, frame) {
  const root=new THREE.Group(), joints={}, parts=frame.parts;
  function joint(name) {
    if (joints[name]) return joints[name];
    const a=parts[name], g=new THREE.Group(), parent=parts[a.parent]; joints[name]=g;
    const b=a.bindPosition, pb=parent?.file ? parent.bindPosition : frame.anchor;
    g.position.set(b[0]-pb[0], -(b[1]-pb[1]), 0);
    (parent?.file ? joint(a.parent) : root).add(g); return g;
  }
  const order=['prop_backpack','leg_R','leg_L','arm_R','torso','arm_L','head','ears'];
  for (const name of order) {
    const a=parts[name]; if (!a?.file || !textures[name]) continue;
    const material=new THREE.MeshBasicMaterial({map:textures[name],transparent:true,depthWrite:false,side:THREE.DoubleSide});
    const [w,h]=a.size, mesh=new THREE.Mesh(new THREE.PlaneGeometry(w,h),material);
    mesh.position.set(w/2-a.pivot[0], -(h/2-a.pivot[1]), 0);
    mesh.renderOrder=order.indexOf(name); joint(name).add(mesh);
  }
  root.userData.joints=joints; return root; // rotate joints[name].rotation.z (CCW).
}
'''


def _write_vector_limb_masks_svg(frame_file: str, size: tuple[int, int], source_box, frame_offset, definitions: dict, output: Path, index: int) -> str:
    """Write an editable SVG with VectorCraft-compatible clipPaths and limb polygons over the raster frame."""
    cw, ch = size
    x0, y0, _, _ = source_box
    ox, oy = frame_offset
    svg_rel = f'parts/{index:02d}_limb_masks.svg'
    polys = []
    for name, spec in definitions.items():
        polygon = spec.get('polygon', [])
        if len(polygon) < 3:
            continue
        pts = ' '.join(f'{px - x0 + ox:.1f},{py - y0 + oy:.1f}' for px, py in polygon)
        piv = spec.get('pivot', [x0, y0])
        piv_x, piv_y = piv[0] - x0 + ox, piv[1] - y0 + oy
        polys.append(
            f'  <g id="limb_{name}" data-part="{name}" data-pivot="{piv_x:.1f},{piv_y:.1f}">\n'
            f'    <clipPath id="clip_{name}"><polygon points="{pts}"/></clipPath>\n'
            f'    <polygon id="mask_path_{name}" points="{pts}" fill="none" stroke="#ff7a18" stroke-width="1.5" stroke-dasharray="4,3"/>\n'
            f'    <circle cx="{piv_x:.1f}" cy="{piv_y:.1f}" r="3.5" fill="#38bdf8"/>\n'
            f'  </g>'
        )
    svg_text = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {cw} {ch}" width="{cw}" height="{ch}">\n'
        f'  <image id="felt_artwork" href="../{frame_file}" x="0" y="0" width="{cw}" height="{ch}"/>\n'
        + '\n'.join(polys) + '\n</svg>\n'
    )
    (output / svg_rel).write_text(svg_text)
    return svg_rel


def cut_sheet(source, out_dir, kind='auto', layout=None, use_ocr=True, tolerance=25, use_photocraft=True):
    source, output = Path(source).resolve(), Path(out_dir).resolve()
    if not source.is_file():
        raise FileNotFoundError(source)
    if not 1 <= tolerance <= 100:
        raise ValueError('tolerance must be in 1..100')
    with Image.open(source) as src:
        image = ImageOps.exif_transpose(src).convert('RGBA')
    if kind == 'auto':
        kind = 'action' if source.name.startswith('Create_pose_sheet') else 'turnaround' if 'turnaround' in source.name.lower() else 'single'
    if kind not in ('action', 'turnaround', 'single', 'exploded'):
        raise ValueError(f'Unknown sheet kind: {kind}')
    if isinstance(layout, (str, Path)):
        layout = json.loads(Path(layout).read_text())
    if layout is None and VAULT_LAYOUTS.exists():
        preset = json.loads(VAULT_LAYOUTS.read_text()).get('sheets', {}).get(source.name)
        if preset and hashlib.sha256(source.read_bytes()).hexdigest() == preset['sha256']:
            layout = preset
    w,h = image.size
    warnings = list((layout or {}).get('warnings', []))
    definitions = (layout or {}).get('frames')
    if definitions is None:
        if kind == 'exploded':
            spec = json.loads(TEMPLATE.read_text())
            definitions = [{'label': s['id'], 'box': [s['cell'][0]*w//3,s['cell'][1]*h//3,
                            (s['cell'][0]+1)*w//3,(s['cell'][1]+1)*h//3]} for s in spec['slots']]
        elif kind == 'single':
            definitions = [{'label':'single','box':[0,0,w,h]}]
        else:
            raw = np.array(image)
            if raw[:,:,3].min() < 255:
                mask = raw[:,:,3] > 16
                # An import/matting derivative can leave the background of
                # grid-separated panels opaque, collapsing a sheet into one
                # blob. Only trust the alpha when it separates the figures;
                # otherwise fall back to colour-based background detection.
                if kind in ('action', 'turnaround') and (
                    mask.mean() > 0.5 or not _components(mask, kind)
                ):
                    mask = ndi.binary_opening(
                        mask & ~_background_candidates(raw[:,:,:3], tolerance),
                        iterations=2,
                    )
            else:
                mask = ~_background_candidates(raw[:,:,:3], tolerance)
                # Separate characters linked by thin printed grid lines.
                mask = ndi.binary_opening(mask, iterations=2)
            boxes = _components(mask, kind)
            if not boxes:
                raise ValueError('No foreground characters detected; supply --layout')
            labels = []
            if use_ocr and kind == 'action':
                try:
                    labels = _ocr_labels(source)
                except (subprocess.SubprocessError, OSError) as exc:
                    warnings.append(f'OCR unavailable: {type(exc).__name__}; supply labels via --layout')
            for i,b in enumerate(boxes):
                # Margin only 3px: avoid neighboring labels and adjacent characters.
                box = [max(0,b[0]-3),max(0,b[1]-3),min(w,b[2]+3),min(h,b[3]+3)]
                candidates = [l for l in labels if abs(l['x']-(b[0]+b[2])/2) < (b[2]-b[0])*.65
                              and b[3]-8 <= l['y'] <= b[3]+h*.11]
                match = min(candidates, key=lambda l: abs(l['y']-b[3])+abs(l['x']-(b[0]+b[2])/2), default=None)
                if match is None and use_ocr and kind == 'action':
                    caption_crop = image.crop([box[0], box[1], box[2], min(h, int(box[3] + h * 0.11))])
                    match = _vlm_ocr_label(caption_crop)
                label = VIEWS[i] if kind == 'turnaround' and len(boxes)==5 else match['label'] if match else f'pose_{i:02d}'
                definitions = definitions or []
                definitions.append({'box':box,'label':label,
                                    'labelSource':'turnaround_order_assumption' if kind=='turnaround' and len(boxes)==5 else 'ocr' if match else 'unlabelled',
                                    'ocr':match})
    if not definitions:
        raise ValueError('No foreground frames specified')
    if len(definitions) > 64:
        raise ValueError('Over 64 frames detected; supply --layout')
    crops = []
    missing_parts = []
    pc_bin = _find_photocraft_cli() if use_photocraft else None
    vc_bin = _find_vectorcraft_cli()
    for item in definitions:
        box = _validate_box(item['box'], image.size)
        try:
            crop = _clean_crop(image,box,tolerance,use_photocraft=bool(pc_bin))
            if not item.get('parts'):
                ca = np.array(crop)[:, :, 3] > 20
                bx0, by0, bx1, by1 = box
                expanded = (
                    max(0, bx0 - (4 if ca[:, 0].any() else 0)),
                    max(0, by0 - (4 if ca[0, :].any() else 0)),
                    min(w, bx1 + (4 if ca[:, -1].any() else 0)),
                    min(h, by1 + (4 if ca[-1, :].any() else 0)),
                )
                if expanded != box:
                    box = expanded
                    crop = _clean_crop(image, box, tolerance, use_photocraft=bool(pc_bin))
        except ValueError:
            if kind == 'exploded' and item.get('label') in ('ears', 'prop_backpack'):
                missing_parts.append(item['label'])
                continue
            raise
        if item.get('excludePolygons'):
            exclusions = Image.new('L',crop.size)
            for polygon in item['excludePolygons']:
                if len(polygon) < 3:
                    raise ValueError('excludePolygons need at least three points')
                ImageDraw.Draw(exclusions).polygon([(x-box[0],y-box[1]) for x,y in polygon],fill=255)
            arr = np.array(crop)
            arr[np.array(exclusions)>0,3] = 0
            crop = Image.fromarray(arr)
        crops.append((item,box,crop))
    padding = 8
    cw = max(c.width for _,_,c in crops)+padding*2
    ch = max(c.height for _,_,c in crops)+padding*2
    columns = min(5, len(crops))
    rows = math.ceil(len(crops)/columns)
    atlas = Image.new('RGBA',(cw*columns,ch*rows))
    contact = Image.new('RGB',(cw*columns,(ch+28)*rows),'#d3d4d6')
    draw = ImageDraw.Draw(contact)
    output.mkdir(parents=True, exist_ok=True)
    (output/'frames').mkdir(exist_ok=True)
    (output/'parts').mkdir(exist_ok=True)
    frames = []
    editable = {'frames':[], 'puppet':(layout or {}).get('puppet',{})}
    for i,(item,box,crop) in enumerate(crops):
        ox,oy = (cw-crop.width)//2, ch-padding-crop.height
        # Preserve source vertical location for jumping/action poses when requested.
        if 'offset' in item:
            ox,oy = item['offset']
            if not (isinstance(ox,int) and isinstance(oy,int) and
                    0 <= ox <= cw-crop.width and 0 <= oy <= ch-crop.height):
                raise ValueError('offset would clip frame outside aligned canvas')
        aligned = Image.new('RGBA',(cw,ch))
        aligned.paste(crop,(ox,oy))
        name = re.sub(r'[^a-zA-Z0-9_]+','_',str(item.get('label',f'pose_{i:02d}')))
        file = f'frames/{i:02d}_{name}.png'
        aligned.save(output/file)
        ax,ay = i%columns*cw, i//columns*ch
        atlas.paste(aligned,(ax,ay))
        cy = i//columns*(ch+28)
        contact.paste(aligned,(ax,cy),aligned.getchannel('A'))
        draw.text((ax+6,cy+ch+5),f'{i}: {name}',fill='#17181a')
        parts_def = item.get('parts') or {}
        parts = _save_parts(crop,box,(ox,oy),parts_def,output,i) if parts_def else {}
        svg_mask_file = _write_vector_limb_masks_svg(file, (cw, ch), box, (ox, oy), parts_def, output, i) if parts_def else None
        editable['frames'].append({**item,'label':item.get('label',name),'box':list(box),'parts':parts_def})
        alpha = np.array(crop)[:,:,3]
        touches = bool(alpha[0].any() or alpha[-1].any() or alpha[:,0].any() or alpha[:,-1].any())
        frames.append({'index':i,'label':item.get('label',name),'file':file,'size':[cw,ch],
            'qa': {'touchesCropBoundary':touches,'alphaReviewRequired':True},
            'sourceBox':list(box),'sourceOffset':[ox,oy], 'anchor':[cw/2,ch-padding],
            'atlasRect':[ax,ay,cw,ch],'uv':[ax/atlas.width,1-(ay+ch)/atlas.height,cw/atlas.width,ch/atlas.height],
            'labelSource':item.get('labelSource','explicit' if layout or kind=='exploded' else 'unlabelled'),
            'ocr':item.get('ocr'),'parts':parts, 'vectorMaskSvg': svg_mask_file})
    atlas.save(output/'atlas.png')
    contact.save(output/'contact_sheet.jpg')
    (output/'runtime.mjs').write_text(RUNTIME)
    (output/'layout.review.json').write_text(json.dumps(editable,indent=2))
    present = {f['label'] for f in frames}
    missing = [a for a in ACTIONS if a not in present] if kind=='action' else []
    if kind=='action':
        warnings.append('Static action references are not sequential walk/run cycle frames.')
        if missing:
            warnings.append('Missing or unrecognized actions: '+', '.join(missing))
    if kind=='turnaround':
        warnings.append('View order is assumed front, quarter-front, side, opposite-quarter, back; verify directions in contact sheet.')
        if len(frames)!=5:
            warnings.append(f'Expected 5 turnaround views, detected {len(frames)}; supply --layout.')
    if kind != 'exploded' and not any(frame['parts'] for frame in frames):
        warnings.append('No part masks supplied; frames contain whole artwork. Supply an explicit part layout before articulation.')
    if kind=='exploded':
        warnings.append('Exploded parts retain sheet spacing; set assembled bind positions and review hidden artwork before using as a puppet.')
    result = {'schemaVersion':1,'sourceImage':str(source),'sourceSize':[w,h],
        'sheetType':kind,'status':'needs_review', 'spriteStatus':'exported',
        'mattingEngine': 'photocraft-0.10.0+floodfill_core' if pc_bin else 'floodfill_decontaminate',
        'vectorCraftAvailable': bool(vc_bin),
        'frames':frames,'atlas':{'file':'atlas.png','size':list(atlas.size),'columns':columns,'rows':rows,'padding':padding},
        'coordinates':'top-left pixel origin; sourceBox right/bottom exclusive; anatomical L/R; Three UV origin bottom-left',
        'alignment':'common scale; centered x, bottom aligned; explicit per-frame offsets supported',
        'partsStatus':'needs_review', 'missingActions':missing, 'missingParts':missing_parts,
        'animation':{'continuousCycle':False,'actions':{name:[f['index'] for f in frames if f['label']==name] for name in sorted(present)}},
        'warnings':warnings,'contactSheet':'contact_sheet.jpg','editableLayout':'layout.review.json',
        'explodedTemplate':str(TEMPLATE)}
    if kind == 'exploded':
        # The sheet is a parts library, not an assembled bind pose. Initial bind
        # coordinates are explicit editable estimates, never readiness evidence.
        defaults = {'torso':(.5,.65), 'head':(.5,.35), 'ears':(.5,.15),
                    'arm_L':(.68,.44), 'arm_R':(.32,.44),
                    'leg_L':(.60,.72), 'leg_R':(.40,.72), 'prop_backpack':(.5,.47)}
        puppet = {'size':[cw,ch*2], 'anchor':[cw/2,ch*2-padding], 'parts':{}}
        slots = {s['id']:s for s in json.loads(TEMPLATE.read_text())['slots']}
        for frame in frames:
            name=frame['label']
            if name not in slots:
                continue
            slot=slots[name]
            with Image.open(output/frame['file']) as part_image:
                bounds=part_image.getchannel('A').getbbox()
            if bounds is None:
                continue
            px,py=slot['pivot']
            pivot=[bounds[0]+px*(bounds[2]-bounds[0]),bounds[1]+py*(bounds[3]-bounds[1])]
            bx,by=defaults[name]
            override=(layout or {}).get('puppet',{}).get(name,{})
            puppet['parts'][name]={'file':frame['file'], 'size':frame['size'],
                'parent':slot['parent'], 'pivot':override.get('pivot',pivot),
                'bindPosition':override.get('bindPosition',[bx*cw,by*ch*2]),
                'status':'ready' if override.get('complete') else 'needs_review',
                'visiblePixelsOnly':False, 'inferred':not bool(override.get('complete'))}
        result['puppet']=puppet
    (output/'manifest.json').write_text(json.dumps(result,indent=2))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', nargs='?', help='Image or directory of vault sheets')
    parser.add_argument('--out-dir',default='exports/pose_and_parts')
    parser.add_argument('--kind',choices=['auto','action','turnaround','single','exploded'],default='auto')
    parser.add_argument('--layout',help='JSON source pixel boxes, polygons and pivots (one source image only)')
    parser.add_argument('--no-ocr',action='store_true')
    parser.add_argument('--no-photocraft',action='store_true',help='Skip PhotoCraft 0.10.0 refine pass')
    parser.add_argument('--tolerance',type=int,default=25)
    parser.add_argument('--prompt',metavar='CHARACTER',help='Print exact exploded sheet prompt')
    parser.add_argument('--description',default='')
    args = parser.parse_args(argv)
    if args.prompt:
        print(exploded_prompt(args.prompt,args.description))
        return 0
    if not args.source:
        parser.error('source or --prompt required')
    src,out = Path(args.source),Path(args.out_dir)
    if src.is_dir():
        if args.layout:
            parser.error('--layout requires one image')
        paths = sorted(p for p in src.iterdir() if p.suffix.lower() in ('.png','.jpg','.jpeg')
                       and (p.name.startswith('Create_pose_sheet') or 'turnaround' in p.name.lower()))
        results=[]
        for path in paths:
            try:
                r = cut_sheet(path,out/path.stem,args.kind,use_ocr=not args.no_ocr,
                              tolerance=args.tolerance,use_photocraft=not args.no_photocraft)
                results.append({'source':str(path),'manifest':str(out/path.stem/'manifest.json'),
                                'status':r['status'],'frames':len(r['frames']),'missingActions':r['missingActions']})
            except (ValueError,OSError) as exc:
                results.append({'source':str(path),'status':'error','error':str(exc)})
        out.mkdir(parents=True,exist_ok=True)
        (out/'batch_report.json').write_text(json.dumps(results,indent=2))
        print(json.dumps({'sheets':len(results),'errors':sum(r['status']=='error' for r in results),
                          'report':str(out/'batch_report.json')},indent=2))
        return 1 if not results or any(r['status']=='error' for r in results) else 0
    try:
        r=cut_sheet(src,out,args.kind,args.layout,not args.no_ocr,args.tolerance,use_photocraft=not args.no_photocraft)
    except (ValueError,OSError) as exc:
        parser.exit(1,f'{exc}\n')
    print(json.dumps({'manifest':str(out/'manifest.json'),'frames':len(r['frames']),
                      'mattingEngine':r.get('mattingEngine'),'status':r['status']},indent=2))
    return 0


if __name__=='__main__':
    raise SystemExit(main())

