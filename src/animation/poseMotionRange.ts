// src/animation/poseMotionRange.ts
// Extracts STILL (0%) → LIMIT (100%) motion ranges from pose-sheet action frames
// and evaluates intermediate poses for walk, run, jump, wave, sit, look_up, look_down.

export interface Bounds3D {
  min: [number, number, number];
  max: [number, number, number];
}

export interface ActionFrame {
  mode: string;
  frameIndex: number;
  bounds: Bounds3D;
  rotations?: Record<string, [number, number, number]>;
}

export interface ModeRange {
  still: Bounds3D;
  limit: Bounds3D;
  range: [number, number, number]; // per-axis extent (limit - still)
  center: [number, number, number];
}

export interface RangeEnvelope {
  [mode: string]: ModeRange;
}

// Modes supported by the pose-sheet motion range system.
export const POSE_MODES = [
  "walk",
  "run",
  "jump",
  "wave",
  "sit",
  "look_up",
  "look_down",
  "idle",
] as const;

export type PoseMode = (typeof POSE_MODES)[number];

function defaultModeRange(standing: Bounds3D): ModeRange {
  const center = [
    (standing.min[0] + standing.max[0]) / 2,
    (standing.min[1] + standing.max[1]) / 2,
    (standing.min[2] + standing.max[2]) / 2,
  ];
  return {
    still: { min: [...standing.min], max: [...standing.max] },
    limit: { min: [...standing.min], max: [...standing.max] },
    range: [0, 0, 0],
    center,
  };
}

// Extract motion range per mode from pose-sheet action frames.
// For each mode, the STILL pose is the standing bind pose and the LIMIT pose is
// the extremal bounds across all frames of that mode.
export function extractPoseMotionRange(
  standingBounds: Bounds3D,
  actionFrames: ActionFrame[]
): RangeEnvelope {
  const envelope: RangeEnvelope = {};

  // Initialize every known mode to the standing bind pose.
  for (const mode of POSE_MODES) {
    envelope[mode] = defaultModeRange(standingBounds);
  }

  // Group frames by mode.
  const framesByMode: Record<string, ActionFrame[]> = {};
  for (const frame of actionFrames) {
    if (!framesByMode[frame.mode]) framesByMode[frame.mode] = [];
    framesByMode[frame.mode].push(frame);
  }

  for (const [mode, frames] of Object.entries(framesByMode)) {
    if (!envelope[mode]) {
      envelope[mode] = defaultModeRange(standingBounds);
    }
    const entry = envelope[mode];

    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;

    for (const f of frames) {
      minX = Math.min(minX, f.bounds.min[0]);
      minY = Math.min(minY, f.bounds.min[1]);
      minZ = Math.min(minZ, f.bounds.min[2]);
      maxX = Math.max(maxX, f.bounds.max[0]);
      maxY = Math.max(maxY, f.bounds.max[1]);
      maxZ = Math.max(maxZ, f.bounds.max[2]);
    }

    entry.limit = { min: [minX, minY, minZ], max: [maxX, maxY, maxZ] };
    entry.range = [
      Math.max(0, maxX - minX),
      Math.max(0, maxY - minY),
      Math.max(0, maxZ - minZ),
    ];
    entry.center = [(minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2];
  }

  return envelope;
}

// Evaluate an interpolated pose between STILL (t=0) and LIMIT (t=1) for a given mode.
// Returns the interpolated bounds, uniform scale relative to still, and offset from
// the still center — suitable for driving a 3D mesh or 2D puppet transform.
export function evaluateStillToLimitPose(
  mode: string,
  limitFactor0to1: number,
  rangeEnvelope: RangeEnvelope
): { bounds: Bounds3D; scale: [number, number, number]; offset: [number, number, number] } {
  const t = Math.max(0, Math.min(1, limitFactor0to1));
  const entry = rangeEnvelope[mode];

  if (!entry) {
    // Unknown mode: identity (no deformation)
    return {
      bounds: { min: [0, 0, 0], max: [0, 0, 0] },
      scale: [1, 1, 1],
      offset: [0, 0, 0],
    };
  }

  const still = entry.still;
  const limit = entry.limit;

  // Linearly interpolate bounds from still to limit.
  const min: [number, number, number] = [
    still.min[0] + (limit.min[0] - still.min[0]) * t,
    still.min[1] + (limit.min[1] - still.min[1]) * t,
    still.min[2] + (limit.min[2] - still.min[2]) * t,
  ];
  const max: [number, number, number] = [
    still.max[0] + (limit.max[0] - still.max[0]) * t,
    still.max[1] + (limit.max[1] - still.max[1]) * t,
    still.max[2] + (limit.max[2] - still.max[2]) * t,
  ];

  const stillW = still.max[0] - still.min[0];
  const stillH = still.max[1] - still.min[1];
  const stillD = still.max[2] - still.min[2];

  const curW = max[0] - min[0];
  const curH = max[1] - min[1];
  const curD = max[2] - min[2];

  const scale: [number, number, number] = [
    stillW > 0 ? curW / stillW : 1,
    stillH > 0 ? curH / stillH : 1,
    stillD > 0 ? curD / stillD : 1,
  ];

  const stillCx = (still.min[0] + still.max[0]) / 2;
  const stillCy = (still.min[1] + still.max[1]) / 2;
  const stillCz = (still.min[2] + still.max[2]) / 2;
  const curCx = (min[0] + max[0]) / 2;
  const curCy = (min[1] + max[1]) / 2;
  const curCz = (min[2] + max[2]) / 2;

  const offset: [number, number, number] = [
    curCx - stillCx,
    curCy - stillCy,
    curCz - stillCz,
  ];

  return {
    bounds: { min, max },
    scale,
    offset,
  };
}
