import base64
import hashlib
import json
import os
import socket
import struct
import subprocess
import time
import urllib.parse
import urllib.request

WORKDIR = "/home/jesse/Work/omhas-animation"
HTML_URL = "http://127.0.0.1:8765/3d_character_turnaround_blender_studio%20(1).html"
EXPORTS_DIR = os.path.join(WORKDIR, "exports")
ARTIFACT_DIR = "/home/jesse/.gemini/antigravity-cli/brain/a9ca0b8d-de87-40b6-96c7-9b14dfc02968"
HOPPER_REF = "/home/jesse/main-vault/Creative/Design/Characters/Hopper-no-background.png_202607180130.jpeg"


class SimpleWS:
    def __init__(self, url):
        u = urllib.parse.urlparse(url)
        self.sock = socket.create_connection((u.hostname, u.port), timeout=30)
        key = base64.b64encode(os.urandom(16)).decode()
        req = (
            f"GET {u.path} HTTP/1.1\r\n"
            f"Host: {u.hostname}:{u.port}\r\n"
            f"Upgrade: websocket\r\n"
            f"Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            f"Sec-WebSocket-Version: 13\r\n\r\n"
        )
        self.sock.sendall(req.encode())
        buf = b""
        while b"\r\n\r\n" not in buf:
            buf += self.sock.recv(4096)
        self.leftover = buf.split(b"\r\n\r\n", 1)[1]

    def _read_exact(self, n):
        while len(self.leftover) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise EOFError("WebSocket closed")
            self.leftover += chunk
        out, self.leftover = self.leftover[:n], self.leftover[n:]
        return out

    def send(self, text):
        data = text.encode("utf-8")
        header = bytearray([0x81])
        length = len(data)
        if length < 126:
            header.append(0x80 | length)
        elif length < 65536:
            header.append(0x80 | 126)
            header.extend(struct.pack("!H", length))
        else:
            header.append(0x80 | 127)
            header.extend(struct.pack("!Q", length))
        mask = os.urandom(4)
        header.extend(mask)
        masked = bytearray(length)
        for i in range(length):
            masked[i] = data[i] ^ mask[i % 4]
        self.sock.sendall(header + masked)

    def recv(self):
        b1, b2 = self._read_exact(2)
        length = b2 & 0x7F
        if length == 126:
            length = struct.unpack("!H", self._read_exact(2))[0]
        elif length == 127:
            length = struct.unpack("!Q", self._read_exact(8))[0]
        if b2 & 0x80:
            mask = self._read_exact(4)
            payload = bytearray(self._read_exact(length))
            for i in range(length):
                payload[i] ^= mask[i % 4]
            return payload.decode("utf-8", errors="replace")
        return self._read_exact(length).decode("utf-8", errors="replace")

    def close(self):
        self.sock.close()


def cdp_eval(ws, expr, msg_id=1):
    ws.send(json.dumps({
        "id": msg_id,
        "method": "Runtime.evaluate",
        "params": {"expression": expr, "returnByValue": True, "awaitPromise": True}
    }))
    while True:
        resp = json.loads(ws.recv())
        if resp.get("id") == msg_id:
            if "exceptionDetails" in resp.get("result", {}):
                print("JS Exception:", resp["result"]["exceptionDetails"])
            return resp.get("result", {}).get("result", {}).get("value")


def cdp_screenshot(ws, out_path, msg_id=100):
    ws.send(json.dumps({
        "id": msg_id,
        "method": "Page.captureScreenshot",
        "params": {"format": "png"}
    }))
    while True:
        resp = json.loads(ws.recv())
        if resp.get("id") == msg_id:
            b64 = resp["result"]["data"]
            with open(out_path, "wb") as f:
                f.write(base64.b64decode(b64))
            break


