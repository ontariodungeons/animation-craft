"""Native source worker; launched only through the shared bounded systemd unit.

No scheduler here. worker.py is the existing Blender lifecycle/finalizer, copied
beside this file by the engine. Native sources and every render/encode child stay
inside that same cgroup. Synfig owns authored motion; this is not an evaluator.
"""
import gzip
import hashlib
import json
import math
import os
from pathlib import Path, PureWindowsPath
import re
import resource
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET


def sha256(path):
    with Path(path).open('rb') as handle:
        return hashlib.file_digest(handle, 'sha256').hexdigest()


def xml_source(path):
    opener = gzip.open if path.suffix == '.sifz' else open
    with opener(path, 'rb') as stream:
        data = stream.read(16*1024*1024+1)
    if len(data) > 16*1024*1024 or b'<!DOCTYPE' in data.upper() or b'<!ENTITY' in data.upper():
        raise ValueError('Oversized or externally defined Synfig XML is unsupported')
    root = ET.fromstring(data)
    if root.tag != 'canvas':
        raise ValueError('Synfig source must have a canvas root')
    return root


def validate_references(source, bundle):
    root = xml_source(source)
    for param in root.iter('param'):
        if param.get('name') == 'filename':
            value = param.find('string')
            if value is None or not value.text:
                raise ValueError('Animated or non-string asset filenames are unsupported')
            path = Path(value.text)
            if path.is_absolute() or not (source.parent/path).resolve().is_relative_to(bundle):
                raise ValueError('Asset reference escapes outside the source bundle')
            if not (source.parent/path).is_file():
                raise ValueError('Missing source dependency: '+value.text)
    for node in root.iter():
        # External canvas references require a separate dependency resolver.
        if node.get('use') and ('.sif' in node.get('use') or '#' in node.get('use')):
            raise ValueError('External canvas references are unsupported; embed the canvas')
    return root


