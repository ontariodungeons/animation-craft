// src/animation/proceduralAnimation.ts
// Modular per-mode animation functions extracted from evaluateProceduralAnimation.
// Each mode returns joint transforms (in radians / world units) for the bunny rig.
// These are applied by the animation loop to the RigPivots.

import type { RigPivots } from "../rig/rigState";
import type { SliderValues } from "./animationState";

export interface PoseTransforms {
  rootY: number;
  rootScale: [number, number, number];
  spine: [number, number, number];
  pelvis: [number, number, number];
  head: [number, number, number];
  earL: [number, number, number];
  earR: [number, number, number];
  armL: [number, number, number];
  armR: [number, number, number];
  elbowL: [number, number, number];
  elbowR: [number, number, number];
  wristL: [number, number, number];
  wristR: [number, number, number];
  legL: [number, number, number];
  legR: [number, number, number];
  kneeL: [number, number, number];
  kneeR: [number, number, number];
  footL: [number, number, number];
  footR: [number, number, number];
  tail: [number, number, number];
}

const zeroPose: PoseTransforms = {
  rootY: 0, rootScale: [1, 1, 1],
  spine: [0, 0, 0], pelvis: [0, 0, 0], head: [0, 0, 0],
  earL: [0, 0, 0], earR: [0, 0, 0],
  armL: [0, 0, 0], armR: [0, 0, 0],
  elbowL: [0, 0, 0], elbowR: [0, 0, 0],
  wristL: [0, 0, 0], wristR: [0, 0, 0],
  legL: [0, 0, 0], legR: [0, 0, 0],
  kneeL: [0, 0, 0], kneeR: [0, 0, 0],
  footL: [0, 0, 0], footR: [0, 0, 0],
  tail: [0, 0, 0],
};

function deg(d: number): number { return (d * Math.PI) / 180; }

// Idle: gentle breathing + sway + ear flick.
export function idlePose(phase: number, energy: number, sliders: SliderValues): PoseTransforms {
  const p = { ...zeroPose };
  const b = Math.sin(phase * 2.2);
  p.rootY = Math.max(0, b * 0.008 * energy);
  p.spine[0] = Math.sin(phase * 2.2) * 0.025 * energy;
  p.head[2] = Math.sin(phase * 1.1) * 0.03 * energy;
  p.head[1] = deg(sliders.headYaw) + Math.sin(phase * 0.7) * 0.05 * energy;
  p.earL[2] = -deg(sliders.earSplay) - Math.sin(phase * 2.2 - 0.5) * 0.035 * energy;
  p.earR[2] = deg(sliders.earSplay) + Math.sin(phase * 2.2 - 0.5) * 0.035 * energy;
  p.armL[0] = Math.sin(phase * 2.2) * 0.04 * energy;
  p.armR[0] = -Math.sin(phase * 2.2) * 0.04 * energy;
  p.tail[1] = Math.sin(phase * 4.4) * 0.08 * energy;
  p.earL[0] = deg(sliders.earPitch);
  p.earR[0] = deg(sliders.earPitch);
  p.head[0] = deg(sliders.headPitch);
  return p;
}

