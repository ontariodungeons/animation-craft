import type { ArchetypeId, UniversalVerb } from "../types/schema";

export interface CutoutRigPartExportSpec {
  part: string;
  label?: string;
  file: string;
  cropped_file?: string;
  parent: string;
  pivot: [number, number];
  bbox?: [number, number, number, number];
  z_index: number;
}

export interface CutoutRigExportPlan {
  character_id: string;
  canvas_size: [number, number];
  parts: CutoutRigPartExportSpec[];
  glb_path?: string;
  mode?: string;
  speed?: number;
  duration?: number;
}

const DEFAULT_10_PART_SPECS: CutoutRigPartExportSpec[] = [
  { part: "leg_right", file: "cutout_layers/09_leg_right_layer.png", parent: "root", pivot: [338, 916], z_index: 10 },
  { part: "foot_right", file: "cutout_layers/10_foot_right_layer.png", parent: "leg_right", pivot: [305, 1118], z_index: 15 },
  { part: "leg_left", file: "cutout_layers/08_leg_left_layer.png", parent: "root", pivot: [451, 908], z_index: 20 },
  { part: "foot_left", file: "cutout_layers/11_foot_left_layer.png", parent: "leg_left", pivot: [478, 1118], z_index: 25 },
  { part: "arm_right", file: "cutout_layers/06_arm_right_layer.png", parent: "torso", pivot: [262, 650], z_index: 30 },
  { part: "torso", file: "cutout_layers/05_torso_layer.png", parent: "root", pivot: [393, 892], z_index: 40 },
  { part: "ear_right", file: "cutout_layers/02_ear_right_layer.png", parent: "head", pivot: [338, 300], z_index: 45 },
  { part: "ear_left", file: "cutout_layers/01_ear_left_layer.png", parent: "head", pivot: [435, 300], z_index: 48 },
  { part: "head", file: "cutout_layers/03_head_layer.png", parent: "torso", pivot: [389, 585], z_index: 50 },
  { part: "arm_left", file: "cutout_layers/07_arm_left_layer.png", parent: "torso", pivot: [524, 650], z_index: 60 },
];

export function generateThreeJsCutoutModule(
  archetype: ArchetypeId,
  verb: UniversalVerb,
  rigPlan?: CutoutRigExportPlan
): string {
  const plan: CutoutRigExportPlan = rigPlan ?? {
    character_id: archetype,
    canvas_size: [787, 1345],
    parts: DEFAULT_10_PART_SPECS,
  };
  return `// Auto-generated Three.js 2.5D Cutout & GLB Animator (${plan.character_id} | ${verb})
import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";

export const RIG_CONFIG = ${JSON.stringify(plan, null, 2)} as const;

export function createThreeCutoutPuppet(baseUrl = ".") {
  const [canvasW, canvasH] = RIG_CONFIG.canvas_size;
  const scale = 2.0 / Math.max(canvasW, canvasH);
  const rootGroup = new THREE.Group();
  rootGroup.name = \`\${RIG_CONFIG.character_id}_root\`;
  const joints: Record<string, THREE.Group> = { root: rootGroup };
  const loader = new THREE.TextureLoader();

  for (const part of RIG_CONFIG.parts) {
    const group = new THREE.Group();
    group.name = part.part;
    joints[part.part] = group;
  }

  for (const part of RIG_CONFIG.parts) {
    const group = joints[part.part];
    const parentGroup = joints[part.parent] || rootGroup;
    const [px, py] = part.pivot;
    const parentSpec = RIG_CONFIG.parts.find((p) => p.part === part.parent);
    const [ppx, ppy] = parentSpec ? parentSpec.pivot : [canvasW * 0.5, canvasH * 0.85];
    group.position.set((px - ppx) * scale, -(py - ppy) * scale, part.z_index * 0.002);

    const tex = loader.load(\`\${baseUrl}/\${part.file}\`);
    tex.colorSpace = THREE.SRGBColorSpace;
    const geo = new THREE.PlaneGeometry(canvasW * scale, canvasH * scale);
    geo.translate((canvasW * 0.5 - px) * scale, -(canvasH * 0.5 - py) * scale, 0);
    const mat = new THREE.MeshStandardMaterial({
      map: tex,
      transparent: true,
      alphaTest: 0.02,
      roughness: 0.85,
      side: THREE.DoubleSide,
    });
    group.add(new THREE.Mesh(geo, mat));
    parentGroup.add(group);
  }

  return { rootGroup, joints };
}

export async function loadThreeGlbCharacter(glbUrl: string): Promise<THREE.Group> {
  const gltfLoader = new GLTFLoader();
  const gltf = await gltfLoader.loadAsync(glbUrl);
  return gltf.scene;
}
`;
}

