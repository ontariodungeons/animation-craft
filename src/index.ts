// src/index.ts
// Modular entry point for the OMHAS animation studio.
// Wires the 6 domain state objects, extracted modular functions, archetypes,
// exporters, and the standalone Three.js / p5.js puppet animator into
// window.OMHASStudio, then bundles to public/studio-bundle.js via bun.

import {
  buildIntakeQuestions,
  classifyArchetypeFromHint,
  runThreeQuestionIntake,
} from "./intake/aiIntakeRouter";
import {
  borderFloodFillAlpha,
  computeMeshTopologyMetrics,
  doesRegionNeedBendLoop,
  insertJointDeformationLoops,
} from "./topology/decideTopology";
import { applyCostumeSkinLayer, stripCostumeToBaseMannequin } from "./topology/skinning";
import { buildArchetypeSkeleton } from "./archetypes/index";
import { evaluateUniversalVerb } from "./animation/universalVerbs";
import { generateBlender53ArchetypeScript } from "./exporters/blender53ScriptExporter";
import { generateVideoComponentCode } from "./exporters/videoCodeExporter";
import type { SkeletonArchetype, UniversalActionVerb } from "./types/schema";

// --- Domain state objects (extracted from monolithic HTML) ---
import {
  createInitialRigState,
  type RigState,
  type RigPivots,
  type RigPart,
  setPartVisibility,
  togglePartVisibility,
  soloPart,
  showAllParts,
  togglePartVerified,
  verifyAllParts,
  setRigExplode,
  setSpecies,
  toggleCategoryVisibility,
  getVerifiedCount,
} from "./rig/rigState";
import { buildBunnyExplorerCharacter, type BuiltRig } from "./rig/rigBuilder";
import {
  createInitialAnimationState,
  type AnimationState,
  type AnimMode,
  type MouthViseme,
  setAnimationMode,
  updateAnimParams,
  triggerOneShotJump,
  setWasdKey,
  setMouthViseme,
  updateSlider,
  isMovingWithKeys,
  isRunningWithKeys,
} from "./animation/animationState";
import { evaluatePose, applyPoseToRig, type PoseTransforms } from "./animation/proceduralAnimation";
import {
  createInitialCameraState,
  type CameraState,
  setTurntable,
  interpolateYaw,
  snapYaw,
  scrubYaw,
  updateHudYaw,
  setLineupMode,
} from "./camera/cameraState";
import {
  createInitialReconstructionState,
  type ReconstructionState,
  type SliceSet,
  setReconstructionType,
  setProjectionBlend,
  toggleReferencePlanes,
} from "./reconstruction/reconstructionState";
import {
  estimateSheetBg,
  detectRowBands,
  detectPoseCells,
  floodFillRemoveBg,
  renderNormalizedSlice,
  buildTextureAtlas,
  type Cell,
} from "./reconstruction/slicePipeline";
import {
  buildFourDirectionAtlas,
  sampleFourDirectionColor,
  buildFourDirectionAtlasWithSampler,
  type FourDirectionAtlas,
} from "./reconstruction/fourDirectionAtlas";
import {
  extractPoseMotionRange,
  evaluateStillToLimitPose,
  POSE_MODES,
  type PoseMode,
  type RangeEnvelope,
  type ModeRange,
  type Bounds3D,
  type ActionFrame,
} from "./animation/poseMotionRange";
import {
  createInitialUIState,
  type UIState,
  type ShadingMode,
  addToast,
  removeToast,
  setModal,
  setShadingMode,
  toggleBackdrop,
} from "./ui/uiState";
import {
  createInitialExportState,
  type ExportState,
  triggerDownloadBlob,
  prepareCleanExportClone,
  restoreAfterExportClone,
} from "./export/exportState";

// --- Standalone Three.js 2.5D cutout + p5.js 2D puppet animator (no Blender) ---
import {
  ThreeCutoutAnimator,
  P5PuppetAnimator,
  createThreeCutoutAnimator,
  createP5PuppetAnimator,
  createP5PuppetSketch,
  loadImage,
} from "./animation/threeAndP5PuppetAnimator";

