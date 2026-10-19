// src/rig/rigState.ts
// Domain state object #1: RigState
// Holds the rig hierarchy, pivots, 30-part registry, visibility/verify/solo state.
// Extracted from the monolithic buildBunnyExplorerCharacter + outliner logic.

export type Species = "bunny" | "whiskers";
export type OutlinerView = "groups" | "parts26";

export interface RigPart {
  id: string;
  name: string;
  zone: string;
  bone: string;
  desc: string;
  explode: [number, number, number];
  node: any;
  visible: boolean;
  verified: boolean;
  baseLocalPos?: { x: number; y: number; z: number };
}

export interface RigPivots {
  root: any;
  spinePivot: any;
  pelvisPivot: any;
  headPivot: any;
  earLPivot: any;
  earRPivot: any;
  earTipLPivot: any;
  earTipRPivot: any;
  smileGroup: any;
  eyelidUpperMeshes: any[];
  eyelidLowerMeshes: any[];
  eyeGazePivots: any[];
  eyebrowMeshes: any[];
  armLPivot: any;
  armRPivot: any;
  elbowLPivot: any;
  elbowRPivot: any;
  wristLPivot: any;
  wristRPivot: any;
  legLPivot: any;
  legRPivot: any;
  kneeLPivot: any;
  kneeRPivot: any;
  footLPivot: any;
  footRPivot: any;
  tailPivot: any;
  tailMesh: any;
  catTailMesh: any;
  backpackGroup: any;
  vestGroup: any;
  torsoGroup: any;
  shortsGroup: any;
  scarfGroup: any;
  satchelGroup: any;
}

export interface OutlinerItem {
  key: string;
  name: string;
  sub: string;
  color: string;
}

export interface RigState {
  pivots: RigPivots | null;
  parts: RigPart[];
  activeCharacterSpecies: Species;
  outlinerViewMode: OutlinerView;
  soloPartId: string | null;
  rigExplodeAmount: number;
  showJointPivotMarkers: boolean;
  visibilityState: Record<string, boolean>;
  outlinerItems: OutlinerItem[];
}

export const DEFAULT_OUTLINER_ITEMS: OutlinerItem[] = [
  { key: "head", name: "Head_Assembly", sub: "Cranium, Cheeks, Fur Tufts", color: "#f1dfc7" },
  { key: "ears", name: "Ears_Left_Right", sub: "Cupped Felt + Pink Inner", color: "#e39787" },
  { key: "face", name: "Facial_Features", sub: "Eyes, Pink Nose, Buck Tooth", color: "#df887d" },
  { key: "shirt", name: "Outfit_Cream_Shirt", sub: "Short Sleeve + V-Collar", color: "#efe6d5" },
  { key: "vest", name: "Outfit_Orange_Vest", sub: "Tailored Felt + 2 Wood Btns", color: "#cc5b16" },
  { key: "backpack", name: "Explorer_Backpack", sub: "Olive Pack + 3 Brown Pouches", color: "#6c7131" },
  { key: "shorts", name: "Outfit_Denim_Shorts", sub: "Cuffed Blue Denim + Stitches", color: "#264b73" },
  { key: "limbs", name: "Arms_Legs_And_Tail", sub: "Plush Forearms, Calves, Tail", color: "#f1dfc7" },
  { key: "shoes", name: "Sneakers_And_Socks", sub: "Blue Canvas + Rubber Toe", color: "#29527a" },
];

export function createInitialRigState(): RigState {
  return {
    pivots: null,
    parts: [],
    activeCharacterSpecies: "bunny",
    outlinerViewMode: "groups",
    soloPartId: null,
    rigExplodeAmount: 0,
    showJointPivotMarkers: false,
    visibilityState: Object.fromEntries(DEFAULT_OUTLINER_ITEMS.map((i) => [i.key, true])),
    outlinerItems: DEFAULT_OUTLINER_ITEMS,
  };
}

// --- modular mutator functions ---

export function setPartVisibility(state: RigState, partId: string, visible: boolean): void {
  const part = state.parts.find((p) => p.id === partId);
  if (!part || !part.node) return;
  part.visible = visible;
  part.node.visible = visible;
}

export function togglePartVisibility(state: RigState, partId: string): boolean {
  const part = state.parts.find((p) => p.id === partId);
  if (!part) return false;
  part.visible = !part.visible;
  if (part.node) part.node.visible = part.visible;
  return part.visible;
}

export function soloPart(state: RigState, partId: string | null): void {
  state.soloPartId = partId;
  state.parts.forEach((p) => {
    if (!p.node) return;
    p.node.visible = partId === null ? p.visible : p.id === partId;
  });
}

export function showAllParts(state: RigState): void {
  state.soloPartId = null;
  state.parts.forEach((p) => {
    p.visible = true;
    if (p.node) p.node.visible = true;
  });
  state.visibilityState = Object.fromEntries(state.outlinerItems.map((i) => [i.key, true]));
}

export function togglePartVerified(state: RigState, partId: string): void {
  const part = state.parts.find((p) => p.id === partId);
  if (!part) return;
  part.verified = !part.verified;
}

export function verifyAllParts(state: RigState, verified: boolean): void {
  state.parts.forEach((p) => {
    p.verified = verified;
  });
}

export function setRigExplode(state: RigState, amount: number): void {
  state.rigExplodeAmount = Math.max(0, Math.min(1, amount));
}

export function setSpecies(state: RigState, species: Species): void {
  state.activeCharacterSpecies = species;
}

export function toggleCategoryVisibility(state: RigState, categoryKey: string): void {
  state.visibilityState[categoryKey] = !state.visibilityState[categoryKey];
}

export function getVerifiedCount(state: RigState): { verified: number; total: number } {
  const verified = state.parts.filter((p) => p.verified).length;
  return { verified, total: state.parts.length };
}