// Walk: stride + bounce + knee flexion + arm swing.
export function walkPose(phase: number, energy: number, sliders: SliderValues): PoseTransforms {
  const p = { ...zeroPose };
  const w = phase * 5.6;
  const stride = Math.sin(w) * energy;
  const bounce = Math.abs(Math.cos(w));
  p.rootY = bounce * 0.042 * energy;
  p.spine[0] = 0.06 * energy + Math.cos(w * 2) * 0.025 * energy;
  p.spine[1] = -stride * 0.12;
  p.spine[2] = Math.cos(w) * 0.035 * energy;
  p.pelvis[1] = stride * 0.14;
  p.pelvis[2] = -Math.cos(w) * 0.045 * energy;
  p.legL[0] = stride * 0.52;
  p.legR[0] = -stride * 0.52;
  p.kneeL[0] = deg(sliders.kneeBend) + Math.max(0, -Math.cos(w)) * 0.62 * energy;
  p.kneeR[0] = deg(sliders.kneeBend) + Math.max(0, Math.cos(w)) * 0.62 * energy;
  p.footL[0] = -p.kneeL[0] * 0.45 - stride * 0.12;
  p.footR[0] = -p.kneeR[0] * 0.45 + stride * 0.12;
  p.armL[0] = -stride * 0.46;
  p.armR[0] = stride * 0.46;
  p.armL[2] = -deg(sliders.armSpread) - 0.06;
  p.armR[2] = deg(sliders.armSpread) + 0.06;
  p.elbowL[0] = -deg(sliders.elbowBend) - Math.max(0, -stride) * 0.35;
  p.elbowR[0] = -deg(sliders.elbowBend) - Math.max(0, stride) * 0.35;
  p.head[1] = deg(sliders.headYaw) + stride * 0.08;
  p.head[2] = -Math.cos(w) * 0.04 * energy;
  p.head[0] = deg(sliders.headPitch) - Math.sin(w * 2) * 0.04 * energy;
  p.earL[0] = deg(sliders.earPitch) - 0.06 + Math.sin(w * 2 - 0.7) * energy * 0.12;
  p.earR[0] = deg(sliders.earPitch) - 0.06 + Math.sin(w * 2 - 0.7) * energy * 0.12;
  p.earL[2] = -deg(sliders.earSplay) - Math.abs(Math.sin(w * 2 - 0.7)) * energy * 0.08;
  p.earR[2] = deg(sliders.earSplay) + Math.abs(Math.sin(w * 2 - 0.7)) * energy * 0.08;
  p.tail[1] = stride * 0.25;
  p.tail[0] = Math.sin(w * 2) * 0.12;
  return p;
}

// Run: faster stride + flight + deeper knee bend.
export function runPose(phase: number, energy: number, sliders: SliderValues): PoseTransforms {
  const p = walkPose(phase, energy, sliders);
  const r = phase * 9.2;
  const stride = Math.sin(r) * energy;
  const flight = Math.max(0, Math.sin(r * 2));
  p.rootY = (Math.abs(Math.cos(r)) * 0.085 + flight * 0.025) * energy;
  p.spine[0] = 0.20 * energy + Math.sin(r * 2) * 0.05 * energy;
  p.spine[1] = -stride * 0.20;
  p.spine[2] = Math.cos(r) * 0.05 * energy;
  p.pelvis[1] = stride * 0.22;
  p.pelvis[2] = -Math.cos(r) * 0.06 * energy;
  p.legL[0] = stride * 0.85;
  p.legR[0] = -stride * 0.85;
  p.kneeL[0] = deg(sliders.kneeBend) + Math.max(0, -Math.cos(r)) * 0.95 * energy;
  p.kneeR[0] = deg(sliders.kneeBend) + Math.max(0, Math.cos(r)) * 0.95 * energy;
  p.footL[0] = -p.kneeL[0] * 0.4 + Math.max(0, -Math.cos(r)) * 0.35 * energy;
  p.footR[0] = -p.kneeR[0] * 0.4 + Math.max(0, Math.cos(r)) * 0.35 * energy;
  p.armL[0] = -stride * 0.78;
  p.armR[0] = stride * 0.78;
  p.armL[2] = -deg(sliders.armSpread) - 0.14;
  p.armR[2] = deg(sliders.armSpread) + 0.14;
  p.elbowL[0] = -0.55 - Math.max(0, -stride) * 0.42;
  p.elbowR[0] = -0.55 - Math.max(0, stride) * 0.42;
  p.head[0] = deg(sliders.headPitch) - 0.12 * energy + Math.cos(r * 2) * 0.06;
  p.head[1] = deg(sliders.headYaw) + stride * 0.12;
  p.earL[0] = deg(sliders.earPitch) - 0.32 * energy + Math.sin(r * 2 - 0.8) * 0.18 * energy;
  p.earR[0] = deg(sliders.earPitch) - 0.32 * energy + Math.sin(r * 2 - 0.8) * 0.18 * energy;
  p.earL[2] = -deg(sliders.earSplay) - 0.08;
  p.earR[2] = deg(sliders.earSplay) + 0.08;
  p.tail[1] = stride * 0.35;
  return p;
}

