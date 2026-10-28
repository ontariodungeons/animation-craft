"""Blender-owned animation: import sampled legacy motion once, then render saved scenes.

Run with Blender --background --factory-startup --python this_file -- request.json.
The caller owns the job directory and process resource limits.
"""
import json
import math
from pathlib import Path
import sys
import subprocess
import resource
import os
import hashlib
import time
import shutil


def write_json(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2))
    temporary.replace(path)


def import_motion(request):
    meta, sample = request['meta'], request['sample']
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    scene = bpy.context.scene
    width, height = meta['canvas']
    scene.render.resolution_x = width
    scene.render.resolution_y = height
    scene.render.fps = math.ceil(sample['fps'])
    scene.render.fps_base = scene.render.fps / sample['fps']
    scene.frame_start = 1
    scene.frame_end = max(1, math.ceil(sample['duration'] * sample['fps']))
    scene['magicstudio_character'] = meta['character']
    scene['magicstudio_track'] = sample['track']
    roots, opacity = {}, {}
    parts = {part['id']: part for part in meta['layers']}
    parents = dict(zip(parts, sample['effective_parents'], strict=True))
    for index, part in enumerate(meta['layers']):
        px, py = part['pivot']
        root = bpy.data.objects.new(part['id'], None)
        scene.collection.objects.link(root)
        roots[part['id']] = root
        x0, y0, x1, y1 = part['crop']
        mesh = bpy.data.meshes.new(part['id'] + '-artwork')
        mesh.from_pydata([(x0-px, py-y1, index*.01), (x1-px, py-y1, index*.01),
                          (x1-px, py-y0, index*.01), (x0-px, py-y0, index*.01)], [], [(0,1,2,3)])
        mesh.uv_layers.new()
        for loop, uv in zip(mesh.uv_layers.active.data, [(0,0),(1,0),(1,1),(0,1)]):
            loop.uv = uv
        obj = bpy.data.objects.new(part['id'] + '-artwork', mesh)
        scene.collection.objects.link(obj)
        obj.parent = root
        material = bpy.data.materials.new(part['id'] + '-fabric')
        material.use_nodes = True
        nodes = material.node_tree.nodes
        nodes.clear()
        texture = nodes.new('ShaderNodeTexImage')
        texture.image = bpy.data.images.load(part['file'], check_existing=True)
        emission = nodes.new('ShaderNodeEmission')
        transparent = nodes.new('ShaderNodeBsdfTransparent')
        mix = nodes.new('ShaderNodeMixShader')
        alpha = nodes.new('ShaderNodeMath')
        alpha.operation = 'MULTIPLY'
        alpha.inputs[1].default_value = 1
        output = nodes.new('ShaderNodeOutputMaterial')
        links = material.node_tree.links
        for source, target in [(texture.outputs['Color'], emission.inputs['Color']),
                               (texture.outputs['Alpha'], alpha.inputs[0]),
                               (alpha.outputs[0], mix.inputs[0]),
                               (transparent.outputs[0], mix.inputs[1]),
                               (emission.outputs[0], mix.inputs[2]),
                               (mix.outputs[0], output.inputs['Surface'])]:
            links.new(source, target)
        obj.data.materials.append(material)
        opacity[part['id']] = alpha.inputs[1]
    for part in meta['layers']:
        if parents[part['id']]:
            roots[part['id']].parent = roots[parents[part['id']]]
    for frame in sample['frames']:
        number = 1 + frame['time'] * sample['fps']
        for part, values in zip(meta['layers'], frame['layers']):
            root = roots[part['id']]
            px, py = part['pivot']
            parent = parts.get(parents[part['id']])
            bx, by = parent['pivot'] if parent else (0, 0)
            dx, dy = values.get('position') or (0, 0)
            root.location = (px-bx+dx, -(py-by+dy), 0)
            root.rotation_euler.z = -math.radians(values.get('rotation') or 0)
            sx, sy = values.get('scale') or (1, 1)
            root.scale = (sx, sy, 1)
            for channel in ('location', 'rotation_euler', 'scale'):
                root.keyframe_insert(data_path=channel, frame=number, group=sample['track'])
            opacity[part['id']].default_value = values['opacity']
            opacity[part['id']].keyframe_insert(data_path='default_value', frame=number)
    camera = bpy.data.cameras.new('Camera')
    camera.type = 'ORTHO'
    camera.sensor_fit = 'HORIZONTAL'
    left, top, right, bottom = sample['bounds']
    left, top, right, bottom = min(0,left), min(0,top), max(width,right), max(height,bottom)
    camera.ortho_scale = max(right-left, (bottom-top)*width/height) * 1.02
    camera.clip_end = max(width, height, right-left, bottom-top) * 4
    obj = bpy.data.objects.new('Camera', camera)
    scene.collection.objects.link(obj)
    obj.location = ((left+right)/2, -(top+bottom)/2, camera.clip_end/2)
    scene.camera = obj
    scene.render.engine = 'CYCLES'
    scene.cycles.device = 'CPU'
    scene.cycles.samples = 8
    scene.cycles.transparent_max_bounces = 64
    scene.render.film_transparent = True
    scene.view_settings.view_transform = 'Standard'
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'RGBA'
    bpy.ops.file.pack_all()


