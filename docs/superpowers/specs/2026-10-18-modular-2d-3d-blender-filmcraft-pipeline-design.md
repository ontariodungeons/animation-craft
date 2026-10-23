# Modular Image-to-Animation Studio, Blender 5.3 Bridge & FilmCraft Video Pipeline Design

> **Historical design reference — not current operating instructions.**
> The current Magic Studio application is the Tauri shell; use [README](../../../README.md)
> and `mc --help` for its canonical CLI. `studio_pipeline_cli.py` remains an internal engine adapter.
> Its older unified-CLI/browser descriptions, shared-checkout steps, ports, tool versions,
> completion claims and proposed features below are historical, not instructions.


Date: 2026-10-18
Status: Historical proposal; consult current runtime and task scope
Workspace: `/home/jesse/Work/omhas-animation`

---

## 1. Intent & Why

The goal of this project is to evolve [3d_character_turnaround_blender_studio (1).html](file:///home/jesse/Work/omhas-animation/3d_character_turnaround_blender_studio%20(1).html) from a monolithic 6,800-line demo focused on a single biped bunny into a **modular TypeScript + Python animation pipeline (`Track B`)** built for fast, reliable motion video production:

```
2D Image Upload
  ──► 1. Entry point, should trigger maybe the default omarchy agent to run this, or the agent will ask for the image and run: AI 3-Question Intake & Part Slicer (Local Web App + TypeSafe Jev / Vision Router)
  ──► 2. Modular Skeleton + Edge-Loop + Costume Skin Builder (TypeScript & Python Functions). Threejs animation simulating the intended blender output in the application.
  ──► 3. Live Blender 5.3 Bridge (Visual Animation Check + Flattened 2D Ortho / 3D Turntable Render)
  ──► 4. FilmCraft (`filmcraft-cli`), EffectCraft (`effectcraft-cli`) & Remotion Video Import
```

### Why This Architecture
1. **No False "One-Click Magic" on Arbitrary Uploads**: A user might upload a farm character (**Hopper**, **Jenny**, **Whiskers**), a **fish** that needs oscillating fins, a **smiling sun** that rises and falls, or a **boat** that banks and turns. Instead of blindly assuming a humanoid rig, an **AI 3-Question Intake Router** inspects the image, asks (or resolves via TypeSafe `jev-latest`) 3 targeted questions, and selects the exact skeleton script and part-slicing strategy.
2. **Base Skeleton + Swappable Costume "Skin" (Video Game Pattern)**: Most biped characters share the same underlying 18-bone body structure and motion library (`walk`, `run`, `jump`, `sit`, `wave`, `talk`). By separating the **Base Mannequin & Skeleton** from the **Costume/Color Skin Layer**, new characters reuse proven animations immediately.
3. **Every Decision is a Function**: Topology decisions (*"Does this need another bend, edge loop, or vertex?"*, *"How does this 3D object move?"*) become small, composable TypeScript and Python functions rather than inline UI callbacks.
4. **Agent-Drivable Browser + Blender 5.3 + FilmCraft CLI Tooling**: A unified CLI (`studio_pipeline_cli.py`) can launch the web studio visibly on the desktop, drive imports and exports over CDP, sync with live **Blender 5.3** (`127.0.0.1:9876`) for a visual animation check and flattened/orthographic render, and load the rendered animation directly into **FilmCraft (`filmcraft-cli`)** and **EffectCraft (`effectcraft-cli`)**.

---

## 2. Goals, Non-Goals & Anti-Goals

### Goals
- **G1 — Refactor Monolithic HTML into Modular TypeScript (`src/`) + Python (`pipeline/`)**:
  - Extract pure, single-decision functions in TypeScript and Python while compiling/serving a clean, fast web studio (`index.html` / `3d_character_turnaround_blender_studio (1).html` compatibility preserved).
- **G2 — 3-Question AI Intake & Script Router**:
  - Before generating geometry for a new image upload, run a 3-question intake (interactive in UI, or automated via TypeSafe `jev-latest` / CLI flags) that determines:
    1. **Archetype & Skeleton Mechanics** (`humanoid_biped`, `axial_fin_creature`, `radial_celestial_prop`, `buoyant_vehicle`).
    2. **Part Slicing & Skin Strategy** (which regions slice into moving sub-parts vs. wrap onto the base skeleton as a painted/costume skin, and where joint deformation edge loops are inserted).
    3. **Universal Action Verbs & Camera Output** (`move_traverse`, `jump_rise_fall`, `turn_bank`, `express_talk` across chosen camera angles or flattened 2D orthographic view).
- **G3 — Universal Action Verb Mapping Across Different Skeletons**:
  - Map the same high-level timeline actions to archetype-specific mechanics:
    - `jump_rise_fall`: Crouch + knee/ankle push on a **Humanoid**; sunrise/sunset vertical arc + ray pulse on a **Smiling Sun**; wave cresting on a **Boat**; breach leap on a **Fish**.
    - `move_traverse`: Walk/run stride on a **Humanoid**; S-wave spine + pectoral/caudal fin flutter on a **Fish**; pitch/roll glide on a **Boat**.
    - `turn_bank`: Multi-angle turnaround swap (`0°`, `40°`, `90°`, `180°`) or 3D banking yaw.
- **G4 — Live Blender 5.3 Visual Check & Flattened 2D/3D Render Bridge**:
  - Export `.glb` + modular Blender 5.3 Python rig script, verify 100% numeric topology parity (`verts`, `edges`, `loops = 3 * tris`, `bones`), preview the animation in Blender 5.3, and render an orthographic/flattened 2D animation clip (`.mp4` / `.png` sequence) ready for video editing.
- **G5 — Direct FilmCraft (`filmcraft-cli`) & EffectCraft (`effectcraft-cli`) Loading**:
  - Automatically import the rendered animation into a **FilmCraft project (`.fcproj`)** via `/home/jesse/.local/bin/filmcraft-cli`, render verification frames/video, and generate an **EffectCraft (`.ecproj` / `.jsx`)** and **Remotion (`.tsx`)** component.
- **G6 — "Trigger Omarchy Agent" UI Button + Zero-Noise Step-by-Step Skill**:
  - Add a prominent **`🤖 Ask Omarchy Agent (3-Question Intake)`** button in the HTML Studio header/intake modal that connects to a local HTTP/WebSocket bridge (`studio_pipeline_cli.py serve`) or launches an Omarchy terminal/agent session to prompt for the image and walk through the 3 intake questions whenever an agent isn't already driving the CLI.
  - Create a concise, zero-noise skill at `/home/jesse/Work/omhas-animation/.agents/skills/image-to-animation/SKILL.md` containing strictly step-by-step commands, and verify it by spawning a brand-new subagent to execute the workflow end-to-end.

### Non-Goals
- Full photorealistic cloth physics simulation in the browser (Blender 5.3 handles heavy offline shader/cloth refinement when needed).
- Replacing Blender 5.3's native sculpting tools.

### Anti-Goals (What Would Make This a Failure Even If It Technically Works)
- **Silent Biped Assumption**: Treating a fish, sun, or boat as a 2-legged bunny with ears.
- **Monolithic Spaghetti**: Keeping all geometry math, UI DOM listeners, and export strings trapped inside one giant unindexed `<script>` tag where individual decisions cannot be called from Python or TypeScript.
- **Headless-Only Opacity**: Running everything invisibly without giving the user (and agent) a single command to pop open the live browser studio, inspect the Blender 5.3 viewport, and see FilmCraft load the exported clip.
- **Bloated Skill Documentation**: Any fluff or narrative filler in `SKILL.md` that confuses a fresh agent instead of giving exact step-by-step commands.

---

## 3. Modular Architecture & File Layout

All new code lives inside `/home/jesse/Work/omhas-animation/`:

```text
/home/jesse/Work/omhas-animation/
├── .agents/skills/image-to-animation/
│   └── SKILL.md                          # Zero-noise, step-by-step agent skill for the full Track B workflow
├── src/                                  # Modular TypeScript source (bundled to dist/studio.bundle.js)
│   ├── types/
│   │   └── schema.ts                     # Shared types: ArchetypeId, SkeletonGraph, EdgeLoopSpec, SkinLayer, UniversalVerb
│   ├── intake/
│   │   └── aiIntakeRouter.ts             # 3-Question AI Intake + triggerOmarchyAgentIntake() bridge
│   ├── topology/
│   │   ├── sliceParts.ts                 # sliceImageIntoParts(), borderFloodFillAlpha(), extractSubPartIslands()
│   │   ├── decideTopology.ts             # decideVertexDensity(), insertJointDeformationLoops(), computeTopologyParity()
│   │   └── skinning.ts                   # bindLinearBlendSkinning(), applyCostumeSkinLayer(), stripOutfitToBaseMannequin()
│   ├── archetypes/
│   │   ├── humanoidBiped.ts              # 18-bone shared biped template (Hopper, Jenny, Puddles, Whiskers, Scout, Old Mac)
│   │   ├── axialCreature.ts              # Fish / Swimming creature (axial spine chain + oscillating pectoral/tail fins)
│   │   ├── radialProp.ts                 # Smiling Sun / Celestial prop (core squash/stretch + radial rays + face visemes)
│   │   └── vehicleHull.ts                # Boat / Vessel (hull pitch/roll buoyancy + banking yaw turn)
│   ├── animation/
│   │   └── universalVerbs.ts             # evaluateUniversalVerb(archetype, verb, phase, energy)
│   ├── exporters/
│   │   ├── glbExporter.ts                # Clean glTF 2.0 / .GLB binary builder
│   │   ├── blender53ScriptExporter.ts    # Generates modular Blender 5.3 Python script (Armature + Ortho Flatten Cam + MP4 render)
│   │   └── videoCodeExporter.ts          # Exports EffectCraft (.jsx), Remotion (.tsx), and FilmCraft (.jsonl) scripts
│   └── index.ts                          # Attaches modular API (`window.OMHASStudio`) to the HTML Studio UI
├── pipeline/                             # Modular Python CLI & MCP/CDP Bridge
│   ├── __init__.py
│   ├── intake_router.py                  # Python 3-Question AI Intake (TypeSafe jev-latest + photocraft-cli)
│   ├── topology_parity.py                # Direct .GLB vs. Blender 5.3 BMesh vertex/edge/loop/bone comparator
│   ├── blender53_bridge.py               # Live Blender 5.3 TCP/MCP client (port 9876) + headless Blender fallback
│   ├── filmcraft_bridge.py               # Drives `filmcraft-cli` (.fcproj creation, media import, timeline render, MP4 export)
│   └── browser_cdp_bridge.py             # Opens visible browser window & drives `window.OMHASStudio` via CDP
├── studio_pipeline_cli.py                # Unified CLI entrypoint (`open`, `serve`, `intake`, `rig`, `blender-check`, `filmcraft-load`, `run-e2e`)
└── 3d_character_turnaround_blender_studio (1).html  # Loads `dist/studio.bundle.js` + exposes the 3-Question AI Intake Wizard & Omarchy Agent Trigger Button
```

---

## 4. Detailed Component Design

### 4.1 The 3-Question AI Intake & Script Router (`aiIntakeRouter.ts` & `intake_router.py`)
When a user uploads an image (or runs `studio_pipeline_cli.py intake <image>`):
1. **Pre-Analysis (`photocraft-cli` + Canvas Silhouette Metrics)**:
   - Measures aspect ratio, symmetry, border-flood-fill alpha silhouette, convexity defects (appendages/fins/rays), and color palette.
2. **The 3 Intake Questions** (pre-filled with AI recommendations via TypeSafe `jev-latest` `choice` primitive, editable by the user in 1 click):
   - **Question 1: Archetype & Motion Mechanics**
     - Options:
       1. `humanoid_biped` — Walk, Run, Jump, Sit, Wave, Talk (18-bone shared rig + removable costume skin)
       2. `axial_creature` — Swim/Glide Left↔Right, Fin Flutter, Breach Jump (Spine wave + sliced fin pivots, e.g. **Fish**)
       3. `radial_prop` — Rise & Fall (Jump), Pulse/Rotate Rays, Smile/Talk (Core + radial ring + face morphs, e.g. **Smiling Sun**)
       4. `vehicle_hull` — Move Left↔Right, Bank & Turn Yaw, Wave Bob (Rigid hull + pitch/roll/yaw pivot, e.g. **Boat**)
   - **Question 2: Part Slicing & Topology Bend Loops**
     - Options:
       1. `base_mannequin_plus_costume_skin` — Use shared base skeleton; apply image/outfit as a swappable skin layer
       2. `auto_slice_appendages` — Slice silhouette into core body + hinged appendages (e.g. Fish Body + Pectoral Fins + Tail Fin) and insert 3 deformation edge loops at each hinge
       3. `single_deformable_sheet` — Single 2.5D inflated mesh with bounded biharmonic / axial bend loops
   - **Question 3: Target Action & Video Pipeline Output**
     - Options:
       1. `track_b_blender_to_filmcraft` *(Default)* — Preview in Web Studio → Sync & flatten/render in Blender 5.3 → Load clip into **FilmCraft (`.fcproj`)**
       2. `effectcraft_and_remotion` — Export layered `.jsx` for **EffectCraft** + `.tsx` for **Remotion / Hyperframes**
       3. `all_targets` — Run Blender 5.3 render + FilmCraft `.fcproj` + EffectCraft `.jsx` + Remotion `.tsx`

### 4.2 Pure Topology & Rig Decision Functions (`src/topology/`)
Each geometric decision is isolated into its own pure function:
- `doesRegionNeedBendLoop(boneChain, vertexRing)` → returns boolean + recommended ring count (`1` for rigid segment, `3` for hinge joint like knee/elbow/fin base, `5` for continuous S-wave spine).
- `insertJointDeformationLoops(geometry, jointPos, jointAxis, ringCount=3)` → inserts concentric vertex rings around the joint plane so bending preserves volume without cylinder gaps.
- `stripCostumeToBaseMannequin(characterRig)` / `applyCostumeSkin(characterRig, skinSpec)` → toggles or swaps the costume shell and projected texture skin independently of the underlying 18-bone mannequin.
- `computeTopologyParity(rootObject)` → returns `{ meshCount, boneCount, bufferVerts, weldedVerts, edges, boundaryEdges, tris, loops }` where `loops === tris * 3`, matching Blender 5.3's `BMesh` 100%.

### 4.3 Live Blender 5.3 Visual Check & Orthographic Flatten Renderer (`pipeline/blender53_bridge.py`)
- Connects to the open **Blender 5.3** session (`127.0.0.1:9876`) or runs `blender --background` if requested.
- Imports the `.glb` and executes the archetype-specific rig & animation generator (`build_<archetype>_blender.py`):
  - Creates the armature bones (`18` bones for bipeds, `7` bones for fish spine+fins, `9` bones for sun core+rays, `4` bones for boat hull+wake).
  - Keyframes the chosen **Universal Action Verb** (`jump_rise_fall`, `move_traverse`, `turn_bank`, `express_talk`) across frames `1..48` at `24 fps`.
  - Sets up an **Orthographic 2D/2.5D Flatten Camera** (with transparent film `film_transparent = True`) so Blender 5.3 can render a crisp 2D-style animated `.mp4` / `.png` sequence from any angle (`0°`, `40°`, `90°`, `180°`).

### 4.4 FilmCraft (`filmcraft-cli`) & EffectCraft (`effectcraft-cli`) Integration (`pipeline/filmcraft_bridge.py`)
- Takes the rendered animation clip from Blender 5.3 (`exports/<asset>_<verb>_blender53.mp4`) and executes `/home/jesse/.local/bin/filmcraft-cli`:
  1. Creates/updates `exports/<asset>_filmcraft.fcproj` and imports the Blender-rendered animation clip onto the active sequence timeline.
  2. Runs `filmcraft-cli --project exports/<asset>_filmcraft.fcproj inspect` to verify timeline track placement.
  3. Runs `filmcraft-cli --project exports/<asset>_filmcraft.fcproj render --seconds 0.5 --out exports/<asset>_filmcraft_program_frame.png` and `export exports/<asset>_filmcraft_final.mp4 --format h264` to prove the end-to-end video pipeline works.
  4. Optionally launches the FilmCraft or Web Studio UI on the user's desktop so the user can inspect both live.

---

## 5. Decisions Left to the Builder (Settled Defaults)
- **TypeScript Bundling**: Use `bun build src/index.ts --outfile dist/studio.bundle.js --target browser` (since `bun` is installed in `/home/jesse/.local/share/mise/installs/bun/latest/bin/bun`) so build times are `< 50ms` with zero heavy webpack config.
- **Browser Launch Mode**: `studio_pipeline_cli.py open` launches Chromium in visible desktop mode (`--remote-debugging-port=9225` without `--headless`) when `DISPLAY` / `WAYLAND_DISPLAY` is active, and falls back to `--headless` only for automated background tests.
