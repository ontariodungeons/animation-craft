import json
import struct


def parse_glb_topology(glb_path: str) -> dict:
    with open(glb_path, "rb") as f:
        data = f.read()

    magic, version, length = struct.unpack_from("<III", data, 0)
    if magic != 0x46546C67:
        raise ValueError(f"Not a valid GLB file: {glb_path}")

    json_len, json_type = struct.unpack_from("<II", data, 12)
    gltf = json.loads(data[20 : 20 + json_len].decode("utf-8"))

    accessors = gltf.get("accessors", [])
    meshes = gltf.get("meshes", [])

    total_verts = 0
    total_tris = 0
    per_mesh = []

    for m in meshes:
        m_verts = 0
        m_tris = 0
        for prim in m.get("primitives", []):
            pos_idx = prim.get("attributes", {}).get("POSITION")
            v_cnt = accessors[pos_idx]["count"] if pos_idx is not None else 0
            idx_acc = prim.get("indices")
            if idx_acc is not None:
                t_cnt = accessors[idx_acc]["count"] // 3
            else:
                t_cnt = v_cnt // 3
            m_verts += v_cnt
            m_tris += t_cnt
        total_verts += m_verts
        total_tris += m_tris
        per_mesh.append({
            "name": m.get("name", "Mesh"),
            "verts": m_verts,
            "tris": m_tris,
            "loops": m_tris * 3,
        })

    return {
        "mesh_count": len(meshes),
        "total_verts": total_verts,
        "total_tris": total_tris,
        "expected_loops": total_tris * 3,
        "per_mesh": per_mesh,
    }
