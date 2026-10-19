// src/camera/cameraState.ts
// Domain state object #3: CameraState
// Camera, OrbitControls, turntable, yaw interpolation, HUD values.
// Extracted from the monolithic camera/turntable/HUD logic.

import * as THREE from "three";

export interface CameraState {
  camera: THREE.PerspectiveCamera | null;
  controls: any; // OrbitControls (THREE.OrbitControls or fallback)
  targetYawRad: number;
  currentYawRad: number;
  isTurntableActive: boolean;
  turntableSpeed: number;
  isLineupMode: boolean;
  lightBackdrop: boolean;
  hudCurrentAngle: string;
  hudYawDeg: string;
}

export function createInitialCameraState(): CameraState {
  const camera = new THREE.PerspectiveCamera(34, 1, 0.1, 100);
  camera.position.set(0, 1.02, 3.35);
  return {
    camera,
    controls: null,
    targetYawRad: 0,
    currentYawRad: 0,
    isTurntableActive: false,
    turntableSpeed: 0.85,
    isLineupMode: false,
    lightBackdrop: true,
    hudCurrentAngle: "FRONT (0°)",
    hudYawDeg: "0.0°",
  };
}

export function setTurntable(state: CameraState, active: boolean): void {
  state.isTurntableActive = active;
}

export function interpolateYaw(state: CameraState, dt: number): void {
  let angleDiff = state.targetYawRad - state.currentYawRad;
  while (angleDiff > Math.PI) angleDiff -= Math.PI * 2;
  while (angleDiff < -Math.PI) angleDiff += Math.PI * 2;
  state.currentYawRad += angleDiff * Math.min(1, dt * 10);
  if (state.camera) {
    state.camera.rotation.y = state.currentYawRad;
  }
}

export function snapYaw(state: CameraState, deg: number, label: string): void {
  state.isTurntableActive = false;
  state.targetYawRad = (deg * Math.PI) / 180;
  state.currentYawRad = state.targetYawRad;
  if (state.camera) state.camera.rotation.y = state.currentYawRad;
  state.hudCurrentAngle = label;
  state.hudYawDeg = `${deg.toFixed(1)}°`;
}

export function scrubYaw(state: CameraState, degVal: number): void {
  state.isTurntableActive = false;
  const deg = Number(degVal);
  state.targetYawRad = (deg * Math.PI) / 180;
  state.currentYawRad = state.targetYawRad;
  if (state.camera) state.camera.rotation.y = state.currentYawRad;
  state.hudCurrentAngle = `CUSTOM (${deg > 0 ? "+" : ""}${deg}°)`;
  state.hudYawDeg = `${deg.toFixed(1)}°`;
}

export function updateHudYaw(state: CameraState): void {
  const deg =
    (((THREE.MathUtils.radToDeg(state.currentYawRad) + 180) % 360) + 360) % 360 - 180;
  state.hudYawDeg = `${deg.toFixed(1)}°`;
}

export function setLineupMode(state: CameraState, enable: boolean): void {
  state.isLineupMode = enable;
  if (enable) {
    if (state.camera) state.camera.position.set(0, 1.05, 6.2);
    state.hudCurrentAngle = "5-VIEW LINEUP";
  } else {
    if (state.camera) state.camera.position.set(0, 1.02, 3.35);
  }
}