const Q1_TO_ARCHETYPE: Record<string, { archetype: SkeletonArchetype; defaultVerb: UniversalActionVerb }> = {
  walk_jump_talk: { archetype: "humanoid_biped", defaultVerb: "move_traverse" },
  swim_wave_fins: { archetype: "axial_creature", defaultVerb: "move_traverse" },
  rise_fall_pulse: { archetype: "radial_prop", defaultVerb: "jump_rise_fall" },
  traverse_left_right_turn: { archetype: "vehicle_hull", defaultVerb: "move_traverse" },
};

export function evaluateThreeQuestionIntake(input: {
  q1_motionIntent: string;
  q2_sliceParts: string[];
  q3_exportTarget: string;
  subjectHint?: string;
}) {
  const mapped = Q1_TO_ARCHETYPE[input.q1_motionIntent] || {
    archetype: classifyArchetypeFromHint(input.subjectHint || input.q1_motionIntent),
    defaultVerb: "move_traverse" as UniversalActionVerb,
  };
  const skeleton = buildArchetypeSkeleton(mapped.archetype);
  const topologyDecisions: Record<string, any> = {};
  for (const partId of input.q2_sliceParts) {
    const needsBend = !partId.includes("core") && !partId.includes("hull");
    const decision = doesRegionNeedBendLoop(
      needsBend ? ("hinge" as const) : ("rigid" as const),
      needsBend ? 65 : 10
    );
    topologyDecisions[partId] = {
      needsExtraBendJoint: decision.needsLoop,
      recommendedSubdivision: decision.ringCount,
      extraSupportLoops: decision.ringCount,
      rationale: decision.needsLoop ? "hinge joint needs bend loop" : "rigid segment",
    };
  }
  return {
    archetype: mapped.archetype,
    defaultVerb: mapped.defaultVerb,
    bones: skeleton.bones,
    topologyDecisions,
    exportTarget: input.q3_exportTarget,
    subjectHint: input.subjectHint || "Character",
  };
}

// --- Combined studio state facade ---
export interface OMHASStudioState {
  rig: RigState;
  animation: AnimationState;
  camera: CameraState;
  reconstruction: ReconstructionState;
  ui: UIState;
  export: ExportState;
}

export function createStudioState(): OMHASStudioState {
  return {
    rig: createInitialRigState(),
    animation: createInitialAnimationState(),
    camera: createInitialCameraState(),
    reconstruction: createInitialReconstructionState(),
    ui: createInitialUIState(),
    export: createInitialExportState(),
  };
}

