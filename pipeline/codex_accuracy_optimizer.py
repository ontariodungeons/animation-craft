"""Register existing cutouts and measure still-to-limit silhouette envelopes.

Only this module owns the implementation. Source PNGs/manifests are read-only.
No synthesis, automatic re-matting, guessed joint angles or rig edits are made.

Example:
  python3 -m pipeline.codex_accuracy_optimizer \
    --turnaround-dir exports/pose_and_parts/hopper_turnaround \
    --poses-dir exports/pose_and_parts/hopper_poses \
    --out-dir exports/pose_and_parts/hopper --tools

Public API: register_turnaround(), measure_motion_range(), optimize(),
load_motion_range(), discover_local_tools(). All JSON uses source pixel edges,
top-left origin and exclusive right/bottom edges. Normalized motion units use
ONE standing silhouette height, including ears, for every pose. A silhouette
crown is not an inferred anatomical skull landmark. Motion envelopes describe
observed references, not proven physiological limits or temporal animation.

Outputs: registered_turnaround/{frames,manifest.json,atlas.png,contact_sheet.jpg},
turnaround_registration.json, motion_range.json, optimizer_report.json. Existing
CLI/web/Blender code is intentionally not patched; consumers must load these
artifacts explicitly. Vertical image registration does not fix UV topology,
independent perspective drawings, colour seams or the silhouette extrusion.
"""
from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage as ndi

from pipeline.craft_cli import find_craft_cli

SCHEMA_VERSION = 1
DIRECTIONS = ('front', 'quarter_front', 'side', 'opposite_quarter', 'back')
REQUESTED_POSES = ('standing', 'walking', 'running', 'jumping', 'waving',
                   'sitting', 'looking_up', 'looking_down')


def _write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    # Avoid half-written contracts if another process reads while writing.
    temporary = path.with_name(f'.{path.name}.{os.getpid()}.tmp')
    try:
        temporary.write_text(json.dumps(value, indent=2, allow_nan=False)+'\n')
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def _read_frames(directory):
    root = Path(directory).resolve()
    manifest_path = root/'manifest.json'
    manifest_data = manifest_path.read_bytes()
    manifest = json.loads(manifest_data)
    frames = []
    hashes = {str(manifest_path): sha256(manifest_data).hexdigest()}
    for item in manifest.get('frames', []):
        path = (root/item['file']).resolve()
        if not path.is_relative_to(root):
            raise ValueError(f'Frame path escapes manifest directory: {item["file"]}')
        data = path.read_bytes()
        hashes[str(path)] = sha256(data).hexdigest()
        with Image.open(BytesIO(data)) as src:
            if 'A' not in src.getbands():
                raise ValueError(f'Expected a matted RGBA cutout: {path}')
            image = src.convert('RGBA')
        if 'size' in item and list(image.size) != item['size']:
            raise ValueError(f'Manifest size differs from PNG: {path}')
        frames.append({'meta': item, 'path': path, 'image': image})
    if not frames:
        raise ValueError(f'No frames in {manifest_path}')
    return frames, hashes


def _measure(image, threshold=128):
    if not 1 <= threshold <= 254:
        raise ValueError('alpha_threshold must be in 1..254')
    alpha = np.asarray(image)[:, :, 3]
    mask = alpha >= threshold
    labels, count = ndi.label(mask)
    sizes = np.bincount(labels.ravel())
    if count == 0:
        raise ValueError('Blank foreground at selected alpha threshold')
    sizes[0] = 0
    # Ignore detached crumbs for measurement; never erase source pixels. Keep
    # substantial detached appendages/props (>=1% of the largest component).
    keep = sizes >= max(1, int(sizes.max()*.01))
    keep[0] = False
    mask = keep[labels]
    ys, xs = np.nonzero(mask)
    left, top, right, bottom = int(xs.min()), int(ys.min()), int(xs.max())+1, int(ys.max())+1
    band = ys >= bottom-max(2, round((bottom-top)*.08))
    foot_x = float(np.median(xs[band]))+.5
    return {'bbox': [left, top, right, bottom], 'crownY': top,
            'footBaselineY': bottom, 'heightPx': bottom-top, 'widthPx': right-left,
            'footCenterX': foot_x, 'centroid': [float(xs.mean())+.5, float(ys.mean())+.5],
            'areaPx': int(len(xs)), 'ignoredOpaquePixels': int((alpha >= threshold).sum()-len(xs)),
            'softAlphaPixels': int(((alpha > 0) & (alpha < 255)).sum()),
            'mask': mask}


