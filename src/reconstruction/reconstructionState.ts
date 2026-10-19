// src/reconstruction/reconstructionState.ts
// Domain state object #4: ReconstructionState
// 2D->3D reconstruction: extracted slices, projection blend, reference planes,
// volumetric mesh group, upload source image.
// Extracted from reprocessUploadedImage / buildVolumetricMeshFromSlices / apply2DProjectionColorsToRig.

export type ReconstructionType = "rig" | "volumetric";

export interface SliceSet {
  front: HTMLImageElement | ImageBitmap | null;
  side: HTMLImageElement | ImageBitmap | null;
  back: HTMLImageElement | ImageBitmap | null;
}

export interface ReconstructionState {
  activeReconstructionType: ReconstructionType;
  currentProjectionBlend: number;
  slices: SliceSet;
  uploadedSourceImage: HTMLImageElement | ImageBitmap | null;
  volumetricMesh: any | null;
  referencePlanes: any | null;
  areRefPlanesVisible: boolean;
}

export function createInitialReconstructionState(): ReconstructionState {
  return {
    activeReconstructionType: "rig",
    currentProjectionBlend: 0,
    slices: { front: null, side: null, back: null },
    uploadedSourceImage: null,
    volumetricMesh: null,
    referencePlanes: null,
    areRefPlanesVisible: false,
  };
}

export function setReconstructionType(state: ReconstructionState, type: ReconstructionType): void {
  state.activeReconstructionType = type;
}

export function setProjectionBlend(state: ReconstructionState, pct: number): void {
  state.currentProjectionBlend = Math.max(0, Math.min(100, pct)) / 100;
}

export function toggleReferencePlanes(state: ReconstructionState): boolean {
  state.areRefPlanesVisible = !state.areRefPlanesVisible;
  return state.areRefPlanesVisible;
}