def snapshot_synfig(request, job):
    source, bundle = Path(request['source']).resolve(), Path(request['bundle']).resolve()
    if not source.is_relative_to(bundle):
        raise ValueError('Source is outside bundle')
    validate_references(source, bundle)
    if job.resolve().is_relative_to(bundle):
        raise ValueError('Bundle must not contain the job output directory')
    target = job/'source-bundle'
    target.mkdir(parents=True)
    manifest, size = [], 0
    for entry in sorted(bundle.rglob('*')):
        if entry.is_symlink():
            raise ValueError('Source bundle symlinks are unsupported')
        if entry.is_dir():
            continue
        if not entry.is_file():
            raise ValueError('Source bundle contains a non-regular file')
        size += entry.stat().st_size
        if size > 1024**3 or len(manifest) >= 4096:
            raise ValueError('Source bundle exceeds 1 GiB or 4096 files')
        relative = entry.relative_to(bundle)
        output = target/relative
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(entry, output)
        frozen_hash = sha256(output)
        if sha256(entry) != frozen_hash:
            raise ValueError('Source changed while snapshotting: '+str(relative))
        manifest.append({'path':str(relative), 'sha256':frozen_hash})
    frozen = target/source.relative_to(bundle)
    validate_references(frozen, target)
    # Digest the exact frozen file manifest, not a later read of the original rig.
    digest = hashlib.sha256(json.dumps(manifest, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    return {'path':str(frozen), 'bundle':str(target), 'original_path':str(source), 'sha256':sha256(frozen),
            'bundle_sha256':digest, 'dependencies':manifest}


def frame_time(text, fps):
    # Synfig authored time is a sum of frame/second/minute/hour tokens.
    tokens = re.findall(r'(-?\d+(?:\.\d+)?)([hmsf])', text.replace(' ', ''))
    if not tokens or ''.join(n+u for n,u in tokens) != text.replace(' ', ''):
        raise ValueError('Unsupported authored time: '+text)
    value = sum(float(n)*{'f':1, 's':fps, 'm':60*fps, 'h':3600*fps}[u] for n,u in tokens)
    if abs(value-round(value)) > 1e-5:
        raise ValueError('Authored endpoint is not on an output frame; choose explicit range')
    return round(value)


def collect_frames(directory, count, width, height):
    from PIL import Image
    files = sorted(directory.iterdir())
    if len(files) != count or any(p.suffix.lower() != '.png' for p in files):
        raise ValueError('Native frame sequence count mismatch')
    numbers = []
    for path in files:
        match = re.search(r'(\d+)\.png$', path.name)
        if not match and count != 1:
            raise ValueError('Native frame sequence has no frame number')
        numbers.append(int(match[1]) if match else 0)
    if numbers != list(range(numbers[0], numbers[0]+count)):
        raise ValueError('Native frame sequence has gaps or extra frames')
    minimum, maximum = 255, 0
    manifest = []
    for path, number in zip(files, numbers):
        with Image.open(path) as image:
            image.load()
            if image.size != (width,height) or image.mode != 'RGBA':
                raise ValueError('Native PNG must decode as RGBA with requested dimensions')
            low, high = image.getchannel('A').getextrema()
            minimum, maximum = min(minimum, low), max(maximum, high)
        manifest.append({'path':str(path), 'native_frame':number, 'sha256':sha256(path)})
    return {'frames':manifest, 'alpha_evidence':{'mode':'RGBA', 'min':minimum, 'max':maximum, 'decoded_frames':count}}


def encode_frames(request, job, paths, fps, width, height):
    from PIL import Image
    from worker import write_json
    count=len(paths)
    if not math.isfinite(fps) or not 0 < fps <= 120 or not 1 <= count <= 18000 or not 1 <= width <= 4096 or not 1 <= height <= 4096 or width*height*count > 256*1024*1024:
        raise ValueError('Native encoding exceeds frame/pixel/rate bounds')
    minimum, maximum = 255, 0
    for path in paths:
        with Image.open(path) as picture:
            picture.load()
            if picture.mode != 'RGBA' or picture.size != (width,height):
                raise ValueError('Expected fully decoded RGBA PNG at declared size')
            low,high=picture.getchannel('A').getextrema();minimum=min(minimum,low);maximum=max(maximum,high)
    result={'alpha_evidence':{'mode':'RGBA','min':minimum,'max':maximum,'decoded_frames':count}}
    # Use our own sequential links only after verifying the native sequence.
    sequence = job/'encode-frames'; sequence.mkdir()
    for i,path in enumerate(paths):
        os.link(path,sequence/f'{i:06d}.png')
    write_json(job/'progress.json', {'state':'encoding','completed':count,'total':count})
    clip = job/'preview.webm'
    subprocess.run([request['ffmpeg'],'-v','error','-nostdin','-framerate',str(fps),
                    '-i',str(sequence/'%06d.png'),'-frames:v',str(count),
                    '-c:v','libvpx-vp9','-pix_fmt','yuva420p','-threads','2','-b:v','0','-crf','24',
                    '-auto-alt-ref','0',str(clip)], check=True)
    # Force the alpha-capable decoder and count actual decoded RGBA frames.
    decoded = job/'decoded.rgba'
    with decoded.open('wb') as output:
        subprocess.run([request['ffmpeg'],'-v','error','-nostdin','-c:v','libvpx-vp9','-i',str(clip),
                        '-f','rawvideo','-pix_fmt','rgba','-threads','2','-'], stdout=output, check=True)
    frame_bytes = width*height*4
    if decoded.stat().st_size != frame_bytes*(count):
        raise ValueError('Encoded clip decoded frame count/dimensions mismatch')
    low, high = 255,0
    with decoded.open('rb') as stream:
        while data := stream.read(frame_bytes):
            alpha = data[3::4]; low=min(low,min(alpha)); high=max(high,max(alpha))
    decoded.unlink()
    if result['alpha_evidence']['min'] < 255 and low == 255:
        raise ValueError('Encoded clip lost source transparency')
    result['alpha_evidence']['clip_min'] = low
    result['alpha_evidence']['clip_max'] = high
    return dict(clip=str(clip), clip_sha256=sha256(clip), alpha_evidence=result['alpha_evidence'])


def render_synfig(request, job):
    from worker import write_json
    frozen = snapshot_synfig(request, job)
    root = xml_source(Path(frozen['path']))
    fps = float(request.get('fps', root.get('fps', '24')))
    width, height = (int(request.get(k, root.get(k, '0'))) for k in ('width','height'))
    first = int(request['first_frame']) if 'first_frame' in request else frame_time(root.get('begin-time','0f'),fps)
    last = int(request['last_frame']) if 'last_frame' in request else frame_time(root.get('end-time','0f'),fps)
    if not math.isfinite(fps) or not 0 < fps <= 120 or not 2 <= width <= 4096 or not 2 <= height <= 4096:
        raise ValueError('Invalid dimensions or FPS')
    if not 0 <= first <= last <= 18000 or last-first+1 > 18000:
        raise ValueError('Invalid or excessive frame range')
    # Persist even on renderer failure so frozen input is inspectable.
    write_json(job/'source.json', frozen)
    tool = request['executable']
    version = subprocess.run([tool,'--version'], capture_output=True, text=True, timeout=10)
    version_text = (version.stdout+'\n'+version.stderr).strip()
    if not re.search(r'synfig\s+\d+\.\d+', version_text, re.I):
        raise ValueError('Executable did not identify itself as Synfig')
    tool_receipt = {'path':tool, 'sha256':sha256(tool), 'version':version_text, 'version_exit_code':version.returncode}
    authored_first = frame_time(root.get('begin-time','0f'), fps)
    authored_last = frame_time(root.get('end-time','0f'), fps)
    if width*height*(last-first+1) > 256*1024*1024:
        raise ValueError('Requested decoded output exceeds 256 million pixels; reduce range or dimensions')
    frames = job/'frames'; frames.mkdir()
    argv = [tool, '-i',frozen['path'], '-t','png', '-o',str(frames/'frame.png'),
            '--fps',str(fps), '--begin-time',f'{first}f', '--end-time',f'{last}f',
            '-w',str(width), '-h',str(height), '-T','2']
    write_json(job/'plan.json', {'argv':argv,'source_bundle':frozen,'tool':tool_receipt,
                               'first_frame':first,'last_frame':last,'fps':fps,'width':width,'height':height})
    write_json(job/'progress.json', {'state':'rendering','completed':0,'total':last-first+1})
    subprocess.run(argv, cwd=Path(frozen['path']).parent, check=True)
    result = collect_frames(frames,last-first+1,width,height)
    for i,item in enumerate(result['frames']): item['source_frame']=first+i
    encoded=encode_frames(request,job,[Path(v['path']) for v in result['frames']],fps,width,height)
    clip=Path(encoded['clip']);result['alpha_evidence']=encoded['alpha_evidence']
    return dict(backend='synfig', state='completed', source_job=request['job'],
                character=request.get('character'), track=request.get('track'), source_bundle=frozen, tool=tool_receipt,
                frames=str(frames), frame_mapping=result['frames'], alpha_evidence=result['alpha_evidence'],
                clip=str(clip), clip_sha256=sha256(clip), first_frame=first,last_frame=last,fps=fps,
                clip_seconds=(last-first+1)/fps,width=width,height=height,canvas=[width,height],world_rect=[0,0,width,height],
                selected_range={'first':first,'last':last,'step':1},
                authored_range={'first':authored_first,'last':authored_last},
                partial=first>authored_first or last<authored_last,
                range_note='partial compares selected range with the saved root canvas timeline')


def snapshot_tree(source, target):
    source=Path(source).resolve()
    if not source.is_dir() or target.resolve().is_relative_to(source):
        raise ValueError('Bundle must be a directory outside job output')
    target.mkdir(parents=True)
    manifest=[];size=0
    for path in sorted(source.rglob('*')):
        if path.is_symlink(): raise ValueError('Bundle symlinks unsupported')
        if path.is_dir(): continue
        if not path.is_file(): raise ValueError('Bundle contains non-regular file')
        size+=path.stat().st_size
        if size>1024**3 or len(manifest)>=4096: raise ValueError('Bundle exceeds 1 GiB/4096 files')
        out=target/path.relative_to(source);out.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(path,out);digest=sha256(out)
        if sha256(path)!=digest: raise ValueError('Source changed while snapshotting')
        manifest.append({'path':str(out.relative_to(target)),'sha256':digest})
    return manifest


def validate_native_references(path):
    # Inspect parsed scalar values, never XML delimiters such as quotes or />.
    # Native .tnz path text can itself be quoted; keep the frozen bytes unchanged.
    if path.stat().st_size > 64*1024*1024:
        raise ValueError('Native XML exceeds 64 MiB')
    content=path.read_bytes()
    if len(content)>64*1024*1024 or b'<!DOCTYPE' in content.upper() or b'<!ENTITY' in content.upper():
        raise ValueError('Oversized or externally defined native XML is unsupported')
    # OpenToonz TIStream is XML-like: strings contain literal ampersands (even
    # in its own stock format options). Escape only in the parser view so XML
    # syntax can be inspected without interpreting native filename text as entities.
    root=ET.fromstring(content.replace(b'&',b'&amp;'))
    for node in root.iter():
        for value in [node.text or '', *node.attrib.values()]:
            value=value.strip()
            if len(value)>=2 and value[0] in ('"', "'") and value[-1]==value[0]:
                value=value[1:-1].strip()
            normalized=value.replace('\\','/')
            if normalized.startswith('/') or PureWindowsPath(value).drive or '..' in normalized.split('/'):
                raise ValueError('Absolute/escaping native resource references unsupported; make bundle self-contained')


def render_opentoonz(request, request_path):
    from worker import write_json
    job=request_path.parent
    original=Path(request['scene']).resolve();root=Path(request['bundle']).resolve()
    if not original.is_relative_to(root): raise ValueError('Scene outside bundle')
    snapshot=job/'source-bundle';manifest=snapshot_tree(root,snapshot)
    frozen=snapshot/original.relative_to(root)
    # Native project aliases must not resolve outside the frozen bundle.
    for path in [frozen,*snapshot.glob('*_otprj.xml')]:
        validate_native_references(path)
    stuff=job/'toonz-stuff';stuff_manifest=snapshot_tree(Path(request['stuff']),stuff)
    prepared=dict(request,scene=str(frozen),bundle=str(snapshot))
    helper_request=job/'opentoonz-request.json';write_json(helper_request,prepared)
    env=dict(os.environ,OPENTOONZ_TCOMPOSER=request['tcomposer'],OPENTOONZ_STUFF=str(stuff),QT_QPA_PLATFORM='offscreen')
    write_json(job/'progress.json',{'state':'rendering'})
    subprocess.run([request['magic_cli'],'exec','opentoonz.render','request='+str(helper_request)],env=env,check=True)
    native=json.loads((job/'opentoonz-manifest.json').read_text())
    if any(sha256(snapshot/v['path'])!=v['sha256'] for v in manifest):
        raise ValueError('Frozen source bundle changed during native rendering')
    first,last,step=(int(native['range'][k]) for k in ('first','last','step'))
    if not 1<=last-first+1<=18000: raise ValueError('Requested timeline exceeds 18000 output frames')
    expected=list(range(first,last+1,step));paths=[Path(p).resolve() for p in native['frames']]
    if len(paths)!=len(expected) or any(p.parent!=job/'frames' for p in paths): raise ValueError('Invalid native frame manifest')
    expanded=[];mapping=[]
    for source_frame,path in zip(expected,paths):
        for frame in range(source_frame,min(last+1,source_frame+step)):
            expanded.append(path);mapping.append({'frame':frame,'scene_frame':source_frame,'path':str(path),'sha256':sha256(path)})
    width,height=int(native['width']),int(native['height']);fps=float(native['fps'])
    encoded=encode_frames(request,job,expanded,fps,width,height)
    bundle={'path':str(frozen),'bundle':str(snapshot),'original_path':str(original),'sha256':sha256(frozen),'dependencies':manifest,
            'bundle_sha256':hashlib.sha256(json.dumps(manifest,sort_keys=True).encode()).hexdigest(),
            'runtime_stuff':{'path':str(stuff),'dependencies':stuff_manifest}}
    return dict(encoded,backend='opentoonz',state='completed',frames=str(job/'frames'),frame_mapping=mapping,
                source_bundle=bundle,tool=native['tool'],first_frame=first,last_frame=last,fps=fps,
                clip_seconds=len(expanded)/fps,width=width,height=height,canvas=[width,height],world_rect=[0,0,width,height],
                selected_range=native['range'],partial=first!=1 or last!=native['scene_frame_count'],
                source_job=request['job'],sampling='step frames held to preserve scene timing')


def main(request_path):
    from worker import write_json, job_metadata, service_metrics
    request_path = request_path.resolve(); job=request_path.parent
    request=json.loads(request_path.read_text())
    write_json(job/'invocation.json', {'invocation_id':os.environ.get('INVOCATION_ID')})
    try:
        job_metadata(request_path, request, service_metrics(request['unit']))
        if request['backend']=='synfig': result=render_synfig(request,job)
        elif request['backend']=='opentoonz': result=render_opentoonz(request,request_path)
        elif request['backend']=='mlt':
            from mlt_worker import render
            result=render(request,job)
        else: raise ValueError('Unsupported native backend')
        result['worker_peak_rss_kib']=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        write_json(job/'rendered.json',result)
    except Exception as error:
        write_json(job/'rendered.json',{'state':'failed','backend':request.get('backend'),'error':str(error)})
        raise

if __name__ == '__main__':
    main(Path(sys.argv[1]))