def _public_measure(measure):
    return {k:v for k,v in measure.items() if k != 'mask'}


def _scaled(image, scale):
    if scale == 1:
        return image.copy()
    # One uniform transform from the original per candidate; never cascade
    # resampling. BICUBIC on Pillow's premultiplied RGBa avoids dark edge halos.
    size = (max(1, math.ceil(image.width*scale)), max(1, math.ceil(image.height*scale)))
    res = image.convert('RGBa').transform(size, Image.Transform.AFFINE,
        (1/scale, 0, 0, 0, 1/scale, 0), Image.Resampling.BICUBIC).convert('RGBA')
    arr = np.array(res)
    fg = arr[:, :, 3] > 20
    core = ndi.binary_erosion(fg, iterations=2)
    if core.any():
        _, near = ndi.distance_transform_edt(~core, return_indices=True)
        ring = fg & (~core)
        arr[ring, :3] = arr[near[0][ring], near[1][ring], :3]
    return Image.fromarray(arr)


def _registered_raster(image, original_height, target_height, threshold):
    # The rasterized threshold can round a transformed silhouette by a pixel.
    # Search a small deterministic subpixel neighborhood against actual alpha,
    # keeping uniform XY scale, until both raster endpoints can be exact.
    best = None
    for adjustment in (0, -.25, .25, -.5, .5, -.75, .75, -1, 1, -1.5, 1.5):
        scale = (target_height+adjustment)/original_height
        candidate = _scaled(image, scale)
        measured = _measure(candidate, threshold)
        error = abs(measured['heightPx']-target_height)
        if best is None or error < best[0]:
            best = (error, candidate, measured, scale)
        if error == 0:
            break
    return best[1:]


