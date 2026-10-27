#!/usr/bin/env python3
"""Stage 4C: Import 2D Motion Data (`footLiftPx` + Gait Cycle) onto 3D Blender Rig.

Third sequential stage of the refactored 3D+2D animation pipeline:
computes 2D-derived world-space foot targets (`gait_frames`) and drives the 3D
Blender armature (`animate_character_rig`), runs manifold & ground-contact QA
(`verify_character`), and exports `.blend`, `.glb`, `.mp4`, and `.png`.
"""
from __future__ import annotations

import json
import math
from pipeline.blender53_bridge import send_blender_mcp_code
from pipeline.step_blender_mesh_rig import BLENDER_MESH_RIG_CODE


def smooth01(t: float) -> float:
    t = min(1.0, max(0.0, t))
    return t * t * (3.0 - 2.0 * t)


def root_at(spec: dict, frame: float, solo: bool = False) -> tuple:
    if solo or spec["role"] == "teacher":
        x, y = (0.0, 0.0) if solo else spec["end_xy"]
        return x, y, 0.0, math.radians(spec.get("facing_yaw_deg", 0))
    t = smooth01((frame - 1) / 34)
    sx, sy = spec["start_xy"]
    ex, ey = spec["end_xy"]
    heading = math.atan2(ex - sx, -(ey - sy))
    turn = smooth01((frame - 35) / 13)
    return sx + (ex - sx) * t, sy + (ey - sy) * t, 0.0, heading * (1.0 - turn)


def gait_frames(spec: dict, count: int, solo: bool = False) -> list:
    """Swing between fixed world-space sole targets; stance feet never track the moving root."""
    a = spec["anatomy"]
    h = spec["height_m"]
    perf = spec["performance"]
    period = perf["period"]
    half = period // 2
    roots = [root_at(spec, f, solo) for f in range(1, count + period + 2)]

    def rest_target(side: str, f: float) -> list[float]:
        x, y, _, yaw = root_at(spec, f, solo)
        lx = (1 if side == "L" else -1) * a["leg_x"] * h
        return [x + math.cos(yaw) * lx, y + math.sin(yaw) * lx, 0.0]

    targets = {side: rest_target(side, 1) for side in ["L", "R"]}
    swing: dict[str, tuple] = {}
    result = []
    for f in range(1, count + 1):
        root = roots[f - 1]
        moving = spec["role"] == "student" and (solo or f <= 35)
        turn = not solo and spec["role"] == "student" and 35 < f <= 47
        cycle = (f - 1 + perf["offset"]) // half
        active = "L" if cycle % 2 == 0 else "R"
        phase = (f - 1 + perf["offset"]) % half
        row = dict(frame=f, root=list(root), feet={}, planted={})
        for side in ["L", "R"]:
            if (moving or turn) and side == active:
                if side not in swing:
                    end = min(48 if turn else 35, f + half - phase - 1) if not solo else f + half - phase - 1
                    goal = rest_target(side, min(35, end + half * 0.5) if moving and not solo else end)
                    if solo:
                        goal[1] = (-1 if cycle % 4 < 2 else 1) * h * 0.055
                    swing[side] = (targets[side][:], goal, f, max(1, end - f))
                start, goal, start_f, length = swing[side]
                u = min(1.0, (f - start_f + 1) / (length + 1))
                eased = smooth01(u)
                foot = [start[i] + (goal[i] - start[i]) * eased for i in range(3)]
                foot[2] = math.sin(math.pi * u) ** 2 * spec["foot_lift_max_m"] * perf["lift"]
                if phase == half - 1 or f in [35, 47]:
                    foot = goal[:]
                    targets[side] = goal[:]
                    swing.pop(side, None)
                row["feet"][side] = foot
                row["planted"][side] = False
            else:
                if side in swing:
                    targets[side] = swing.pop(side)[1][:]
                row["feet"][side] = targets[side][:]
                row["planted"][side] = True
        if moving or turn:
            l1 = math.hypot(0.04, a["hip"] - a["knee"]) * h
            l2 = math.hypot(0.04, a["knee"] - a["ankle"]) * h
            reach = (l1 + l2) * 0.998
            pelvis_z = 0.0
            for side, sign in [("L", 1), ("R", -1)]:
                hip_x = root[0] + math.cos(root[3]) * sign * a["leg_x"] * h
                hip_y = root[1] + math.sin(root[3]) * sign * a["leg_x"] * h
                foot = row["feet"][side]
                horizontal = math.hypot(foot[0] - hip_x, foot[1] - hip_y)
                if horizontal >= reach:
                    raise ValueError(f"{spec['slug']} frame {f}: stride exceeds anatomical reach")
                pelvis_z = min(pelvis_z, foot[2] + a["ankle"] * h + math.sqrt(reach * reach - horizontal * horizontal) - a["hip"] * h)
            row["root"][2] = pelvis_z
        result.append(row)
    return result


BLENDER_2D_ANIM_CODE = r'''
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

BLENDER_HELPERS = BLENDER_MESH_RIG_CODE + "\n" + BLENDER_2D_ANIM_CODE


def run_blender_stage(code: str, timeout: float = 3600) -> dict:
    full_script = BLENDER_HELPERS + "\n" + code
    try:
        response = send_blender_mcp_code(full_script, timeout=timeout)
        if response.get("status") != "success":
            raise RuntimeError(f"Blender MCP failed: {response}")
        output = response.get("result", {}).get("result", "")
    except (ConnectionRefusedError, OSError):
        from pathlib import Path
        import shutil
        import subprocess
        import tempfile
        blender_bin = shutil.which("blender") or "/usr/bin/blender"
        with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False, encoding="utf-8") as tf:
            tf.write(full_script)
            script_path = tf.name
        try:
            proc = subprocess.run(
                [blender_bin, "--background", "--python", script_path],
                capture_output=True, text=True, timeout=timeout,
            )
            output = (proc.stdout or "") + "\n" + (proc.stderr or "")
        finally:
            Path(script_path).unlink(missing_ok=True)
    marker = "FARM_SCHOOL_RESULT="
    lines = [line[len(marker):] for line in output.splitlines() if line.startswith(marker)]
    if not lines:
        raise RuntimeError(f"Blender returned no verified result: {output[-3000:]}")
    return json.loads(lines[-1])