export function generateReactMotionCutoutComponent(
  archetype: ArchetypeId,
  verb: UniversalVerb,
  rigPlan?: CutoutRigExportPlan
): string {
  const plan: CutoutRigExportPlan = rigPlan ?? {
    character_id: archetype,
    canvas_size: [787, 1345],
    parts: DEFAULT_10_PART_SPECS,
  };
  return `import React from "react";
import { useCurrentFrame, useVideoConfig, interpolate } from "remotion";

export const RIG_CONFIG = ${JSON.stringify(plan, null, 2)} as const;

export const AnimatedAsset_${archetype}: React.FC<{
  src?: string;
  baseUrl?: string;
  partAnglesDeg?: Record<string, number>;
}> = ({ src, baseUrl = ".", partAnglesDeg = {} }) => {
  const frame = useCurrentFrame();
  const { fps, durationInFrames } = useVideoConfig();
  const phase = (frame % fps) / fps;
  const s = Math.sin(phase * Math.PI * 2);
  const c = Math.cos(phase * Math.PI * 2);
  const yOffset = "${verb}" === "jump_rise_fall"
    ? -Math.sin((frame / durationInFrames) * Math.PI) * 140
    : Math.sin(phase * Math.PI * 2) * 18;
  const xOffset = "${verb}" === "move_traverse"
    ? interpolate(frame, [0, durationInFrames], [-220, 220])
    : 0;
  const rotDeg = "${verb}" === "turn_bank"
    ? Math.sin(phase * Math.PI * 2) * 18
    : 0;

  if (src) {
    return (
      <div style={{
        transform: \`translate3d(\${xOffset}px, \${yOffset}px, 0) rotate(\${rotDeg}deg)\`,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "transparent",
      }}>
        <img src={src} style={{ maxHeight: 480 }} />
      </div>
    );
  }

  const [canvasW, canvasH] = RIG_CONFIG.canvas_size;
  const defaultAngles: Record<string, number> = {
    torso: s * 3,
    head: c * 4,
    ear_left: s * 10,
    ear_right: -s * 10,
    arm_left: -s * 24,
    arm_right: s * 24,
    leg_left: s * 22,
    leg_right: -s * 22,
    foot_left: c * 14,
    foot_right: -c * 14,
    ...partAnglesDeg,
  };

  const renderSubtree = (parentName: string): React.ReactNode => {
    const children = RIG_CONFIG.parts
      .filter((p) => p.parent === parentName)
      .sort((a, b) => a.z_index - b.z_index);
    return children.map((part) => {
      const [px, py] = part.pivot;
      const deg = defaultAngles[part.part] ?? 0;
      return (
        <div
          key={part.part}
          style={{
            position: "absolute",
            inset: 0,
            zIndex: part.z_index,
            transformOrigin: \`\${(px / canvasW) * 100}% \${(py / canvasH) * 100}%\`,
            transform: \`rotate(\${deg.toFixed(2)}deg)\`,
            pointerEvents: "none",
          }}
        >
          <img
            src={\`\${baseUrl}/\${part.file}\`}
            alt={part.part}
            style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }}
          />
          {renderSubtree(part.part)}
        </div>
      );
    });
  };

  return (
    <div style={{
      position: "relative",
      width: canvasW,
      height: canvasH,
      background: "transparent",
      transform: \`translate3d(\${xOffset}px, \${yOffset}px, 0) rotate(\${rotDeg}deg)\`,
    }}>
      {renderSubtree("root")}
    </div>
  );
};
`;
}

export function generateVideoComponentCode(
  archetype: ArchetypeId,
  verb: UniversalVerb,
  target: "remotion" | "react_motion" | "threejs" | "effectcraft" = "remotion",
  rigPlan?: CutoutRigExportPlan
): string {
  if (target === "effectcraft") {
    const plan = rigPlan ?? {
      character_id: archetype,
      canvas_size: [1280, 720],
      parts: DEFAULT_10_PART_SPECS,
    };
    const [w, h] = plan.canvas_size;
    return `// EffectCraft (.JSX) Auto-Generated Multi-Layer Cutout Animation Script
// Archetype: ${archetype} | Verb: ${verb} | Transparent Alpha Composition
var comp = app.project.items.addComp("${plan.character_id}_${verb}", ${w}, ${h}, 1.0, 2.0, 24);
var rootNull = comp.layers.addNull();
rootNull.name = "${plan.character_id}_root";
writeLn("EffectCraft transparent cutout comp created for ${plan.character_id} (${verb})");
`;
  }

  if (target === "threejs") {
    return generateThreeJsCutoutModule(archetype, verb, rigPlan);
  }

  return generateReactMotionCutoutComponent(archetype, verb, rigPlan);
}