def register_turnaround(turnaround_dir, out_dir, alpha_threshold=128, reference='front',
                        _loaded=None):
    """Write common-crown/common-foot rasters and exact pixel/UV transforms.

    Requires four or five distinct named directions including front/side/back.
    No horizontal reflection or invented view angle is inferred from a label.
    The opposite quarter in the supplied sheet is an opposite-facing 3/4 view,
    not evidence of a 135-degree rear view.
    """
    frames, hashes = _loaded if _loaded is not None else _read_frames(turnaround_dir)
    by_label = {}
    for f in frames:
        label = f['meta']['label']
        if label not in DIRECTIONS or label in by_label:
            raise ValueError(f'Unknown or duplicate turnaround direction: {label}')
        by_label[label] = f
    if len(by_label) < 4 or not {'front', 'side', 'back'}.issubset(by_label):
        raise ValueError('Need 4+ named turnaround views including front, side and back')
    if reference not in by_label:
        raise ValueError(f'Missing reference direction: {reference}')
    for f in frames:
        f['measure'] = _measure(f['image'], alpha_threshold)
    target_height = by_label[reference]['measure']['heightPx']
    transformed = []
    for label in DIRECTIONS:
        if label not in by_label:
            continue
        f = by_label[label]
        image, measured, scale = _registered_raster(f['image'], f['measure']['heightPx'],
                                                   target_height, alpha_threshold)
        transformed.append((label, f, image, measured, scale))
    padding = 16
    # Include all soft alpha and transparent input margins, centered on feet.
    left_room = math.ceil(max(m['footCenterX'] for _,_,_,m,_ in transformed))
    right_room = math.ceil(max(im.width-m['footCenterX'] for _,_,im,m,_ in transformed))
    top_room = max(m['crownY'] for _,_,_,m,_ in transformed)
    bottom_room = max(im.height-m['footBaselineY'] for _,_,im,m,_ in transformed)
    crown = padding+top_room
    baseline = crown+target_height
    center = padding+left_room
    size = (padding*2+left_room+right_room, baseline+bottom_room+padding)
    output = Path(out_dir).resolve()
    destination = output/'registered_turnaround'
    (destination/'frames').mkdir(parents=True, exist_ok=True)
    result_frames = []
    atlas = Image.new('RGBA',(size[0]*len(transformed),size[1]))
    contact = Image.new('RGB',(atlas.width,size[1]+24),'#60636a')
    contact_draw = ImageDraw.Draw(contact)
    for i,(label,frame,image,measure,scale) in enumerate(transformed):
        tx = round(center-measure['footCenterX'])
        ty = crown-measure['crownY']
        if tx < 0 or ty < 0 or tx+image.width > size[0] or ty+image.height > size[1]:
            raise ValueError('Registration would clip source; refusing')
        aligned = Image.new('RGBA',size)
        aligned.paste(image,(tx,ty))
        final = _measure(aligned,alpha_threshold)
        filename = f'frames/{i:02d}_{label}.png'
        aligned.save(destination/filename,compress_level=3)
        atlas.paste(aligned,(i*size[0],0))
        contact.paste(aligned,(i*size[0],0),aligned.getchannel('A'))
        contact_draw.line((i*size[0],crown,(i+1)*size[0]-1,crown),fill='#ffca48')
        contact_draw.line((i*size[0],baseline,(i+1)*size[0]-1,baseline),fill='#58d9b0')
        contact_draw.text((i*size[0]+5,size[1]+3),label,fill='white')
        iw,ih = frame['image'].size
        # UV transform: registered u/v -> original texture u/v (bottom-left).
        uv_scale = [size[0]/(scale*iw), size[1]/(scale*ih)]
        uv_offset = [-tx/(scale*iw), 1-(size[1]-ty)/(scale*ih)]
        result_frames.append({'index':i,'label':label,'file':filename,'size':list(size),
            'sourceFile':str(frame['path']), 'sourceSize':[iw,ih],
            'sourceBox':frame['meta'].get('sourceBox'),
            'input':_public_measure(frame['measure']), 'registered':_public_measure(final),
            'sourceToRegisteredPixels':[[scale,0,tx],[0,scale,ty],[0,0,1]],
            'registeredToSourceUV':{'scale':uv_scale,'offset':uv_offset,'origin':'bottom-left'},
            'uniformScale':scale, 'anchor':[center,baseline],
            'atlasRect':[i*size[0],0,*size], 'uv':[i/len(transformed),0,1/len(transformed),1]})
    atlas.save(destination/'atlas.png',compress_level=3)
    contact.save(destination/'contact_sheet.jpg',quality=92)
    crown_values = [f['registered']['crownY'] for f in result_frames]
    foot_values = [f['registered']['footBaselineY'] for f in result_frames]
    drift = max(max(crown_values)-min(crown_values),max(foot_values)-min(foot_values))
    before_crown = [f['input']['crownY'] for f in result_frames]
    before_foot = [f['input']['footBaselineY'] for f in result_frames]
    result = {'schemaVersion':SCHEMA_VERSION, 'kind':'registered_turnaround',
        'assetRoot':str(destination),
        'alphaThreshold':alpha_threshold,'referenceDirection':reference,'frames':result_frames,
        'target':{'size':list(size),'crownY':crown,'footBaselineY':baseline,'heightPx':target_height,
                  'footCenterX':center,'crownDefinition':'measured silhouette top, including ear tips'},
        'before':{'crownDriftPx':max(before_crown)-min(before_crown),
                  'footDriftPx':max(before_foot)-min(before_foot)},
        'maxVerticalDriftPx':drift,'rasterRegistrationPassed':drift==0,
        'atlas':{'file':'atlas.png','size':list(atlas.size)},'sourceHashes':hashes,
        'missingDirections':[d for d in DIRECTIONS if d not in by_label],
        'scope':{'verticalAlignment':'measured on thresholded output alpha',
                 'horizontalAlignment':'lower 8% silhouette foot-band median; no reflections',
                 'horizontalSeamMismatchCorrected':False,'colourSeamsCorrected':False,
                 'geometryOrUVTopologyCorrected':False},
        'warnings':['Uniform per-view scale corrects framing, not anatomical differences or perspective.',
                    'Use registered files OR original files plus UV transforms, never both.',
                    'Existing source part cutouts/pivots were not re-registered; do not reuse their old transforms.']}
    _write_json(destination/'manifest.json',result)
    _write_json(output/'turnaround_registration.json',result)
    return result


