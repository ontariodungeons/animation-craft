import type { ArchetypeId, SkeletonGraph } from "../types/schema";

export function buildArchetypeSkeleton(archetype: ArchetypeId): SkeletonGraph {
  if (archetype === "axial_creature") {
    return {
      archetype: "axial_creature",
      bones: [
        { name: "DEF_Spine_Head", parent: null, head: [-0.35, 0.5, 0], tail: [-0.10, 0.5, 0], jointType: "spline" },
        { name: "DEF_Spine_Mid", parent: "DEF_Spine_Head", head: [-0.10, 0.5, 0], tail: [0.18, 0.5, 0], jointType: "spline" },
        { name: "DEF_Spine_Tail", parent: "DEF_Spine_Mid", head: [0.18, 0.5, 0], tail: [0.42, 0.5, 0], jointType: "spline" },
        { name: "DEF_Fin_Caudal", parent: "DEF_Spine_Tail", head: [0.42, 0.5, 0], tail: [0.64, 0.5, 0], jointType: "hinge" },
        { name: "DEF_Fin_Pectoral.L", parent: "DEF_Spine_Head", head: [-0.12, 0.44, 0.12], tail: [0.04, 0.34, 0.26], jointType: "hinge" },
        { name: "DEF_Fin_Pectoral.R", parent: "DEF_Spine_Head", head: [-0.12, 0.44, -0.12], tail: [0.04, 0.34, -0.26], jointType: "hinge" },
        { name: "DEF_Fin_Dorsal", parent: "DEF_Spine_Mid", head: [0.0, 0.66, 0], tail: [0.14, 0.84, 0], jointType: "hinge" }
      ]
    };
  }

  if (archetype === "radial_prop") {
    return {
      archetype: "radial_prop",
      bones: [
        { name: "DEF_SunCore", parent: null, head: [0, 1.0, 0], tail: [0, 1.25, 0], jointType: "spline" },
        { name: "DEF_RayRing", parent: "DEF_SunCore", head: [0, 1.0, 0], tail: [0.35, 1.0, 0], jointType: "hinge" },
        { name: "DEF_FaceJaw", parent: "DEF_SunCore", head: [0, 0.92, 0.18], tail: [0, 0.84, 0.24], jointType: "hinge" }
      ]
    };
  }

  if (archetype === "vehicle_hull") {
    return {
      archetype: "vehicle_hull",
      bones: [
        { name: "DEF_Hull", parent: null, head: [0, 0.25, 0], tail: [0, 0.55, 0], jointType: "hinge" },
        { name: "DEF_SailMast", parent: "DEF_Hull", head: [0, 0.55, 0], tail: [0, 1.35, 0], jointType: "spline" },
        { name: "DEF_Rudder", parent: "DEF_Hull", head: [-0.45, 0.25, 0], tail: [-0.62, 0.20, 0], jointType: "hinge" }
      ]
    };
  }

  return {
    archetype: "humanoid_biped",
    bones: [
      { name: "DEF_Pelvis", parent: null, head: [0, 0.52, 0], tail: [0, 0.56, 0], jointType: "rigid" },
      { name: "DEF_Spine", parent: "DEF_Pelvis", head: [0, 0.56, 0], tail: [0, 0.74, 0], jointType: "spline" },
      { name: "DEF_Chest", parent: "DEF_Spine", head: [0, 0.74, 0], tail: [0, 0.96, 0], jointType: "spline" },
      { name: "DEF_Head", parent: "DEF_Chest", head: [0, 0.96, 0], tail: [0, 1.38, 0], jointType: "hinge" },
      { name: "DEF_Jaw", parent: "DEF_Head", head: [0, 1.12, 0.18], tail: [0, 1.07, 0.26], jointType: "hinge" },
      { name: "DEF_Ear.L", parent: "DEF_Head", head: [0.105, 1.42, -0.01], tail: [0.165, 1.88, -0.01], jointType: "hinge" },
      { name: "DEF_Ear.R", parent: "DEF_Head", head: [-0.105, 1.42, -0.01], tail: [-0.165, 1.88, -0.01], jointType: "hinge" },
      { name: "DEF_UpperArm.L", parent: "DEF_Chest", head: [0.215, 0.91, 0], tail: [0.242, 0.76, 0], jointType: "hinge" },
      { name: "DEF_Forearm.L", parent: "DEF_UpperArm.L", head: [0.242, 0.76, 0], tail: [0.260, 0.62, 0.012], jointType: "hinge" },
      { name: "DEF_Hand.L", parent: "DEF_Forearm.L", head: [0.260, 0.62, 0.012], tail: [0.268, 0.53, 0.025], jointType: "hinge" },
      { name: "DEF_UpperArm.R", parent: "DEF_Chest", head: [-0.215, 0.91, 0], tail: [-0.242, 0.76, 0], jointType: "hinge" },
      { name: "DEF_Forearm.R", parent: "DEF_UpperArm.R", head: [-0.242, 0.76, 0], tail: [-0.260, 0.62, 0.012], jointType: "hinge" },
      { name: "DEF_Hand.R", parent: "DEF_Forearm.R", head: [-0.260, 0.62, 0.012], tail: [-0.268, 0.53, 0.025], jointType: "hinge" },
      { name: "DEF_Thigh.L", parent: "DEF_Pelvis", head: [0.102, 0.48, 0], tail: [0.102, 0.33, 0.01], jointType: "hinge" },
      { name: "DEF_Shin.L", parent: "DEF_Thigh.L", head: [0.102, 0.33, 0.01], tail: [0.102, 0.14, 0], jointType: "hinge" },
      { name: "DEF_Foot.L", parent: "DEF_Shin.L", head: [0.102, 0.14, 0], tail: [0.102, 0.05, 0.14], jointType: "hinge" },
      { name: "DEF_Thigh.R", parent: "DEF_Pelvis", head: [-0.102, 0.48, 0], tail: [-0.102, 0.33, 0.01], jointType: "hinge" },
      { name: "DEF_Shin.R", parent: "DEF_Thigh.R", head: [-0.102, 0.33, 0.01], tail: [-0.102, 0.14, 0], jointType: "hinge" },
      { name: "DEF_Foot.R", parent: "DEF_Shin.R", head: [-0.102, 0.14, 0], tail: [-0.102, 0.05, 0.14], jointType: "hinge" },
      { name: "DEF_Tail", parent: "DEF_Pelvis", head: [0, 0.555, -0.195], tail: [0, 0.575, -0.285], jointType: "hinge" }
    ]
  };
}
