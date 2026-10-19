import type { ArchetypeId, UniversalVerb } from "../types/schema";

export type BoneTransformMap = Record<
  string,
  { pos: [number, number, number]; rot: [number, number, number]; scale: [number, number, number] }
>;

export function evaluateUniversalVerb(
  archetype: ArchetypeId,
  verb: UniversalVerb,
  phase: number,
  energy = 1.0
): BoneTransformMap {
  const w = phase * Math.PI * 2;

  if (archetype === "axial_creature") {
    const swimY = verb === "jump_rise_fall" ? Math.sin(phase * Math.PI) * 0.45 * energy : Math.sin(w) * 0.04 * energy;
    const yawTurn = verb === "turn_bank" ? Math.sin(w * 0.5) * 0.85 : 0;
    return {
      DEF_Spine_Head: { pos: [0, swimY, 0], rot: [0, yawTurn + Math.sin(w) * 0.12 * energy, 0], scale: [1, 1, 1] },
      DEF_Spine_Mid: { pos: [0, 0, 0], rot: [0, Math.sin(w - 0.9) * 0.26 * energy, 0], scale: [1, 1, 1] },
      DEF_Spine_Tail: { pos: [0, 0, 0], rot: [0, Math.sin(w - 1.8) * 0.38 * energy, 0], scale: [1, 1, 1] },
      DEF_Fin_Caudal: { pos: [0, 0, 0], rot: [0, Math.sin(w - 2.4) * 0.52 * energy, 0], scale: [1, 1, 1] },
      "DEF_Fin_Pectoral.L": { pos: [0, 0, 0], rot: [0, 0, Math.sin(w * 2) * 0.42 * energy], scale: [1, 1, 1] },
      "DEF_Fin_Pectoral.R": { pos: [0, 0, 0], rot: [0, 0, -Math.sin(w * 2) * 0.42 * energy], scale: [1, 1, 1] },
      DEF_Fin_Dorsal: { pos: [0, 0, 0], rot: [0, 0, Math.sin(w) * 0.15 * energy], scale: [1, 1, 1] }
    };
  }

  if (archetype === "radial_prop") {
    const arcY = verb === "jump_rise_fall" ? Math.sin(phase * Math.PI) * 0.65 * energy : Math.sin(w) * 0.08 * energy;
    const pulse = 1.0 + Math.sin(w * 2) * 0.08 * energy;
    const yaw = verb === "turn_bank" ? Math.sin(w) * 0.6 : 0;
    return {
      DEF_SunCore: { pos: [0, arcY, 0], rot: [0, yaw, Math.sin(w) * 0.08], scale: [pulse, 2 - pulse, 1] },
      DEF_RayRing: { pos: [0, 0, 0], rot: [0, 0, phase * Math.PI * 0.5], scale: [pulse, pulse, 1] },
      DEF_FaceJaw: {
        pos: [0, 0, 0],
        rot: [verb === "express_talk" ? Math.abs(Math.sin(w * 3)) * 0.32 : 0.08, 0, 0],
        scale: [1, 1, 1]
      }
    };
  }

  if (archetype === "vehicle_hull") {
    const xMove = verb === "move_traverse" ? Math.sin(w) * 0.45 * energy : 0;
    const yCrest = verb === "jump_rise_fall" ? Math.sin(phase * Math.PI) * 0.35 * energy : Math.sin(w * 2) * 0.05 * energy;
    const bankRoll = verb === "turn_bank" ? Math.sin(w) * 0.28 * energy : Math.sin(w) * 0.07 * energy;
    const yaw = verb === "turn_bank" ? Math.sin(w) * 0.75 * energy : 0;
    return {
      DEF_Hull: { pos: [xMove, yCrest, 0], rot: [Math.cos(w) * 0.08 * energy, yaw, bankRoll], scale: [1, 1, 1] },
      DEF_SailMast: { pos: [0, 0, 0], rot: [0, 0, -bankRoll * 0.4], scale: [1, 1, 1] },
      DEF_Rudder: { pos: [0, 0, 0], rot: [0, -yaw * 0.5, 0], scale: [1, 1, 1] }
    };
  }

  // Default: humanoid_biped
  const stride = Math.sin(w) * energy;
  const kneeL = Math.max(0, -Math.cos(w)) * 0.65 * energy;
  const kneeR = Math.max(0, Math.cos(w)) * 0.65 * energy;
  const rootY = verb === "jump_rise_fall" ? Math.sin(phase * Math.PI) * 0.42 * energy : Math.abs(Math.cos(w)) * 0.045 * energy;
  return {
    DEF_Pelvis: { pos: [0, rootY, 0], rot: [0, verb === "turn_bank" ? Math.sin(w) * 0.65 : stride * 0.12, 0], scale: [1, 1, 1] },
    "DEF_Thigh.L": { pos: [0, 0, 0], rot: [stride * 0.52, 0, 0], scale: [1, 1, 1] },
    "DEF_Shin.L": { pos: [0, 0, 0], rot: [kneeL, 0, 0], scale: [1, 1, 1] },
    "DEF_Thigh.R": { pos: [0, 0, 0], rot: [-stride * 0.52, 0, 0], scale: [1, 1, 1] },
    "DEF_Shin.R": { pos: [0, 0, 0], rot: [kneeR, 0, 0], scale: [1, 1, 1] },
    "DEF_UpperArm.L": { pos: [0, 0, 0], rot: [-stride * 0.46, 0, -0.14], scale: [1, 1, 1] },
    "DEF_Forearm.L": { pos: [0, 0, 0], rot: [-0.25 - Math.max(0, -stride) * 0.35, 0, 0], scale: [1, 1, 1] },
    DEF_Jaw: { pos: [0, 0, 0], rot: [verb === "express_talk" ? Math.abs(Math.sin(w * 3)) * 0.28 : 0.05, 0, 0], scale: [1, 1, 1] }
  };
}