// Jump sub-phases: anticipation crouch (0-0.22), airborne arc (0.22-0.78), landing (0.78-1).
export function jumpPose(jumpProgress: number, energy: number, sliders: SliderValues): PoseTransforms {
  const p = { ...zeroPose };
  const j = jumpProgress; // 0..1
  if (j < 0.22) {
    const t = j / 0.22;
    const crouch = Math.sin(t * Math.PI * 0.5);
    p.rootY = -0.09 * crouch * energy;
    p.rootScale = [1 + 0.04 * crouch * energy, 1 - 0.07 * crouch * energy, 1 + 0.04 * crouch * energy];
    p.spine[0] = 0.28 * crouch * energy;
    p.legL[0] = -0.48 * crouch * energy;
    p.legR[0] = -0.48 * crouch * energy;
    p.kneeL[0] = 0.85 * crouch * energy;
    p.kneeR[0] = 0.85 * crouch * energy;
    p.footL[0] = -0.35 * crouch * energy;
    p.footR[0] = -0.35 * crouch * energy;
    p.armL[0] = 0.45 * crouch * energy;
    p.armR[0] = 0.45 * crouch * energy;
    p.elbowL[0] = -0.45 * crouch * energy;
    p.elbowR[0] = -0.45 * crouch * energy;
    p.earL[0] = deg(sliders.earPitch) + 0.22 * crouch;
    p.earR[0] = deg(sliders.earPitch) + 0.22 * crouch;
  } else if (j < 0.78) {
    const airT = (j - 0.22) / 0.56;
    const arc = 4.0 * airT * (1.0 - airT);
    const vel = 1.0 - 2.0 * airT;
    p.rootY = arc * 0.42 * energy;
    p.rootScale = [1 - 0.035 * Math.abs(vel) * energy, 1 + 0.07 * Math.abs(vel) * energy, 1 - 0.035 * Math.abs(vel) * energy];
    p.spine[0] = -0.10 * vel * energy;
    p.legL[0] = (-0.52 * arc + 0.22 * vel) * energy;
    p.legR[0] = (0.22 * arc - 0.18 * vel) * energy;
    p.kneeL[0] = 0.78 * arc * energy;
    p.kneeR[0] = 0.55 * arc * energy;
    p.footL[0] = -0.35 * arc * energy;
    p.footR[0] = -0.25 * arc * energy;
    p.armL[2] = -deg(sliders.armSpread) - 0.85 * arc * energy;
    p.armR[2] = deg(sliders.armSpread) + 0.85 * arc * energy;
    p.armL[0] = -0.35 * arc * energy;
    p.armR[0] = -0.35 * arc * energy;
    p.elbowL[0] = -0.38 * arc * energy;
    p.elbowR[0] = -0.38 * arc * energy;
    p.earL[0] = deg(sliders.earPitch) - 0.42 * vel * energy;
    p.earR[0] = deg(sliders.earPitch) - 0.42 * vel * energy;
    p.earL[2] = -deg(sliders.earSplay) - 0.18 * (1 - vel) * 0.5;
    p.earR[2] = deg(sliders.earSplay) + 0.18 * (1 - vel) * 0.5;
  } else {
    const landT = (j - 0.78) / 0.22;
    const impact = Math.sin(landT * Math.PI);
    p.rootY = -0.07 * impact * energy;
    p.rootScale = [1 + 0.04 * impact * energy, 1 - 0.06 * impact * energy, 1 + 0.04 * impact * energy];
    p.spine[0] = 0.22 * impact * energy;
    p.legL[0] = -0.38 * impact * energy;
    p.legR[0] = -0.38 * impact * energy;
    p.kneeL[0] = 0.68 * impact * energy;
    p.kneeR[0] = 0.68 * impact * energy;
    p.footL[0] = -0.28 * impact * energy;
    p.footR[0] = -0.28 * impact * energy;
    p.earL[0] = deg(sliders.earPitch) + 0.35 * impact * energy;
    p.earR[0] = deg(sliders.earPitch) + 0.35 * impact * energy;
  }
  return p;
}

