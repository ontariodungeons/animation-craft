export type ArchetypeId =
  | "humanoid_biped"
  | "axial_creature"
  | "radial_prop"
  | "vehicle_hull";

export type UniversalVerb =
  | "move_traverse"
  | "jump_rise_fall"
  | "turn_bank"
  | "express_talk";

export type SlicingStrategy =
  | "base_mannequin_plus_costume_skin"
  | "auto_slice_appendages"
  | "single_deformable_sheet";

export interface IntakeQuestion {
  id: "q1_archetype" | "q2_slicing" | "q3_verb_output";
  question: string;
  options: Array<{ value: string; label: string }>;
}

export interface IntakeAnswers {
  archetype: ArchetypeId;
  slicingStrategy: SlicingStrategy;
  primaryVerb: UniversalVerb;
  cameraAngles: number[];
  stripCostumeToBase: boolean;
}

export interface BoneNode {
  name: string;
  parent: string | null;
  head: [number, number, number];
  tail: [number, number, number];
  jointType: "rigid" | "hinge" | "spline";
}

export interface SkeletonGraph {
  archetype: ArchetypeId;
  bones: BoneNode[];
}

export interface CostumeSkinSpec {
  id: string;
  primaryHex: string;
  secondaryHex: string;
  textureUrl?: string;
}

export interface MannequinSkinState {
  baseMannequinVisible: boolean;
  costumeVisible: boolean;
  skinSpec: CostumeSkinSpec | null;
}