def sha256(path):
    with open(path, 'rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def service_metrics(unit):
    properties = ['Description', 'InvocationID', 'ActiveState', 'Result', 'ExecMainStatus',
                  'MemoryCurrent', 'MemoryPeak', 'MemoryMax', 'MemorySwapCurrent',
                  'MemorySwapMax', 'CPUUsageNSec', 'ExecMainStartTimestamp']
    response = subprocess.run(['systemctl', '--user', 'show', unit,
                               '--property=' + ','.join(properties), '--timestamp=unix'], capture_output=True, text=True, check=True)
    return dict(line.split('=', 1) for line in response.stdout.splitlines() if '=' in line)


def job_metadata(request_path, request, properties):
    job = request_path.parent
    path = job/'job.json'
    metadata = json.loads(path.read_text())
    if properties.get('Description') != request['job'] or properties.get('InvocationID') != os.environ.get('INVOCATION_ID'):
        raise RuntimeError('Refusing metadata from another job or invocation')
    raw_start = properties.get('ExecMainStartTimestamp', '')
    try:
        started_at = str(int(float(raw_start[1:])*1000)) if raw_start.startswith('@') and float(raw_start[1:]) > 0 else None
    except (ValueError, OverflowError):
        started_at = None
    marker = job/'cancel-requested'
    cancellation = json.loads(marker.read_text()) if marker.exists() else {}
    metadata.update(limits=request.get('limits'), submitted_at=request.get('submitted_at'),
                    started_at=started_at, cancellation_requested_at=cancellation.get('cancellation_requested_at', cancellation.get('updated_at')),
                    invocation_id=properties.get('InvocationID'))
    write_json(path, metadata)
    return metadata


def finalize(request_path):
    request = json.loads(request_path.read_text())
    job = request_path.parent
    properties = service_metrics(request['unit'])
    invocation = os.environ.get('INVOCATION_ID')
    if properties.get('Description') != request['job'] or properties.get('InvocationID') != invocation:
        raise RuntimeError('Refusing telemetry from another job or invocation')
    metadata = job_metadata(request_path, request, properties)
    result_path = job/'rendered.json'
    result = json.loads(result_path.read_text()) if result_path.exists() else {}
    reason = os.environ.get('SERVICE_RESULT', properties.get('Result'))
    cancelled = (job/'cancel-requested').exists()
    state = 'cancelled' if cancelled else ('completed' if reason == 'success' and result.get('state') == 'completed' else 'failed')
    metrics = {}
    for field, key in [('memory_current_bytes', 'MemoryCurrent'), ('memory_peak_bytes', 'MemoryPeak'),
                       ('memory_max_bytes', 'MemoryMax'), ('swap_current_bytes', 'MemorySwapCurrent'),
                       ('swap_max_bytes', 'MemorySwapMax'), ('cpu_usage_ns', 'CPUUsageNSec')]:
        value = properties.get(key, '')
        metrics[field] = int(value) if value.isdigit() and int(value) != 2**64-1 else None
    progress_path = job/'progress.json'
    progress = json.loads(progress_path.read_text()) if progress_path.exists() else {}
    result.update(backend=request.get('backend', 'blender'), job=request['job'], job_id=request['job'], project=request['project'],
                  unit=request['unit'], invocation_id=invocation, updated_at=str(time.time_ns()//1_000_000),
                  state=state, limits=metadata['limits'], submitted_at=metadata['submitted_at'],
                  started_at=metadata['started_at'], cancellation_requested_at=metadata['cancellation_requested_at'],
                  phase=progress.get('state'), frames_completed=progress.get('completed'),
                  frames_total=progress.get('total'), telemetry=metrics, service_result=reason,
                  exec_main_status=int(properties['ExecMainStatus']) if properties.get('ExecMainStatus', '').isdigit() else None,
                  exit_code=os.environ.get('EXIT_CODE'), exit_status=os.environ.get('EXIT_STATUS'),
                  termination_reason='cancel' if cancelled else ('oom' if reason == 'oom-kill' else
                      ('timeout' if reason == 'timeout' else (None if state == 'completed' else 'failure'))))
    write_json(job/'result.json', result)


def registration(scene):
    camera = scene.camera
    if camera is None or camera.data.type != 'ORTHO':
        raise RuntimeError('Only orthographic saved migration scenes have canvas registration')
    if camera.matrix_world.to_quaternion().angle > 1e-6 or any(abs(v-1) > 1e-6 for v in camera.matrix_world.to_scale()):
        raise RuntimeError('Rotated or scaled camera registration is unsupported')
    points = [camera.matrix_world @ vertex for vertex in camera.data.view_frame(scene=scene)]
    return [min(p.x for p in points), -max(p.y for p in points),
            max(p.x for p in points), -min(p.y for p in points)]


def main():
    global bpy
    import bpy
    request_path = Path(sys.argv[sys.argv.index('--') + 1]).resolve()
    job = request_path.parent
    request = json.loads(request_path.read_text())
    write_json(job/'invocation.json', {'invocation_id': os.environ.get('INVOCATION_ID')})
    try:
        job_metadata(request_path, request, service_metrics(request['unit']))
        source_blend_sha256 = None
        if request.get('blend'):
            write_json(job/'progress.json', {'state': 'loading'})
            frozen_blend = job/'source.blend'
            shutil.copyfile(request['blend'], frozen_blend)
            source_blend_sha256 = sha256(frozen_blend)
            bpy.ops.wm.open_mainfile(filepath=str(frozen_blend))
            scene = bpy.context.scene
            provenance_text = scene.get('magicstudio_provenance')
            if not provenance_text:
                raise RuntimeError('Saved scene has no frozen Magic Studio migration provenance; reimport the rig')
            provenance = json.loads(provenance_text)
            for key in ('character', 'track'):
                if request.get(key) != provenance.get(key):
                    raise RuntimeError('Requested ' + key + ' does not match saved scene provenance')
        else:
            write_json(job/'progress.json', {'state': 'preparing'})
            subprocess.run([request['magic_cli'], 'exec', 'blender.prepare', 'request=' + str(request_path)], check=True)
            prepared = json.loads((job/'sample.json').read_text())
            import_motion(prepared)
            scene = bpy.context.scene
            meta, sample = prepared['meta'], prepared['sample']
            provenance = dict(backend='blender', character=meta['character'], track=sample['track'], spec=meta['spec'],
                              rig_sha256=meta['rig_sha256'], canvas=meta['canvas'], source_job=request['job'],
                              artwork=[dict(id=part['id'], source_file=part.get('source_file'),
                                            packed_crop_sha256=sha256(part['file'])) for part in meta['layers']])
            scene['magicstudio_provenance'] = json.dumps(provenance)
        # Imported scenes may retain GPU/compositor/driver behavior; this backend only supports
        # its own packed CPU migration scenes and explicitly disables the compositor/sequencer.
        scene.render.engine = 'CYCLES'
        scene.cycles.device = 'CPU'
        scene.render.use_compositing = False
        scene.render.use_sequencer = False
        scene.render.threads_mode = 'FIXED'
        scene.render.threads = 2
        original_size = (scene.render.resolution_x, scene.render.resolution_y)
        scene.render.resolution_x = max(1, int(original_size[0] * request['resolution_percent']/100))
        scene.render.resolution_y = max(1, int(original_size[1] * request['resolution_percent']/100))
        scene.render.resolution_percentage = 100
        scene.render.pixel_aspect_x = scene.render.pixel_aspect_y = 1
        scene.render.image_settings.file_format = 'PNG'
        scene.render.image_settings.color_mode = 'RGBA'
        scene.render.film_transparent = True
        scene.frame_set(scene.frame_start)
        world_rect = registration(scene)
        render_size = (scene.render.resolution_x, scene.render.resolution_y)
        # Keep the editable scene's base resolution stable across repeated preview renders.
        scene.render.resolution_x, scene.render.resolution_y = original_size
        bpy.ops.wm.save_as_mainfile(filepath=str(job/'animation.blend'))
        scene.render.resolution_x, scene.render.resolution_y = render_size
        frames = job/'frames'
        frames.mkdir()
        start = scene.frame_start
        end = min(scene.frame_end, start + request['max_frames'] - 1)
        for frame in range(start, end+1):
            scene.frame_set(frame)
            if any(abs(a-b) > 1e-4 for a,b in zip(registration(scene), world_rect)):
                raise RuntimeError('Animated camera registration is unsupported')
            scene.render.filepath = str(frames/f'{frame:06d}.png')
            bpy.ops.render.render(write_still=True)
            write_json(job/'progress.json', {'state':'rendering', 'completed':frame-start+1,
                                            'total':end-start+1})
        write_json(job/'progress.json', {'state':'encoding', 'completed':end-start+1, 'total':end-start+1})
        clip = job/'preview.webm'
        fps = scene.render.fps/scene.render.fps_base
        subprocess.run(['ffmpeg','-v','error','-nostdin','-framerate',str(fps),
                        '-start_number',str(start),'-i',str(frames/'%06d.png'),'-frames:v',str(end-start+1),
                        '-c:v','libvpx-vp9','-pix_fmt','yuva420p','-threads','2','-b:v','0','-crf','24',
                        '-auto-alt-ref','0',str(clip)], check=True)
        write_json(job/'rendered.json', dict(provenance, state='completed', blend=str(job/'animation.blend'),
                    frames=str(frames), clip=str(clip), first_frame=start, last_frame=end,
                    fps=fps, clip_seconds=(end-start+1)/fps, world_rect=world_rect,
                    width=scene.render.resolution_x, height=scene.render.resolution_y,
                    source_blend=request.get('blend'), source_blend_sha256=source_blend_sha256,
                    clip_sha256=sha256(clip), partial=end<scene.frame_end, blender_version=bpy.app.version_string,
                    worker_peak_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss))
    except Exception as error:
        write_json(job/'rendered.json', {'state':'failed', 'error':str(error)})
        raise


if __name__ == '__main__':
    if '--finalize' in sys.argv:
        finalize(Path(sys.argv[sys.argv.index('--finalize')+1]))
    else:
        main()
