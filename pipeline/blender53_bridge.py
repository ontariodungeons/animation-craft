import json
import os
import socket
from PIL import Image


def send_blender_mcp_code(code: str, host: str = "127.0.0.1", port: int = 9876, timeout: float = 90.0) -> dict:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(timeout)
    s.connect((host, port))
    s.sendall(json.dumps({"type": "execute_code", "params": {"code": code}}).encode("utf-8"))
    chunks = []
    while True:
        data = s.recv(65536)
        if not data:
            break
        chunks.append(data)
        try:
            resp = json.loads(b"".join(chunks).decode("utf-8"))
            s.close()
            return resp
        except Exception:
            continue
    s.close()
    return {"status": "error", "message": "Incomplete response from Blender MCP"}


def _build_combined_turnaround_atlas(sliced: dict, out_dir: str) -> str:
    """
    Combines the Jev-decontaminated, 12px UV-dilated Front (0°), Side (90°), and Back (180°, mirrored
    horizontally so world +X aligns in 3D) views into a single 1024x1024 UV atlas.
    """
    tv = sliced["turnaroundViews"]
    front_dil = Image.open(tv["frontDilated"]).convert("RGBA").resize((512, 1024), Image.Resampling.LANCZOS)
    back_dil = Image.open(tv["backDilated"]).convert("RGBA").resize((512, 1024), Image.Resampling.LANCZOS)
    back_mirrored = back_dil.transpose(Image.Transpose.FLIP_LEFT_RIGHT)

    atlas = Image.new("RGBA", (1024, 1024), (165, 140, 118, 255))
    atlas.paste(front_dil, (0, 0))
    atlas.paste(back_mirrored, (512, 0))
    atlas_path = os.path.join(out_dir, "turnaround_uv_atlas.png")
    atlas.save(atlas_path)
    return atlas_path