def run_browser_exports_and_screenshots():
    # Ensure HTTP server is running on 8765
    http_proc = None
    try:
        urllib.request.urlopen(HTML_URL, timeout=1)
    except Exception:
        http_proc = subprocess.Popen(
            ["python3", "-m", "http.server", "8765", "--bind", "127.0.0.1"],
            cwd=WORKDIR, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        time.sleep(0.8)

    proc = subprocess.Popen([
        "chromium", "--headless", "--no-sandbox", "--enable-webgl",
        "--ignore-gpu-blocklist", "--use-gl=angle", "--use-angle=swiftshader",
        "--window-size=1680,1020", "--remote-debugging-port=9225", HTML_URL
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    try:
        time.sleep(3.5)
        tabs = json.loads(urllib.request.urlopen("http://127.0.0.1:9225/json").read().decode())
        ws_url = next(t["webSocketDebuggerUrl"] for t in tabs if "url" in t and "3d_character" in t["url"])
        ws = SimpleWS(ws_url)

        diag = cdp_eval(ws, """(() => {
            const topo = computeLiveTopologyParitySummary();
            return {
                rigParts: RIG_26_PARTS.length,
                topoSummary: {
                    meshCount: topo.meshCount,
                    boneCount: topo.boneCount,
                    totalBufferVerts: topo.totalBufferVerts,
                    totalWeldedVerts: topo.totalWeldedVerts,
                    totalEdges: topo.totalEdges,
                    totalBoundaryEdges: topo.totalBoundaryEdges,
                    totalTris: topo.totalTris,
                    totalLoops: topo.totalLoops
                }
            };
        })()""", 1)
        print("HTML Live Topology Summary:", json.dumps(diag, indent=2))

        # 1. Screenshot 1: Default startup view with 2D Puppet Studio Dock + Upgraded Hopper (Rounded Spoon Ears, 5-Digit Paws, Knee Joints)
        cdp_eval(ws, "setAnimationMode('none'); resetPoseSliders(); snapTurnaroundAngle(0, 'FRONT 0°'); render2DPuppetStage();", 2)
        time.sleep(0.5)
        s1 = os.path.join(ARTIFACT_DIR, "html_v2_01_hopper_front_and_2d_studio.png")
        cdp_screenshot(ws, s1, 10)
        print("Saved:", s1)

        # 2. Export upgraded GLB & Python script directly from the browser
        glb_b64 = cdp_eval(ws, """new Promise((resolve) => {
            const state = prepareCleanExportClone();
            if (typeof THREE.GLTFExporter === 'function') {
                const exporter = new THREE.GLTFExporter();
                exporter.parse(state.exportTarget, (res) => {
                    restoreAfterExportClone(state);
                    const bytes = new Uint8Array(res);
                    let binary = '';
                    const chunk = 0x8000;
                    for (let i = 0; i < bytes.length; i += chunk) {
                        binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
                    }
                    resolve(btoa(binary));
                }, { binary: true, embedImages: true });
            } else {
                const glbBuffer = buildGLTF2Fallback(state.exportTarget, true);
                restoreAfterExportClone(state);
                const bytes = new Uint8Array(glbBuffer);
                let binary = '';
                const chunk = 0x8000;
                for (let i = 0; i < bytes.length; i += chunk) {
                    binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
                }
                resolve(btoa(binary));
            }
        })""", 3)
        glb_path = os.path.join(EXPORTS_DIR, "felt_bunny_explorer_blender.glb")
        with open(glb_path, "wb") as f:
            f.write(base64.b64decode(glb_b64))
        print("Exported upgraded GLB:", glb_path, os.path.getsize(glb_path), "bytes")

        py_code = cdp_eval(ws, "getBlenderPythonScript()", 4)
        py_path = os.path.join(EXPORTS_DIR, "build_bunny_blender.py")
        with open(py_path, "w") as f:
            f.write(py_code)
        print("Exported upgraded Blender 5.3 Python script:", py_path)

        # 3. Screenshot 2: Show Articulated Knee Bend (40°), Elbow/Paw Flex (36°), Eyelid Squint (32%), Laugh Viseme, and 18 Joint Pivot Markers at +32° angle
        cdp_eval(ws, """(() => {
            document.getElementById('sliderKneeBend').value = 40;
            document.getElementById('sliderElbowBend').value = 36;
            document.getElementById('sliderEyeBlink').value = 32;
            document.getElementById('sliderEyeGaze').value = 14;
            setMouthViseme('laugh');
            showJointPivotMarkers = true;
            rebuildJointPivotMarkers();
            snapTurnaroundAngle(34, '3/4 KNEE & PAW DEMO');
        })()""", 5)
        time.sleep(0.7)
        s2 = os.path.join(ARTIFACT_DIR, "html_v2_02_hopper_knees_paws_visemes.png")
        cdp_screenshot(ws, s2, 11)
        print("Saved:", s2)

        # 4. Screenshot 3: Open Header Jev/Grep & Live Topology Numbers Inspector
        cdp_eval(ws, """(() => {
            document.getElementById('headerGrepInput').value = 'knee';
            toggleTopologyNumbersModal();
        })()""", 6)
        time.sleep(0.5)
        s3 = os.path.join(ARTIFACT_DIR, "html_v2_03_header_jev_grep_topology.png")
        cdp_screenshot(ws, s3, 12)
        print("Saved:", s3)

        # 5. Screenshot 4: Load Hopper-no-background.png into the Import Modal to prove Border Flood-Fill no longer clips Hopper's cream ears/head
        with open(HOPPER_REF, "rb") as f:
            hopper_b64 = "data:image/jpeg;base64," + base64.b64encode(f.read()).decode()
        cdp_eval(ws, f"""new Promise((resolve) => {{
            document.getElementById('headerGrepDropdown').classList.add('hidden');
            openImportCharacterModal();
            const img = new Image();
            img.onload = () => {{
                uploadedSourceImage = img;
                reprocessUploadedImage();
                render2DPuppetStage();
                resolve(true);
            }};
            img.src = "{hopper_b64}";
        }})""", 7)
        time.sleep(0.6)
        s4 = os.path.join(ARTIFACT_DIR, "html_v2_04_hopper_floodfill_no_clipping.png")
        cdp_screenshot(ws, s4, 13)
        print("Saved:", s4)

        ws.close()
    finally:
        proc.terminate()
        if http_proc:
            http_proc.terminate()


def update_live_blender53_and_render():
    glb_path = os.path.join(EXPORTS_DIR, "felt_bunny_explorer_blender.glb")
    py_path = os.path.join(EXPORTS_DIR, "build_bunny_blender.py")
    out_render_front = os.path.join(ARTIFACT_DIR, "blender53_v2_hopper_18bone_front.png")
    out_render_34 = os.path.join(ARTIFACT_DIR, "blender53_v2_hopper_18bone_34pose.png")

    blender_code = f"""
import bpy
import math
from mathutils import Vector, Euler

# Clear previous objects so we load the fresh upgraded Hopper GLB + 18-bone rig cleanly
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
for block in list(bpy.data.meshes):
    if block.users == 0:
        bpy.data.meshes.remove(block)
for block in list(bpy.data.armatures):
    if block.users == 0:
        bpy.data.meshes
        bpy.data.armatures.remove(block)

bpy.ops.import_scene.gltf(filepath={repr(glb_path)})

with open({repr(py_path)}, 'r') as f:
    exec(compile(f.read(), {repr(py_path)}, 'exec'), globals())

# Ensure studio lighting & world exist for viewport render
if not bpy.data.lights:
    key_data = bpy.data.lights.new("Studio_Key", type='AREA')
    key_data.energy = 220.0
    key_data.size = 3.0
    key_obj = bpy.data.objects.new("Studio_Key", key_data)
    bpy.context.collection.objects.link(key_obj)
    key_obj.location = (2.2, -2.6, 2.8)
    key_obj.rotation_euler = (math.radians(52), 0, math.radians(38))

    fill_data = bpy.data.lights.new("Studio_Fill", type='AREA')
    fill_data.energy = 110.0
    fill_data.size = 3.5
    fill_obj = bpy.data.objects.new("Studio_Fill", fill_data)
    bpy.context.collection.objects.link(fill_obj)
    fill_obj.location = (-2.4, -2.0, 1.9)
    fill_obj.rotation_euler = (math.radians(65), 0, math.radians(-42))

cam = bpy.data.objects.get("Turnaround_Camera")
if cam:
    bpy.context.scene.camera = cam
    cam.location = (0.0, -3.55, 0.96)
    cam.rotation_euler = (math.radians(90), 0.0, 0.0)

# Configure viewport to show bones in front + solid/material preview + frame Hopper nicely
for win in bpy.context.window_manager.windows:
    for area in win.screen.areas:
        if area.type == 'VIEW_3D':
            for space in area.spaces:
                if space.type == 'VIEW_3D':
                    space.shading.type = 'MATERIAL'
                    space.overlay.show_overlays = True
                    space.overlay.show_bones = True
                    if space.region_3d:
                        space.region_3d.view_perspective = 'PERSP'
                        space.region_3d.view_location = Vector((0.0, 0.0, 0.94))
                        space.region_3d.view_distance = 2.65
                        space.region_3d.view_rotation = Euler((math.radians(86), 0.0, math.radians(8))).to_quaternion()

bpy.context.view_layer.update()
bpy.context.scene.render.filepath = {repr(out_render_front)}
bpy.context.scene.render.resolution_x = 1280
bpy.context.scene.render.resolution_y = 960
bpy.ops.render.opengl(write_still=True, view_context=True)

# Now pose the 18-bone armature in Blender 5.3 (bend knee DEF_Shin.L, elbow DEF_Forearm.L, paw DEF_Hand.L, jaw DEF_Jaw)
arm_obj = bpy.data.objects.get("Bunny_Explorer_Armature")
if arm_obj:
    bpy.context.view_layer.objects.active = arm_obj
    for bname, rot_deg in [
        ("DEF_Thigh.L", (-20, 0, 0)),
        ("DEF_Shin.L", (44, 0, 0)),
        ("DEF_Foot.L", (-22, 0, 0)),
        ("DEF_UpperArm.L", (-18, 0, -24)),
        ("DEF_Forearm.L", (-38, 0, 0)),
        ("DEF_Hand.L", (0, 0, 18)),
        ("DEF_Head", (-4, 14, 0)),
    ]:
        pb = arm_obj.pose.bones.get(bname)
        if pb:
            pb.rotation_mode = 'XYZ'
            pb.rotation_euler = (math.radians(rot_deg[0]), math.radians(rot_deg[1]), math.radians(rot_deg[2]))

for win in bpy.context.window_manager.windows:
    for area in win.screen.areas:
        if area.type == 'VIEW_3D':
            for space in area.spaces:
                if space.type == 'VIEW_3D' and space.region_3d:
                    space.region_3d.view_rotation = Euler((math.radians(84), 0.0, math.radians(28))).to_quaternion()

bpy.context.view_layer.update()
bpy.context.scene.render.filepath = {repr(out_render_34)}
bpy.ops.render.opengl(write_still=True, view_context=True)

# Reset pose to rest for clean topology parity check, keeping nice 3/4 front view in live Blender 5.3
if arm_obj:
    for pb in arm_obj.pose.bones:
        pb.rotation_euler = (0, 0, 0)
bpy.context.view_layer.update()
result = {{"status": "ok", "bones": len(arm_obj.data.bones) if arm_obj else 0, "meshes": sum(1 for o in bpy.data.objects if o.type == 'MESH')}}
"""
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(30.0)
    s.connect(("127.0.0.1", 9876))
    s.sendall(json.dumps({"type": "execute_code", "params": {"code": blender_code}}).encode("utf-8"))
    chunks = []
    while True:
        data = s.recv(65536)
        if not data:
            break
        chunks.append(data)
        try:
            resp = json.loads(b"".join(chunks).decode("utf-8"))
            print("Blender 5.3 Live Response:", resp)
            break
        except Exception:
            continue
    s.close()


if __name__ == "__main__":
    run_browser_exports_and_screenshots()
    update_live_blender53_and_render()
