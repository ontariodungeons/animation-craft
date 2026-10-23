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

import bpy


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
    scene.render.fps = round(sample['fps'])
    scene.frame_start = 1
    scene.frame_end = max(1, math.ceil(sample['duration'] * sample['fps']))
    scene['magicstudio_character'] = meta['character']
    scene['magicstudio_track'] = sample['track']
    roots, opacity = {}, {}
    parts = {part['id']: part for part in meta['layers']}
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
        if part.get('parent'):
            roots[part['id']].parent = roots[part['parent']]
    for frame in sample['frames']:
        number = 1 + frame['time'] * sample['fps']
        for part, values in zip(meta['layers'], frame['layers']):
            root = roots[part['id']]
            px, py = part['pivot']
            parent = parts.get(part.get('parent'))
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


def main():
    request_path = Path(sys.argv[sys.argv.index('--') + 1]).resolve()
    job = request_path.parent
    request = json.loads(request_path.read_text())
    try:
        write_json(job/'progress.json', {'state': 'loading'})
        if request.get('blend'):
            bpy.ops.wm.open_mainfile(filepath=request['blend'])
        else:
            import_motion(request)
        scene = bpy.context.scene
        scene.render.threads_mode = 'FIXED'
        scene.render.threads = 2
        scene.render.resolution_percentage = request['resolution_percent']
        scene.render.image_settings.file_format = 'PNG'
        scene.render.image_settings.color_mode = 'RGBA'
        scene.render.film_transparent = True
        bpy.ops.wm.save_as_mainfile(filepath=str(job/'animation.blend'))
        frames = job/'frames'
        frames.mkdir()
        start = scene.frame_start
        end = min(scene.frame_end, start + request['max_frames'] - 1)
        for frame in range(start, end+1):
            scene.frame_set(frame)
            scene.render.filepath = str(frames/f'{frame:06d}.png')
            bpy.ops.render.render(write_still=True)
            write_json(job/'progress.json', {'state':'rendering', 'completed':frame-start+1,
                                            'total':end-start+1})
        write_json(job/'progress.json', {'state':'encoding'})
        clip = job/'preview.webm'
        subprocess.run(['ffmpeg','-v','error','-nostdin','-framerate',str(scene.render.fps/scene.render.fps_base),
                        '-start_number',str(start),'-i',str(frames/'%06d.png'),'-frames:v',str(end-start+1),
                        '-c:v','libvpx-vp9','-pix_fmt','yuva420p','-threads','2','-b:v','0','-crf','24',
                        '-auto-alt-ref','0',str(clip)], check=True)
        write_json(job/'result.json', {'state':'completed', 'blend':str(job/'animation.blend'),
                                     'frames':str(frames), 'clip':str(clip), 'first_frame':start, 'last_frame':end,
                                     'fps':scene.render.fps/scene.render.fps_base,
                                     'width':int(scene.render.resolution_x*scene.render.resolution_percentage/100),
                                     'height':int(scene.render.resolution_y*scene.render.resolution_percentage/100),
                                     'partial':end<scene.frame_end, 'blender_version':bpy.app.version_string,
                                     'worker_peak_rss_kib':resource.getrusage(resource.RUSAGE_SELF).ru_maxrss})
    except Exception as error:
        write_json(job/'result.json', {'state':'failed', 'error':str(error)})
        raise


if __name__ == '__main__':
    main()
