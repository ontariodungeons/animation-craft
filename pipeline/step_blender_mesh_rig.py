#!/usr/bin/env python3
"""Stage 4B: Blender 3D Mesh Remesh, Multi-View Projection Bake & `ARM_<slug>` Rigging.

Second sequential stage of the refactored 3D+2D animation pipeline:
loads `visual_hull.npz`, performs manifold voxel remeshing and arm-torso contact
partitioning (`img2threejs` L0/L1/L3 separation), bakes view-projected reference
textures into `baked_albedo.png`, and binds the `ARM_<slug>` armature.
"""
from __future__ import annotations

BLENDER_MESH_RIG_CODE = r'''
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
    head=nt.nodes.new('ShaderNodeMath');head.operation='GREATER_THAN';head.inputs[1].default_value=(spec['anatomy']['hip']-.02)*spec['height_m'];nt.links.new(xyz.outputs['Z'],head.inputs[0])
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
    for poly in me.polygons:
        center=poly.center;normal=poly.normal
        scores=[]
        for v in spec['views']:
            t=math.radians(v['angle']);direction=Vector((math.sin(t),-math.cos(t),0))
            score=normal.dot(direction)
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
'''