// Wave: left arm raised, elbow + wrist flutter.
export function wavePose(phase: number, energy: number, sliders: SliderValues): PoseTransforms {
  const p = { ...zeroPose };
  const w = phase * 4.5;
  p.rootY = Math.abs(Math.sin(phase * 2.2)) * 0.012 * energy;
  p.spine[2] = -0.06 * energy;
  p.head[2] = 0.12 * energy + Math.sin(w * 0.5) * 0.04;
  p.head[1] = deg(sliders.headYaw) + 0.12 * energy;
  p.armL[2] = -2.15 + Math.sin(w * 1.4) * 0.22 * energy;
  p.armL[0] = -0.22;
  p.elbowL[0] = -0.45 + Math.sin(w * 1.4) * 0.25 * energy;
  p.wristL[2] = Math.sin(w * 2.8) * 0.32 * energy;
  p.armR[0] = Math.sin(phase * 2.2) * 0.05;
  p.earL[2] = -deg(sliders.earSplay) - 0.12 * energy + Math.sin(w) * 0.06;
  p.earR[2] = deg(sliders.earSplay) + 0.05 * energy;
  p.tail[1] = Math.sin(w) * 0.18;
  return p;
}

// Explore: head scan + body lean.
export function explorePose(phase: number, energy: number, sliders: SliderValues): PoseTransforms {
  const p = { ...zeroPose };
  const s = phase * 1.8;
  const scan = Math.sin(s);
  p.rootY = Math.max(0, Math.sin(s * 2)) * 0.025 * energy;
  p.spine[1] = scan * 0.22 * energy;
  p.spine[0] = 0.08 * Math.abs(scan) * energy;
  p.head[1] = deg(sliders.headYaw) + scan * 0.48 * energy;
  p.head[0] = deg(sliders.headPitch) - 0.06 * Math.cos(s * 2) * energy;
  p.head[2] = -scan * 0.08 * energy;
  p.earL[0] = deg(sliders.earPitch) + 0.15 * Math.sin(s * 2) * energy;
  p.earR[0] = deg(sliders.earPitch) - 0.15 * Math.sin(s * 2) * energy;
  p.earL[2] = -deg(sliders.earSplay) - 0.08 * Math.abs(scan);
  p.earR[2] = deg(sliders.earSplay) + 0.08 * Math.abs(scan);
  p.armL[0] = -0.22 * Math.abs(scan);
  p.armR[0] = -0.22 * Math.abs(scan);
  p.elbowL[0] = -0.35 * Math.abs(scan);
  p.elbowR[0] = -0.35 * Math.abs(scan);
  p.kneeL[0] = deg(sliders.kneeBend) + Math.max(0, Math.sin(s * 2)) * 0.24;
  p.kneeR[0] = deg(sliders.kneeBend) + Math.max(0, Math.sin(s * 2)) * 0.24;
  p.footL[0] = -p.kneeL[0] * 0.5;
  p.footR[0] = -p.kneeR[0] * 0.5;
  return p;
}

// Blink amount (auto cycle + manual slider).
export function blinkAmount(phase: number, sliders: SliderValues): number {
  const sliderVal = sliders.eyeBlink / 100;
  const autoBlinkCycle = (phase * 0.85) % 4.0;
  const autoBlink = autoBlinkCycle > 3.82 ? Math.sin(((autoBlinkCycle - 3.82) / 0.18) * Math.PI) : 0;
  return Math.max(sliderVal, autoBlink);
}

// Talk viseme jaw scale.
export function talkJawScale(phase: number, viseme: string): number {
  if (viseme === "talk") return 0.55 + 0.65 * Math.abs(Math.sin(phase * 11.5));
  return 1.0;
}

// Dispatcher: pick the active mode. Returns transforms + blink/talk metadata.
export function evaluatePose(
  mode: string,
  phase: number,
  energy: number,
  sliders: SliderValues,
  jumpProgress: number,
  mouthViseme: string
): { pose: PoseTransforms; blink: number; jawScale: number } {
  let pose: PoseTransforms;
  if (mode === "idle") pose = idlePose(phase, energy, sliders);
  else if (mode === "walk") pose = walkPose(phase, energy, sliders);
  else if (mode === "run") pose = runPose(phase, energy, sliders);
  else if (mode === "jump") pose = jumpPose(jumpProgress >= 0 ? jumpProgress : (phase * 1.15) % 1.0, energy, sliders);
  else if (mode === "wave") pose = wavePose(phase, energy, sliders);
  else if (mode === "explore") pose = explorePose(phase, energy, sliders);
  else pose = zeroPose; // "none" -> bind pose (sliders applied by caller)
  return { pose, blink: blinkAmount(phase, sliders), jawScale: talkJawScale(phase, mouthViseme) };
}

