# Modular Image-to-Animation Studio, Blender 5.3 Bridge & FilmCraft Pipeline Implementation Plan

> **Historical design reference — not current operating instructions.**
> The current Magic Studio application is the Tauri shell; use [README](../../../README.md)
> and `mc --help` for its canonical CLI. `studio_pipeline_cli.py` remains an internal engine adapter.
> Its older unified-CLI/browser descriptions, shared-checkout steps, ports, tool versions,
> completion claims and proposed features below are historical, not instructions.


> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the monolithic HTML turnaround studio into composable TypeScript (`src/`) and Python (`pipeline/`) functions with a 3-Question AI Intake + "Ask Omarchy Agent" UI trigger, multi-archetype rigging (`humanoid_biped`, `axial_creature`, `radial_prop`, `vehicle_hull`), live Blender 5.3 visual check & orthographic render, `filmcraft-cli` / `effectcraft-cli` video loading, and a zero-noise step-by-step agent skill verified by a fresh subagent.

**Architecture:** Pure TypeScript modules in `src/` (bundled via `bun build` into `dist/studio.bundle.js` and attached to `window.OMHASStudio`) implement every geometry, topology loop, skin/costume separation, and universal verb decision as a standalone function. A companion Python package (`pipeline/` + `studio_pipeline_cli.py`) provides the 3-Question AI Intake router (TypeSafe `jev-latest` + `photocraft-cli`), live desktop browser CDP controller, Blender 5.3 TCP/MCP bridge (`127.0.0.1:9876`), and `filmcraft-cli` / `effectcraft-cli` timeline builder.

**Tech Stack:** TypeScript (`bun build` + `bun test`), Three.js r128, Python 3.12 (`pytest` / `unittest`), Blender 5.3 Python API (`bpy`, `bmesh`), `/home/jesse/.local/bin/filmcraft-cli`, `/home/jesse/.local/bin/effectcraft-cli`, `/home/jesse/.local/bin/photocraft-cli`, TypeSafe System One API (`jev-latest`).