def _rows(frames):
    """Cluster source boxes by overlapping row centers, not filename indices."""
    groups = []
    for f in sorted(frames,key=lambda f:sum(f['meta'].get('sourceBox',[0,0,0,0])[1::2])/2):
        meta=f['meta']
        if 'sourceBox' not in meta or 'sourceOffset' not in meta:
            f['row']=None
            continue
        box=meta['sourceBox'];cy=(box[1]+box[3])/2;height=box[3]-box[1]
        match=next((g for g in groups if abs(cy-g['center']) < .55*min(height,g['height'])),None)
        if match is None:
            match={'center':cy,'height':height,'frames':[]};groups.append(match)
        match['frames'].append(f)
        f['row']=groups.index(match)
    return groups


def measure_motion_range(poses_dir, out_dir, registration=None, alpha_threshold=128,
                         _loaded=None):
    """Measure observed extents against standing without per-pose rescaling.

    Sheet row origins are removed using median foot baselines within each row.
    This recovers relative jump lift erased by bottom-aligned frame exports.
    It is a documented layout estimate, not a measured ground plane or root
    trajectory. With no source coordinates footLiftPx is null, never invented.
    """
    frames,hashes = _loaded if _loaded is not None else _read_frames(poses_dir)
    labelled={}
    for f in frames:
        label=f['meta']['label']
        if label in labelled:
            raise ValueError(f'Duplicate pose label {label}; select the intended reference explicitly')
        labelled[label]=f
        f['measure']=_measure(f['image'],alpha_threshold)
    if 'standing' not in labelled:
        raise ValueError('Missing standing (STILL) reference')
    still=labelled['standing'];height=still['measure']['heightPx']
    rows=_rows(frames)
    row_info=[]
    for i,row in enumerate(rows):
        grounds=[f['measure']['footBaselineY']-f['meta']['sourceOffset'][1]+f['meta']['sourceBox'][1] for f in row['frames']]
        row_info.append({'row':i,'baselineSourceY':float(np.median(grounds)),
                         'method':'median measured foot baseline of source-sheet row',
                         'poseLabels':[f['meta']['label'] for f in row['frames']],
                         'baselineSpreadPx':float(max(grounds)-min(grounds))})
    def values(frame):
        m=frame['measure'];meta=frame['meta'];left,top,right,bottom=m['bbox']
        anchor_x=float(meta.get('anchor',[frame['image'].width/2,0])[0])
        if frame.get('row') is not None:
            ground=row_info[frame['row']]['baselineSourceY']
            dy=meta['sourceBox'][1]-meta['sourceOffset'][1]-ground
            lift=-(bottom+dy)
            origin='source_row_median_baseline'
        else:
            dy=-m['footBaselineY'];lift=None;origin='own_feet; source row unavailable'
        bounds=[left-anchor_x,top+dy,right-anchor_x,bottom+dy]
        metrics={'widthPx':m['widthPx'],'heightPx':m['heightPx'],'heightRatioToStill':m['heightPx']/height,
                 'areaPx':m['areaPx'],'areaRatioToStill':m['areaPx']/still['measure']['areaPx'],
                 'leftPx':bounds[0],'topPx':bounds[1],'rightPx':bounds[2],'bottomPx':bounds[3],
                 'centroidXPx':m['centroid'][0]-anchor_x,'centroidYPx':m['centroid'][1]+dy,
                 'footLiftPx':lift}
        return metrics,bounds,dy,origin
    still_metrics,_,_,_=values(still)
    poses={}
    for label,frame in labelled.items():
        metrics,bounds,dy,origin=values(frame)
        profile=[]
        mask=frame['measure']['mask']
        for h in (.10,.25,.50,.75,.90):
            # Measured cross-sections at the SAME standing-relative heights.
            y=int(round(-h*height-dy))
            xs=np.flatnonzero(mask[y]) if 0 <= y < mask.shape[0] else np.array([])
            cx=float(frame['meta'].get('anchor',[frame['image'].width/2,0])[0])
            profile.append({'heightAboveGroundUnits':h,
                            'leftUnits':float((xs.min()-cx)/height) if len(xs) else None,
                            'rightUnits':float((xs.max()+1-cx)/height) if len(xs) else None})
        poses[label]={'sourceFile':str(frame['path']),'frameIndex':frame['meta'].get('index'),
            'sourceMeasurement':_public_measure(frame['measure']),'row':frame.get('row'),
            'verticalOrigin':origin,'metrics':metrics,
            'deltaFromStill':{k:(v-still_metrics[k] if v is not None and still_metrics[k] is not None else None)
                              for k,v in metrics.items()},
            'bboxStandingUnits':[v/height for v in bounds], 'profile':profile,
            'jointAnglesRadians':None}
    limits={}
    for key in still_metrics:
        observed=[(p['metrics'][key],label) for label,p in poses.items() if p['metrics'][key] is not None]
        if not observed:
            limits[key]=None;continue
        lo,low_label=min(observed,key=lambda v:v[0]);hi,high_label=max(observed,key=lambda v:v[0])
        limits[key]={'min':lo,'max':hi,'minPose':low_label,'maxPose':high_label,
            'still':still_metrics[key],
            'minDeltaFromStill':lo-still_metrics[key] if still_metrics[key] is not None else None,
            'maxDeltaFromStill':hi-still_metrics[key] if still_metrics[key] is not None else None}
    result={'schemaVersion':SCHEMA_VERSION,'kind':'observed_silhouette_motion_range',
        'alphaThreshold':alpha_threshold,'still':{'label':'standing','file':str(still['path']),
            'heightPx':height,'metrics':still_metrics},'poses':poses,'limits':limits,
        'units':{'pixels':'original pose frame pixels; no per-pose resizing',
                 'standingUnits':'one standing crown-to-foot silhouette height = 1',
                 'axes':'x right, y down; y=0 is estimated row ground; image edges exclusive'},
        'requestedPoses':list(REQUESTED_POSES),'missingRequestedPoses':[p for p in REQUESTED_POSES if p not in poses],
        'rowBaselines':row_info,'jointAngleLimits':None,'rootTrajectory':None,
        'sourceHashes':hashes,'registrationFile':'turnaround_registration.json' if registration else None,
        'turnaroundHeightPx':registration['target']['heightPx'] if registration else None,
        'poseToTurnaroundScale':registration['target']['heightPx']/height if registration else None,
        'warnings':['These are sampled 2D silhouette envelopes, not physiological joint limits or temporal motion.',
                    'Pose artwork includes props/costume and viewpoint changes; envelopes include their pixels.',
                    'Row-median ground is a sheet-layout estimate; hovering whole rows cannot establish ground.',
                    'Horizontal root translation cannot be recovered from independent centered pose drawings.',
                    'CLI, browser and Blender must explicitly read this file; existing consumers were not edited.']}
    _write_json(Path(out_dir)/'motion_range.json',result)
    return result