// Apply computed transforms to RigPivots (plus slider-driven bind-pose values when mode === "none").
export function applyPoseToRig(
  pivots: RigPivots,
  pose: PoseTransforms,
  sliders: SliderValues,
  jumpProgress: number,
  mode: string = "idle"
): void {
  if (!pivots.root) return;
  pivots.root.position.y = pose.rootY;
  pivots.root.scale.set(pose.rootScale[0], pose.rootScale[1], pose.rootScale[2]);

  if (pivots.spinePivot) pivots.spinePivot.rotation.set(pose.spine[0], pose.spine[1], pose.spine[2]);
  if (pivots.pelvisPivot) pivots.pelvisPivot.rotation.set(0, pose.pelvis[1], pose.pelvis[2]);

  if (pivots.headPivot) pivots.headPivot.rotation.set(pose.head[0], pose.head[1], pose.head[2]);
  if (pivots.earLPivot) { pivots.earLPivot.rotation.x = pose.earL[0]; pivots.earLPivot.rotation.z = pose.earL[2]; }
  if (pivots.earRPivot) { pivots.earRPivot.rotation.x = pose.earR[0]; pivots.earRPivot.rotation.z = pose.earR[2]; }

  if (pivots.armLPivot) pivots.armLPivot.rotation.set(pose.armL[0], 0, pose.armL[2]);
  if (pivots.armRPivot) pivots.armRPivot.rotation.set(pose.armR[0], 0, pose.armR[2]);
  if (pivots.elbowLPivot) pivots.elbowLPivot.rotation.set(pose.elbowL[0], 0, 0);
  if (pivots.elbowRPivot) pivots.elbowRPivot.rotation.set(pose.elbowR[0], 0, 0);
  if (pivots.wristLPivot) pivots.wristLPivot.rotation.set(pose.wristL[0], 0, pose.wristL[2]);
  if (pivots.wristRPivot) pivots.wristRPivot.rotation.set(pose.wristR[0], 0, pose.wristR[2]);

  if (pivots.legLPivot) pivots.legLPivot.rotation.set(pose.legL[0], 0, pose.legL[2]);
  if (pivots.legRPivot) pivots.legRPivot.rotation.set(pose.legR[0], 0, pose.legR[2]);
  if (pivots.kneeLPivot) pivots.kneeLPivot.rotation.set(pose.kneeL[0], 0, 0);
  if (pivots.kneeRPivot) pivots.kneeRPivot.rotation.set(pose.kneeR[0], 0, 0);
  if (pivots.footLPivot) pivots.footLPivot.rotation.set(pose.footL[0], 0, 0);
  if (pivots.footRPivot) pivots.footRPivot.rotation.set(pose.footR[0], 0, 0);
  if (pivots.tailPivot) pivots.tailPivot.rotation.set(pose.tail[0], pose.tail[1], 0);

  // Only when in static bind pose ("none"), apply slider-driven knee/elbow values directly.
  if (sliders && (mode === "none" || pose === zeroPose)) {
    const kneeRad = deg(sliders.kneeBend);
    if (pivots.legLPivot) pivots.legLPivot.rotation.x = -kneeRad * 0.48;
    if (pivots.legRPivot) pivots.legRPivot.rotation.x = -kneeRad * 0.48;
    if (pivots.kneeLPivot) pivots.kneeLPivot.rotation.x = kneeRad;
    if (pivots.kneeRPivot) pivots.kneeRPivot.rotation.x = kneeRad;
    if (pivots.footLPivot) pivots.footLPivot.rotation.x = -kneeRad * 0.52;
    if (pivots.footRPivot) pivots.footRPivot.rotation.x = -kneeRad * 0.52;
    if (pivots.elbowLPivot) pivots.elbowLPivot.rotation.x = -deg(sliders.elbowBend);
    if (pivots.elbowRPivot) pivots.elbowRPivot.rotation.x = -deg(sliders.elbowBend);
  }
}
