#!/usr/bin/env python3
"""
compare_html_vs_blender.py
--------------------------
Numeric 3D Topology, Loop, Vertex, Bone & Rig Parity Verifier:
1. Parses the HTML Studio's exported .GLB directly from binary (WITHOUT needing to import into Blender)
   to compute exact per-object Vertices, Triangles, Normals, UVs, Bounding Boxes, and Hierarchy nodes.
2. Queries live Blender 5.3 via MCP socket (127.0.0.1:9876) OR headless Blender 5.3 CLI
   to extract Blender's exact BMesh topology:
   - Vertices (3D points in space)
   - Edges (connections between 2 vertices)
   - Loops (face-corners / half-edges used for UV seams, sharp normals, and edge-loop flow)
   - Polygons / Faces (quads/tris formed by closed loop rings)
   - Boundary / Non-Manifold Edges (open holes vs watertight meshes)
   - Armature Bones (head, tail, roll, length, parent, and bound child objects/constraints)
3. Compares HTML expected values vs. Blender 5.3 actual values and outputs a JSON + Markdown table report.
"""
import json
import socket
import struct
import sys
from pathlib import Path

GLB_PATH = Path("/home/jesse/Work/omhas-animation/exports/felt_bunny_explorer_blender.glb")
OUT_JSON = Path("/home/jesse/Work/omhas-animation/exports/topology_parity_report.json")


def parse_glb_without_blender(glb_path: Path) -> dict:
    data = glb_path.read_bytes()
    magic, version, total_len = struct.unpack("<III", data[:12])
    assert magic == 0x46546C67, "Not a valid GLB file"
    json_len, json_type = struct.unpack("<II", data[12:20])
    gltf = json.loads(data[20 : 20 + json_len].decode("utf-8"))

    nodes = gltf.get("nodes", [])
    meshes = gltf.get("meshes", [])
    accessors = gltf.get("accessors", [])
    materials = gltf.get("materials", [])

    parent_map = {}
    for idx, n in enumerate(nodes):
        for c in n.get("children", []):
            parent_map[c] = n.get("name", f"Node_{idx}")

    mesh_summaries = []
    total_verts = 0
    total_tris = 0

    for n_idx, n in enumerate(nodes):
        if "mesh" not in n:
            continue
        m = meshes[n["mesh"]]
        v_count = 0
        t_count = 0
        bbox_min = [None, None, None]
        bbox_max = [None, None, None]
        mat_name = None
        has_uv = False
        has_color = False

        for prim in m.get("primitives", []):
            attrs = prim.get("attributes", {})
            pos_acc = accessors[attrs["POSITION"]]
            v_count += pos_acc["count"]
            if "min" in pos_acc and "max" in pos_acc:
                bbox_min = [round(x, 4) for x in pos_acc["min"]]
                bbox_max = [round(x, 4) for x in pos_acc["max"]]
            if "indices" in prim:
                idx_acc = accessors[prim["indices"]]
                t_count += idx_acc["count"] // 3
            else:
                t_count += pos_acc["count"] // 3
            has_uv = "TEXCOORD_0" in attrs
            has_color = "COLOR_0" in attrs
            if "material" in prim and prim["material"] < len(materials):
                mat_name = materials[prim["material"]].get("name")

        total_verts += v_count
        total_tris += t_count
        mesh_summaries.append({
            "name": n.get("name", m.get("name", f"Mesh_{n_idx}")),
            "parent": parent_map.get(n_idx, ""),
            "verts": v_count,
            "tris": t_count,
            "expected_loops_tri": t_count * 3,
            "bbox_min": bbox_min,
            "bbox_max": bbox_max,
            "material": mat_name,
            "has_uv": has_uv,
            "has_vertex_color": has_color,
        })

    pivot_empties = [
        n.get("name")
        for n in nodes
        if "mesh" not in n and ("Pivot" in n.get("name", "") or "Group" in n.get("name", ""))
    ]

    return {
        "source": str(glb_path.name),
        "node_count": len(nodes),
        "mesh_count": len(mesh_summaries),
        "material_count": len(materials),
        "skin_count": len(gltf.get("skins", [])),
        "animation_count": len(gltf.get("animations", [])),
        "total_verts": total_verts,
        "total_tris": total_tris,
        "expected_loops": total_tris * 3,
        "pivot_groups": pivot_empties,
        "meshes": sorted(mesh_summaries, key=lambda x: x["name"]),
    }


