// src/animation/animationState.ts
// Domain state object #2: AnimationState
// Animation mode, phase, speed/energy, jump timer, WASD, sprint, world position,
// mouth viseme, eye blink/gaze, slider values.
// Extracted from the monolithic animation + rig slider logic.

import type { UniversalVerb } from "../types/schema";

export type AnimMode = "idle" | "walk" | "run" | "jump" | "wave" | "explore" | "none";
export type MouthViseme = "smile" | "laugh" | "ooh" | "closed" | "talk";

export interface WasdState {
  up: boolean;
  down: boolean;
  left: boolean;
  right: boolean;
  shift: boolean;
}

export interface SliderValues {
  earSplay: number;
  earPitch: number;
  headYaw: number;
  headPitch: number;
  smileOpen: number;
  armSpread: number;
  kneeBend: number;
  elbowBend: number;
  eyeBlink: number;
  eyeGaze: number;
}

export interface AnimationState {
  activeAnimMode: AnimMode;
  animSpeedMultiplier: number;
  animEnergyMultiplier: number;
  animPhase: number;
  oneShotJumpTimer: number; // >= 0 when performing an on-demand jump
  wasdState: WasdState;
  isSprintLocked: boolean;
  characterWorldPos: { x: number; z: number; vx: number; vz: number };
  activeMouthViseme: MouthViseme;
  sliders: SliderValues;
}

export const DEFAULT_SLIDERS: SliderValues = {
  earSplay: 14,
  earPitch: -4,
  headYaw: 0,
  headPitch: 0,
  smileOpen: 100,
  armSpread: 8,
  kneeBend: 0,
  elbowBend: 12,
  eyeBlink: 0,
  eyeGaze: 0,
};

export function createInitialAnimationState(): AnimationState {
  return {
    activeAnimMode: "idle",
    animSpeedMultiplier: 1.0,
    animEnergyMultiplier: 1.0,
    animPhase: 0,
    oneShotJumpTimer: -1,
    wasdState: { up: false, down: false, left: false, right: false, shift: false },
    isSprintLocked: false,
    characterWorldPos: { x: 0, z: 0, vx: 0, vz: 0 },
    activeMouthViseme: "smile",
    sliders: { ...DEFAULT_SLIDERS },
  };
}

export function setAnimationMode(state: AnimationState, mode: AnimMode): void {
  state.activeAnimMode = mode;
}

export function updateAnimParams(state: AnimationState, speed: number, energyPct: number): void {
  state.animSpeedMultiplier = speed;
  state.animEnergyMultiplier = energyPct / 100;
}

export function triggerOneShotJump(state: AnimationState): void {
  state.oneShotJumpTimer = 0;
}

export function setWasdKey(state: AnimationState, dir: keyof WasdState, isDown: boolean): void {
  state.wasdState[dir] = isDown;
}

export function setMouthViseme(state: AnimationState, viseme: MouthViseme): void {
  state.activeMouthViseme = viseme;
}

export function updateSlider(state: AnimationState, key: keyof SliderValues, value: number): void {
  state.sliders[key] = value;
}

export function isMovingWithKeys(state: AnimationState): boolean {
  return state.wasdState.left || state.wasdState.right || state.wasdState.up || state.wasdState.down;
}

export function isRunningWithKeys(state: AnimationState): boolean {
  return isMovingWithKeys(state) && (state.wasdState.shift || state.isSprintLocked);
}

export function effectiveVerb(state: AnimationState): UniversalVerb {
  if (!isMovingWithKeys(state)) return "move_traverse" as UniversalVerb;
  return isRunningWithKeys(state) ? ("move_traverse" as UniversalVerb) : ("move_traverse" as UniversalVerb);
}