def load_motion_range(path):
    """Consumer-side strict loader; identical JSON values can be read in JS/bpy."""
    value=json.loads(Path(path).read_text())
    if value.get('schemaVersion') != SCHEMA_VERSION or value.get('kind') != 'observed_silhouette_motion_range':
        raise ValueError('Unsupported motion range schema')
    return value


def discover_local_tools():
    """Inventory existing candidates only; never install, extract or launch GUIs."""
    result={}
    for name in ('photocraft','vectorcraft','meshcraft','artcraft','filmcraft','effectcraft'):
        cli=find_craft_cli(f'{name}-cli')
        candidates=[cli] if cli else []
        result[name]={'paths':list(dict.fromkeys(candidates)),'commandProbe':None}
        if name in ('photocraft','vectorcraft'):
            known=cli
            if known:
                try:
                    args=[known,'commands']+(['--json'] if name=='photocraft' else [])
                    start=time.perf_counter();proc=subprocess.run(args,capture_output=True,text=True,timeout=10,check=True)
                    catalogue=json.loads(proc.stdout)
                    useful={'select.object','select.refineEdge','select.subject','layer.removeBackground',
                            'layer.matting.defringe','layer.matting.colorDecontaminate',
                            'imageTrace.make','object.path.simplify','object.clippingMask.make'}
                    result[name]['commandProbe']={'binary':known,'seconds':time.perf_counter()-start,
                        'commands':[{'id':c['id'],'params':c.get('params')} for c in catalogue if c['id'] in useful]}
                except (OSError,subprocess.SubprocessError,ValueError) as exc:
                    result[name]['commandProbe']={'error':str(exc)}
    return result