def execute_archetype_in_blender53(
    archetype: str,
    verb: str,
    glb_path: str | None,
    out_mp4: str,
    out_preview_png: str,
    strip_costume: bool = False,
    image_path: str | None = None,
) -> dict:
    out_dir = os.path.dirname(os.path.abspath(out_mp4))
    os.makedirs(out_dir, exist_ok=True)
    os.makedirs(os.path.dirname(os.path.abspath(out_preview_png)), exist_ok=True)

    slice_dir = os.path.join(out_dir, "slices")

    motion_range_summary = None
    char_hint = os.path.splitext(os.path.basename(image_path or "hopper"))[0].lower()
    opt_slug = "whiskers" if ("whiskers" in char_hint or "20" in char_hint) else "hopper"
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    turnaround_cut_dir = os.path.join(repo_root, "exports", "pose_and_parts", f"{opt_slug}_turnaround")
    poses_cut_dir = os.path.join(repo_root, "exports", "pose_and_parts", f"{opt_slug}_poses")
    opt_out_dir = os.path.join(repo_root, "exports", "pose_and_parts", opt_slug)
    if os.path.isdir(turnaround_cut_dir) and os.path.isdir(poses_cut_dir):
        try:
            from pipeline.codex_accuracy_optimizer import optimize, load_motion_range
            opt_rep = optimize(turnaround_cut_dir, poses_cut_dir, opt_out_dir)
            mr = load_motion_range(opt_rep["motionRangeFile"])
            motion_range_summary = mr.get("limits", {})
        except Exception:
            motion_range_summary = None

    atlas_path = _build_combined_turnaround_atlas(sliced, slice_dir)
    blend_path = os.path.join(out_dir, "character_turnaround_rig.blend")

    blender_py = f"""
import os
import glob
import json
import shutil
import math
import bpy
import bmesh
import numpy as np
from PIL import Image
from mathutils import Vector, Euler

ARCHETYPE = {repr(archetype)}
VERB = {repr(verb)}
OUT_MP4 = {repr(os.path.abspath(out_mp4))}
OUT_PNG = {repr(os.path.abspath(out_preview_png))}
BLEND_PATH = {repr(os.path.abspath(blend_path))}
SLICED = {repr(sliced)}
ATLAS_PATH = {repr(atlas_path)}
MOTION_LIMITS = {repr(motion_range_summary)}

# 1. Clear scene cleanly
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
for coll in list(bpy.data.collections):
    bpy.data.collections.remove(coll)
for block in list(bpy.data.meshes):
    if block.users == 0:
        bpy.data.meshes.remove(block)
for block in list(bpy.data.armatures):
    if block.users == 0:
        bpy.data.armatures.remove(block)
for block in list(bpy.data.materials):
    if block.users == 0:
        bpy.data.materials.remove(block)
for block in list(bpy.data.images):
    if block.users == 0:
        bpy.data.images.remove(block)

def make_collection(name, hide_render=False):
    c = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(c)
    c.hide_render = hide_render
    return c

col_ref = make_collection("COL_Reference", hide_render=True)
col_char = make_collection("COL_Character", hide_render=False)
col_rig = make_collection("COL_Rig", hide_render=False)
col_studio = make_collection("COL_Studio", hide_render=False)

def move_to_collection(obj, target_col):
    for c in list(obj.users_collection):
        c.objects.unlink(obj)
    target_col.objects.link(obj)

world = bpy.context.scene.world
if not world:
    world = bpy.data.worlds.new("StudioWorld")
    bpy.context.scene.world = world
world.use_nodes = True
bg_node = world.node_tree.nodes.get("Background")
if bg_node:
    bg_node.inputs[0].default_value = (0.14, 0.15, 0.18, 1.0)
    bg_node.inputs[1].default_value = 1.15

hw = max(1, SLICED["heroSize"]["width"])
hh = max(1, SLICED["heroSize"]["height"])
char_h = 1.68
char_w = char_h * (hw / hh)

# 2. Setup Orthographic Reference Planes in COL_Reference (Front 0°, Side 90°, Back 180°) with 1:1 Scale & Alpha Textures
tv = SLICED["turnaroundViews"]
front_pil = Image.open(tv["front"]).convert("RGBA")
side_pil = Image.open(tv["side"]).convert("RGBA")
back_pil = Image.open(tv["back"]).convert("RGBA")
side_dilated_pil = Image.open(tv["sideDilated"]).convert("RGBA")

# Auto-orient 90° Side view so u_side=0 is Front (-Y) and u_side=1 is Back (+Y)
f_np = np.array(front_pil.resize((48, 64), Image.Resampling.BILINEAR))
b_np = np.array(back_pil.resize((48, 64), Image.Resampling.BILINEAR))
s_np = np.array(side_pil.resize((48, 64), Image.Resampling.BILINEAR))
# Compare torso band (rows 22..44) left vs right of side image against back torso color (e.g. backpack/back fur)
b_torso_mask = b_np[22:44, 14:34, 3] > 80
b_mean_rgb = np.mean(b_np[22:44, 14:34, :3][b_torso_mask], axis=0) if np.any(b_torso_mask) else np.array([128.0, 128.0, 128.0])
s_left_mask = s_np[22:44, :24, 3] > 80
s_right_mask = s_np[22:44, 24:, 3] > 80
s_left_rgb = np.mean(s_np[22:44, :24, :3][s_left_mask], axis=0) if np.any(s_left_mask) else b_mean_rgb
s_right_rgb = np.mean(s_np[22:44, 24:, :3][s_right_mask], axis=0) if np.any(s_right_mask) else b_mean_rgb
# If left half of side image is closer to back/backpack color than right half, flip side image so u=0 is Front (-Y) and u=1 is Back (+Y)
if np.linalg.norm(s_left_rgb - b_mean_rgb) < np.linalg.norm(s_right_rgb - b_mean_rgb):
    side_pil = side_pil.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    side_dilated_pil = side_dilated_pil.transpose(Image.Transpose.FLIP_LEFT_RIGHT)

side_oriented_path = os.path.join(os.path.dirname(ATLAS_PATH), "side_oriented_90deg.png")
side_oriented_dilated_path = os.path.join(os.path.dirname(ATLAS_PATH), "side_oriented_dilated.png")
side_pil.save(side_oriented_path)
side_dilated_pil.save(side_oriented_dilated_path)

side_w_m = char_h * (side_pil.width / max(1, side_pil.height))

def build_ref_plane_mat(mat_name, img_path):
    rm = bpy.data.materials.new(mat_name)
    rm.use_nodes = True
    if hasattr(rm, "blend_method"):
        rm.blend_method = 'CLIP'
    nt = rm.node_tree
    nt.nodes.clear()
    out_n = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf_n = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf_n.inputs["Roughness"].default_value = 1.0
    if "Emission Strength" in bsdf_n.inputs:
        bsdf_n.inputs["Emission Strength"].default_value = 0.65
    tex_n = nt.nodes.new("ShaderNodeTexImage")
    tex_n.image = bpy.data.images.load(img_path)
    nt.links.new(tex_n.outputs["Color"], bsdf_n.inputs["Base Color"])
    if "Emission Color" in bsdf_n.inputs:
        nt.links.new(tex_n.outputs["Color"], bsdf_n.inputs["Emission Color"])
    nt.links.new(tex_n.outputs["Alpha"], bsdf_n.inputs["Alpha"])
    nt.links.new(bsdf_n.outputs["BSDF"], out_n.inputs["Surface"])
    return rm

ref_specs = [
    ("Ref_Front_0deg", tv["front"], (0.0, 1.25, 0.04 + char_h * 0.5), (math.radians(90), 0.0, 0.0), char_w),
    ("Ref_Side_90deg", side_oriented_path, (-1.25, 0.0, 0.04 + char_h * 0.5), (math.radians(90), 0.0, math.radians(-90)), side_w_m),
    ("Ref_Back_180deg", tv["back"], (0.0, -1.25, 0.04 + char_h * 0.5), (math.radians(90), 0.0, math.radians(180)), char_w),
]
for rname, rpath, rloc, rrot, rw in ref_specs:
    bpy.ops.mesh.primitive_plane_add(size=1.0, location=rloc, rotation=rrot)
    robj = bpy.context.active_object
    robj.name = rname
    robj.scale = (rw, char_h, 1.0)
    robj.data.materials.append(build_ref_plane_mat(f"MAT_{{rname}}", rpath))
    move_to_collection(robj, col_ref)

# 3. Build Seamless Watertight 3D Character Mesh from Front (0°) + Side (90°) + Back (180°) Turnaround Views
grid_cols = 56
grid_rows = 80
f_small = np.array(front_pil.resize((grid_cols, grid_rows), Image.Resampling.BILINEAR))
pmask = (f_small[:, :, 3] > 65).astype(np.uint8)

# Measure exact per-row Front (-Y) and Back (+Y) contour [u_front_side(r), u_back_side(r)] from the 90° Side view
s_small = np.array(side_pil.resize((grid_cols, grid_rows), Image.Resampling.BILINEAR))
s_alpha = s_small[:, :, 3] > 65
row_u_front = np.full(grid_rows, 0.25, dtype=np.float32)
row_u_back = np.full(grid_rows, 0.75, dtype=np.float32)
for r in range(grid_rows):
    xs = np.where(s_alpha[r, :])[0]
    if len(xs) >= 2:
        row_u_front[r] = float(xs.min()) / float(grid_cols - 1)
        row_u_back[r] = float(xs.max()) / float(grid_cols - 1)
    elif r > 0:
        row_u_front[r] = row_u_front[r - 1]
        row_u_back[r] = row_u_back[r - 1]

row_u_front = np.convolve(row_u_front, [0.2, 0.6, 0.2], mode='same')
row_u_back = np.convolve(row_u_back, [0.2, 0.6, 0.2], mode='same')

# Compute per-span local elliptical thickness sqrt(1 - xi^2) so narrow limbs/legs/ears get full side profile depth!
pmask[0, :] = 0
pmask[-1, :] = 0
pmask[:, 0] = 0
pmask[:, -1] = 0

local_ellipse_t = np.zeros((grid_rows, grid_cols), dtype=np.float32)
for r in range(grid_rows):
    in_span = False
    c0 = 0
    for c in range(grid_cols + 1):
        val = pmask[r, c] if c < grid_cols else 0
        if val and not in_span:
            in_span = True
            c0 = c
        elif not val and in_span:
            c1 = c - 1
            in_span = False
            c_mid = 0.5 * (c0 + c1)
            c_half = max(1.0, 0.5 * (c1 - c0) + 0.65)
            for cc in range(c0, c1 + 1):
                xi = (cc - c_mid) / c_half
                local_ellipse_t[r, cc] = math.sqrt(max(0.0, 1.0 - xi * xi))

dist = np.zeros((grid_rows, grid_cols), dtype=np.float32)
for r in range(grid_rows):
    for c in range(grid_cols):
        if pmask[r, c]:
            dist[r, c] = 999.0
for r in range(1, grid_rows - 1):
    for c in range(1, grid_cols - 1):
        if pmask[r, c]:
            dist[r, c] = min(dist[r, c], dist[r - 1, c] + 1.0, dist[r, c - 1] + 1.0, dist[r - 1, c - 1] + 1.414)
for r in range(grid_rows - 2, 0, -1):
    for c in range(grid_cols - 2, 0, -1):
        if pmask[r, c]:
            dist[r, c] = min(dist[r, c], dist[r + 1, c] + 1.0, dist[r, c + 1] + 1.0, dist[r + 1, c + 1] + 1.414)

side_depth_scale = min(0.58, side_w_m * 0.78)

quad_active = np.zeros((grid_rows - 1, grid_cols - 1), dtype=bool)
for r in range(grid_rows - 1):
    for c in range(grid_cols - 1):
        if pmask[r, c] and pmask[r, c + 1] and pmask[r + 1, c] and pmask[r + 1, c + 1]:
            quad_active[r, c] = True

interior_u = np.zeros((grid_rows, grid_cols), dtype=np.float32)
for r in range(grid_rows):
    xs = np.where(pmask[r, :] > 0)[0]
    if len(xs) >= 4:
        c_min = float(xs.min() + 1.5) / float(grid_cols - 1)
        c_max = float(xs.max() - 1.5) / float(grid_cols - 1)
        if c_max <= c_min:
            c_min = float(xs.min()) / float(grid_cols - 1)
            c_max = float(xs.max()) / float(grid_cols - 1)
        for c in range(grid_cols):
            raw_u = c / float(grid_cols - 1)
            interior_u[r, c] = float(np.clip(raw_u, c_min, c_max))
    else:
        for c in range(grid_cols):
            interior_u[r, c] = c / float(grid_cols - 1)

bm = bmesh.new()
uv_layer = bm.loops.layers.uv.new("UVMap")
uv_side_layer = bm.loops.layers.uv.new("UVMap_Side")

f_verts = {{}}
b_verts = {{}}
uv_coords = {{}}

for r in range(grid_rows):
    v = 1.0 - (r / float(grid_rows - 1))
    wz = 0.04 + v * char_h
    uf_row = float(row_u_front[r])
    ub_row = float(row_u_back[r])
    u_mid_row = 0.5 * (uf_row + ub_row)
    u_half_span = max(0.08, 0.5 * (ub_row - uf_row))

    for c in range(grid_cols):
        adj_quads = 0
        for qr in (r - 1, r):
            for qc in (c - 1, c):
                if 0 <= qr < grid_rows - 1 and 0 <= qc < grid_cols - 1 and quad_active[qr, qc]:
                    adj_quads += 1
        if adj_quads == 0:
            continue

        is_perimeter = (adj_quads < 4)
        u = c / float(grid_cols - 1)
        wx = (u - 0.5) * char_w

        cap_t = min(1.0, float(dist[r, c]) / 2.8)
        profile_t = 0.16 if is_perimeter else (float(local_ellipse_t[r, c]) * (0.72 + 0.28 * cap_t))

        u_side_f = u_mid_row - u_half_span * profile_t
        u_side_b = u_mid_row + u_half_span * profile_t

        y_front = (u_side_f - 0.5) * side_depth_scale
        y_back = (u_side_b - 0.5) * side_depth_scale

        fv = bm.verts.new((wx, y_front, wz))
        bv = bm.verts.new((wx, y_back, wz))
        f_verts[(r, c)] = fv
        b_verts[(r, c)] = bv
        v_clamped = float(np.clip(v, 0.03, 0.97))
        uv_coords[(r, c)] = (float(interior_u[r, c]), v_clamped)

def assign_front_back_uvs(face, keys):
    for loop, k in zip(face.loops, keys):
        loop[uv_layer].uv = uv_coords[k]

for r in range(grid_rows - 1):
    for c in range(grid_cols - 1):
        if not quad_active[r, c]:
            continue
        k00, k10 = (r, c), (r, c + 1)
        k01, k11 = (r + 1, c), (r + 1, c + 1)

        try:
            ff = bm.faces.new((f_verts[k00], f_verts[k01], f_verts[k11], f_verts[k10]))
            ff.material_index = 0
            assign_front_back_uvs(ff, (k00, k01, k11, k10))
        except ValueError:
            pass

        try:
            bf = bm.faces.new((b_verts[k00], b_verts[k10], b_verts[k11], b_verts[k01]))
            bf.material_index = 0
            assign_front_back_uvs(bf, (k00, k10, k11, k01))
        except ValueError:
            pass

        if r == 0 or not quad_active[r - 1, c]:
            try:
                sf = bm.faces.new((f_verts[k00], f_verts[k10], b_verts[k10], b_verts[k00]))
                sf.material_index = 0
                assign_front_back_uvs(sf, (k00, k10, k10, k00))
            except ValueError:
                pass
        if r == grid_rows - 2 or not quad_active[r + 1, c]:
            try:
                sf = bm.faces.new((f_verts[k11], f_verts[k01], b_verts[k01], b_verts[k11]))
                sf.material_index = 0
                assign_front_back_uvs(sf, (k11, k01, k01, k11))
            except ValueError:
                pass
        if c == 0 or not quad_active[r, c - 1]:
            try:
                sf = bm.faces.new((f_verts[k01], f_verts[k00], b_verts[k00], b_verts[k01]))
                sf.material_index = 0
                assign_front_back_uvs(sf, (k01, k00, k00, k01))
            except ValueError:
                pass
        if c == grid_cols - 2 or not quad_active[r, c + 1]:
            try:
                sf = bm.faces.new((f_verts[k10], f_verts[k11], b_verts[k11], b_verts[k10]))
                sf.material_index = 0
                assign_front_back_uvs(sf, (k10, k11, k11, k10))
            except ValueError:
                pass

# Smooth Y-depth across perimeter so the side cross-section is a seamless rounded cylinder with zero pinch ridge
for _ in range(3):
    bmesh.ops.smooth_vert(bm, verts=bm.verts, factor=0.45, use_axis_x=False, use_axis_y=True, use_axis_z=False)

# Compute UVMap_Side directly from exact rest-pose 3D (Y, Z) so the 90° Side Profile maps 1:1 from nose to backpack!
for face in bm.faces:
    for loop in face.loops:
        v_co = loop.vert.co
        r_idx = int(np.clip(round((1.0 - (v_co.z - 0.04) / max(0.01, char_h)) * (grid_rows - 1)), 0, grid_rows - 1))
        uf_r = float(row_u_front[r_idx])
        ub_r = float(row_u_back[r_idx])
        u_s = float(np.clip((v_co.y / max(0.01, side_depth_scale)) + 0.5, uf_r + 0.01, ub_r - 0.01))
        v_s = float(np.clip((v_co.z - 0.04) / max(0.01, char_h), 0.03, 0.97))
        loop[uv_side_layer].uv = (u_s, v_s)

bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
mesh_data = bpy.data.meshes.new("SM_Char_Body_Mesh")
bm.to_mesh(mesh_data)
bm.free()

for poly in mesh_data.polygons:
    poly.use_smooth = True

hero_mesh = bpy.data.objects.new("SM_Char_Body", mesh_data)
col_char.objects.link(hero_mesh)

subsurf = hero_mesh.modifiers.new(name="Subsurf", type='SUBSURF')
subsurf.levels = 1
subsurf.render_levels = 1

back_mirrored_path = os.path.join(os.path.dirname(ATLAS_PATH), "back_mirrored_dilated.png")
Image.open(tv["backDilated"]).convert("RGBA").transpose(Image.Transpose.FLIP_LEFT_RIGHT).save(back_mirrored_path)

# Build Unified 4-Direction Turnaround Material (0° Front + ±90° Left/Right Side + 180° Back)
mat = bpy.data.materials.new("MAT_Char_TriPlanar_360")
mat.use_nodes = True
nt = mat.node_tree
nt.nodes.clear()
out_node = nt.nodes.new("ShaderNodeOutputMaterial")
bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
bsdf.inputs["Roughness"].default_value = 0.88
if "Specular IOR Level" in bsdf.inputs:
    bsdf.inputs["Specular IOR Level"].default_value = 0.22
if "Emission Strength" in bsdf.inputs:
    bsdf.inputs["Emission Strength"].default_value = 0.42

uv_front_node = nt.nodes.new("ShaderNodeUVMap")
uv_front_node.uv_map = "UVMap"
uv_side_node = nt.nodes.new("ShaderNodeUVMap")
uv_side_node.uv_map = "UVMap_Side"

tex_front = nt.nodes.new("ShaderNodeTexImage")
tex_front.image = bpy.data.images.load(tv["frontDilated"])
nt.links.new(uv_front_node.outputs["UV"], tex_front.inputs["Vector"])

tex_back = nt.nodes.new("ShaderNodeTexImage")
tex_back.image = bpy.data.images.load(back_mirrored_path)
nt.links.new(uv_front_node.outputs["UV"], tex_back.inputs["Vector"])

tex_side = nt.nodes.new("ShaderNodeTexImage")
tex_side.image = bpy.data.images.load(side_oriented_dilated_path)
nt.links.new(uv_side_node.outputs["UV"], tex_side.inputs["Vector"])

# Transform both Normal and Camera Incoming Vector -> Object Space so all 4 directions map cleanly without side seams!
geom_node = nt.nodes.new("ShaderNodeNewGeometry")
vec_xform = nt.nodes.new("ShaderNodeVectorTransform")
vec_xform.vector_type = 'NORMAL'
vec_xform.convert_from = 'WORLD'
vec_xform.convert_to = 'OBJECT'
sep_xyz = nt.nodes.new("ShaderNodeSeparateXYZ")

nt.links.new(geom_node.outputs["Normal"], vec_xform.inputs["Vector"])
nt.links.new(vec_xform.outputs["Vector"], sep_xyz.inputs["Vector"])

cam_xform = nt.nodes.new("ShaderNodeVectorTransform")
cam_xform.vector_type = 'VECTOR'
cam_xform.convert_from = 'WORLD'
cam_xform.convert_to = 'OBJECT'
sep_cam = nt.nodes.new("ShaderNodeSeparateXYZ")
nt.links.new(geom_node.outputs["Incoming"], cam_xform.inputs["Vector"])
nt.links.new(cam_xform.outputs["Vector"], sep_cam.inputs["Vector"])

# 1) Front (-Y) vs Back (+Y) blend
map_fb = nt.nodes.new("ShaderNodeMapRange")
map_fb.interpolation_type = 'SMOOTHSTEP'
map_fb.inputs["From Min"].default_value = -0.15
map_fb.inputs["From Max"].default_value = 0.15
mix_fb = nt.nodes.new("ShaderNodeMix")
mix_fb.data_type = 'RGBA'
nt.links.new(sep_xyz.outputs["Y"], map_fb.inputs["Value"])
nt.links.new(map_fb.outputs["Result"], mix_fb.inputs["Factor"])
nt.links.new(tex_front.outputs["Color"], mix_fb.inputs["A"])
nt.links.new(tex_back.outputs["Color"], mix_fb.inputs["B"])

# 2) Side (±X) blend combining Object Normal.X (0.46..0.74) and near-90° Camera Incoming.X (0.84..0.94)
abs_nx = nt.nodes.new("ShaderNodeMath")
abs_nx.operation = 'ABSOLUTE'
nt.links.new(sep_xyz.outputs["X"], abs_nx.inputs[0])

map_nx = nt.nodes.new("ShaderNodeMapRange")
map_nx.interpolation_type = 'SMOOTHSTEP'
map_nx.inputs["From Min"].default_value = 0.46
map_nx.inputs["From Max"].default_value = 0.74
nt.links.new(abs_nx.outputs["Value"], map_nx.inputs["Value"])

abs_cx = nt.nodes.new("ShaderNodeMath")
abs_cx.operation = 'ABSOLUTE'
nt.links.new(sep_cam.outputs["X"], abs_cx.inputs[0])

map_cx = nt.nodes.new("ShaderNodeMapRange")
map_cx.interpolation_type = 'SMOOTHSTEP'
map_cx.inputs["From Min"].default_value = 0.84
map_cx.inputs["From Max"].default_value = 0.94
nt.links.new(abs_cx.outputs["Value"], map_cx.inputs["Value"])

max_side = nt.nodes.new("ShaderNodeMath")
max_side.operation = 'MAXIMUM'
nt.links.new(map_nx.outputs["Result"], max_side.inputs[0])
nt.links.new(map_cx.outputs["Result"], max_side.inputs[1])

mix_side = nt.nodes.new("ShaderNodeMix")
mix_side.data_type = 'RGBA'
nt.links.new(max_side.outputs["Value"], mix_side.inputs["Factor"])
nt.links.new(mix_fb.outputs["Result"], mix_side.inputs["A"])
nt.links.new(tex_side.outputs["Color"], mix_side.inputs["B"])

nt.links.new(mix_side.outputs["Result"], bsdf.inputs["Base Color"])
if "Emission Color" in bsdf.inputs:
    nt.links.new(mix_side.outputs["Result"], bsdf.inputs["Emission Color"])
nt.links.new(bsdf.outputs["BSDF"], out_node.inputs["Surface"])
hero_mesh.data.materials.append(mat)

# 4. Build Production 16-Bone Armature (ARM_Character) per `.claude/skills/rigging/SKILL.md`
bpy.ops.object.armature_add(enter_editmode=True, location=(0, 0, 0))
arm_obj = bpy.context.active_object
arm_obj.name = "ARM_Character"
arm_obj.show_in_front = False
move_to_collection(arm_obj, col_rig)
arm = arm_obj.data

b_root = arm.edit_bones[0]
b_root.name = "root"
b_root.head = (0, 0, 0.0)
b_root.tail = (0, 0, 0.25)

b_pelvis = arm.edit_bones.new("spine_01")
b_pelvis.head = (0, 0, 0.48)
b_pelvis.tail = (0, 0, 0.68)
b_pelvis.parent = b_root

b_spine = arm.edit_bones.new("spine_02")
b_spine.head = (0, 0, 0.68)
b_spine.tail = (0, 0, 0.92)
b_spine.parent = b_pelvis
b_spine.use_connect = True

b_head = arm.edit_bones.new("head")
b_head.head = (0, 0, 0.92)
b_head.tail = (0, 0, 1.32)
b_head.parent = b_spine
b_head.use_connect = True

b_pack = arm.edit_bones.new("backpack")
b_pack.head = (0, 0.08, 0.72)
b_pack.tail = (0, 0.22, 0.90)
b_pack.parent = b_spine

for side_lbl, sx in [("L", -1.0), ("R", 1.0)]:
    b_ear = arm.edit_bones.new(f"{{side_lbl}}_ear")
    b_ear.head = (sx * 0.14, 0, 1.30)
    b_ear.tail = (sx * 0.20, 0, 1.66)
    b_ear.parent = b_head

    b_uarm = arm.edit_bones.new(f"{{side_lbl}}_arm_upper")
    b_uarm.head = (sx * (char_w * 0.25), 0, 0.86)
    b_uarm.tail = (sx * (char_w * 0.33), 0, 0.68)
    b_uarm.parent = b_spine

    b_larm = arm.edit_bones.new(f"{{side_lbl}}_arm_lower")
    b_larm.head = (sx * (char_w * 0.33), 0, 0.68)
    b_larm.tail = (sx * (char_w * 0.37), -0.02, 0.50)
    b_larm.parent = b_uarm
    b_larm.use_connect = True

    b_uleg = arm.edit_bones.new(f"{{side_lbl}}_leg_upper")
    b_uleg.head = (sx * (char_w * 0.14), 0, 0.44)
    b_uleg.tail = (sx * (char_w * 0.14), -0.01, 0.25)
    b_uleg.parent = b_pelvis

    b_lleg = arm.edit_bones.new(f"{{side_lbl}}_leg_lower")
    b_lleg.head = (sx * (char_w * 0.14), -0.01, 0.25)
    b_lleg.tail = (sx * (char_w * 0.14), 0.0, 0.09)
    b_lleg.parent = b_uleg
    b_lleg.use_connect = True

    b_foot = arm.edit_bones.new(f"{{side_lbl}}_foot")
    b_foot.head = (sx * (char_w * 0.14), 0.0, 0.09)
    b_foot.tail = (sx * (char_w * 0.14), -0.10, 0.04)
    b_foot.parent = b_lleg
    b_foot.use_connect = True

bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.update()

# 5. Smooth Gradient Bone Weight Binding (Zero hard tears at joints!)
all_bones = [b.name for b in arm_obj.data.bones]
vgroups = {{bname: hero_mesh.vertex_groups.new(name=bname) for bname in all_bones}}

for v in hero_mesh.data.vertices:
    vx, vy, vz = v.co.x, v.co.y, v.co.z
    rel_z = (vz - 0.04) / max(0.01, char_h)
    rel_x = vx / max(0.01, char_w)
    side = "L" if vx < 0 else "R"

    weights = []
    if rel_z < 0.27:
        # Smooth transition from pelvis/shorts into upper leg -> knee -> foot
        pelvis_blend = max(0.0, min(1.0, (rel_z - 0.19) / 0.08))
        if vz > 0.24:
            weights = [(f"{{side}}_leg_upper", 1.0 - pelvis_blend * 0.55), ("spine_01", pelvis_blend * 0.55)]
        elif vz > 0.11:
            t = max(0.0, min(1.0, (vz - 0.11) / 0.13))
            weights = [(f"{{side}}_leg_upper", t * 0.45), (f"{{side}}_leg_lower", 1.0 - t * 0.45)]
        else:
            weights = [(f"{{side}}_foot", 0.75), (f"{{side}}_leg_lower", 0.25)]
    elif 0.27 <= rel_z <= 0.52 and abs(rel_x) > 0.25:
        shoulder_blend = max(0.0, min(1.0, (rel_z - 0.44) / 0.08))
        if vz > 0.66:
            weights = [(f"{{side}}_arm_upper", 1.0 - shoulder_blend * 0.45), ("spine_02", shoulder_blend * 0.45)]
        else:
            weights = [(f"{{side}}_arm_lower", 0.72), (f"{{side}}_arm_upper", 0.28)]
    elif rel_z > 0.52:
        if rel_z > 0.76:
            ear_blend = max(0.0, min(1.0, (rel_z - 0.76) / 0.08))
            weights = [(f"{{side}}_ear", ear_blend * 0.85), ("head", 1.0 - ear_blend * 0.85)]
        else:
            neck_blend = max(0.0, min(1.0, (0.58 - rel_z) / 0.06))
            weights = [("head", 1.0 - neck_blend * 0.4), ("spine_02", neck_blend * 0.4)]
    else:
        if vy > 0.08 and 0.30 <= rel_z <= 0.54:
            weights = [("backpack", 0.65), ("spine_02", 0.35)]
        else:
            t = max(0.0, min(1.0, (rel_z - 0.27) / 0.25))
            weights = [("spine_02", t), ("spine_01", 1.0 - t)]

    for bname, w in weights:
        if w > 0.01:
            vgroups[bname].add([v.index], float(w), 'REPLACE')

arm_mod = hero_mesh.modifiers.new(name="Armature", type='ARMATURE')
arm_mod.object = arm_obj
bpy.context.view_layer.objects.active = hero_mesh
bpy.ops.object.modifier_move_to_index(modifier="Armature", index=0)

# 6. Keyframe Full 3D Walk + Wave + 360° Turnaround Animation (48 frames @ 24fps) per `walk-cycle.md`
bpy.context.scene.frame_start = 1
bpy.context.scene.frame_end = 48
bpy.context.scene.render.fps = 24

bpy.ops.object.empty_add(type='PLAIN_AXES', location=(0, 0, 0))
turntable = bpy.context.active_object
turntable.name = "Turntable_Pivot"
move_to_collection(turntable, col_rig)
hero_mesh.parent = turntable
arm_obj.parent = turntable

for pb in arm_obj.pose.bones:
    pb.rotation_mode = 'XYZ'

for f in range(1, 49):
    bpy.context.scene.frame_set(f)
    cycle_t = ((f - 1) % 24) / 24.0
    w = cycle_t * math.pi * 2.0

    turn_angle = ((f - 1) / 48.0) * math.pi * 2.0
    turntable.rotation_euler = (0.0, 0.0, turn_angle)
    turntable.keyframe_insert(data_path="rotation_euler", frame=f)

    stride_l = math.sin(w)
    stride_r = -math.sin(w)
    lift_l = max(0.0, math.cos(w))
    lift_r = max(0.0, -math.cos(w))
    bob = -math.cos(w * 2.0) * 0.038 if VERB != "jump_rise_fall" else abs(math.sin(w)) * 0.20

    pb_pelvis = arm_obj.pose.bones["spine_01"]
    pb_pelvis.location = (math.sin(w) * 0.02, 0.0, bob)
    pb_pelvis.rotation_euler = (math.radians(3.0), math.sin(w) * 0.05, math.sin(w) * 0.10)
    pb_pelvis.keyframe_insert(data_path="location", frame=f)
    pb_pelvis.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_spine = arm_obj.pose.bones["spine_02"]
    pb_spine.rotation_euler = (math.radians(2.0), -math.sin(w) * 0.04, -math.sin(w) * 0.12)
    pb_spine.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_head = arm_obj.pose.bones["head"]
    pb_head.rotation_euler = (math.sin(w * 2.0) * 0.07, math.sin(w) * 0.07, -math.sin(w) * 0.09)
    pb_head.keyframe_insert(data_path="rotation_euler", frame=f)

    w_lag = w - 0.65
    for side_lbl, sgn in [("L", -1.0), ("R", 1.0)]:
        pb_ear = arm_obj.pose.bones[f"{{side_lbl}}_ear"]
        pb_ear.rotation_euler = (-math.sin(w_lag * 2.0) * 0.16, sgn * (0.06 + abs(math.sin(w_lag)) * 0.10), 0.0)
        pb_ear.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_pack = arm_obj.pose.bones["backpack"]
    pb_pack.location = (0.0, max(0.0, -math.sin(w_lag * 2.0)) * 0.015, -math.cos(w_lag * 2.0) * 0.012)
    pb_pack.rotation_euler = (-math.sin(w_lag * 2.0) * 0.08, 0.0, 0.0)
    pb_pack.keyframe_insert(data_path="location", frame=f)
    pb_pack.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_uleg_l = arm_obj.pose.bones["L_leg_upper"]
    pb_uleg_l.rotation_euler = (stride_l * 0.46, 0.0, 0.0)
    pb_uleg_l.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_lleg_l = arm_obj.pose.bones["L_leg_lower"]
    pb_lleg_l.rotation_euler = (-lift_l * 0.55 - 0.05, 0.0, 0.0)
    pb_lleg_l.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_foot_l = arm_obj.pose.bones["L_foot"]
    pb_foot_l.rotation_euler = (-stride_l * 0.22 + lift_l * 0.18, 0.0, 0.0)
    pb_foot_l.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_uleg_r = arm_obj.pose.bones["R_leg_upper"]
    pb_uleg_r.rotation_euler = (stride_r * 0.46, 0.0, 0.0)
    pb_uleg_r.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_lleg_r = arm_obj.pose.bones["R_leg_lower"]
    pb_lleg_r.rotation_euler = (-lift_r * 0.55 - 0.05, 0.0, 0.0)
    pb_lleg_r.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_foot_r = arm_obj.pose.bones["R_foot"]
    pb_foot_r.rotation_euler = (-stride_r * 0.22 + lift_r * 0.18, 0.0, 0.0)
    pb_foot_r.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_uarm_l = arm_obj.pose.bones["L_arm_upper"]
    pb_uarm_l.rotation_euler = (stride_r * 0.40, -0.06, 0.0)
    pb_uarm_l.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_larm_l = arm_obj.pose.bones["L_arm_lower"]
    pb_larm_l.rotation_euler = (max(0.08, stride_r * 0.35 + 0.18), 0.0, 0.0)
    pb_larm_l.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_uarm_r = arm_obj.pose.bones["R_arm_upper"]
    pb_uarm_r.rotation_euler = (stride_l * 0.40, 0.06, 0.0)
    pb_uarm_r.keyframe_insert(data_path="rotation_euler", frame=f)

    pb_larm_r = arm_obj.pose.bones["R_arm_lower"]
    pb_larm_r.rotation_euler = (max(0.08, stride_l * 0.35 + 0.18), 0.0, 0.0)
    pb_larm_r.keyframe_insert(data_path="rotation_euler", frame=f)

if arm_obj.animation_data and arm_obj.animation_data.action:
    arm_obj.animation_data.action.name = f"AN_Char_{{VERB}}"

# 7. Studio Floor, Camera & 3-Point Lighting in COL_Studio
bpy.ops.mesh.primitive_circle_add(vertices=64, radius=1.35, fill_type='NGON', location=(0, 0, 0.0))
pedestal = bpy.context.active_object
pedestal.name = "SM_Studio_Pedestal"
move_to_collection(pedestal, col_studio)
mat_ped = bpy.data.materials.new("MAT_Studio_Pedestal")
mat_ped.use_nodes = True
mat_ped.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.18, 0.20, 0.24, 1.0)
mat_ped.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.65
pedestal.data.materials.append(mat_ped)

bpy.ops.object.camera_add(location=(0.0, -3.45, 0.90), rotation=(math.radians(88.5), 0.0, 0.0))
cam = bpy.context.active_object
cam.name = "CAM_Hero"
cam.data.lens = 52
move_to_collection(cam, col_studio)
bpy.context.scene.camera = cam

bpy.ops.object.light_add(type='SUN', location=(2.2, -2.8, 3.6), rotation=(math.radians(48), math.radians(15), math.radians(24)))
sun = bpy.context.active_object
sun.name = "LGT_Key_Sun"
sun.data.energy = 3.2
move_to_collection(sun, col_studio)

bpy.ops.object.light_add(type='AREA', location=(-2.4, -1.8, 1.8), rotation=(math.radians(65), 0, math.radians(-38)))
fill_lgt = bpy.context.active_object
fill_lgt.name = "LGT_Fill_Area"
fill_lgt.data.energy = 110.0
fill_lgt.data.size = 2.5
move_to_collection(fill_lgt, col_studio)

bpy.ops.object.light_add(type='SPOT', location=(0.0, 2.5, 2.4), rotation=(math.radians(-52), 0, math.radians(180)))
rim_lgt = bpy.context.active_object
rim_lgt.name = "LGT_Rim_Spot"
rim_lgt.data.energy = 180.0
move_to_collection(rim_lgt, col_studio)

scene = bpy.context.scene
scene.render.resolution_x = 960
scene.render.resolution_y = 720
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = 'PNG'

for area in bpy.context.screen.areas:
    if area.type == 'VIEW_3D':
        for space in area.spaces:
            if space.type == 'VIEW_3D':
                space.shading.type = 'MATERIAL'
                space.overlay.show_bones = False
                space.overlay.show_relationship_lines = False
                space.overlay.show_extras = False
                space.overlay.show_floor = True
                space.region_3d.view_perspective = 'CAMERA'

frames_dir = OUT_MP4 + "_frames"
os.makedirs(frames_dir, exist_ok=True)
for old_f in glob.glob(os.path.join(frames_dir, "*.png")):
    os.remove(old_f)

for f in range(1, 49):
    scene.frame_set(f)
    scene.render.filepath = os.path.join(frames_dir, f"frame_{{f:04d}}.png")
    bpy.ops.render.render(write_still=True)

# Park timeline at frame 1 (0deg aligned with reference planes) and save .blend
scene.frame_set(1)
bpy.ops.wm.save_as_mainfile(filepath=BLEND_PATH)

key_frames = [
    (4, "0° FRONT WALK STRIDE"),
    (9, "45° 3/4 WALK & ARM SWING"),
    (13, "90° SIDE PROFILE STRIDE (WATERTIGHT)"),
    (25, "180° BACKPACK & REAR STRIDE"),
]
panel_imgs = [Image.open(os.path.join(frames_dir, f"frame_{{kf:04d}}.png")).convert("RGBA") for kf, _ in key_frames]
pw, ph = panel_imgs[0].size
grid_img = Image.new("RGBA", (pw * 2, ph * 2), (28, 30, 36, 255))
grid_img.paste(panel_imgs[0], (0, 0))
grid_img.paste(panel_imgs[1], (pw, 0))
grid_img.paste(panel_imgs[2], (0, ph))
grid_img.paste(panel_imgs[3], (pw, ph))
grid_img.save(OUT_PNG)

result = {{
    "archetype": ARCHETYPE,
    "verb": VERB,
    "blendPath": BLEND_PATH,
    "sourceImage": SLICED["sourceImage"],
    "viewsCount": len(SLICED["views"]),
    "vertices": len(hero_mesh.data.vertices),
    "polygons": len(hero_mesh.data.polygons),
    "bones": len(arm_obj.data.bones),
    "framesDir": frames_dir,
    "previewPng": OUT_PNG,
}}
print(json.dumps(result))
"""
    mcp_resp = send_blender_mcp_code(blender_py)
    frames_dir = out_mp4 + "_frames"
    if os.path.isdir(frames_dir):
        cmd = (
            f'ffmpeg -y -framerate 24 -i "{frames_dir}/frame_%04d.png" '
            f'-c:v libx264 -pix_fmt yuv420p -crf 18 "{out_mp4}" >/dev/null 2>&1'
        )
        os.system(cmd)
    return {
        "mcpResponse": mcp_resp,
        "outMp4": out_mp4,
        "outPreviewPng": out_preview_png,
        "blendPath": blend_path,
        "mp4Exists": os.path.exists(out_mp4) and os.path.getsize(out_mp4) > 0,
        "pngExists": os.path.exists(out_preview_png) and os.path.getsize(out_preview_png) > 0,
    }
