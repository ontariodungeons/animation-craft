#!/usr/bin/env python3
"""
Blender 5.3 Multi-Character & Farm School Lineup Scene Builder.

Driven by `mc exec export.blender` and
`mc exec scene.farm_school_lineup`.

Builds:
  1. Individual rigged 3D/2.5D Blender 5.3 animations (.mp4, .png, .glb, .blend)
     for all 6 characters:
       - `miss_hayley` (Miss Hayley — Teacher)
       - `old_mac` (Old Mac — Principal)
       - `hopper` (Hopper — Cream Rabbit Student)
       - `whiskers` (Whiskers — Grey Tabby Student)
       - `puddles` (Puddles — Yellow Duckling Student)
       - `rusty` (Rusty — Brown Foal Student)
  2. Combined 6-Character Farm School Lineup Scene in Blender 5.3:
     Miss Hayley and Old Mac standing in front of the Farm School
     (`/home/jesse/main-vault/Creative/Old MacDonald/oldmac-school.png`)
     inviting the 4 students into class and getting them to line up.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from pipeline.blender53_bridge import send_blender_mcp_code
from pipeline.character_iteration_exporter import (
    CHARACTER_PRESETS,
    POSE_PARTS_ROOT,
    SHARE_ROOT,
)

ROOT = Path(__file__).resolve().parents[1]
SCHOOL_BG = Path("/home/jesse/main-vault/Creative/Old MacDonald/oldmac-school.png")
CLASS_BG = Path("/home/jesse/main-vault/Creative/old-macs-class.png")

SIX_CAST_ORDER = [
    "miss_hayley",
    "old_mac",
    "hopper",
    "whiskers",
    "puddles",
    "rusty",
]

CHARACTER_STAGE_SPECS: dict[str, dict] = {
    "miss_hayley": {
        "role": "teacher",
        "height_m": 1.82,
        "action_style": "teacher_welcome",
        "start_xy": (-2.10, 0.88),
        "end_xy": (-2.00, 0.82),
        "start_yaw_deg": 16.0,
        "facing_yaw_deg": 12.0,
    },
    "old_mac": {
        "role": "teacher",
        "height_m": 1.84,
        "action_style": "principal_beckon",
        "start_xy": (2.10, 0.88),
        "end_xy": (2.00, 0.82),
        "start_yaw_deg": -16.0,
        "facing_yaw_deg": -12.0,
    },
    "hopper": {
        "role": "student",
        "height_m": 1.34,
        "action_style": "bunny_hop_walk",
        "start_xy": (-1.78, -0.85),
        "end_xy": (-1.26, -0.15),
        "start_yaw_deg": 22.0,
        "facing_yaw_deg": 0.0,
    },
    "whiskers": {
        "role": "student",
        "height_m": 1.26,
        "action_style": "cat_walk_wave",
        "start_xy": (-0.78, -1.00),
        "end_xy": (-0.42, -0.15),
        "start_yaw_deg": 14.0,
        "facing_yaw_deg": 0.0,
    },
    "puddles": {
        "role": "student",
        "height_m": 1.20,
        "action_style": "duck_waddle_hop",
        "start_xy": (0.78, -1.00),
        "end_xy": (0.42, -0.15),
        "start_yaw_deg": -14.0,
        "facing_yaw_deg": 0.0,
    },
    "rusty": {
        "role": "student",
        "height_m": 1.40,
        "action_style": "foal_trot_wave",
        "start_xy": (1.78, -0.85),
        "end_xy": (1.26, -0.15),
        "start_yaw_deg": -22.0,
        "facing_yaw_deg": 0.0,
    },
}


# Camera azimuths were checked against the artwork, not inferred from column numbers.
VIEW_ANGLES = {
    'hopper': [0, -45, -90, 45, 180],
    'whiskers': [0, -45, 90, 135, 180],
    'puddles': [0, -45, -90, 45, 180],
    'rusty': [0, 45, 90, -45, 180],
    'miss_hayley': [0, -45, -90, 45, 180],
    'old_mac': [0, -45, -90, 45, 180],
}
# Authored rig landmarks fitted to the reference proportions; these are not measured joint limits.
ANATOMY = {
    'hopper': dict(hip=.265, knee=.155, ankle=.055, neck=.535, shoulder=.49, elbow=.37, wrist=.29, ear=.79, leg_x=.075, arm_x=.16),
    'whiskers': dict(hip=.28, knee=.16, ankle=.045, neck=.57, shoulder=.52, elbow=.40, wrist=.32, ear=.89, leg_x=.075, arm_x=.17),
    'puddles': dict(hip=.19, knee=.12, ankle=.045, neck=.56, shoulder=.49, elbow=.37, wrist=.29, ear=.91, leg_x=.085, arm_x=.16),
    'rusty': dict(hip=.33, knee=.19, ankle=.055, neck=.60, shoulder=.54, elbow=.43, wrist=.34, ear=.88, leg_x=.065, arm_x=.14),
    'miss_hayley': dict(hip=.36, knee=.19, ankle=.075, neck=.70, shoulder=.64, elbow=.49, wrist=.37, ear=.91, leg_x=.065, arm_x=.165),
    'old_mac': dict(hip=.28, knee=.16, ankle=.065, neck=.60, shoulder=.55, elbow=.40, wrist=.28, ear=.88, leg_x=.085, arm_x=.19),
}
PERFORMANCES = {
    'hopper': dict(period=10, offset=0, lift=.80, sway=.010, gesture='ear_perk'),
    'whiskers': dict(period=12, offset=3, lift=.55, sway=.006, gesture='small_wave'),
    'puddles': dict(period=8, offset=1, lift=.65, sway=.024, gesture='wing_greeting'),
    'rusty': dict(period=10, offset=2, lift=.70, sway=.012, gesture='confident_wave'),
    'miss_hayley': dict(period=12, offset=0, lift=0, sway=.003, gesture='welcome'),
    'old_mac': dict(period=14, offset=4, lift=0, sway=.002, gesture='beckon'),
}


def resolve_character_assets(slug: str, reference_root: Path | None = None) -> dict:
    """Read existing inputs only. Never ask the upstream exporter to fabricate missing views."""
    import math
    if slug not in SIX_CAST_ORDER:
        raise ValueError(f'Unknown character: {slug}')
    preset = CHARACTER_PRESETS[slug]
    motion_path = SHARE_ROOT / slug / 'motion_range.json'
    motion = json.loads(motion_path.read_text())
    lift = motion.get('limits', {}).get('footLiftPx', {}).get('max')
    still_h = motion.get('still', {}).get('heightPx')
    if lift is None or still_h is None or not math.isfinite(float(lift)) or float(still_h) <= 0:
        raise ValueError(f'{slug}: measured footLiftPx.max and standing height are required: {motion_path}')
    views = []
    if slug in ('miss_hayley', 'old_mac'):
        reference_root = reference_root or ROOT / 'exports/farm_school_milestone/references'
        sheet = reference_root / slug / 'turnaround.png'
        provenance = reference_root / slug / 'provenance.json'
        if not sheet.is_file() or not provenance.is_file():
            raise ValueError(f'{slug}: a genuine or explicitly generated five-view sheet with provenance is required at {sheet}')
        for i, (label, angle) in enumerate(zip(['front', 'quarter_front', 'side', 'opposite_quarter', 'back'], VIEW_ANGLES[slug])):
            views.append(dict(label=label, angle=angle, sheet=str(sheet), cell=i))
        provenance_data = json.loads(provenance.read_text())
    else:
        reg_path = SHARE_ROOT / slug / 'turnaround_registration.json'
        reg = json.loads(reg_path.read_text())
        for fr, angle in zip(reg['frames'], VIEW_ANGLES[slug]):
            views.append(dict(label=fr['label'], angle=angle,
                              file=str(Path(reg['assetRoot']) / fr['file']),
                              baseline=fr['registered']['footBaselineY'],
                              crown=fr['registered']['crownY'],
                              center=fr['registered']['footCenterX']))
        provenance_data = dict(kind='existing_turnaround_artwork', registration=str(reg_path),
                               source=json.loads((Path(preset['turnaround_dir']) / 'manifest.json').read_text())['sourceImage'])
    return dict(slug=slug, title=preset.get('title', slug), **CHARACTER_STAGE_SPECS[slug],
                anatomy=ANATOMY[slug], performance=PERFORMANCES[slug], views=views,
                foot_lift_max_px=float(lift), standing_height_px=float(still_h),
                foot_lift_max_m=max(0.0, float(lift)) / float(still_h) * CHARACTER_STAGE_SPECS[slug]['height_m'],
                motion_source=str(motion_path), motion_kind=motion.get('kind'), provenance=provenance_data)


def prepare_volume(spec: dict, out_dir: Path, resolution: int = 144) -> dict:
    """Intersect the supplied silhouette cones; retain real image projections on the closed hull."""
    import math
    import numpy as np
    from scipy import ndimage
    from PIL import Image
    out_dir.mkdir(parents=True, exist_ok=True)
    height = spec['height_m']
    textures = []
    for vi, view in enumerate(spec['views']):
        if 'sheet' in view:
            sheet = Image.open(view['sheet']).convert('RGBA')
            sheet_rgba = np.array(sheet)
            components, _ = ndimage.label(sheet_rgba[..., 3] >= 128)
            sizes = np.bincount(components.ravel()); sizes[0] = 0
            ids = sorted(np.argsort(sizes)[-5:], key=lambda k:float(np.where(components == k)[1].mean()))
            if len(ids) != 5 or min(sizes[k] for k in ids) < 1000:
                raise ValueError(f"{spec['slug']}: teacher sheet must contain five separate full-body figures")
            component = components == ids[vi]
            yy0, xx0 = np.where(component)
            sheet_rgba[..., 3] = np.where(component, sheet_rgba[..., 3], 0)
            image = Image.fromarray(sheet_rgba).crop((max(0,int(xx0.min())-4),0,min(sheet.width,int(xx0.max())+5),sheet.height))
        else:
            image = Image.open(view['file']).convert('RGBA')
        rgba = np.array(image)
        mask = rgba[..., 3] >= 128
        labels, count = ndimage.label(mask)
        if count == 0:
            raise ValueError(f"{spec['slug']} {view['label']}: empty silhouette")
        areas = np.bincount(labels.ravel()); areas[0] = 0
        mask = labels == areas.argmax()
        # Derived textures only: upstream artwork and matting remain untouched.
        yy, xx = np.where(mask)
        crown, baseline = int(yy.min()), int(yy.max()) + 1
        if 'sheet' in view:
            foot = mask[max(crown, baseline - max(2, (baseline - crown) // 10)):baseline]
            fx = np.where(foot)[1]
            center = (float(fx.min()) + float(fx.max())) / 2 if len(fx) else float(np.median(xx))
        else:
            crown, baseline, center = view['crown'], view['baseline'], view['center']
        scale = (baseline - crown) / height
        # Fill RGB outside alpha with nearest foreground, so geometry silhouettes have no white fringes.
        nearest = ndimage.distance_transform_edt(~mask, return_distances=False, return_indices=True)
        rgb = rgba[..., :3][tuple(nearest)]
        path = out_dir / f"{vi:02d}_{view['label']}_projection.png"
        Image.fromarray(rgb.astype('uint8')).save(path)
        textures.append(dict(view, texture=str(path.resolve()), center=center, baseline=baseline,
                             pixels_per_m=scale, size=list(image.size), silhouette=mask,
                             distance=ndimage.distance_transform_edt(mask) - ndimage.distance_transform_edt(~mask)))
    front = textures[0]
    fy, fx = np.where(front['silhouette'])
    half_x = max(abs(fx - front['center'])) / front['pixels_per_m'] + height * .025
    profile = next(v for v in textures if abs(v['angle']) == 90)
    py, px = np.where(profile['silhouette'])
    half_y = max(abs(px - profile['center'])) / profile['pixels_per_m'] + height * .025
    step = height / resolution
    axes = [np.arange(-half_x, half_x + step, step, dtype=np.float32),
            np.arange(-half_y, half_y + step, step, dtype=np.float32),
            np.arange(-step * 2, height + step * 3, step, dtype=np.float32)]
    x, y, z = np.meshgrid(*axes, indexing='ij')
    field = np.full(x.shape, 100.0, dtype=np.float32)
    for view in textures:
        angle = math.radians(view['angle'])
        u = view['center'] + (x * math.cos(angle) + y * math.sin(angle)) * view['pixels_per_m']
        v = view['baseline'] - z * view['pixels_per_m']
        sdf = ndimage.map_coordinates(view['distance'], [v, u], order=1, mode='constant', cval=-10000)
        field = np.minimum(field, sdf / view['pixels_per_m'])
    occupied = ndimage.gaussian_filter(field, .65) > -step * .35
    occupied[[0, -1], :, :] = False; occupied[:, [0, -1], :] = False; occupied[:, :, [0, -1]] = False
    labs, n = ndimage.label(occupied)
    counts = np.bincount(labs.ravel()); counts[0] = 0
    occupied = labs == counts.argmax()
    if occupied.sum() < 100:
        raise ValueError(f"{spec['slug']}: supplied views do not produce a usable common volume")
    # Boundary quads share vertices. Unlike an extruded card, depth comes from every supplied view.
    face_corners = [
        [(0,0,0),(0,0,1),(0,1,1),(0,1,0)], [(1,0,0),(1,1,0),(1,1,1),(1,0,1)],
        [(0,0,0),(1,0,0),(1,0,1),(0,0,1)], [(0,1,0),(0,1,1),(1,1,1),(1,1,0)],
        [(0,0,0),(0,1,0),(1,1,0),(1,0,0)], [(0,0,1),(1,0,1),(1,1,1),(0,1,1)],
    ]
    faces = []
    for axis in range(3):
        for positive in [False, True]:
            neighbor = np.roll(occupied, -1 if positive else 1, axis=axis)
            cells = np.argwhere(occupied & ~neighbor)
            faces.append(cells[:, None, :] + np.array(face_corners[axis * 2 + positive])[None, :, :])
    corners = np.concatenate(faces).reshape(-1, 3)
    unique, inverse = np.unique(corners, axis=0, return_inverse=True)
    vertices = unique.astype(np.float32) * step + np.array([a[0] - step / 2 for a in axes])
    quads = inverse.reshape(-1, 4)
    mesh_path = out_dir / 'visual_hull.npz'
    np.savez_compressed(mesh_path, vertices=vertices.astype(np.float32), faces=quads.astype(np.int32))
    clean_views = [{k:v for k,v in t.items() if k not in ('silhouette', 'distance')} for t in textures]
    spec = dict(spec, views=clean_views, mesh_file=str(mesh_path.resolve()), output_dir=str(out_dir.resolve()),
                reconstruction=dict(method='five_view_silhouette_intersection', resolution=resolution,
                                    vertices=len(vertices), faces=len(quads), joint_landmarks='authored_from_reference_proportions'))
    (out_dir / 'asset_sources.json').write_text(json.dumps(spec, indent=2) + '\n')
    return spec


def smooth01(t: float) -> float:
    t = min(1.0, max(0.0, t))
    return t * t * (3 - 2 * t)


def root_at(spec: dict, frame: float, solo: bool = False) -> tuple:
    import math
    if solo or spec['role'] == 'teacher':
        x, y = (0., 0.) if solo else spec['end_xy']
        return x, y, 0.0, math.radians(spec.get('facing_yaw_deg', 0))
    t = smooth01((frame - 1) / 34)
    sx, sy = spec['start_xy']; ex, ey = spec['end_xy']
    heading = math.atan2(ex - sx, -(ey - sy))
    turn = smooth01((frame - 35) / 13)
    return sx + (ex - sx) * t, sy + (ey - sy) * t, 0., heading * (1 - turn)


def gait_frames(spec: dict, count: int, solo: bool = False) -> list:
    """Swing between fixed world-space sole targets; stance feet never track the moving root."""
    import math
    a = spec['anatomy']; h = spec['height_m']; perf = spec['performance']
    period = perf['period']; half = period // 2
    roots = [root_at(spec, f, solo) for f in range(1, count + period + 2)]
    def rest_target(side, f):
        x,y,_,yaw = root_at(spec, f, solo)
        lx = (1 if side == 'L' else -1) * a['leg_x'] * h
        return [x + math.cos(yaw) * lx, y + math.sin(yaw) * lx, 0.]
    targets = {side:rest_target(side,1) for side in ['L','R']}
    swing = {}
    result = []
    for f in range(1,count+1):
        root = roots[f-1]; moving = spec['role']=='student' and (solo or f <= 35)
        # Solo clips demonstrate articulation in place; combined clips plant in world space.
        turn = not solo and spec['role']=='student' and 35 < f <= 47
        cycle = (f - 1 + perf['offset']) // half
        active = 'L' if cycle % 2 == 0 else 'R'
        phase = (f - 1 + perf['offset']) % half
        row = dict(frame=f, root=list(root), feet={}, planted={})
        for side in ['L','R']:
            if (moving or turn) and side==active:
                if side not in swing:
                    end = min(48 if turn else 35, f + half - phase - 1) if not solo else f + half - phase - 1
                    goal = rest_target(side,min(35,end+half*.5) if moving and not solo else end)
                    if solo:
                        goal[1] = (-1 if cycle%4<2 else 1) * h*.055
                    swing[side] = (targets[side][:], goal, f, max(1,end-f))
                start,goal,start_f,length=swing[side]
                u=min(1.,(f-start_f+1)/(length+1)); eased=smooth01(u)
                foot=[start[i]+(goal[i]-start[i])*eased for i in range(3)]
                foot[2]=math.sin(math.pi*u)**2*spec['foot_lift_max_m']*perf['lift']
                if phase==half-1 or f in [35,47]:
                    foot=goal[:];targets[side]=goal[:];swing.pop(side,None)
                row['feet'][side]=foot;row['planted'][side]=False
            else:
                if side in swing:
                    targets[side]=swing.pop(side)[1][:]
                row['feet'][side]=targets[side][:];row['planted'][side]=True
        if moving or turn:
            l1=math.hypot(.04,a['hip']-a['knee'])*h
            l2=math.hypot(.04,a['knee']-a['ankle'])*h
            reach=(l1+l2)*.998
            pelvis_z=0.
            for side,sign in [('L',1),('R',-1)]:
                hip_x=root[0]+math.cos(root[3])*sign*a['leg_x']*h
                hip_y=root[1]+math.sin(root[3])*sign*a['leg_x']*h
                foot=row['feet'][side];horizontal=math.hypot(foot[0]-hip_x,foot[1]-hip_y)
                if horizontal>=reach:
                    raise ValueError(f"{spec['slug']} frame {f}: stride exceeds anatomical reach")
                pelvis_z=min(pelvis_z,foot[2]+a['ankle']*h+math.sqrt(reach*reach-horizontal*horizontal)-a['hip']*h)
            row['root'][2]=pelvis_z
        result.append(row)
    return result


def _blender_rig_helper_code() -> str:
    return BLENDER_HELPERS


BLENDER_HELPERS = r'''
import bpy, math, json, os, subprocess, shutil
import numpy as np
from mathutils import Vector, Matrix
from bpy_extras.object_utils import world_to_camera_view

def clear_scene():
    bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
    for block in list(bpy.data.collections):bpy.data.collections.remove(block)
    bpy.data.orphans_purge(do_recursive=True)

def make_collection(name):
    c=bpy.data.collections.new(name);bpy.context.scene.collection.children.link(c);return c

def material_for_view(slug,view):
    m=bpy.data.materials.new('MAT_'+slug+'_'+view['label']);m.use_nodes=True
    nt=m.node_tree;bs=nt.nodes.get('Principled BSDF');bs.inputs['Roughness'].default_value=.95
    bs.inputs['Specular IOR Level'].default_value=.12
    tex=nt.nodes.new('ShaderNodeTexImage');tex.image=bpy.data.images.load(view['texture'],check_existing=True)
    tex.image.pack();tex.extension='EXTEND'
    nt.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
    nt.links.new(tex.outputs['Color'],bs.inputs['Emission Color']);bs.inputs['Emission Strength'].default_value=.18
    return m

def bake_reference_projections(obj,spec):
    # Bake supported Blender projection nodes once; GLB receives a conventional portable texture.
    me=obj.data;mat=bpy.data.materials.new('MAT_'+spec['slug']+'_BakedReference');mat.use_nodes=True
    nt=mat.node_tree;nt.nodes.clear();geo=nt.nodes.new('ShaderNodeNewGeometry')
    xyz=nt.nodes.new('ShaderNodeSeparateXYZ');nt.links.new(geo.outputs['Position'],xyz.inputs[0])
    head=nt.nodes.new('ShaderNodeMath');head.operation='GREATER_THAN';head.inputs[1].default_value=(spec['anatomy']['neck']-.025)*spec['height_m'];nt.links.new(xyz.outputs['Z'],head.inputs[0])
    body=nt.nodes.new('ShaderNodeMath');body.operation='SUBTRACT';body.inputs[0].default_value=1;nt.links.new(head.outputs[0],body.inputs[1])
    ax=nt.nodes.new('ShaderNodeMath');ax.operation='ABSOLUTE';nt.links.new(xyz.outputs['X'],ax.inputs[0])
    outer=nt.nodes.new('ShaderNodeMath');outer.operation='GREATER_THAN';outer.inputs[1].default_value=spec['anatomy']['arm_x']*.85*spec['height_m'];nt.links.new(ax.outputs[0],outer.inputs[0])
    above=nt.nodes.new('ShaderNodeMath');above.operation='GREATER_THAN';above.inputs[1].default_value=min(spec['anatomy']['hip']-.02,spec['anatomy']['wrist']-.07)*spec['height_m'];nt.links.new(xyz.outputs['Z'],above.inputs[0])
    below=nt.nodes.new('ShaderNodeMath');below.operation='LESS_THAN';below.inputs[1].default_value=(spec['anatomy']['shoulder']+.035)*spec['height_m'];nt.links.new(xyz.outputs['Z'],below.inputs[0])
    arm=nt.nodes.new('ShaderNodeMath');arm.operation='MULTIPLY';nt.links.new(outer.outputs[0],arm.inputs[0]);nt.links.new(above.outputs[0],arm.inputs[1])
    arm2=nt.nodes.new('ShaderNodeMath');arm2.operation='MULTIPLY';nt.links.new(arm.outputs[0],arm2.inputs[0]);nt.links.new(below.outputs[0],arm2.inputs[1])
    stable_features=nt.nodes.new('ShaderNodeMath');stable_features.operation='MAXIMUM';nt.links.new(head.outputs[0],stable_features.inputs[0]);nt.links.new(arm2.outputs[0],stable_features.inputs[1]);head=stable_features
    nt.links.new(head.outputs[0],body.inputs[1])
    front_half=nt.nodes.new('ShaderNodeMath');front_half.operation='LESS_THAN';front_half.inputs[1].default_value=0;nt.links.new(xyz.outputs['Y'],front_half.inputs[0])
    back_half=nt.nodes.new('ShaderNodeMath');back_half.operation='SUBTRACT';back_half.inputs[0].default_value=1;nt.links.new(front_half.outputs[0],back_half.inputs[1])
    color_sum=None;weight_sum=None
    for view in spec['views']:
        layer=me.uv_layers.new(name='Project_'+view['label']);angle=math.radians(view['angle']);w,ih=view['size']
        for loop in me.loops:
            p=me.vertices[loop.vertex_index].co
            layer.data[loop.index].uv=((view['center']+(p.x*math.cos(angle)+p.y*math.sin(angle))*view['pixels_per_m'])/w,1-(view['baseline']-p.z*view['pixels_per_m'])/ih)
        uvnode=nt.nodes.new('ShaderNodeUVMap');uvnode.uv_map=layer.name
        tex=nt.nodes.new('ShaderNodeTexImage');tex.image=bpy.data.images.load(view['texture'],check_existing=True);tex.extension='EXTEND';nt.links.new(uvnode.outputs['UV'],tex.inputs['Vector'])
        dot=nt.nodes.new('ShaderNodeVectorMath');dot.operation='DOT_PRODUCT';dot.inputs[1].default_value=(math.sin(angle),-math.cos(angle),0);nt.links.new(geo.outputs['Normal'],dot.inputs[0])
        positive=nt.nodes.new('ShaderNodeMath');positive.operation='MAXIMUM';positive.inputs[1].default_value=.01;nt.links.new(dot.outputs['Value'],positive.inputs[0])
        weight=nt.nodes.new('ShaderNodeMath');weight.operation='POWER';weight.inputs[1].default_value=8;nt.links.new(positive.outputs[0],weight.inputs[0])
        masked=nt.nodes.new('ShaderNodeMath');masked.operation='MULTIPLY';nt.links.new(weight.outputs[0],masked.inputs[0]);nt.links.new(body.outputs[0],masked.inputs[1])
        if view['label'] in ('front','back'):
            face=nt.nodes.new('ShaderNodeMath');face.operation='MULTIPLY';nt.links.new(head.outputs[0],face.inputs[0]);nt.links.new((front_half if view['label']=='front' else back_half).outputs[0],face.inputs[1])
            total=nt.nodes.new('ShaderNodeMath');total.operation='ADD';nt.links.new(masked.outputs[0],total.inputs[0]);nt.links.new(face.outputs[0],total.inputs[1]);weight=total
        else:weight=masked
        scaled=nt.nodes.new('ShaderNodeVectorMath');scaled.operation='SCALE';nt.links.new(tex.outputs['Color'],scaled.inputs[0]);nt.links.new(weight.outputs[0],scaled.inputs['Scale'])
        if color_sum is None:color_sum=scaled.outputs[0];weight_sum=weight.outputs[0]
        else:
            add=nt.nodes.new('ShaderNodeVectorMath');add.operation='ADD';nt.links.new(color_sum,add.inputs[0]);nt.links.new(scaled.outputs[0],add.inputs[1]);color_sum=add.outputs[0]
            total=nt.nodes.new('ShaderNodeMath');total.operation='ADD';nt.links.new(weight_sum,total.inputs[0]);nt.links.new(weight.outputs[0],total.inputs[1]);weight_sum=total.outputs[0]
    divide=nt.nodes.new('ShaderNodeVectorMath');divide.operation='DIVIDE';nt.links.new(color_sum,divide.inputs[0]);nt.links.new(weight_sum,divide.inputs[1])
    emit=nt.nodes.new('ShaderNodeEmission');nt.links.new(divide.outputs[0],emit.inputs[0]);out=nt.nodes.new('ShaderNodeOutputMaterial');nt.links.new(emit.outputs[0],out.inputs[0])
    me.materials.clear();me.materials.append(mat)
    for poly in me.polygons:poly.material_index=0
    target=me.uv_layers.new(name='BakedUV');me.uv_layers.active=target;target.active_render=True
    bpy.context.view_layer.objects.active=obj;bpy.ops.object.select_all(action='DESELECT');obj.select_set(True)
    bpy.ops.object.mode_set(mode='EDIT');bpy.ops.mesh.select_all(action='SELECT');bpy.ops.uv.smart_project(angle_limit=1.1519,island_margin=.006);bpy.ops.object.mode_set(mode='OBJECT')
    image=bpy.data.images.new(spec['slug']+'_Albedo',width=2048,height=2048,alpha=False)
    dest=nt.nodes.new('ShaderNodeTexImage');dest.image=image;nt.nodes.active=dest
    s=bpy.context.scene;old=s.render.engine;s.render.engine='CYCLES';s.cycles.device='CPU';s.cycles.samples=1
    bpy.ops.object.bake(type='EMIT',margin=8,use_clear=True)
    s.render.engine=old;image.filepath_raw=os.path.join(spec['output_dir'],'baked_albedo.png');image.file_format='PNG';image.save();image.pack()
    nt.nodes.clear();out=nt.nodes.new('ShaderNodeOutputMaterial');bs=nt.nodes.new('ShaderNodeBsdfPrincipled');bs.inputs['Roughness'].default_value=1.;bs.inputs['Specular IOR Level'].default_value=.05
    tex=nt.nodes.new('ShaderNodeTexImage');tex.image=image;nt.links.new(tex.outputs['Color'],bs.inputs['Base Color']);nt.links.new(tex.outputs['Color'],bs.inputs['Emission Color']);bs.inputs['Emission Strength'].default_value=.45;nt.links.new(bs.outputs[0],out.inputs[0])
    for layer in list(me.uv_layers):
        if layer.name!='BakedUV':me.uv_layers.remove(layer)
    me.uv_layers.active=me.uv_layers['BakedUV'];me.uv_layers.active.active_render=True

def separate_contacting_arms(obj,spec):
    # Silhouettes cannot distinguish a hand touching clothing from a fused surface. Cap that contact
    # boundary before skinning, so raising a forearm never stretches a web of torso faces with it.
    import bmesh
    me=obj.data;h=spec['height_m'];a=spec['anatomy'];groups={0:[],1:[],2:[]}
    for p in me.polygons:
        c=p.center;z=c.z/h;x=c.x/h
        is_arm=abs(x)>a['arm_x']*.85 and min(a['hip']-.02,a['wrist']-.07)<z<a['shoulder']+.035
        region=(1 if x>=0 else 2) if is_arm else 0
        groups[region].append(tuple(p.vertices))
    vertices=[];faces=[];regions=[]
    for region,polys in groups.items():
        used=sorted({i for p in polys for i in p});indices={old:new for new,old in enumerate(used)}
        part=bpy.data.meshes.new('ContactPartition');part.from_pydata([me.vertices[i].co[:] for i in used],[],[[indices[i] for i in p] for p in polys]);part.update()
        bm=bmesh.new();bm.from_mesh(part);boundary=[e for e in bm.edges if e.is_boundary]
        bmesh.ops.holes_fill(bm,edges=boundary,sides=0);bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(part);bm.free()
        offset=len(vertices);vertices.extend(v.co[:] for v in part.vertices);faces.extend(tuple(offset+i for i in p.vertices) for p in part.polygons);regions.extend([region]*len(part.vertices));bpy.data.meshes.remove(part)
    new=bpy.data.meshes.new('MESH_'+spec['slug']+'_Articulated');new.from_pydata(vertices,[],faces);new.update()
    attr=new.attributes.new('anatomical_region','INT','POINT')
    for item,region in zip(attr.data,regions):item.value=region
    obj.data=new;bpy.data.meshes.remove(me)

def build_character_rig(spec,col_char,col_rig):
    slug=spec['slug'];h=spec['height_m'];a=spec['anatomy'];data=np.load(spec['mesh_file'])
    me=bpy.data.meshes.new('MESH_'+slug);me.from_pydata(data['vertices'].tolist(),[],data['faces'].tolist());me.update()
    obj=bpy.data.objects.new('CHAR_'+slug,me);col_char.objects.link(obj);bpy.context.view_layer.objects.active=obj;obj.select_set(True)
    remesh=obj.modifiers.new('ManifoldVolume','REMESH');remesh.mode='VOXEL';remesh.voxel_size=h/144;remesh.use_smooth_shade=True
    bpy.ops.object.modifier_apply(modifier=remesh.name);me=obj.data
    smooth=obj.modifiers.new('ReferenceHullSmooth','SMOOTH');smooth.factor=.8;smooth.iterations=4
    bpy.ops.object.modifier_apply(modifier=smooth.name)
    zmin=min(v.co.z for v in me.vertices);zmax=max(v.co.z for v in me.vertices)
    for v in me.vertices:v.co.z=(v.co.z-zmin)*(h-.0002)/(zmax-zmin)+.0002
    me.update()
    separate_contacting_arms(obj,spec);me=obj.data
    for v in spec['views']:me.materials.append(material_for_view(slug,v))
    uv=me.uv_layers.new(name='ReferenceProjection')
    # A face uses the reference most aligned to its outward normal; material alpha is fully opaque.
    for poly in me.polygons:
        center=poly.center;normal=poly.normal
        scores=[]
        for v in spec['views']:
            t=math.radians(v['angle']);direction=Vector((math.sin(t),-math.cos(t),0))
            score=normal.dot(direction)
            # Horizontal caps prefer front/back rather than arbitrary unrelated side pixels.
            if abs(normal.z)>.8:score+=.2 if v['label'] in ('front','back') else 0
            scores.append(score)
        vi=max(range(len(scores)),key=lambda i:scores[i]);view=spec['views'][vi];poly.material_index=vi;poly.use_smooth=True
        angle=math.radians(view['angle']);w,ih=view['size']
        for li in poly.loop_indices:
            p=me.vertices[me.loops[li].vertex_index].co
            u=(view['center']+(p.x*math.cos(angle)+p.y*math.sin(angle))*view['pixels_per_m'])/w
            v=1-(view['baseline']-p.z*view['pixels_per_m'])/ih
            uv.data[li].uv=(u,v)
    bake_reference_projections(obj,spec)
    arm=bpy.data.armatures.new('ARM_'+slug);rig=bpy.data.objects.new('RIG_'+slug,arm);col_rig.objects.link(rig)
    bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);bpy.context.view_layer.objects.active=rig;bpy.ops.object.mode_set(mode='EDIT')
    defs={}
    def bone(name,head,tail,parent=None):
        b=arm.edit_bones.new(name);b.head=Vector(head)*h;b.tail=Vector(tail)*h
        if parent:b.parent=arm.edit_bones[parent]
        defs[name]=(list(b.head),list(b.tail),parent)
    bone('root',(0,0,0),(0,0,.08))
    bone('hips',(0,0,a['hip']),(0,0,a['hip']+.07),'root')
    bone('spine',(0,0,a['hip']+.07),(0,0,a['neck']-.025),'hips')
    bone('neck',(0,0,a['neck']-.025),(0,0,a['neck']),'spine')
    bone('head',(0,0,a['neck']),(0,0,a['ear']),'neck')
    for side,sign in [('L',1),('R',-1)]:
        lx=sign*a['leg_x'];ax=sign*a['arm_x']
        bone('thigh_'+side,(lx,0,a['hip']),(lx,-.04,a['knee']),'hips')
        bone('shin_'+side,(lx,-.04,a['knee']),(lx,0,a['ankle']),'thigh_'+side)
        bone('foot_'+side,(lx,0,a['ankle']),(lx,-.075,a['ankle']),'shin_'+side)
        bone('toe_'+side,(lx,-.075,a['ankle']),(lx,-.105,a['ankle']),'foot_'+side)
        bone('upper_arm_'+side,(ax,0,a['shoulder']),(ax*1.12,0,a['elbow']),'spine')
        bone('forearm_'+side,(ax*1.12,0,a['elbow']),(ax*1.2,-.005,a['wrist']),'upper_arm_'+side)
        bone('hand_'+side,(ax*1.2,-.005,a['wrist']),(ax*1.2,-.005,a['wrist']-.045),'forearm_'+side)
        bone('ear_'+side,(sign*.065,0,a['ear']),(sign*.095,0,.99),'head')
    bone('backpack',(0,.07,a['hip']+.08),(0,.07,a['neck']-.04),'spine')
    bone('tail',(0,.09,a['hip']),(0,.15,a['hip']+.1),'hips')
    bpy.ops.object.mode_set(mode='OBJECT')
    for b in arm.bones:obj.vertex_groups.new(name=b.name)
    def distance(p,head,tail):
        ab=tail-head;t=max(0.,min(1.,(p-head).dot(ab)/max(1e-8,ab.length_squared)))
        return (p-(head+ab*t)).length
    # Anatomical partitions stop the torso influencing soles and preserve independent limb chains.
    for v in me.vertices:
        p=v.co;z=p.z/h;x=p.x/h;side='L' if x>=0 else 'R'
        region=me.attributes['anatomical_region'].data[v.index].value
        if region:names=['upper_arm_'+('L' if region==1 else 'R'),'forearm_'+('L' if region==1 else 'R'),'hand_'+('L' if region==1 else 'R')]
        elif z<a['ankle']*1.7:names=['foot_'+side]
        elif z<a['hip']:
            names=['thigh_'+side,'shin_'+side] if z>a['ankle']*2 else ['shin_'+side,'foot_'+side]
        elif z>=a['ear'] and slug in ('hopper','whiskers','rusty'):names=['ear_'+side,'head']
        elif z>=a['neck']:names=['head']
        else:names=['hips','spine','neck']
        dd=sorted((distance(p,arm.bones[n].head_local,arm.bones[n].tail_local),n) for n in names)[:2]
        if len(dd)==1:weights=[(dd[0][1],1.)]
        else:
            ww=[1/max(.002*h,d)**4 for d,n in dd];total=sum(ww);weights=[(dd[i][1],w/total) for i,w in enumerate(ww)]
        for n,w in weights:obj.vertex_groups[n].add([v.index],w,'REPLACE')
    obj.parent=rig;mod=obj.modifiers.new('Armature','ARMATURE');mod.object=rig
    rig['asset_sources']=json.dumps(spec['provenance']);rig['motion_source']=spec['motion_source']
    rig['measured_foot_lift_max_m']=spec['foot_lift_max_m'];rig['anatomy_status']='authored reference-fit landmarks, not measured joint angles'
    rig['rest_bones']=json.dumps(defs);rig['gait']=json.dumps(spec['gait'])
    return rig,obj

def two_link(hip,ankle,l1,l2,forward):
    delta=ankle-hip;d=delta.length
    if d>l1+l2-1e-5:raise ValueError('Unreachable planted-foot target: %.4f > %.4f'%(d,l1+l2))
    axis=delta/max(1e-8,d);along=(l1*l1-l2*l2+d*d)/(2*max(1e-8,d))
    bend=forward-axis*forward.dot(axis)
    if bend.length<1e-6:bend=Vector((0,-1,0))
    bend.normalize();return hip+axis*along+bend*math.sqrt(max(0,l1*l1-along*along))

def animate_character_rig(rig,spec):
    h=spec['height_m'];a=spec['anatomy'];perf=spec['performance'];defs=json.loads(rig['rest_bones'])
    for pb in rig.pose.bones:pb.rotation_mode='QUATERNION'
    rig['gait']=json.dumps(spec['gait'])
    rig.rotation_mode='XYZ'
    for row in spec['gait']:
        f=row['frame'];x,y,z,yaw=row['root'];moving=spec['role']=='student' and f<=35
        # Lower the pelvis to leave reach for the swing; soles are solved independently.
        drop=0.
        rig.location=(x,y,z-drop);rig.rotation_euler=(0,0,yaw)
        rig.keyframe_insert('location',frame=f);rig.keyframe_insert('rotation_euler',frame=f)
        desired={}
        def set_matrix(name,matrix):
            bone=rig.data.bones[name];parent=bone.parent
            base=desired[parent.name]@parent.matrix_local.inverted()@bone.matrix_local if parent else bone.matrix_local
            pb=rig.pose.bones[name];pb.matrix_basis=base.inverted()@matrix;desired[name]=matrix
            pb.keyframe_insert('location',frame=f);pb.keyframe_insert('rotation_quaternion',frame=f);pb.keyframe_insert('scale',frame=f)
        def orient(name,head,tail):
            b=rig.data.bones[name];q=(b.tail_local-b.head_local).rotation_difference(tail-head)
            m=(q.to_matrix()@b.matrix_local.to_3x3()).to_4x4();m.translation=head;set_matrix(name,m)
        for name in ['root','hips','spine','neck','head']:
            b=rig.data.bones[name];m=b.matrix_local.copy()
            if name=='head':
                phase=f*.12+perf['offset'];rot=Matrix.Rotation(math.sin(phase)*.025,4,'Z')
                m=Matrix.Translation(b.head_local)@rot@Matrix.Translation(-b.head_local)@m
            set_matrix(name,m)
        inv=Matrix.Translation(Vector((x,y,z-drop)))@Matrix.Rotation(yaw,4,'Z')
        inv=inv.inverted()
        for side,sign in [('L',1),('R',-1)]:
            ankle=inv@Vector(row['feet'][side]);ankle.z+=a['ankle']*h
            hip=Vector(defs['thigh_'+side][0]);b1=rig.data.bones['thigh_'+side];b2=rig.data.bones['shin_'+side]
            knee=two_link(hip,ankle,b1.length,b2.length,Vector((0,-1,0)))
            orient('thigh_'+side,hip,knee);orient('shin_'+side,knee,ankle)
            foot=rig.data.bones['foot_'+side].matrix_local.copy();foot.translation=ankle;set_matrix('foot_'+side,foot)
            toe=foot@rig.data.bones['foot_'+side].matrix_local.inverted()@rig.data.bones['toe_'+side].matrix_local;set_matrix('toe_'+side,toe)
            shoulder=Vector(defs['upper_arm_'+side][0]);elbow=Vector(defs['upper_arm_'+side][1]);wrist=Vector(defs['forearm_'+side][1])
            wave=spec['role']=='teacher' or f>35
            amplitude={'welcome':.65,'beckon':.42,'small_wave':.35,'confident_wave':.55,'wing_greeting':.38,'ear_perk':.20}[perf['gesture']]
            if wave and side=='L':
                angle=amplitude*(.75+.25*math.sin(f*.28+perf['offset']))
                rot=Matrix.Rotation(-angle,4,'Y');elbow=shoulder+rot.to_3x3()@(elbow-shoulder)
                bend=Matrix.Rotation(-.7-.25*math.sin(f*.4+perf['offset']),4,'Y')
                wrist=elbow+bend.to_3x3()@(Vector(defs['forearm_'+side][1])-Vector(defs['forearm_'+side][0]))
            elif moving:
                swing=math.sin(f*2*math.pi/perf['period']+sign*math.pi*.5)*.22
                rot=Matrix.Rotation(swing,4,'X');elbow=shoulder+rot.to_3x3()@(elbow-shoulder);wrist=elbow+rot.to_3x3()@(wrist-Vector(defs['forearm_'+side][0]))
            orient('upper_arm_'+side,shoulder,elbow);orient('forearm_'+side,elbow,wrist)
            hand=desired['forearm_'+side]@rig.data.bones['forearm_'+side].matrix_local.inverted()@rig.data.bones['hand_'+side].matrix_local;set_matrix('hand_'+side,hand)
            ear=desired['head']@rig.data.bones['head'].matrix_local.inverted()@rig.data.bones['ear_'+side].matrix_local
            if spec['slug']=='hopper':ear=ear@Matrix.Rotation(math.sin(f*.27+sign)*.065,4,'X')
            set_matrix('ear_'+side,ear)
        for name,parent in [('backpack','spine'),('tail','hips')]:
            set_matrix(name,desired[parent]@rig.data.bones[parent].matrix_local.inverted()@rig.data.bones[name].matrix_local)
    rig.animation_data.action.name='FarmSchoolLineup'
    for layer in rig.animation_data.action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for fc in bag.fcurves:
                    for key in fc.keyframe_points:key.interpolation='LINEAR'

def verify_character(rig,obj,spec):
    scene=bpy.context.scene;orig=scene.frame_current;mins=[];max_weight_error=0.
    for v in obj.data.vertices:max_weight_error=max(max_weight_error,abs(sum(g.weight for g in v.groups)-1))
    target_error=0.
    for row in spec['gait']:
        scene.frame_set(row['frame']);dg=bpy.context.evaluated_depsgraph_get();e=obj.evaluated_get(dg);me=e.to_mesh()
        low=min((e.matrix_world@v.co).z for v in me.vertices);mins.append(low);e.to_mesh_clear()
        if low < -0.0001:raise ValueError('%s frame %s penetrates ground: %.6f'%(spec['slug'],row['frame'],low))
        for side in ['L','R']:
            actual=rig.matrix_world@rig.pose.bones['foot_'+side].head
            target=Vector(row['feet'][side]);target.z+=spec['anatomy']['ankle']*spec['height_m']
            target_error=max(target_error,(actual-target).length)
    import bmesh
    bm=bmesh.new();bm.from_mesh(obj.data);bad_edges=sum(not e.is_manifold for e in bm.edges);bm.free()
    if bad_edges:raise ValueError('%s has %d non-manifold edges'%(spec['slug'],bad_edges))
    if target_error>1e-5:raise ValueError('%s foot target error %.8f'%(spec['slug'],target_error))
    scene.frame_set(orig)
    return dict(min_world_z=min(mins),max_weight_error=max_weight_error,bones=len(rig.data.bones),vertices=len(obj.data.vertices),frames=len(spec['gait']),non_manifold_edges=bad_edges,max_foot_target_error_m=target_error,measured_foot_lift_max_m=spec['foot_lift_max_m'],used_foot_lift_max_m=max(foot[2] for r in spec['gait'] for foot in r['feet'].values()))

def add_contact_shadows(spec):
    image=bpy.data.images.get('ContactShadow')
    if not image:
        image=bpy.data.images.new('ContactShadow',width=64,height=64,alpha=True)
        grid=np.linspace(-1,1,64);xx,yy=np.meshgrid(grid,grid);alpha=np.exp(-(xx*xx+yy*yy)*4)*.25
        rgba=np.zeros((64,64,4),dtype=np.float32);rgba[:,:,3]=alpha;image.pixels.foreach_set(rgba.ravel());image.update();image.pack()
    mat=bpy.data.materials.get('MAT_ContactShadow')
    if not mat:
        mat=bpy.data.materials.new('MAT_ContactShadow');mat.use_nodes=True;nt=mat.node_tree;bs=nt.nodes.get('Principled BSDF');bs.inputs['Base Color'].default_value=(0,0,0,1);bs.inputs['Roughness'].default_value=1
        tex=nt.nodes.new('ShaderNodeTexImage');tex.image=image;nt.links.new(tex.outputs['Alpha'],bs.inputs['Alpha'])
    for side in ['L','R']:
        bpy.ops.mesh.primitive_plane_add(size=1);obj=bpy.context.object;obj.name='Contact_'+spec['slug']+'_'+side;obj.data.materials.append(mat)
        for row in spec['gait']:
            foot=row['feet'][side];obj.location=(foot[0],foot[1],.001)
            opacity_scale=max(.25,1-foot[2]/max(.001,spec['foot_lift_max_m']))
            obj.scale=(spec['height_m']*.16*opacity_scale,spec['height_m']*.20*opacity_scale,1)
            obj.keyframe_insert('location',frame=row['frame']);obj.keyframe_insert('scale',frame=row['frame'])
        obj.animation_data.action.name='FarmSchoolLineup'

def setup_camera(name,solo_height=None):
    cam=bpy.data.cameras.new(name);obj=bpy.data.objects.new(name,cam);bpy.context.scene.collection.objects.link(obj)
    if solo_height:
        obj.location=(0,-4.8,solo_height*.53);target=Vector((0,0,solo_height*.51));cam.lens=48
    else:
        obj.location=(0,-8.2,2.45);target=Vector((0,1.4,2.0));cam.lens=38
    obj.rotation_euler=(target-obj.location).to_track_quat('-Z','Y').to_euler();bpy.context.scene.camera=obj;return obj

def build_environment(camera,background):
    scene=bpy.context.scene;scene.world=bpy.data.worlds.new('FarmSchoolWorld');scene.world.use_nodes=True
    scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.8,.85,.95,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.6
    # One continuous camera-projected cyclorama carries both school and yard: no flat colour boundary.
    control=[(-12.,0.),(-5.,0.),(0.,0.),(1.5,0.),(2.,.1),(2.4,.4),(2.7,.9),(2.9,1.6),(3.,2.6),(3.,4.),(3.,7.)]
    profile=[]
    for (y0,z0),(y1,z1) in zip(control,control[1:]):
        count=max(1,int(math.hypot(y1-y0,z1-z0)/.12))
        profile.extend((y0+(y1-y0)*i/count,z0+(z1-z0)*i/count) for i in range(count))
    profile.append(control[-1]);xs=np.linspace(-20.,20.,241);nx=len(xs)
    vv=[(float(x),y,z) for y,z in profile for x in xs];ff=[(j*nx+i,j*nx+i+1,(j+1)*nx+i+1,(j+1)*nx+i) for j in range(len(profile)-1) for i in range(nx-1)]
    me=bpy.data.meshes.new('SchoolCyclorama');me.from_pydata(vv,[],ff);me.update();obj=bpy.data.objects.new('Env_SchoolCyclorama',me);scene.collection.objects.link(obj)
    uv=me.uv_layers.new(name='CameraProjection')
    bpy.context.view_layer.update()
    for loop in me.loops:
        co=world_to_camera_view(scene,camera,me.vertices[loop.vertex_index].co);uv.data[loop.index].uv=(co.x,co.y)
    mat=bpy.data.materials.new('MAT_SchoolProjection');mat.use_nodes=True;nt=mat.node_tree;nt.nodes.clear()
    tex=nt.nodes.new('ShaderNodeTexImage');tex.image=bpy.data.images.load(background,check_existing=True);tex.image.pack();tex.extension='EXTEND'
    emit=nt.nodes.new('ShaderNodeEmission');nt.links.new(tex.outputs['Color'],emit.inputs[0]);out=nt.nodes.new('ShaderNodeOutputMaterial');nt.links.new(emit.outputs[0],out.inputs['Surface']);me.materials.append(mat)
    for name,location,energy,size in [('Key',(-3,-4,6),120,5),('Fill',(4,-2,4),70,4)]:
        light=bpy.data.lights.new(name,'AREA');light.energy=energy;light.shape='DISK';light.size=size;o=bpy.data.objects.new(name,light);o.location=location;o.rotation_euler=(Vector((0,0,1))-o.location).to_track_quat('-Z','Y').to_euler();scene.collection.objects.link(o)

def configure_scene(end,width,height):
    s=bpy.context.scene;s.frame_start=1;s.frame_end=end;s.render.fps=24;s.render.fps_base=1
    s.render.engine='BLENDER_EEVEE';s.render.resolution_x=width;s.render.resolution_y=height;s.render.resolution_percentage=100
    s.render.image_settings.file_format='PNG';s.render.image_settings.color_mode='RGBA';s.render.film_transparent=False
    s.view_settings.view_transform='Standard';s.view_settings.look='None';s.view_settings.exposure=0;s.view_settings.gamma=1
    if hasattr(s,'eevee') and hasattr(s.eevee,'taa_render_samples'):s.eevee.taa_render_samples=32
    return s

def export_character(rig,obj,blend,glb):
    scene=bpy.context.scene;scene.frame_set(1)
    bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);obj.select_set(True);bpy.context.view_layer.objects.active=rig
    bpy.ops.export_scene.gltf(filepath=glb,export_format='GLB',use_selection=True,export_animations=True,export_force_sampling=True,export_animation_mode='SCENE',export_anim_slide_to_zero=True)
    # Save a minimal .blend by temporarily unlinking preview environment, camera, and lights.
    removed=[]
    for o in list(scene.objects):
        if o not in (rig,obj):
            for c in list(o.users_collection):removed.append((c,o));c.objects.unlink(o)
    camera=scene.camera;scene.camera=None
    try:bpy.ops.wm.save_as_mainfile(filepath=blend)
    finally:
        for c,o in removed:c.objects.link(o)
        scene.camera=camera

def load_character_rig(spec,path,col_char,col_rig):
    if os.path.realpath(path)==os.path.realpath(bpy.data.filepath):
        alternate=os.path.join(spec['output_dir'],'assembly_source.blend');shutil.copy2(path,alternate);path=alternate
    with bpy.data.libraries.load(path,link=False) as (source,target):
        target.objects=[n for n in source.objects if n in ('RIG_'+spec['slug'],'CHAR_'+spec['slug'])]
    rig=next(o for o in target.objects if o.type=='ARMATURE');obj=next(o for o in target.objects if o.type=='MESH')
    col_rig.objects.link(rig);col_char.objects.link(obj)
    return rig,obj

def render_frames_to_mp4(scene,out_mp4,out_png,frame_start,frame_end,**kwargs):
    from pathlib import Path
    frames_dir=Path(os.path.splitext(out_mp4)[0]+'_frames');frames_dir.mkdir(parents=True,exist_ok=True)
    # Unique input list prevents stale trailing frames from a previous, longer render entering the video.
    entries=[];snapshots={}
    for f in range(frame_start,frame_end+1):
        scene.frame_set(f);p=frames_dir/('frame_%04d.png'%f);scene.render.filepath=str(p);bpy.ops.render.render(write_still=True)
        entries.append("file '"+str(p).replace("'","'\\''")+"'\nduration 0.041666666666666666\n")
        if f in (1,24,48):snapshots[str(f)]=str(p)
    expected={'frame_%04d.png'%f for f in range(frame_start,frame_end+1)}
    for stale in frames_dir.glob('frame_*.png'):
        if stale.name not in expected:stale.unlink()
    concat=frames_dir/'frames.ffconcat';concat.write_text('ffconcat version 1.0\n'+''.join(entries))
    subprocess.run(['ffmpeg','-v','error','-y','-f','concat','-safe','0','-i',str(concat),'-frames:v',str(frame_end-frame_start+1),'-r','24','-c:v','libx264','-crf','18','-pix_fmt','yuv420p',out_mp4],check=True)
    shutil.copy2(str(frames_dir/('frame_%04d.png'%frame_end)),out_png)
    return dict(frame_count=frame_end-frame_start+1,snapshots=snapshots,frames_dir=str(frames_dir))
'''


def _run_blender(code: str, timeout: float = 3600) -> dict:
    response = send_blender_mcp_code(BLENDER_HELPERS + '\n' + code, timeout=timeout)
    if response.get('status') != 'success':
        raise RuntimeError(f'Blender MCP failed: {response}')
    output = response.get('result', {}).get('result', '')
    marker = 'FARM_SCHOOL_RESULT='
    lines = [line[len(marker):] for line in output.splitlines() if line.startswith(marker)]
    if not lines:
        raise RuntimeError(f'Blender returned no verified result: {output[-3000:]}')
    return json.loads(lines[-1])


def render_single_character_blender53(slug: str, out_mp4: str, out_png: str,
                                      out_glb: str | None = None, out_blend: str | None = None) -> dict:
    out = Path(out_mp4).resolve().parent
    spec = prepare_volume(resolve_character_assets(slug), out / 'construction')
    spec['gait'] = gait_frames(spec, 48, solo=True)
    paths = dict(mp4=str(Path(out_mp4).resolve()), png=str(Path(out_png).resolve()),
                 glb=str(Path(out_glb).resolve()) if out_glb else str(Path(out_mp4).resolve().with_suffix('.glb')),
                 blend=str(Path(out_blend).resolve()) if out_blend else str(Path(out_mp4).resolve().with_suffix('.blend')))
    code = f'''
SPEC={spec!r}
PATHS={paths!r}
clear_scene()
configure_scene(48,960,540)
rig,obj=build_character_rig(SPEC,make_collection('Character'),make_collection('Rig'))
animate_character_rig(rig,SPEC)
qa=verify_character(rig,obj,SPEC)
cam=setup_camera('CharacterPreview',SPEC['height_m'])
build_environment(cam,{str(SCHOOL_BG)!r})
add_contact_shadows(SPEC)
render=render_frames_to_mp4(bpy.context.scene,PATHS['mp4'],PATHS['png'],1,48)
export_character(rig,obj,PATHS['blend'],PATHS['glb'])
print('FARM_SCHOOL_RESULT='+json.dumps(dict(ok=True,character=SPEC['slug'],qa=qa,render=render,**PATHS)))
'''
    report = _run_blender(code)
    for p in paths.values():
        if not Path(p).is_file() or Path(p).stat().st_size == 0:
            raise RuntimeError(f'Blender did not write {p}')
    (out / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def build_farm_school_milestone(out_dir: str | Path, only_character: str | None = None) -> dict:
    out = Path(out_dir).resolve();out.mkdir(parents=True,exist_ok=True)
    if only_character and only_character != 'all':
        slug=only_character.lower().strip();folder=out/'characters'/slug
        return render_single_character_blender53(slug,str(folder/f'{slug}_blender53.mp4'),str(folder/f'{slug}_blender53_preview.png'),str(folder/f'{slug}_rig.glb'),str(folder/f'{slug}_rig.blend'))
    reports=[];specs=[]
    # Validate all references before replacing any deliveries.
    sources=[resolve_character_assets(slug) for slug in SIX_CAST_ORDER]
    for source in sources:
        slug=source['slug'];folder=out/'characters'/slug
        reports.append(render_single_character_blender53(slug,str(folder/f'{slug}_blender53.mp4'),str(folder/f'{slug}_blender53_preview.png'),str(folder/f'{slug}_rig.glb'),str(folder/f'{slug}_rig.blend')))
        spec=json.loads((folder/'construction/asset_sources.json').read_text());spec['gait']=gait_frames(spec,48);specs.append(spec)
    paths={ext:str(out/f'farm_school_lineup_scene.{ext}') for ext in ['blend','glb','mp4','png']}
    code=f'''
SPECS={specs!r}
PATHS={paths!r}
clear_scene();configure_scene(48,1280,720)
chars=make_collection('Characters');rigs=make_collection('Rigs');qa={{}}
for spec in SPECS:
    rig,obj=load_character_rig(spec,os.path.join({str(out)!r},'characters',spec['slug'],spec['slug']+'_rig.blend'),chars,rigs)
    animate_character_rig(rig,spec);qa[spec['slug']]=verify_character(rig,obj,spec)
cam=setup_camera('FarmSchoolLineupCamera');build_environment(cam,{str(SCHOOL_BG)!r})
for spec in SPECS:add_contact_shadows(spec)
render=render_frames_to_mp4(bpy.context.scene,PATHS['mp4'],PATHS['png'],1,48)
for frame,name in [(1,'enter'),(24,'walking'),(48,'lined_up')]:
    shutil.copy2(render['snapshots'][str(frame)],os.path.join({str(out)!r},'farm_school_lineup_scene_frame%02d_%s.png'%(frame,name)))
bpy.context.scene.frame_set(48)
bpy.ops.wm.save_as_mainfile(filepath=PATHS['blend'])
bpy.ops.export_scene.gltf(filepath=PATHS['glb'],export_format='GLB',export_animations=True,export_force_sampling=True,export_animation_mode='SCENE',export_anim_slide_to_zero=True,export_cameras=True,export_lights=True)
print('FARM_SCHOOL_RESULT='+json.dumps(dict(ok=True,qa=qa,render=render,**PATHS)))
'''
    combined=_run_blender(code)
    report=dict(ok=True,characters=reports,combined_scene=combined)
    (out/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
    return report


def self_test() -> None:
    import math
    for slug in SIX_CAST_ORDER:
        spec=dict(slug=slug,**CHARACTER_STAGE_SPECS[slug],anatomy=ANATOMY[slug],performance=PERFORMANCES[slug],foot_lift_max_m=.03)
        rows=gait_frames(spec,48)
        for side in ['L','R']:
            for before,after in zip(rows,rows[1:]):
                assert after['feet'][side][2]>=0
                assert after['feet'][side][2]<=spec['foot_lift_max_m']+1e-9
                if before['planted'][side] and after['planted'][side]:
                    assert before['feet'][side]==after['feet'][side],(slug,side,after['frame'])
        assert all(math.isfinite(v) for row in rows for v in row['root'])
    compile(BLENDER_HELPERS,'<blender helpers>','exec')
    print('Gait contact, measured lift bounds, and embedded Blender syntax checks passed.')


def main(argv: list[str] | None = None) -> int:
    parser=argparse.ArgumentParser(description='Build reference-derived volumetric characters and a verified 24 fps Farm School scene')
    parser.add_argument('--out-dir',default='exports/farm_school_milestone')
    parser.add_argument('--character',default=None)
    parser.add_argument('--self-test',action='store_true')
    args=parser.parse_args(argv)
    if args.self_test:self_test();return 0
    print(json.dumps(build_farm_school_milestone(args.out_dir,args.character),indent=2));return 0


if __name__=='__main__':
    raise SystemExit(main())