def optimize(turnaround_dir, poses_dir, out_dir, alpha_threshold=128, tools=False):
    """One-process, single-decode analysis with content-verified result reuse."""
    start=time.perf_counter();out=Path(out_dir).resolve()
    for source in (Path(turnaround_dir).resolve(),Path(poses_dir).resolve()):
        if out == source or source.is_relative_to(out):
            raise ValueError('Output must not contain either input directory')
    turnaround=_read_frames(turnaround_dir);poses=_read_frames(poses_dir)
    source_hashes={**turnaround[1],**poses[1]}
    key=sha256(json.dumps({'sources':source_hashes,'outputDirectory':str(out),
            'alphaThreshold':alpha_threshold,'tools':tools,
            'implementation':sha256(Path(__file__).read_bytes()).hexdigest()},sort_keys=True).encode()).hexdigest()
    report_path=out/'optimizer_report.json'
    if report_path.is_file():
        try:
            previous=json.loads(report_path.read_text())
            if previous.get('cacheKey')==key and all((out/p).is_file() and sha256((out/p).read_bytes()).hexdigest()==h
                                                   for p,h in previous['outputHashes'].items()):
                return {**previous,'cacheHit':True,'elapsedSeconds':time.perf_counter()-start}
        except (OSError,ValueError,KeyError):
            pass
    t=time.perf_counter();registration=register_turnaround(turnaround_dir,out,alpha_threshold,_loaded=turnaround)
    registration_time=time.perf_counter()-t
    t=time.perf_counter();motion=measure_motion_range(poses_dir,out,registration,alpha_threshold,_loaded=poses)
    motion_time=time.perf_counter()-t
    produced=[out/'motion_range.json',out/'turnaround_registration.json',out/'registered_turnaround/manifest.json',
              out/'registered_turnaround/atlas.png',out/'registered_turnaround/contact_sheet.jpg']
    produced.extend(out/'registered_turnaround'/f['file'] for f in registration['frames'])
    report={'schemaVersion':SCHEMA_VERSION,'cacheKey':key,'cacheHit':False,
        'sourceHashes':source_hashes,'outputHashes':{str(p.relative_to(out)):sha256(p.read_bytes()).hexdigest() for p in produced},
        'elapsedSeconds':time.perf_counter()-start,'registrationSeconds':registration_time,'motionSeconds':motion_time,
        'frames':{'turnaround':len(registration['frames']),'poses':len(motion['poses'])},
        'registrationPassed':registration['rasterRegistrationPassed'],
        'maxVerticalDriftPx':registration['maxVerticalDriftPx'],
        'motionRangeFile':str(out/'motion_range.json'),'registrationFile':str(out/'turnaround_registration.json'),
        'optimization':'Reuse matted PNGs; decode once; vectorized alpha metrics; uniform premultiplied resampling; PNG compression level 3; SHA-256 cache',
        'scope':'Existing artwork only; no mesh, rig, source or consumer code changed.'}
    if tools:report['localTools']=discover_local_tools()
    _write_json(report_path,report)
    return report


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--turnaround-dir',default='exports/pose_and_parts/hopper_turnaround')
    parser.add_argument('--poses-dir',default='exports/pose_and_parts/hopper_poses')
    parser.add_argument('--out-dir',default='exports/pose_and_parts/hopper')
    parser.add_argument('--alpha-threshold',type=int,default=128)
    parser.add_argument('--tools',action='store_true',help='Record verified local CLI capabilities')
    args=parser.parse_args(argv)
    try:
        result=optimize(args.turnaround_dir,args.poses_dir,args.out_dir,args.alpha_threshold,args.tools)
    except (ValueError,OSError,KeyError) as exc:
        parser.exit(1,f'{exc}\n')
    print(json.dumps({k:v for k,v in result.items() if k not in ('sourceHashes','outputHashes','localTools')},indent=2))
    return 0 if result['registrationPassed'] else 1


if __name__=='__main__':
    raise SystemExit(main())
