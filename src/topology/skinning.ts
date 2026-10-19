import type { CostumeSkinSpec, MannequinSkinState } from "../types/schema";

export function stripCostumeToBaseMannequin(
  _currentOutfit?: Record<string, boolean>
): MannequinSkinState {
  return {
    baseMannequinVisible: true,
    costumeVisible: false,
    skinSpec: null
  };
}

export function applyCostumeSkinLayer(
  _state: MannequinSkinState,
  skinSpec: CostumeSkinSpec
): MannequinSkinState {
  return {
    baseMannequinVisible: true,
    costumeVisible: true,
    skinSpec
  };
}
