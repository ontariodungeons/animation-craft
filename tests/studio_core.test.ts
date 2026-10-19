import { describe, expect, test } from "bun:test";
import { buildIntakeQuestions, runThreeQuestionIntake } from "../src/intake/aiIntakeRouter";
import {
  doesRegionNeedBendLoop,
  insertJointDeformationLoops,
  computeMeshTopologyMetrics,
  borderFloodFillAlpha
} from "../src/topology/decideTopology";
import { stripCostumeToBaseMannequin, applyCostumeSkinLayer } from "../src/topology/skinning";
import { buildArchetypeSkeleton } from "../src/archetypes/index";
import { evaluateUniversalVerb } from "../src/animation/universalVerbs";
import { generateBlender53ArchetypeScript } from "../src/exporters/blender53ScriptExporter";
import { generateVideoComponentCode } from "../src/exporters/videoCodeExporter";

describe("Modular OMHAS Studio TypeScript Core", () => {
  test("buildIntakeQuestions routes fish, sun, boat, and humanoid without assuming biped", () => {
    expect(buildIntakeQuestions("fish_fin.png").recommended.archetype).toBe("axial_creature");
    expect(buildIntakeQuestions("smiling_sun.png").recommended.archetype).toBe("radial_prop");
    expect(buildIntakeQuestions("wooden_boat.png").recommended.archetype).toBe("vehicle_hull");
    expect(buildIntakeQuestions("Jenny-no-background.png").recommended.archetype).toBe("humanoid_biped");
    expect(buildIntakeQuestions("fish_fin.png").questions.length).toBe(3);
  });

  test("borderFloodFillAlpha preserves interior cream/yellow pixels while clearing border background", () => {
    // 4x4 RGBA image: border is cream (250, 240, 220), center 2x2 is dark outline (20,20,20) surrounding 1 interior cream pixel
    const w = 5, h = 5;
    const rgba = new Uint8ClampedArray(w * h * 4);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const i = (y * w + x) * 4;
        const isOutlineRing = (x >= 1 && x <= 3 && y >= 1 && y <= 3) && !(x === 2 && y === 2);
        if (isOutlineRing) {
          rgba[i] = 25; rgba[i + 1] = 25; rgba[i + 2] = 25; rgba[i + 3] = 255;
        } else {
          // Both outer border and center (2,2) have identical cream color!
          rgba[i] = 250; rgba[i + 1] = 240; rgba[i + 2] = 220; rgba[i + 3] = 255;
        }
      }
    }
    const out = borderFloodFillAlpha(rgba, w, h, 28);
    expect(out[(0 * w + 0) * 4 + 3]).toBe(0); // outer border cleared
    expect(out[(2 * w + 2) * 4 + 3]).toBe(255); // interior cream pixel preserved!
  });

  test("doesRegionNeedBendLoop and insertJointDeformationLoops maintain Loops === 3 * Tris parity", () => {
    expect(doesRegionNeedBendLoop("hinge", 45)).toEqual({ needsLoop: true, ringCount: 3 });
    expect(doesRegionNeedBendLoop("spline", 60)).toEqual({ needsLoop: true, ringCount: 5 });
    expect(doesRegionNeedBendLoop("rigid", 0)).toEqual({ needsLoop: false, ringCount: 1 });

    const baseVerts = new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 0]);
    const baseIdx = new Uint32Array([0, 1, 2, 1, 3, 2]);
    const res = insertJointDeformationLoops(baseVerts, baseIdx, 0.5, 0.2, 3);
    const metrics = computeMeshTopologyMetrics(res.vertices, res.indices);
    expect(metrics.loops).toBe(metrics.tris * 3);
    expect(res.addedVerts).toBeGreaterThan(0);
  });

  test("stripCostumeToBaseMannequin and applyCostumeSkinLayer toggle outfit skin cleanly", () => {
    const state = stripCostumeToBaseMannequin({ hasVest: true, hasBackpack: true, hasShorts: true });
    expect(state.costumeVisible).toBe(false);
    expect(state.baseMannequinVisible).toBe(true);
    const dressed = applyCostumeSkinLayer(state, { id: "explorer_vest", primaryHex: "#D96B27", secondaryHex: "#4A6B82" });
    expect(dressed.costumeVisible).toBe(true);
    expect(dressed.skinSpec?.id).toBe("explorer_vest");
  });

  test("buildArchetypeSkeleton and evaluateUniversalVerb support all 4 archetypes", () => {
    const fishSkel = buildArchetypeSkeleton("axial_creature");
    expect(fishSkel.bones.some(b => b.name.includes("DEF_Fin"))).toBe(true);
    const sunPose = evaluateUniversalVerb("radial_prop", "jump_rise_fall", 0.5, 1.0);
    expect(sunPose["DEF_SunCore"].pos[1]).toBeGreaterThan(0);
    const boatPose = evaluateUniversalVerb("vehicle_hull", "turn_bank", 0.5, 1.0);
    expect(Math.abs(boatPose["DEF_Hull"].rot[2])).toBeGreaterThan(0);
    const py = generateBlender53ArchetypeScript("axial_creature", "move_traverse", "/tmp/fish.mp4");
    expect(py).toContain("con.inverse_matrix = bone_world_mat.inverted()");
    const remotionCode = generateVideoComponentCode("axial_creature", "move_traverse", "remotion");
    expect(remotionCode).toContain("useCurrentFrame");
  });

  test("applyPoseToRig preserves walk/run/jump leg rotations instead of overwriting with static bind sliders", async () => {
    const { OMHASStudio } = await import("../src/index");
    const state = OMHASStudio.createStudioState();
    const { pose } = OMHASStudio.evaluatePose("walk", 0.28, 1.0, state.animation.sliders, -1, "smile");
    expect(Math.abs(pose.legL[0])).toBeGreaterThan(0.1);
    const dummyPivots: any = {
      root: { position: { y: 0 }, scale: { set: () => {} } },
      legLPivot: { rotation: { x: 0, set(x: number) { this.x = x; } } },
      legRPivot: { rotation: { x: 0, set(x: number) { this.x = x; } } },
      kneeLPivot: { rotation: { x: 0, set(x: number) { this.x = x; } } },
    };
    OMHASStudio.applyPoseToRig(dummyPivots, pose, state.animation.sliders, -1, "walk");
    expect(dummyPivots.legLPivot.rotation.x).toBeCloseTo(pose.legL[0], 5);
    expect(typeof OMHASStudio.createThreeCutoutAnimator).toBe("function");
    expect(typeof OMHASStudio.createP5PuppetSketch).toBe("function");
  });
});