**Spec:** [2026-10-18-modular-2d-3d-blender-filmcraft-pipeline-design.md](file:///home/jesse/Work/omhas-animation/docs/superpowers/specs/2026-10-18-modular-2d-3d-blender-filmcraft-pipeline-design.md)

## Global Constraints

- Do not modify `/home/jesse/.gemini/config/mcp_config.json`.
- Keep all project code inside `/home/jesse/Work/omhas-animation/`.
- Every geometry, topology, skeleton, and animation decision must be an individually callable function in TypeScript (`src/`) and Python (`pipeline/`).
- Preserve compatibility with [3d_character_turnaround_blender_studio (1).html](file:///home/jesse/Work/omhas-animation/3d_character_turnaround_blender_studio%20(1).html) while loading `dist/studio.bundle.js` and adding the **`🤖 Ask Omarchy Agent`** trigger button + **3-Question AI Intake Modal**.
- The skill file `/home/jesse/Work/omhas-animation/.agents/skills/image-to-animation/SKILL.md` must contain strictly step-by-step commands with zero filler text.

## Review Focus

1. **Non-biped image upload (e.g., Fish or Smiling Sun)**: Must route to `axial_creature` or `radial_prop` via `runThreeQuestionIntake()` and generate the matching spine/fin or core/ray bone hierarchy rather than a 2-legged bunny.
2. **Interior light-colored pixels during alpha cutout**: `borderFloodFillAlpha()` must seed strictly from the image perimeter so cream/white/yellow interiors (Hopper's face, Sun's core) retain `alpha = 255`.
3. **Joint deformation loop insertion on zero-length or degenerate segments**: `insertJointDeformationLoops()` must clamp `ringCount` to `[1, 7]` and return valid `loops === tris * 3` parity without NaN vertices.
4. **Offline or unstarted agent when user clicks `🤖 Ask Omarchy Agent`**: Must gracefully fall back from the local HTTP bridge (`127.0.0.1:8766`) to opening the interactive 3-Question Intake Wizard in-page AND copying the exact `studio_pipeline_cli.py intake` command for Omarchy terminal execution.
5. **Blender 5.3 `CHILD_OF` constraint double-inheritance**: `generateBlender53RigAndRenderScript()` must unparent constrained pivot nodes to world space (`mw = obj.matrix_world.copy(); obj.parent = None; obj.matrix_world = mw`) and invert `arm_obj.matrix_world @ pose_bone.matrix` so rest poses never shift.

---

### Task 1: Modular TypeScript Core — Schema, 3-Question Intake, Topology Decisions, 4 Archetypes & Universal Verbs

**Files:**
- Create: `src/types/schema.ts`
- Create: `src/intake/aiIntakeRouter.ts`
- Create: `src/topology/decideTopology.ts`
- Create: `src/topology/skinning.ts`
- Create: `src/archetypes/humanoidBiped.ts`
- Create: `src/archetypes/axialCreature.ts`
- Create: `src/archetypes/radialProp.ts`
- Create: `src/archetypes/vehicleHull.ts`
- Create: `src/animation/universalVerbs.ts`
- Create: `src/exporters/blender53ScriptExporter.ts`
- Create: `src/exporters/videoCodeExporter.ts`
- Create: `src/index.ts`
- Test: `tests/studio_core.test.ts`

**Interfaces:**
- Consumes: Standard TypeScript types / Three.js scene graph primitives
- Produces:
  - `ArchetypeId = 'humanoid_biped' | 'axial_creature' | 'radial_prop' | 'vehicle_hull'`
  - `UniversalVerb = 'move_traverse' | 'jump_rise_fall' | 'turn_bank' | 'express_talk'`
  - `SlicingStrategy = 'base_mannequin_plus_costume_skin' | 'auto_slice_appendages' | 'single_deformable_sheet'`
  - `IntakeAnswers { archetype: ArchetypeId; slicingStrategy: SlicingStrategy; primaryVerb: UniversalVerb; cameraAngles: number[]; stripCostumeToBase: boolean }`
  - `buildIntakeQuestions(hintName?: string, aspect?: number): { questions: IntakeQuestion[]; recommended: IntakeAnswers }`
  - `doesRegionNeedBendLoop(jointType: 'rigid' | 'hinge' | 'spline', maxBendDeg: number): { needsLoop: boolean; ringCount: number }`
  - `insertJointDeformationLoops(vertices: Float32Array, indices: Uint32Array, jointY: number, radius: number, ringCount?: number): { vertices: Float32Array; indices: Uint32Array; addedVerts: number; addedLoops: number }`
  - `computeMeshTopologyMetrics(vertices: Float32Array, indices: Uint32Array): { bufferVerts: number; weldedVerts: number; edges: number; boundaryEdges: number; tris: number; loops: number }`
  - `buildArchetypeSkeleton(archetype: ArchetypeId): SkeletonGraph`
  - `evaluateUniversalVerb(archetype: ArchetypeId, verb: UniversalVerb, phase: number, energy?: number): Record<string, { pos: [number,number,number]; rot: [number,number,number]; scale: [number,number,number] }>`
  - `generateBlender53ArchetypeScript(archetype: ArchetypeId, verb: UniversalVerb, renderOutPath: string): string`

- [ ] **Step 1: Write the failing TypeScript unit test suite in `tests/studio_core.test.ts`**

```typescript
import { describe, expect, test } from "bun:test";
import { buildIntakeQuestions } from "../src/intake/aiIntakeRouter";
import { doesRegionNeedBendLoop, insertJointDeformationLoops, computeMeshTopologyMetrics } from "../src/topology/decideTopology";
import { buildArchetypeSkeleton } from "../src/archetypes/index";
import { evaluateUniversalVerb } from "../src/animation/universalVerbs";
import { generateBlender53ArchetypeScript } from "../src/exporters/blender53ScriptExporter";

describe("Modular OMHAS Studio TypeScript Core", () => {
  test("buildIntakeQuestions routes fish, sun, boat, and humanoid without assuming biped", () => {
    expect(buildIntakeQuestions("fish_fin.png").recommended.archetype).toBe("axial_creature");
    expect(buildIntakeQuestions("smiling_sun.png").recommended.archetype).toBe("radial_prop");
    expect(buildIntakeQuestions("wooden_boat.png").recommended.archetype).toBe("vehicle_hull");
    expect(buildIntakeQuestions("Jenny-no-background.png").recommended.archetype).toBe("humanoid_biped");
    expect(buildIntakeQuestions("fish_fin.png").questions.length).toBe(3);
  });

  test("doesRegionNeedBendLoop and insertJointDeformationLoops maintain Loops === 3 * Tris parity", () => {
    expect(doesRegionNeedBendLoop("hinge", 45)).toEqual({ needsLoop: true, ringCount: 3 });
    expect(doesRegionNeedBendLoop("spline", 60)).toEqual({ needsLoop: true, ringCount: 5 });
    expect(doesRegionNeedBendLoop("rigid", 0)).toEqual({ needsLoop: false, ringCount: 1 });

    const baseVerts = new Float32Array([0,0,0, 1,0,0, 0,1,0, 1,1,0]);
    const baseIdx = new Uint32Array([0,1,2, 1,3,2]);
    const res = insertJointDeformationLoops(baseVerts, baseIdx, 0.5, 0.2, 3);
    const metrics = computeMeshTopologyMetrics(res.vertices, res.indices);
    expect(metrics.loops).toBe(metrics.tris * 3);
    expect(res.addedVerts).toBeGreaterThan(0);
  });

  test("buildArchetypeSkeleton and evaluateUniversalVerb support all 4 archetypes", () => {
    const fishSkel = buildArchetypeSkeleton("axial_creature");
    expect(fishSkel.bones.some(b => b.name.includes("DEF_Fin"))).toBe(true);
    const sunPose = evaluateUniversalVerb("radial_prop", "jump_rise_fall", 0.5, 1.0);
    expect(sunPose["DEF_SunCore"].pos[1]).toBeGreaterThan(0);
    const py = generateBlender53ArchetypeScript("axial_creature", "move_traverse", "/tmp/fish.mp4");
    expect(py).toContain("con.inverse_matrix = bone_world_mat.inverted()");
  });
});
```

- [ ] **Step 2: Run `bun test tests/studio_core.test.ts` to verify it fails**

Run: `bun test tests/studio_core.test.ts`
Expected: FAIL (`Cannot find module '../src/intake/aiIntakeRouter'`)

- [ ] **Step 3: Implement the modular TypeScript files (`src/types/schema.ts`, `src/intake/aiIntakeRouter.ts`, `src/topology/decideTopology.ts`, `src/topology/skinning.ts`, `src/archetypes/*`, `src/animation/universalVerbs.ts`, `src/exporters/*`, `src/index.ts`) and build `dist/studio.bundle.js`**

Run: `bun build src/index.ts --outfile dist/studio.bundle.js --target browser`

- [ ] **Step 4: Run `bun test tests/studio_core.test.ts` to verify all TypeScript tests pass**

Run: `bun test tests/studio_core.test.ts`
Expected: PASS (all assertions green)

---

### Task 2: Modular Python Pipeline Package (`pipeline/`) & Unified CLI (`studio_pipeline_cli.py`)

**Files:**
- Create: `pipeline/__init__.py`
- Create: `pipeline/intake_router.py`
- Create: `pipeline/topology_parity.py`
- Create: `pipeline/blender53_bridge.py`
- Create: `pipeline/filmcraft_bridge.py`
- Create: `pipeline/browser_cdp_bridge.py`
- Create: `studio_pipeline_cli.py`
- Test: `tests/test_pipeline_cli.py`

**Interfaces:**
- Consumes: `/home/jesse/.local/bin/photocraft-cli`, `/home/jesse/.local/bin/filmcraft-cli`, `/home/jesse/.local/bin/effectcraft-cli`, Live Blender 5.3 MCP port `127.0.0.1:9876`
- Produces:
  - `pipeline.intake_router.run_three_question_intake(image_path: str, answers_override: dict | None = None) -> dict`
  - `pipeline.topology_parity.parse_glb_topology(glb_path: str) -> dict`
  - `pipeline.blender53_bridge.execute_archetype_in_blender53(archetype: str, verb: str, glb_path: str | None, out_mp4: str, out_preview_png: str) -> dict`
  - `pipeline.filmcraft_bridge.load_animation_into_filmcraft(media_path: str, project_out: str, frame_png_out: str, final_mp4_out: str | None = None) -> dict`
  - CLI subcommands in `studio_pipeline_cli.py`: `intake`, `open-browser`, `blender-check`, `filmcraft-load`, `run-e2e`, `serve-agent-bridge`

- [ ] **Step 1: Write the failing Python test suite in `tests/test_pipeline_cli.py`**

```python
import os
import unittest
from pipeline.intake_router import run_three_question_intake
from pipeline.topology_parity import parse_glb_topology

class TestPipelineCLI(unittest.TestCase):
    def test_three_question_intake_archetype_detection(self):
        fish_res = run_three_question_intake("fish_with_fins.png")
        self.assertEqual(fish_res["recommended"]["archetype"], "axial_creature")
        self.assertEqual(len(fish_res["questions"]), 3)

        sun_res = run_three_question_intake("smiling_sun.png")
        self.assertEqual(sun_res["recommended"]["archetype"], "radial_prop")
        self.assertEqual(sun_res["recommended"]["primaryVerb"], "jump_rise_fall")

    def test_glb_topology_parser_matches_loops_formula(self):
        glb_path = "/home/jesse/Work/omhas-animation/exports/felt_bunny_explorer_blender.glb"
        if os.path.exists(glb_path):
            topo = parse_glb_topology(glb_path)
            self.assertEqual(topo["expected_loops"], topo["total_tris"] * 3)
            self.assertGreater(topo["mesh_count"], 0)
```

- [ ] **Step 2: Run `python3 -m unittest tests/test_pipeline_cli.py` to verify it fails**

Run: `python3 -m unittest tests/test_pipeline_cli.py`
Expected: FAIL (`ModuleNotFoundError: No module named 'pipeline'`)

- [ ] **Step 3: Implement `pipeline/*` and `studio_pipeline_cli.py`**

Implement `intake_router.py`, `topology_parity.py`, `blender53_bridge.py` (generating procedural 3D meshes + rigs in Blender 5.3 for `humanoid_biped`, `axial_creature` Fish, `radial_prop` Smiling Sun, and `vehicle_hull` Boat when no `.glb` exists yet, animating frames `1..36`, and rendering both an orthographic/flattened `.png` sequence / `.mp4` and viewport preview), `filmcraft_bridge.py` (driving `/home/jesse/.local/bin/filmcraft-cli`), and `browser_cdp_bridge.py`.

- [ ] **Step 4: Run `python3 -m unittest tests/test_pipeline_cli.py` to verify it passes**

Run: `python3 -m unittest tests/test_pipeline_cli.py`
Expected: PASS

---

### Task 3: Wire HTML Studio UI to `dist/studio.bundle.js`, Add `🤖 Ask Omarchy Agent` Button & 3-Question Intake Modal

**Files:**
- Modify: `3d_character_turnaround_blender_studio (1).html`

**Interfaces:**
- Consumes: `dist/studio.bundle.js` (`window.OMHASStudio`) and `http://127.0.0.1:8766/trigger-agent-intake`
- Produces:
  - Header button `#btnAskOmarchyAgent` (`🤖 Ask Omarchy Agent`)
  - Interactive 3-Question Intake Wizard Modal (`#ai3QuestionIntakeModal`) with live Archetype switcher (`Humanoid Biped`, `Fish / Axial Fins`, `Smiling Sun / Radial`, `Boat / Vehicle Hull`), Costume Skin Strip toggle (`Strip Outfit to Base Mannequin`), Universal Action Verb trigger (`Move`, `Jump / Rise & Fall`, `Turn / Bank`, `Express / Talk`), and `Export → Blender 5.3 → FilmCraft` button.

- [ ] **Step 1: Add `<script src="dist/studio.bundle.js"></script>`, the `🤖 Ask Omarchy Agent` button, and the 3-Question Intake Modal to `3d_character_turnaround_blender_studio (1).html`**
- [ ] **Step 2: Verify via headless/visible Chromium CDP that `window.OMHASStudio` is loaded and `openOmarchyAgentIntakeModal()` renders the 3 questions cleanly**

Run: `python3 studio_pipeline_cli.py intake "/home/jesse/main-vault/Creative/Design/Characters/Jenny-no-background.png_202607180130.jpeg" --json`
Expected: JSON output with `questions` (length 3) and `recommended` answers.

---

### Task 4: Create Zero-Noise Step-by-Step Agent Skill (`.agents/skills/image-to-animation/SKILL.md`) & Test with a Fresh Subagent

**Files:**
- Create: `/home/jesse/Work/omhas-animation/.agents/skills/image-to-animation/SKILL.md`

**Interfaces:**
- Consumes: `studio_pipeline_cli.py`
- Produces: Step-by-step skill instructions for any fresh agent to run the Track B workflow (`intake` → `open-browser` → `blender-check` → `filmcraft-load` / `run-e2e`).

- [ ] **Step 1: Write `/home/jesse/Work/omhas-animation/.agents/skills/image-to-animation/SKILL.md` with strictly numbered step-by-step commands and zero filler**
- [ ] **Step 2: Dispatch a fresh subagent using `invoke_subagent` (`self`) instructed only to read `.agents/skills/image-to-animation/SKILL.md` and run an end-to-end Track B animation export (testing both a non-humanoid object like a `fish` or `smiling_sun` AND a cast character like `Jenny`) into Blender 5.3 and FilmCraft (`filmcraft-cli`)**
- [ ] **Step 3: Inspect the subagent's execution transcript and output files (`.fcproj`, Blender 5.3 render, FilmCraft program frame PNG, and browser screenshot) and iterate if any step required guessing**