export {
  // intake / topology / skinning / archetypes / verbs / exporters (re-exported)
  classifyArchetypeFromHint,
  buildIntakeQuestions,
  runThreeQuestionIntake,
  doesRegionNeedBendLoop,
  insertJointDeformationLoops,
  computeMeshTopologyMetrics,
  borderFloodFillAlpha,
  stripCostumeToBaseMannequin,
  applyCostumeSkinLayer,
  buildArchetypeSkeleton,
  evaluateUniversalVerb,
  generateBlender53ArchetypeScript,
  generateVideoComponentCode,
  // rig state + builder
  createInitialRigState,
  buildBunnyExplorerCharacter,
  setPartVisibility,
  togglePartVisibility,
  soloPart,
  showAllParts,
  togglePartVerified,
  verifyAllParts,
  setRigExplode,
  setSpecies,
  toggleCategoryVisibility,
  getVerifiedCount,
  type RigState,
  type RigPivots,
  type RigPart,
  type BuiltRig,
  // animation state + procedural animation
  createInitialAnimationState,
  setAnimationMode,
  updateAnimParams,
  triggerOneShotJump,
  setWasdKey,
  setMouthViseme,
  updateSlider,
  isMovingWithKeys,
  isRunningWithKeys,
  evaluatePose,
  applyPoseToRig,
  type AnimationState,
  type AnimMode,
  type MouthViseme,
  type PoseTransforms,
  // camera
  createInitialCameraState,
  setTurntable,
  interpolateYaw,
  snapYaw,
  scrubYaw,
  updateHudYaw,
  setLineupMode,
  type CameraState,
  // reconstruction + slice pipeline
  createInitialReconstructionState,
  setReconstructionType,
  setProjectionBlend,
  toggleReferencePlanes,
  estimateSheetBg,
  detectRowBands,
  detectPoseCells,
  floodFillRemoveBg,
  renderNormalizedSlice,
  buildTextureAtlas,
  buildFourDirectionAtlas,
  sampleFourDirectionColor,
  buildFourDirectionAtlasWithSampler,
  type ReconstructionState,
  type SliceSet,
  type Cell,
  type FourDirectionAtlas,
  // pose motion range
  extractPoseMotionRange,
  evaluateStillToLimitPose,
  POSE_MODES,
  type PoseMode,
  type RangeEnvelope,
  type ModeRange,
  type Bounds3D,
  type ActionFrame,
  // UI
  createInitialUIState,
  addToast,
  removeToast,
  setModal,
  setShadingMode,
  toggleBackdrop,
  type UIState,
  type ShadingMode,
  // export
  createInitialExportState,
  triggerDownloadBlob,
  prepareCleanExportClone,
  restoreAfterExportClone,
  type ExportState,
  // standalone animator
  ThreeCutoutAnimator,
  P5PuppetAnimator,
  createThreeCutoutAnimator,
  createP5PuppetAnimator,
  createP5PuppetSketch,
  loadImage,
};

export const OMHASStudio = {
  evaluateThreeQuestionIntake,
  createStudioState,
  classifyArchetypeFromHint,
  buildIntakeQuestions,
  runThreeQuestionIntake,
  doesRegionNeedBendLoop,
  insertJointDeformationLoops,
  computeMeshTopologyMetrics,
  borderFloodFillAlpha,
  stripCostumeToBaseMannequin,
  applyCostumeSkinLayer,
  buildArchetypeSkeleton,
  evaluateUniversalVerb,
  generateBlender53ArchetypeScript,
  generateVideoComponentCode,
  createInitialRigState,
  buildBunnyExplorerCharacter,
  setPartVisibility,
  togglePartVisibility,
  soloPart,
  showAllParts,
  togglePartVerified,
  verifyAllParts,
  setRigExplode,
  setSpecies,
  toggleCategoryVisibility,
  getVerifiedCount,
  createInitialAnimationState,
  setAnimationMode,
  updateAnimParams,
  triggerOneShotJump,
  setWasdKey,
  setMouthViseme,
  updateSlider,
  isMovingWithKeys,
  isRunningWithKeys,
  evaluatePose,
  applyPoseToRig,
  createInitialCameraState,
  setTurntable,
  interpolateYaw,
  snapYaw,
  scrubYaw,
  updateHudYaw,
  setLineupMode,
  createInitialReconstructionState,
  setReconstructionType,
  setProjectionBlend,
  toggleReferencePlanes,
  estimateSheetBg,
  detectRowBands,
  detectPoseCells,
  floodFillRemoveBg,
  renderNormalizedSlice,
  buildTextureAtlas,
  buildFourDirectionAtlas,
  sampleFourDirectionColor,
  buildFourDirectionAtlasWithSampler,
  extractPoseMotionRange,
  evaluateStillToLimitPose,
  POSE_MODES,
  createInitialUIState,
  addToast,
  removeToast,
  setModal,
  setShadingMode,
  toggleBackdrop,
  createInitialExportState,
  triggerDownloadBlob,
  prepareCleanExportClone,
  restoreAfterExportClone,
  ThreeCutoutAnimator,
  P5PuppetAnimator,
  createThreeCutoutAnimator,
  createP5PuppetAnimator,
  createP5PuppetSketch,
  loadImage,
};

if (typeof window !== "undefined") {
  (window as any).OMHASStudio = OMHASStudio;
}