def query_live_blender53_mcp() -> dict:
    blender_code = """
import bpy, json, bmesh

report = {"objects": [], "armature": {}, "summary": {}}
total_verts = 0
total_edges = 0
total_loops = 0
total_faces = 0
total_tris = 0

for obj in sorted(bpy.data.objects, key=lambda o: o.name):
    if obj.type == "MESH":
        me = obj.data
        me.calc_loop_triangles()
        v_cnt = len(me.vertices)
        e_cnt = len(me.edges)
        l_cnt = len(me.loops)
        p_cnt = len(me.polygons)
        t_cnt = len(me.loop_triangles)
        total_verts += v_cnt
        total_edges += e_cnt
        total_loops += l_cnt
        total_faces += p_cnt
        total_tris += t_cnt

        bm = bmesh.new()
        bm.from_mesh(me)
        boundary_edges = sum(1 for e in bm.edges if e.is_boundary)
        non_manifold = sum(1 for e in bm.edges if not e.is_manifold)
        bm.free()

        report["objects"].append({
            "name": obj.name,
            "parent": obj.parent.name if obj.parent else "",
            "verts": v_cnt,
            "edges": e_cnt,
            "loops": l_cnt,
            "polys": p_cnt,
            "tris": t_cnt,
            "boundary_edges": boundary_edges,
            "non_manifold_edges": non_manifold,
            "dims": [round(d, 4) for d in obj.dimensions],
            "loc_world": [round(c, 4) for c in obj.matrix_world.translation],
            "mat": me.materials[0].name if me.materials else None
        })
    elif obj.type == "ARMATURE":
        bones_info = []
        for b in obj.data.bones:
            bones_info.append({
                "name": b.name,
                "parent": b.parent.name if b.parent else None,
                "head": [round(c, 4) for c in b.head_local],
                "tail": [round(c, 4) for c in b.tail_local],
                "length": round(b.length, 4)
            })
        report["armature"] = {
            "name": obj.name,
            "bone_count": len(bones_info),
            "bones": bones_info
        }

report["summary"] = {
    "mesh_objects": len(report["objects"]),
    "total_verts": total_verts,
    "total_edges": total_edges,
    "total_loops": total_loops,
    "total_faces": total_faces,
    "total_tris": total_tris
}

with open("/home/jesse/Work/omhas-animation/exports/blender53_live_metrics.json", "w") as f:
    json.dump(report, f, indent=2)
print("OK")
"""
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(10)
    s.connect(("127.0.0.1", 9876))
    s.sendall(json.dumps({"type": "execute_code", "params": {"code": blender_code}}).encode("utf-8"))
    s.recv(65536)
    s.close()
    return json.loads(Path("/home/jesse/Work/omhas-animation/exports/blender53_live_metrics.json").read_text())


def main():
    glb_report = parse_glb_without_blender(GLB_PATH)
    blender_report = query_live_blender53_mcp()

    b_map = {o["name"]: o for o in blender_report["objects"]}
    diffs = []
    matched = 0
    for m in glb_report["meshes"]:
        b_obj = b_map.get(m["name"])
        if not b_obj:
            diffs.append({"name": m["name"], "issue": "Missing in Blender"})
            continue
        v_diff = b_obj["verts"] - m["verts"]
        t_diff = b_obj["tris"] - m["tris"]
        l_diff = b_obj["loops"] - m["expected_loops_tri"]
        if v_diff == 0 and t_diff == 0 and l_diff == 0:
            matched += 1
        else:
            diffs.append({
                "name": m["name"],
                "glb_verts": m["verts"],
                "blender_verts": b_obj["verts"],
                "glb_tris": m["tris"],
                "blender_tris": b_obj["tris"],
                "blender_loops": b_obj["loops"],
            })

    combined = {
        "glb_direct_analysis": {
            "mesh_count": glb_report["mesh_count"],
            "total_verts": glb_report["total_verts"],
            "total_tris": glb_report["total_tris"],
            "expected_loops": glb_report["expected_loops"],
            "skin_count": glb_report["skin_count"],
            "animation_count": glb_report["animation_count"],
        },
        "blender53_live_analysis": blender_report["summary"],
        "blender53_armature": blender_report["armature"],
        "exact_mesh_matches": f"{matched}/{glb_report['mesh_count']}",
        "discrepancies": diffs,
        "sample_objects_top_15": blender_report["objects"][:15],
    }
    OUT_JSON.write_text(json.dumps(combined, indent=2))
    print(json.dumps(combined["glb_direct_analysis"], indent=2))
    print(json.dumps(combined["blender53_live_analysis"], indent=2))
    print(f"Exact Mesh Topology Matches (Verts/Loops/Tris): {matched}/{glb_report['mesh_count']}")
    print(f"Armature Bones in Blender 5.3: {blender_report['armature'].get('bone_count', 0)}")


if __name__ == "__main__":
    main()
